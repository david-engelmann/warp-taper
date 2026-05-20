# warpdotdev/warp PR #11360 — `+ Add` save now invokes the predicate

Fix-in-action evidence for [PR #11360](https://github.com/warpdotdev/warp/pull/11360).

Build state for this recording: PR #10839's predicate + PR #11360's new-server detection loop + DEMO-PATCH-B (so `SECRETS_REGEX` recompiles when the toggle is flipped). `headers.Authorization` is intentionally avoided in the demo so the behavior isolates **this PR's** fix and is not confounded by the still-open #11263 (env/headers templatization wipes secrets before the scan).

Four phases:

- **A** Settings → Privacy → Secret redaction toggle is normalized to OFF.
- **B** Click toggle ON. "Secret visual redaction mode" dropdown appears.
- **C** Settings → MCP Servers → `+ Add` → paste a config with `sk-FAKE0000000000FAKEdemoWARP11360fix` in the top-level `url`.
- **D** Click Save → error toast appears: *"This MCP server contains secrets. Visit Settings > Privacy to modify your secret redaction settings."* The modal stays open with the secret-bearing JSON; no entry is created in MY MCPS.

Pre-PR (against the unpatched build) this same save proceeded silently and the entry landed in MY MCPS — see `../11265-mcp-add-server-bypass/master.gif` for the contrasting bug reproduction.
