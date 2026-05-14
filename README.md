# warp-taper

Evidence-recording toolkit for [warpdotdev/warp](https://github.com/warpdotdev/warp) PRs. Runs a scenario against a local Warp build, captures screen + logs, runs programmatic assertions, and emits a bundle ready to attach to a PR comment.

## Pipeline

```
build      cargo build -p warp against $WARP_SOURCE
deploy     run the scenario's setup, launch warp-oss in background
record     screen capture + log tail during the scenario
evaluate   run the scenario's assertions
bundle     emit a PR-ready README.md referencing the captured artifacts
```

## Usage

```sh
./bin/warp-taper run scenarios/<scenario-dir>/
```

Output lands in `tapes/<scenario>/` (gitignored).

## Scenario format

See [docs/scenario-format.md](docs/scenario-format.md). Reference scenario: `scenarios/10874-mcp-log-rotation/`.

## Requirements

macOS. Uses `screencapture` for video + stills.

## License

MIT.
