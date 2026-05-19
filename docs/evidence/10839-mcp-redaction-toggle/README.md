# warpdotdev/warp#10839 — secret-redaction toggle gates MCP save

Truth-table evidence for PR #10839 (fixes #8761). Patched build (all
three follow-up demo patches applied). 6 phases recorded:

- A: confirm toggle is OFF
- B: flip toggle ON → "Secret visual redaction mode" dropdown appears
- C: try to save MCP config with `Bearer sk-…` in headers → BLOCKED (toast)
- D: flip toggle OFF
- E: save the same secret config → SUCCEEDS (server lands in MY MCPS)
- F: flip toggle back ON → dropdown returns
