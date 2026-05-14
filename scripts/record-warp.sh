#!/usr/bin/env bash
#
# record-warp.sh
#
# Records a video of *your currently-running Warp window* and lands it at
# docs/sample-tape/master.mov. Bounded strictly to Warp's window via
# screencapture's -l<windowid> flag — nothing outside Warp's pixels can
# enter the .mov.
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

echo "==> looking for the front window of process \"${PROC_NAME}\""
WID=$(osascript <<EOF
tell application "System Events"
    try
        set procs to (every process whose name is "${PROC_NAME}")
        if (count procs) is 0 then return ""
        set wins to (windows of (item 1 of procs))
        if (count wins) is 0 then return ""
        return id of (item 1 of wins)
    on error
        return ""
    end try
end tell
EOF
)
WID="$(echo "${WID}" | tr -d '[:space:]')"

if [[ -z "${WID}" ]]; then
    echo "record-warp.sh: no visible window found for process \"${PROC_NAME}\"." >&2
    echo "  - Is the app open?" >&2
    echo "  - If you launched warp-oss directly (not via Warp.app), try PROC_NAME=warp-oss." >&2
    echo "  - Run \`osascript -e 'tell application \"System Events\" to get name of every process whose visible is true'\`" >&2
    echo "    to see the names macOS knows your app by." >&2
    exit 1
fi

echo "==> found window id ${WID}"
mkdir -p "$(dirname "${OUTPUT}")"
rm -f "${OUTPUT}"

echo "==> recording ${DURATION_S}s. starting in:"
for i in 3 2 1; do
    echo "    ${i}..."
    sleep 1
done
echo "==> RECORDING (do not switch focus away from the Warp window)"

# `-l<id>` is window-scoped: only the chosen window's pixels are
# captured, even if other apps overlap it. Safe by construction.
screencapture -v -V "${DURATION_S}" -l"${WID}" "${OUTPUT}"

if [[ ! -s "${OUTPUT}" ]]; then
    echo "record-warp.sh: screencapture exited but produced no bytes at ${OUTPUT}." >&2
    echo "  - Is Screen Recording permission granted to the terminal running this script?" >&2
    exit 1
fi

BYTES="$(wc -c <"${OUTPUT}" | tr -d ' ')"
echo
echo "wrote ${OUTPUT}"
echo "  size: ${BYTES} bytes"
echo
echo "To embed in README.md (GitHub renders <video> inline on the rendered file view):"
echo
echo "    <video src=\"docs/sample-tape/master.mov\" controls width=\"720\"></video>"
