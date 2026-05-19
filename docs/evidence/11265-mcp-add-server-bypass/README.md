# warpdotdev/warp#11265 — `+ Add` (new MCP server) bypasses redaction

Build = DEMO-PATCH-B + DEMO-PATCH-C, no DEMO-PATCH-C-NEW. The
recording shows:

- A: toggle starts OFF
- B: click toggle ON → dropdown appears
- C: `+ Add` → paste a baseline (no-secret) config → Save succeeds,
  establishes `demo-baseline-11265` in MY MCPS
- D: `+ Add` again → paste config with `sk-…` in the top-level `url` →
  Save → server lands in MY MCPS — **BUG**: the new-server save branch
  in `MCPServersEditPageView::handle_action` calls
  `ParsedTemplatableMCPServerResult::from_user_json` directly, skipping
  `parse_templatable_json` where the predicate (and DEMO-PATCH-C's raw
  scan) lives. Same JSON saved via edit-existing routes through the
  predicate and is blocked.
