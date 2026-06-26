# Contributing to plutonium_engine

Thanks for helping improve plutonium_engine. This project is a Rust 2D graphics engine built on `wgpu`, with optional retained-mode widgets and wasm support.

## Ground rules

- Keep changes focused. Prefer small pull requests with one behavior change, bug fix, or documentation update.
- Preserve public API stability where possible. If an API change is necessary, document the migration impact in the pull request.
- Keep hot paths allocation-conscious. Rendering, text layout, texture upload, and frame presentation code should avoid avoidable per-frame allocations or GPU stalls.
- Add or update tests for behavior changes. Do not add mocks for engine behavior that can be exercised directly.
- Keep large generated assets, local captures, and perf snapshots out of CI and pull requests unless they are required for review.

## Development setup

Install stable Rust and clone the repository:

```bash
git clone https://github.com/zeke13dev/plutonium_engine.git
cd plutonium_engine
rustup toolchain install stable
rustup target add wasm32-unknown-unknown
```

Optional API checks use `cargo-public-api`:

```bash
cargo install cargo-public-api --locked
```

## Cargo.lock policy

`Cargo.lock` is tracked. Update it with dependency changes and keep it committed so CI, release packaging, examples, and snapshot tooling use the same dependency resolution.

## Local verification

Before opening a pull request, run the checks that match your change. For broad engine changes, run the full gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings \
  -A clippy::too_many_arguments \
  -A clippy::type_complexity \
  -A clippy::explicit_auto_deref \
  -A clippy::manual_clamp \
  -A clippy::collapsible_else_if \
  -A clippy::derivable_impls
cargo test --all --no-fail-fast
cargo run --bin snapshots
bash scripts/check-api.sh
```

For wasm-facing changes, also run:

```bash
cargo check --workspace --target wasm32-unknown-unknown --features wasm
```

## Pull request checklist

- Describe the user-visible change and why it is needed.
- Note any public API additions, removals, or behavior changes.
- Include screenshots or snapshot updates for visual changes when useful.
- Include performance evidence for hot-path render changes when practical.
- Confirm which local checks you ran.

## Reporting bugs

When filing a bug, include:

- crate version or commit SHA
- platform and GPU/backend details when rendering is involved
- minimal reproduction steps or a small example
- expected behavior and actual behavior
- logs, panic output, or snapshot diffs when available

For vulnerabilities, do not open a public issue. Follow `SECURITY.md`.
