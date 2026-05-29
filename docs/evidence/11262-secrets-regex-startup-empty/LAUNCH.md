# Re-record evidence for warpdotdev/warp PR #11457

Lucie's [review](https://github.com/warpdotdev/warp/pull/11457) asked for native
MP4 footage with explicit before-and-after states. The warp-taper scenarios
and recipes are prepped (committed in this repo); this runbook fires them.

**Total wall-clock:** ~30–40 minutes. macOS UI is commandeered during the
recording phases — do not touch the machine while warp-taper is driving
warp-oss.

---

## 0. Prep checkouts

Two source trees, one warp-oss build each.

```bash
# Master (the BEFORE binary)
cd /Users/david/personal/warp
git fetch upstream master
git worktree add /tmp/warp-master upstream/master

# Patched (the AFTER binary — PR #11457 head)
git fetch fork david/11262-secrets-regex-safe-mode-subscription
git worktree add /tmp/warp-patched fork/david/11262-secrets-regex-safe-mode-subscription
```

## 1. Build both warp-oss release binaries (in parallel)

```bash
cd /tmp/warp-master && cargo build -p warp --release --features local_tty,local_fs &
cd /tmp/warp-patched && cargo build -p warp --release --features local_tty,local_fs &
wait
```

Each takes ~10–15 min on a cold cache. Verify both binaries exist:

```bash
ls -lh /tmp/warp-master/target/release/warp /tmp/warp-patched/target/release/warp
```

## 2. Build warp-taper release binary

```bash
cd /Users/david/personal/warp-taper && cargo build --release
ls -lh target/release/warp-taper-cli
```

## 3. Run the BEFORE recording (master binary)

Memory says: **full nuke between runs.** Kill warp-oss, wipe SQLite + group container.

```bash
pkill -9 warp-oss 2>/dev/null; sleep 2
rm -f ~/Library/Group\ Containers/2BBY89MBSN.dev.warp/Library/Application\ Support/dev.warp.WarpOss/warp.sqlite*
# (re-prime auth if needed — keep the existing prime-warp-oss-auth.sh helper)
```

Launch warp-oss-master in the background, then drive the recipe:

```bash
/tmp/warp-master/target/release/warp &
sleep 8  # let UI come up; OCR gating handles the rest
cd /Users/david/personal/warp-taper && ./target/release/warp-taper-cli \
  run-scenario 11262-secrets-regex-startup-empty \
  --recipe scripts/recipes/11262-secrets-regex-startup-empty.json \
  --tape-dir tapes/11262-secrets-regex-startup-empty
```

Output lands at:

- `tapes/11262-secrets-regex-startup-empty/master.mov` (raw native MP4 container)
- `tapes/11262-secrets-regex-startup-empty/master-captioned.mp4` ← **ship this**
- `tapes/11262-secrets-regex-startup-empty/master-captioned.gif`
- `/tmp/11262-phase-{a,b,c,d}-*.png` (per-phase screenshots)

## 4. Run the AFTER recording (patched binary)

Full nuke again:

```bash
pkill -9 warp-oss 2>/dev/null; sleep 2
rm -f ~/Library/Group\ Containers/2BBY89MBSN.dev.warp/Library/Application\ Support/dev.warp.WarpOss/warp.sqlite*
```

Launch the patched binary, drive the patched recipe:

```bash
/tmp/warp-patched/target/release/warp &
sleep 8
cd /Users/david/personal/warp-taper && ./target/release/warp-taper-cli \
  run-scenario 11262-secrets-regex-startup-empty-patched \
  --recipe scripts/recipes/11262-secrets-regex-startup-empty-patched.json \
  --tape-dir tapes/11262-secrets-regex-startup-empty-patched
```

Output at `tapes/11262-secrets-regex-startup-empty-patched/master-captioned.mp4`
← **ship this**.

## 5. Review (REQUIRED — do not skip)

The user explicitly asked for review before posting. Concretely:

```bash
# (a) durations should be close (BEFORE ~145s, AFTER ~140-150s)
ffprobe -v error -show_entries format=duration -of csv=p=0 \
  tapes/11262-secrets-regex-startup-empty/master-captioned.mp4
ffprobe -v error -show_entries format=duration -of csv=p=0 \
  tapes/11262-secrets-regex-startup-empty-patched/master-captioned.mp4

# (b) extract key frames for visual inspection
for tape in 11262-secrets-regex-startup-empty 11262-secrets-regex-startup-empty-patched; do
  mkdir -p /tmp/review-$tape
  ffmpeg -loglevel error -y -i tapes/$tape/master-captioned.mp4 \
    -vf "select='eq(n,0)+eq(n,300)+eq(n,1200)+eq(n,2700)+eq(n,3500)+eq(n,4200)'" \
    -vsync vfr /tmp/review-$tape/frame-%02d.png
done

# (c) verify the per-phase screenshots show the expected UI state
ls -lh /tmp/11262-*phase-*.png
```

Then visually walk through each frame set in order:

**BEFORE — checklist:**
1. Frame ~0s shows the BEFORE intro caption ("BEFORE - Bug #11262: SECRETS_REGEX silently empty…").
2. Frame ~28s shows the Settings → Secret redaction page, toggle still OFF, Step 1 caption visible.
3. Frame ~48s shows the "Secret visual redaction mode" dropdown (UI says enabled), Step 2 caption visible.
4. Frame ~98s shows the MCP +Add modal with the sk-FAKE… secret pasted, Step 3 caption visible.
5. Frame ~110s shows the Save click moment, Step 4 caption visible.
6. Frame ~140s shows `demo-regex-empty-11262` *appearing in MY MCPS* + "BUG: Save SUCCEEDED" caption visible.

**AFTER — checklist:**
1. Frame ~0s shows the AFTER intro caption (mentions PR #11457).
2. Frames ~28–110s should look indistinguishable from BEFORE except caption wording.
3. Frame ~140s shows the toast "This MCP server contains secrets…" + "FIX: Save BLOCKED" caption visible. `demo-regex-empty-11262` should NOT be in MY MCPS.

If any of the above fails (caption missing, UI not visible, recipe stuck) — DO NOT POST. Re-record the failing one with a higher screen-recording resolution or wider window. Lucie was already burned once.

## 6. Upload + comment on PR #11457

Once reviewed and confirmed:

```bash
# copy artifacts into docs/evidence so they get a public raw URL
cp tapes/11262-secrets-regex-startup-empty/master-captioned.mp4 \
   docs/evidence/11262-secrets-regex-startup-empty/before-captioned.mp4
mkdir -p docs/evidence/11262-secrets-regex-startup-empty-patched
cp tapes/11262-secrets-regex-startup-empty-patched/master-captioned.mp4 \
   docs/evidence/11262-secrets-regex-startup-empty-patched/after-captioned.mp4
git add docs/evidence/11262-secrets-regex-startup-empty/before-captioned.mp4 \
        docs/evidence/11262-secrets-regex-startup-empty-patched/
git commit -m "evidence: re-recorded #11262 BEFORE/AFTER for warp PR #11457"
git push
```

Then post on warpdotdev/warp#11457:

> @lucieleblanc thanks for the clear ask. Re-recorded as two captioned `.mp4`s (no more GIF re-encode) walking the same UI path through master and the patched build. Captions name each step, and the divergence is the last 30 s of each: BEFORE — `demo-regex-empty-11262` appears in MY MCPS (save succeeded, SECRETS_REGEX was empty). AFTER — `"This MCP server contains secrets…"` toast (save blocked, the new `SafeModeEnabled` subscription compiled the regex DFA from the TOML list).
>
> BEFORE: https://github.com/david-engelmann/warp-taper/raw/main/docs/evidence/11262-secrets-regex-startup-empty/before-captioned.mp4
> AFTER: https://github.com/david-engelmann/warp-taper/raw/main/docs/evidence/11262-secrets-regex-startup-empty-patched/after-captioned.mp4
