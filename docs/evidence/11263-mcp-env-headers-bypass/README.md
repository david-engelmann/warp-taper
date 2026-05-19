# warpdotdev/warp#11263 — secrets in env/headers bypass redaction

Build = DEMO-PATCH-B only (SECRETS_REGEX recompiles on toggle), no
DEMO-PATCH-C. The recording shows:

- A: toggle starts OFF
- B: click toggle ON → dropdown appears, SECRETS_REGEX is populated
- C: save MCP config with `Bearer sk-…` in `headers.Authorization` →
  server lands in MY MCPS — **BUG**: `ParsedTemplatableMCPServerResult::from_user_json`
  templatized the bearer to `{{ServerName_API_KEY}}` before
  `find_secrets_in_text` scanned `template.json`, so the predicate
  saw no secret and let the save through.
