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

## Priming warp-oss with your Warp auth (macOS)

The pipeline's `deploy` stage launches a `warp-oss` binary built from
`$WARP_SOURCE`. Because warp-oss uses a different bundle ID
(`dev.warp.WarpOss`) than the public Warp release
(`dev.warp.Warp-Stable`), it starts out with an empty keychain and no
logged-in user — most agent-side code paths are gated on auth and stay
dark, which makes evidence capture for any PR that touches them awkward.

[`scripts/prime-warp-oss-auth.sh`](scripts/prime-warp-oss-auth.sh) copies
the `User` keychain entry from `dev.warp.Warp-Stable` to
`dev.warp.WarpOss` so warp-oss boots as the same Firebase user as your
day-to-day Warp install. After running it once, `warp-taper run …`
exercises the same authenticated code paths Warp-Stable does.

```sh
# defaults work for a standard Warp release + locally-built warp-oss
bash scripts/prime-warp-oss-auth.sh

# clean up when you're done
security delete-generic-password -s dev.warp.WarpOss -a User
```

Override the defaults via env vars if your install differs; see
[`scripts/prime-warp-oss-auth.env.sample`](scripts/prime-warp-oss-auth.env.sample)
for the expected shape. **Never commit a file with real keychain
values or tokens** — the helper reads the keychain directly and never
needs them on disk.

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
