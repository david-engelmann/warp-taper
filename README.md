# warp-taper

Evidence-recording toolkit for [warpdotdev/warp](https://github.com/warpdotdev/warp) PRs. Runs a scenario against a local Warp build, captures screen + logs, runs programmatic assertions, and emits a bundle ready to attach to a PR comment.

## Pipeline

```
build      cargo build -p warp against $WARP_SOURCE
deploy     launch warp-oss
record     screen capture + log tail during the scenario
evaluate   run the scenario's assertions
bundle     emit a PR-ready README.md referencing the captured artifacts
```

## Install

```sh
cargo install --path crates/warp-taper-cli
```

## Usage

Run a built-in scenario:

```sh
warp-taper run-builtin mcp-log-rotation --warp-source ~/personal/warp
```

Run a YAML-defined scenario directory (`metadata.yaml` + optional `assertions.sh`):

```sh
warp-taper run path/to/scenario-dir --warp-source ~/personal/warp
```

List the built-in scenarios:

```sh
warp-taper list-builtins
```

Output lands in `tapes/<slug>/` (gitignored). Override with `--tape-dir`.

## Authoring scenarios

Scenarios are pure Rust. Each one is a function that returns a `(Scenario, Vec<Box<dyn Assertion>>)`. See [crates/warp-taper-core/src/scenarios/](crates/warp-taper-core/src/scenarios/) for the reference implementation (`mcp_log_rotation.rs`).

YAML+bash scenarios are supported via the `ShellScriptAssertion` adapter for back-compat; see [docs/PLAN.md](docs/PLAN.md) for the format.

## Requirements

macOS for real screen recording (`/usr/sbin/screencapture`). Use `--no-screencapture` on other platforms to fall back to the no-op recorder.

## License

MIT.
