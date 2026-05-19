# warpdotdev/warp#11262 — SECRETS_REGEX silently empty after toggle ON

Unpatched build (no DEMO-PATCH-B). The runtime SECRETS_REGEX is only
recompiled on `CustomSecretRegexList` change events — `SafeMode`
toggle changes never trigger a recompile. The recording shows:

- A: toggle starts OFF (visual)
- B: click toggle ON → "Secret visual redaction mode" dropdown appears
  (UI says safe_mode_enabled=true)
- C: save MCP config with `sk-…` in the URL
- D: server lands in MY MCPS — **BUG**: PR #10839's predicate ran but
  `find_secrets_in_text` returned `[]` because SECRETS_REGEX is still
  empty, so `contains_secrets=false` and the save was not blocked.
