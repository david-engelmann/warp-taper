# Scenario — MCP log rotation kicks in at the size cap

This scenario reproduces the bug fixed by PR #10874: an MCP server's log file
should rotate after writing past the configured size threshold (default
10 MiB × 5 rotated copies = 60 MiB cap per server). Before the fix the file
grew without bound.

## Steps

1. **Setup** — `setup.sh` clears any rotated files from previous runs in the
   MCP log directory so the post-recording assertions start from a clean slate.
2. **Deploy** — `warp-taper` launches `warp-oss` against the current branch.
3. **Drive (manual)** — in the running Warp window:
   - Open or create any MCP server that streams stderr at a reasonable cadence.
     The chattiest options:
       - **`david-rocks`** (prompts-only FastMCP — minimal logging; not ideal
         for this scenario unless you crank the verbosity)
       - **`glasspane` debugger** (run a long debug session — moderate volume)
       - **Any verbose third-party MCP server** (iMCP, GitHub MCP, etc.)
   - Generate enough log traffic that the active log crosses the rotation
     threshold. For the default cap (10 MiB) this is several hundred thousand
     log lines.
   - You can shortcut this by running `assertions.sh`'s built-in volume
     generator (`--simulate-volume`) which appends synthetic lines directly
     to the active log file. Set `WARP_TAPER_SIMULATE=1` in `setup.sh` to
     enable.

4. **Stop the recording** — once the rotation has visibly happened (you can
   watch the log directory in a sidecar terminal: `watch -n 1 ls -la $MCP_LOG_DIR`).

5. **Evaluate** — `assertions.sh` checks that at least `.1` exists in the
   MCP log directory, the active file is smaller than the threshold, and
   no error toasts appeared in the recording (manual inspection of
   `master.mov`).

## What to look for in the recording

- The MCP server still operates normally during rotation (no toasts, no
  drops, the connection in Settings → MCP Servers stays green).
- At the moment of rotation, the active file's size drops back to near zero
  (the rotation event ends a file and starts a fresh one).

## What to look for in the session log

- `warp-oss.session.log` contains the rotation log line:
  `SimpleLogger: rotation completed for ...`  
  (or, if the build didn't have INFO logging enabled, a corresponding
  `WARN`-level line on failure paths).
