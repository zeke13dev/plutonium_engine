#!/usr/bin/env bash
# check-api.sh — regenerate public-API snapshots and diff against the committed baselines.
# Exits non-zero if the public surface has changed for either host or wasm32 targets.
#
# Requirements:
#   cargo-public-api >= 0.52 (cargo install cargo-public-api)
#   nightly toolchain installed (rustup toolchain install nightly)
#   wasm32-unknown-unknown target for nightly (rustup target add wasm32-unknown-unknown --toolchain nightly)
#
# cargo-public-api requires rustdoc's unstable JSON output, which is only available on the nightly
# compiler. We invoke it by pointing RUSTDOC/RUSTC at the nightly toolchain binaries while keeping
# the regular (stable) cargo as the driver. The "+toolchain" cargo proxy form is NOT used because
# this repo's CI may have cargo installed outside rustup (e.g. MacPorts).
#
# If nightly is unavailable this script exits 1 with a clear error message.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

BASELINE_HOST="${REPO_ROOT}/api-baseline-host.txt"
BASELINE_WASM="${REPO_ROOT}/api-baseline-wasm.txt"

CARGO_PUBLIC_API="${CARGO_PUBLIC_API:-cargo-public-api}"

# Resolve the binaries through rustup instead of parsing `toolchain list --verbose`. The latter
# inserts status fields such as `(active, default)` before the path, so its column positions are
# not stable across rustup versions or CI environments.
if NIGHTLY_RUSTDOC="$(rustup which --toolchain nightly rustdoc 2>/dev/null)" &&
    NIGHTLY_RUSTC="$(rustup which --toolchain nightly rustc 2>/dev/null)"; then
    :
else
    echo "error: nightly toolchain not found." >&2
    echo "Install with: rustup toolchain install nightly" >&2
    echo "Then: rustup target add wasm32-unknown-unknown --toolchain nightly" >&2
    exit 1
fi

if [[ ! -x "${NIGHTLY_RUSTDOC}" ]]; then
    echo "error: nightly rustdoc not found at ${NIGHTLY_RUSTDOC}" >&2
    exit 1
fi

export RUSTDOC="${NIGHTLY_RUSTDOC}"
export RUSTC="${NIGHTLY_RUSTC}"

cd "${REPO_ROOT}"

# normalize() — reduce cargo-public-api output to a canonical SET of public items.
#
# The public API is the SET of fully-qualified item descriptors emitted by
# cargo-public-api; line ORDER and impl-block GROUPING are not part of the API.
# When inherent-impl methods on the same type are split across multiple source
# files (exactly what the decompose-lib refactor does), cargo-public-api emits a
# separate `impl<..> Type` header line per file and groups each file's methods
# under it — reshuffling lines and duplicating the impl header, even though the
# actual public surface is the same set of items.
#
# rustdoc's canonical paths and internal auto-trait spelling can change between compiler versions
# without changing the callable API. Normalize the known equivalents before comparing snapshots.
#
# `sort -u` makes the comparison set-based:
#   - duplicated `impl<..> Type` header lines collapse to one (byte-identical),
#   - methods sort into a canonical order regardless of which file emitted them,
#   - a REAL change (added/removed/renamed item, or any signature/type change)
#     still alters a unique descriptor line, so the sets differ and it is caught.
# This trades line-order sensitivity (not meaningful for an API surface) for
# immunity to the multi-file impl-grouping artifact.
normalize() {
    sed \
        -e 's/core::marker::UnsafeUnpin/core::marker::Unpin/g' \
        -e 's/std::io::/core::io::/g' \
        | grep -v '^[[:space:]]*$' \
        | sort -u
}

fail=0

echo "=== Checking host public API ==="
if ! diff \
    <("${CARGO_PUBLIC_API}" --simplified --all-features 2>/dev/null | normalize) \
    <(normalize < "${BASELINE_HOST}"); then
    echo "FAIL: host public API has changed from baseline (${BASELINE_HOST})" >&2
    fail=1
else
    echo "OK: host public API matches baseline."
fi

echo ""
echo "=== Checking wasm32 public API ==="
if ! diff \
    <("${CARGO_PUBLIC_API}" --simplified --target wasm32-unknown-unknown --all-features 2>/dev/null | normalize) \
    <(normalize < "${BASELINE_WASM}"); then
    echo "FAIL: wasm32 public API has changed from baseline (${BASELINE_WASM})" >&2
    fail=1
else
    echo "OK: wasm32 public API matches baseline."
fi

exit "${fail}"
