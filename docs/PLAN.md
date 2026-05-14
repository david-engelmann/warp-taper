# warp-taper v0 — implementation plan

This is the design doc for the Rust-based, test-first rewrite of warp-taper.
The current bash pipeline is the working prototype; v0 supersedes it.

## Stance

- **Library-first.** `warp-taper-core` is a Rust library with all logic. The
  CLI is a thin clap binary on top. External consumers (warp itself, future
  tooling) can depend on the core crate without dragging in clap or the
  recorder.
- **Test-first.** Every module ships with tests in the same PR. Coverage is
  enforced in CI: **85% core / 70% cli**.
- **Scenarios are Rust.** `Scenario` is a struct built via a typed API. The
  existing YAML+bash format becomes one loader among many.

## Workspace

```
warp-taper/
├── Cargo.toml                  # [workspace] members = [...]
├── crates/
│   ├── warp-taper-core/        # library
│   ├── warp-taper-cli/         # binary
│   └── warp-taper-fixtures/    # dev-only: sample tapes, golden bundles, test-doubles
├── tests/                      # workspace-level e2e (gated on WARP_SOURCE)
├── .github/workflows/ci.yml
└── README.md
```

## `warp-taper-core` module map

| Module | Public API surface |
|---|---|
| `scenario` | `Scenario`, `ScenarioBuilder`, `Metadata`; `from_yaml(&Path)` loader |
| `pipeline` | `Pipeline`, `Context`; `run()` orchestrates stages |
| `stages::build` | `BuildStage` — wraps `cargo build -p warp` |
| `stages::deploy` | `DeployStage` — launches binary, owns lifecycle |
| `stages::record` | `RecordStage` — uses `Recorder` trait + `LogTail` |
| `stages::evaluate` | `EvaluateStage` — runs `Assertion`s |
| `stages::bundle` | `BundleStage` — writes README + copies metadata |
| `recorder` | `Recorder` trait, `MacOsScreencapture`, `NoOpRecorder` |
| `log_tail` | `LogTail::seek_to_end`, `LogTail::slice_since` |
| `assertion` | `Assertion` trait, builtins (`FileExists`, `LogContains`, `LogLacks`, `McpRotationOccurred`), `ShellScriptAssertion` adapter |
| `bundle` | `BundleWriter` — README via `format!` + insta snapshots |
| `error` | `Error` (thiserror) + `Result<T>` |

Async runtime: **tokio** (needed for subprocess lifecycles + concurrent log tailing).

## Testing pyramid

### L1 — Unit (`#[cfg(test)]` per module)
- Scenario parsing edge cases + validation
- `log_tail` seek/copy on temp files
- Bundle README formatting via insta snapshots
- Assertion result aggregation, pass/fail bookkeeping

### L2 — Integration (`crates/warp-taper-core/tests/`)
- Stage chaining with in-process test-doubles (`StubBuilder`, `StubDeployer`, `NoOpRecorder`)
- Full-pipeline run with all stages stubbed → produces a valid bundle
- Real `log_tail` against real temp files growing in another thread
- Real `BundleWriter` against fixture tapes

### L3 — End-to-end (`tests/e2e/`, `#[ignore]` by default)
- Require `WARP_SOURCE`; build + run real `warp-oss`
- Drive the compiled `warp-taper` binary via `assert_cmd`
- Verify the produced tape matches a golden bundle (insta directory snapshots, with non-deterministic fields normalized)
- Run locally via `cargo nextest run --run-ignored`; in nightly CI on macOS

### L4 — Property (`proptest`)
- YAML parse round-trip
- `log_tail` invariants (slice ≤ file size, monotone offsets)
- Bundle path resolution doesn't escape the tape root

### L5 — Smoke (`--features smoke`, opt-in, macOS-only)
- 1-second `screencapture` → verify `.mov` exists and is non-empty
- Real `cargo build -p warp` on a known commit

## Tooling

- **cargo-nextest** — test runs
- **cargo-llvm-cov** — coverage; CI gate
- **insta** — snapshot tests (README, bundle layout)
- **proptest** — property tests
- **assert_cmd** + **predicates** — CLI tests
- **tempfile** + **rstest** — fixture ergonomics
- **thiserror** — error types
- **tokio** — async runtime

## CI (`.github/workflows/ci.yml`)

- Matrix: `macos-latest` (full), `ubuntu-latest` (everything except macOS-only modules + smoke)
- Pipeline: `fmt-check` → `clippy -D warnings` → `nextest run` → `llvm-cov` → coverage upload
- **Coverage gate**: PR fails if line coverage drops > 2pp from main; absolute floor 85% core / 70% cli
- **E2E is not in default CI** (real Warp build is slow). A separate nightly workflow runs `nextest run --run-ignored` on macOS with a cached Warp checkout.

## Phased rollout (one PR per phase)

| Phase | Scope | Tests added |
|---|---|---|
| **P0** | Workspace scaffold, CI skeleton (no coverage gate yet), `error` module | Smoke CI |
| **P1** | `scenario` + `log_tail` + `bundle` (pure logic) | L1 unit, L4 property, L1 insta |
| **P2** | `assertion` engine + builtins + shell adapter | L1 unit |
| **P3** | `stages::build` + `stages::deploy` with stubs | L1 unit, L2 integration with stubs |
| **P4** | `recorder` trait + macOS impl + no-op | L1 unit, L5 smoke (macOS) |
| **P5** | `pipeline` orchestrator + CLI (clap) | L2 integration full pipeline, L3 e2e gated |
| **P6** | Turn on coverage gate, port `10874-mcp-log-rotation` scenario to Rust, delete bash | All gates active |
| **P7** | Tag `0.1.0`, publish to crates.io (optional) | — |

## Migration of the 10874 scenario

The existing `scenarios/10874-mcp-log-rotation/` becomes a Rust file:
`crates/warp-taper-core/src/scenarios/mcp_log_rotation.rs`. The `Scenario`
builder declares metadata, MCP log paths, and a `Vec<Box<dyn Assertion>>`
with the same checks the bash script does today. The bash `assertions.sh`
remains as a back-compat adapter target through P5; removed in P6.

## Out of scope for v0

- Cross-platform recording (Linux/Windows)
- Hosted bundle viewer / web UI
- Network capture / HAR
- Publishing tapes to a remote store
