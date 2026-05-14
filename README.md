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

```text
$ warp-taper --help
Record Warp behavior into PR-ready evidence bundles.

Usage: warp-taper <COMMAND>

Commands:
  run            Run the full pipeline against a scenario directory
  run-builtin    Run a built-in (Rust-authored) scenario by name. Use `list-builtins` to see what's available
  list-builtins  List built-in scenarios available to `run-builtin`
  describe       Print metadata for a built-in scenario without running it
  init           Print a starter Rust scenario file to stdout
  version        Print version
  help           Print this message or the help of the given subcommand(s)
```

### Discover built-in scenarios

```text
$ warp-taper list-builtins
mcp-log-rotation

$ warp-taper describe mcp-log-rotation
slug:      10874-mcp-log-rotation
title:     MCP log rotation kicks in at the size cap
ticket:    warpdotdev/warp#10874
expected:
  An MCP server's log file rotates after writing past the configured size
  threshold (10 MiB by default, 5 rotated copies = 60 MiB cap per server).
  The MCP server continues to operate normally during rotation: no error
  toasts, no dropped connections, no crashed processes. Before PR #10874
  the active log grew without bound.
mcp_log_paths:
  - ~/Library/Group Containers/2BBY89MBSN.dev.warp/Library/Application Support/dev.warp.Warp-Stable/mcp
assertions (3):
  - mcp_log_snapshot_captured
  - mcp_rotation_occurred
  - log_lacks
```

### Run a built-in scenario

```sh
warp-taper run-builtin mcp-log-rotation \
    --warp-source ~/personal/warp \
    --duration-ms 60000
```

Or a YAML-defined scenario directory (`metadata.yaml` + optional `assertions.sh`):

```sh
warp-taper run path/to/scenario-dir --warp-source ~/personal/warp
```

Output lands in `tapes/<slug>/` (gitignored). Override with `--tape-dir`.

### Scaffold a new scenario

```text
$ warp-taper init 12345-fancy-fix --title "Fancy fix" --ticket "warpdotdev/warp#12345"
//! Built-in scenario: Fancy fix.

use crate::assertion::{Assertion, McpLogSnapshotCaptured};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

pub fn _12345_fancy_fix() -> Result<Builtin> {
    let scenario = Scenario::builder("12345-fancy-fix")
        .title("Fancy fix")
        .ticket("warpdotdev/warp#12345")
        .expected("TODO: describe the expected behavior")
        .build()?;

    let assertions: Vec<Box<dyn Assertion>> = vec![
        Box::new(McpLogSnapshotCaptured),
    ];

    Ok((scenario, assertions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds() {
        _12345_fancy_fix().unwrap();
    }
}
```

Pipe into `crates/warp-taper-core/src/scenarios/` and register it in
[`scenarios/mod.rs`](crates/warp-taper-core/src/scenarios/mod.rs).

## Example output: a recorded tape

Running the pipeline produces a directory like:

```
tapes/<slug>/
├── README.md                     # PR-ready, paste-into-comment
├── master.mov                    # screen recording
├── logs/
│   ├── warp-oss.session.log      # slice of warp-oss.log during the run
│   └── mcp/                      # snapshots of declared MCP log dirs
└── stages/
    ├── 01-build.log
    ├── 02-deploy.log
    ├── 03-record.log
    └── 04-evaluate.log
```

The generated `README.md` looks like this (full file:
[docs/sample-tape-README.md](docs/sample-tape-README.md)):

```markdown
# Tape: Demo: pipeline outputs a tape README

**Ticket:** warpdotdev/warp#0  
**Branch / head:** `demo-branch` @ `abc123`  
**Recorded:** 2026-05-14T14:32:50Z  
**Evaluation:** `pass`

## Expected behavior

Running `warp-taper run` on this scenario produces a bundle directory
containing master.mov, logs/, stages/, and a PR-ready README.md.

## Artifacts

- [master.mov](master.mov) — screen recording
- [logs/warp-oss.session.log](logs/warp-oss.session.log) — 0 bytes …

## Stages

<details><summary>01-build.log</summary>

  ```
  build: cargo build started at 2026-05-14T14:32:50Z
  build: duration 621ms
  build: produced binary <tmp>/warp/target/debug/warp-oss
  …
  ```
</details>

<details><summary>04-evaluate.log</summary>

  ```
  evaluate: 0 pass, 0 fail, 0 info
  evaluate: pass
  ```
</details>
```

When the bundle is attached to a PR, the `<details>` blocks collapse the per-stage logs by default so reviewers see the summary first.

## Authoring scenarios

Scenarios are pure Rust. Each one is a function that returns a `(Scenario, Vec<Box<dyn Assertion>>)`. See [crates/warp-taper-core/src/scenarios/](crates/warp-taper-core/src/scenarios/) for the reference implementation (`mcp_log_rotation.rs`).

YAML+bash scenarios are supported via the `ShellScriptAssertion` adapter; see [CONTRIBUTING.md](CONTRIBUTING.md) for the format.

## Requirements

macOS for real screen recording (`/usr/sbin/screencapture`). Use `--no-screencapture` on other platforms to fall back to the no-op recorder.

## Project layout

| Crate | Purpose |
|---|---|
| `warp-taper-core` | Library: scenario, assertion, recorder, stages, pipeline, bundle |
| `warp-taper-cli`  | `warp-taper` binary |
| `warp-taper-fixtures` | Dev-only: tiny cargo workspace fixture used by tests |

See [docs/PLAN.md](docs/PLAN.md) for the design doc and [CHANGELOG.md](CHANGELOG.md) for what shipped in each phase.

## License

MIT.
