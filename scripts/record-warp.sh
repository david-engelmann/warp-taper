#!/usr/bin/env bash
#
# record-warp.sh
#
# Records a video of *your currently-running Warp window* and lands it at
# docs/sample-tape/master.mov. Uses ScreenCaptureKit (via the Swift
# helper at scripts/record-window.swift) to capture the window's own
# backing buffer — overlapping apps and notifications cannot enter the
# .mov even if they're on top of Warp during the recording.
#
# This is intentionally different from `screencapture -v -l<id>`, which
# captures the screen region at the window's coordinates and so picks up
# whatever is rendered there (overlapping apps included).
#
# Workflow:
#   1. Open the Warp window you want to record (build from any commit;
#      show whatever feature/fix you're demonstrating).
#   2. Run this script from another terminal (NOT inside the Warp window
#      being recorded). The script counts down 3 seconds, then records
#      for DURATION_S seconds, then exits.
#   3. Commit docs/sample-tape/master.mov and update the README's <video>
#      tag if you want it embedded.
#
# Env overrides:
#   PROC_NAME      Process to record (default: Warp). Pass "warp-oss" if
#                  you launched warp-oss directly without the .app bundle.
#   DURATION_S     Recording length in seconds (default: 8).
#   OUTPUT         Destination .mov path (default: docs/sample-tape/master.mov).
#
# Requirements:
#   - macOS
#   - Screen Recording permission for the parent app (Terminal / iTerm /
#     VSCode). System Settings → Privacy & Security → Screen & System
#     Audio Recording.

set -euo pipefail

PROC_NAME="${PROC_NAME:-Warp}"
DURATION_S="${DURATION_S:-8}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${OUTPUT:-${REPO_ROOT}/docs/sample-tape/master.mov}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "record-warp.sh: macOS only (uses /usr/sbin/screencapture)." >&2
    exit 1
fi

SWIFT_RECORDER="${REPO_ROOT}/scripts/record-window.swift"
if [[ ! -f "${SWIFT_RECORDER}" ]]; then
    echo "record-warp.sh: missing ${SWIFT_RECORDER}." >&2
    exit 1
fi

mkdir -p "$(dirname "${OUTPUT}")"
rm -f "${OUTPUT}"

echo "==> recording ${PROC_NAME}'s window for ${DURATION_S}s using ScreenCaptureKit."
echo "    interact with the window during the recording (idle windows compress to almost nothing)."
echo "    starting in:"
for i in 3 2 1; do
    echo "    ${i}..."
    sleep 1
done

# Window-buffer capture — no screen region involved.
swift "${SWIFT_RECORDER}" "${PROC_NAME}" "${DURATION_S}" "${OUTPUT}"

if [[ ! -s "${OUTPUT}" ]]; then
    echo "record-warp.sh: recorder exited but produced no bytes at ${OUTPUT}." >&2
    echo "  - Is Screen Recording permission granted to the terminal running this script?" >&2
    exit 1
fi

BYTES="$(wc -c <"${OUTPUT}" | tr -d ' ')"
echo
echo "wrote ${OUTPUT}"
echo "  size: ${BYTES} bytes"
echo
echo "Tip: a static (no-interaction) recording can be only a few KB — ScreenCaptureKit emits"
echo "frames only when content changes. If the file feels too small, re-record while typing"
echo "or running a command in the Warp window."
echo
echo "Embed in README.md:"
echo
echo "    <video src=\"docs/sample-tape/master.mov\" controls width=\"720\"></video>"
