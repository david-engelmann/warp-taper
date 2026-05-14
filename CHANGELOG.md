# Changelog

All notable changes to this project. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project is
pre-1.0 so breaking changes are expected and may not always bump a major
version.

## Unreleased

### Added
- `BuildStage::with_timeout(Duration)` + `--build-timeout-seconds` CLI flag.
  Aborts the cargo build if it exceeds the timeout instead of hanging the
  pipeline.
- Pipeline now writes per-stage log files to `<tape>/stages/0X-name.log`
  and tails the last 50 lines of each into the bundle README's `## Stages`
  section. Matches the bash pipeline's behavior.
- `Pipeline::with_deploy_spawned_callback(fn(u32))` — invoked with the
  deployed child's PID. Used by the CLI to wire the PID into a SIGINT
  handler so Ctrl-C kills warp-oss before exiting.
- `Pipeline::with_build_finished_callback(fn(&Path))` — informational hook
  for CLI progress output.
- CLI: `describe <name>` subcommand prints metadata for a built-in scenario
  (title, ticket, expected, mcp_log_paths, registered assertions).
- CLI: `init <slug>` subcommand prints a starter Rust scenario module to
  stdout — pipe to a file under `crates/warp-taper-core/src/scenarios/`.
- CLI: `--branch` and `--head` flags to override the auto-detected git refs.
- CLI: auto-detects branch + short HEAD via `git -C $WARP_SOURCE rev-parse`.
  The bash pipeline did this; the Rust port had been hardcoding
  `<unknown>` in the bundle README.
- CLI: SIGINT handler installed at pipeline start. Sends SIGTERM to the
  tracked deploy PID before exiting with code 130. Prevents orphaned
  warp-oss processes when a user Ctrl-Cs.
- `Recorder` + `RecordingHandle` trait round-trip unit test that closes
  the trait coverage gap on Linux runners.

### Changed
- `bundle::StageLog` now owns its name + tail (`String` instead of
  `&'a str`) so the pipeline can populate it at runtime.

## P6 — Rust scenario for 10874, coverage gate, delete bash pipeline

- Pure-Rust port of `scenarios/10874-mcp-log-rotation/` at
  `warp_taper_core::scenarios::mcp_log_rotation`.
- `warp-taper run-builtin <name>` + `warp-taper list-builtins` CLI subcommands.
- CI coverage gate: `warp-taper-core` ≥ 85% lines, `warp-taper-cli` ≥ 70%.
- Removed `bin/`, `lib/*.sh`, `scenarios/`, `docs/scenario-format.md`.

## P5 — pipeline orchestrator + CLI run subcommand + L3 e2e

- `pipeline::Pipeline` wires build → deploy → record → evaluate → bundle.
- `Recorder` + `RecordingHandle` traits with delegating impls for
  `NoOpRecorder` and `MacOsScreencapture`.
- `warp-taper run <scenario-dir>` CLI subcommand.
- `RecordTrigger::{Interactive, Duration(d)}`.

## P4 — recorder

- `MacOsScreencapture` driving `screencapture -v -V <secs>` + SIGINT-finalize.
- `NoOpRecorder` for tests.
- Gated `smoke` feature for 1-second real-screencapture verification.

## P3 — build + deploy stages

- `BuildStage` (`cargo build -p <package>`) + `DeployStage` (spawns binary).
- `warp-taper-fixtures::tiny_warp` cargo workspace for tests.

## P2 — assertion engine

- `Assertion` trait + builtins
  (`FileExists`, `DirNotEmpty`, `LogContains`, `LogLacks`,
  `McpLogSnapshotCaptured`, `McpRotationOccurred`).
- `ShellScriptAssertion` adapter for legacy `assertions.sh`.

## P1 — pure-logic core

- `scenario::Scenario` + YAML loader.
- `log_tail::LogTail` byte-offset slicer.
- `bundle::render_readme` markdown generator with insta snapshot tests.

## P0 — workspace scaffold

- Three-crate workspace (`warp-taper-core`, `warp-taper-cli`,
  `warp-taper-fixtures`).
- CI: fmt + clippy + nextest on Ubuntu + macOS.
