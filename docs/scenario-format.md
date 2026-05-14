# Scenario format

A scenario lives in `scenarios/<ticket>-<slug>/` and contains four files:

```
scenarios/10874-mcp-log-rotation/
├── metadata.yaml      # required — title, ticket, expected, mcp_log_paths
├── scenario.md        # required — human-readable repro steps
├── setup.sh           # optional — fixture seeding before the recording
└── assertions.sh      # optional — programmatic checks after the recording
```

## `metadata.yaml`

```yaml
title: "Short, sentence-style title — what this scenario proves"
ticket: "owner/repo#1234 (fix in PR #5678)"
expected: |
  Multi-line description of the expected behavior. This is what
  reviewers see at the top of the generated README in the output bundle.

# Optional: MCP server log directories whose contents should be snapshotted
# at end-of-recording. Each entry is a directory path; the runner copies
# every file in it to tapes/<scenario>/logs/mcp/.
mcp_log_paths:
  - ~/Library/Group Containers/2BBY89MBSN.dev.warp/Library/Application Support/dev.warp.Warp-Stable/mcp
```

The YAML parser is `awk`-grade. Keep it simple. Lists inside lists aren't supported. Comments are fine. Tilde expansion in paths is handled by the runner.

## `scenario.md`

A markdown file describing the repro steps. The runner shows this to the user before recording starts, and embeds it (blockquoted) in the bundle's generated `README.md`. Focus on:

- Which Warp build / branch you expect to be running
- Exactly which clicks / inputs the user performs
- What state the assertions will look for

## `setup.sh`

Runs before the binary launches. Use this for:

- Seeding `~/.warp/.mcp.json` or other config fixtures
- Toggling a Privacy setting
- Clearing previous-run artifacts so post-recording assertions are clean
- Recording pre-state of files the assertions will later compare to

The script's working directory is the scenario directory. The runner invokes it as `bash setup.sh`.

## `assertions.sh`

Runs after recording finishes. The runner exports these env vars before invoking it:

| Variable | Description |
|---|---|
| `TAPE_DIR` | Absolute path to `tapes/<scenario>/` |
| `TAPE_LOGS` | `${TAPE_DIR}/logs` |
| `TAPE_PATCHES` | `${TAPE_DIR}/patches` |
| `TAPE_SESSION` | `${TAPE_DIR}/logs/warp-oss.session.log` (may not exist) |
| `TAPE_MCP_LOGS` | `${TAPE_DIR}/logs/mcp` (may not exist) |
| `WARP_SOURCE` | Resolved Warp checkout |

The script can source `lib/evaluate.sh` for helpers:

- `assert_file_exists <path> <description>`
- `assert_log_contains <log_path> <pattern> <description>`
- `assert_log_lacks <log_path> <pattern> <description>`

Each helper writes a `  ✓ ...` or `  ✗ ...` line to stdout. The bundle extracts those lines for the README's assertion summary.

Exit `0` for "scenario verified," non-zero for "did not verify."

## Output bundle

After `warp-taper run scenarios/<scenario>/`, the bundle directory contains:

```
tapes/10874-mcp-log-rotation/
├── README.md                    # PR-ready summary
├── metadata.yaml                # copy of the scenario's metadata
├── master.mov                   # screen recording
├── patches/                     # named stills
│   ├── 01-before-rotation.png
│   └── 02-after-rotation.png
├── logs/
│   ├── warp-oss.session.log     # warp-oss.log slice during the session
│   └── mcp/                     # snapshot of declared MCP log dirs
│       ├── <uuid>.log
│       └── <uuid>.log.1
└── stages/
    ├── 01-build.log
    ├── 02-deploy.log
    ├── 03-record.log
    └── 04-evaluate.log
```

## What scenarios are good for

**Good fit** — verifiable across a deterministic boundary:

- File-existence checks (rotation files appear)
- Log-content greps (a specific WARN does/doesn't appear)
- Settings state (configurable via `defaults write` for some settings)
- API responses where the assertion can curl/grep something

**Stretch / manual review** — visual-only behavior:

- UI rendering correctness ("the chip doesn't overflow the pane")
- Animation smoothness
- Color / contrast issues

For the second category the value is the recording itself, plus the human eye on it. Assertions can still be useful (e.g. "no crash logs appeared") but won't fully verify the visual claim.
