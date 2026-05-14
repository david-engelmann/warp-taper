# warp-taper

> A taper at the Dead show, but for Warp.

`warp-taper` is a recording-evidence toolkit for Warp Terminal: it **builds**, **deploys**, **records**, and **evaluates** Warp behavior from a single repo, producing PR-ready "tapes" (evidence bundles) you can drag straight into a GitHub issue or PR comment.


## Why this exists

When you ship a fix to [warpdotdev/warp](https://github.com/warpdotdev/warp), reviewers want evidence. Screenshots, recordings, log tails. Producing that evidence manually is fiddly — there are five tools (Cmd+Shift+5, screencapture, `tail`, the right file paths, the right window framing) and each repro is a fresh setup.

`warp-taper` automates the four stages so a single command yields a complete bundle:

```
 build   → cargo build -p warp-oss in your warp checkout
 deploy  → launch the built binary with a known fixture
 record  → screen capture + log capture during the session
 evaluate → structured assertions about outcome
   ↓
 tape    → bundled output: video + stills + logs + README, ready to attach to a PR
```


## Pipeline stages

### 1. build
Runs `cargo build -p warp-oss` against a configurable Warp checkout (default `~/personal/warp`). Captures the compile output so reviewers can see what was built. Skips if `--no-build` is passed and the binary already exists.

### 2. deploy
Launches the built `warp-oss` binary, optionally with a fixture config (e.g. seeding `~/.warp/.mcp.json` with a test server). Captures the launch logs.

### 3. record
Starts screen capture (`screencapture -v` for video, individual `screencapture` calls at named "patches") and parallel log tailing (warp.log, MCP server logs, anything else relevant).

### 4. evaluate
Runs the scenario's `assertions.sh` — typically grep-on-logs, file-exists checks, exit-code checks. Result is captured and rendered in the tape's `README.md` as a pass/fail table.


