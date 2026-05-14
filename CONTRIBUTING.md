# Contributing to warp-taper

Thanks for your interest. The project is small and opinionated; this guide
covers the two contribution paths that come up most often.

## Setting up

```sh
git clone git@github.com:david-engelmann/warp-taper.git
cd warp-taper
cargo build --workspace
cargo test --workspace
```

You'll want `cargo-nextest` and `cargo-llvm-cov` installed locally to mirror
CI:

```sh
cargo install cargo-nextest cargo-llvm-cov cargo-insta
```

## Adding a built-in scenario

Built-in scenarios live in
[`crates/warp-taper-core/src/scenarios/`](crates/warp-taper-core/src/scenarios/).
Each one is a Rust file returning a `Builtin` (a tuple of `Scenario` and a
`Vec<Box<dyn Assertion>>`).

The fastest way to start a new one:

```sh
warp-taper init 12345-my-fix \
    --title "What this scenario proves" \
    --ticket "owner/repo#12345" \
    > crates/warp-taper-core/src/scenarios/my_fix.rs
```

Then:

1. Add `pub mod my_fix;` to
   [`crates/warp-taper-core/src/scenarios/mod.rs`](crates/warp-taper-core/src/scenarios/mod.rs)
2. Register the slug in `by_name()` and `names()` in the same file
3. Customize the assertions in your new file
4. `cargo test --workspace` to verify
5. `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`

The reference port to study is
[`mcp_log_rotation.rs`](crates/warp-taper-core/src/scenarios/mcp_log_rotation.rs).

## Adding a new builtin assertion

Builtins live in
[`crates/warp-taper-core/src/assertion/builtins.rs`](crates/warp-taper-core/src/assertion/builtins.rs).
Each is a struct implementing the `Assertion` trait — `name(&self)` and
`run(&self, ctx) -> AssertionResult`. Ship the unit test in the same file.

Pattern:

```rust
pub struct MyAssertion {
    description: String,
}

impl Assertion for MyAssertion {
    fn name(&self) -> &str {
        "my_assertion"
    }
    fn run(&self, ctx: &AssertionContext) -> AssertionResult {
        // …
        if /* check */ {
            AssertionResult::pass(&self.description)
        } else {
            AssertionResult::fail(format!("{} (why)", self.description))
        }
    }
}
```

## CI gates

Every PR must pass:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo nextest run --workspace` on macOS and Ubuntu
- Coverage floor: 85% lines for `warp-taper-core`, 70% for `warp-taper-cli`

The CI workflow is at
[`.github/workflows/ci.yml`](.github/workflows/ci.yml).

## Test layers

| Layer | Where | When to add |
|---|---|---|
| L1 unit | `#[cfg(test)]` per module | Any new logic. Default home. |
| L2 integration | `crates/*/tests/*.rs` | Cross-module flows; pipeline wiring. |
| L3 e2e | `#[ignore]` tests gated on `WARP_SOURCE` | Driving real warp builds. Don't auto-run in CI. |
| L4 property | proptest in `tests/` | Invariants worth fuzzing. |
| L5 smoke | gated by `--features smoke` | Real macOS screencapture; manual only. |

## Plan + changelog

The implementation plan lives in [docs/PLAN.md](docs/PLAN.md). Notable
changes go in [CHANGELOG.md](CHANGELOG.md) under `## Unreleased`.

## Reporting bugs

Open an issue at <https://github.com/david-engelmann/warp-taper/issues>.
Include the failing tape directory if you have one.
