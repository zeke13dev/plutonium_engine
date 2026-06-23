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

# Locate the nightly toolchain binaries.  On systems where rustup is the cargo driver the
# +toolchain syntax works; on systems where cargo is installed separately (e.g. MacPorts) we fall
# back to the explicit toolchain binary path.
if rustup toolchain list 2>/dev/null | grep -q 'nightly'; then
    NIGHTLY_HOME="$(rustup toolchain list --verbose 2>/dev/null \
        | awk '/^nightly/{print $2; exit}' || true)"
    if [[ -z "${NIGHTLY_HOME}" ]]; then
        # Older rustup: derive path from well-known convention
        NIGHTLY_HOME="$(rustup show home)"/toolchains/"$(rustup toolchain list | awk '/^nightly/{print $1; exit}')"
    fi
    NIGHTLY_RUSTDOC="${NIGHTLY_HOME}/bin/rustdoc"
    NIGHTLY_RUSTC="${NIGHTLY_HOME}/bin/rustc"
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

fail=0

echo "=== Checking host public API ==="
if ! diff <("${CARGO_PUBLIC_API}" --simplified --all-features 2>/dev/null) "${BASELINE_HOST}"; then
    echo "FAIL: host public API has changed from baseline (${BASELINE_HOST})" >&2
    fail=1
else
    echo "OK: host public API matches baseline."
fi

echo ""
echo "=== Checking wasm32 public API ==="
if ! diff <("${CARGO_PUBLIC_API}" --simplified --target wasm32-unknown-unknown --features wasm 2>/dev/null) "${BASELINE_WASM}"; then
    echo "FAIL: wasm32 public API has changed from baseline (${BASELINE_WASM})" >&2
    fail=1
else
    echo "OK: wasm32 public API matches baseline."
fi

exit "${fail}"
