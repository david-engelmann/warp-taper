# warp-taper

> A taper at the Dead show, but for Warp.

`warp-taper` is a recording-evidence toolkit for Warp Terminal: it **builds**, **deploys**, **records**, and **evaluates** Warp behavior from a single repo, producing PR-ready "tapes" (evidence bundles) you can drag straight into a GitHub issue or PR comment.

The name nods to Grateful Dead taper culture — the community of fans who recorded shows with high-fidelity gear and shared them under the Dead's open-tape policy. Same idea here: a clean recording of behavior, archived for posterity, ready to share.

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

## Lingo (Dead-taper-ish, for fun)

The vocabulary follows the source material:

| `warp-taper` term  | What it means                                           | Dead-taper analogue |
|--------------------|---------------------------------------------------------|---------------------|
| **tape**           | An evidence bundle for one scenario.                    | A single show recording |
| **vault**          | The collection of all tapes in `tapes/`.                | The Vault (official archive) |
| **master**         | The primary screen recording in a tape.                 | The master copy of a recording |
| **patch**          | A snapshot capture at a named point in a scenario.      | A patched audience+SBD recording |
| **setlist**        | The ordered steps a scenario runs.                      | The list of songs played |
| **SBD**            | A "soundboard" recording — direct log capture, no UI.   | Direct soundboard feed (highest fidelity) |
| **AUD**            | An "audience" recording — screen capture of the UI.     | Audience mic recording |
| **encore**         | Optional follow-up assertions run after the main bundle.| The encore at the end of the show |

## Quick start

```bash
# Drop a scenario in scenarios/<ticket>/
$ ls scenarios/7723-mcp-log-rotation/
scenario.md     # human-readable steps
setup.sh        # fixture setup before recording
assertions.sh   # post-recording programmatic checks
metadata.yaml   # title, ticket ref, expected behavior

# Tape it
$ warp-taper run scenarios/7723-mcp-log-rotation/

# A full bundle lands in:
$ ls tapes/7723-mcp-log-rotation/
README.md       # PR-ready summary (drag into a PR comment)
metadata.yaml   # what was recorded, when, against which branch
master.mov      # screen capture
patches/        # named stills (01-before.png, 02-after.png, ...)
logs/           # warp.log tail, MCP server logs, build output
stages/         # per-stage output from build/deploy/record/evaluate
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

## Status

Early v1. Manual driving (you click; the tool captures). Auto-driving Warp via AppleScript is a v2 add-on if needed.

Built to scratch the itch of producing evidence for [my own Warp PRs](https://github.com/warpdotdev/warp/pulls?q=is%3Apr+author%3Adavid-engelmann). Fork-friendly if your itch is similar.

## License

MIT — see [LICENSE](LICENSE).
