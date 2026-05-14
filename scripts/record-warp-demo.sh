#!/usr/bin/env bash
#
# record-warp-demo.sh
#
# End-to-end "Playwright-for-Warp" demo:
#
#   1. Start ScreenCaptureKit recording the Warp window (buffer-scoped).
#   2. After a brief warmup, activate Warp and synthesize a typed command
#      via CGEventPost (scripts/drive-warp.swift).
#   3. Let the command run for a few seconds so its output scrolls in the
#      window.
#   4. Stop the recording. Output lands at docs/sample-tape/master.mov.
#
# The window-buffer recorder ignores anything overlapping Warp on screen,
# so other apps cannot leak into the .mov.
#
# Env knobs:
#     DEMO_COMMAND     Text the driver types into Warp. Default exercises
#                      the scripts/demo-rotation binary checked in here,
#                      which simulates the warp PR #10882 rotation flow.
#     DURATION_S       Recording length in seconds (default 14).
#     WARP_ACTIVATE_DELAY_S
#                      Delay before activating Warp & typing (default 2).
#     PROC_NAME        Process to record; default "Warp".
#     OUTPUT           Destination .mov (default docs/sample-tape/master.mov).
#
# Requirements:
#   - macOS 12.3+ (ScreenCaptureKit)
#   - Screen Recording permission for the parent app
#   - Accessibility permission for the parent app (so CGEventPost can
#     drive keystrokes into Warp). System Settings → Privacy & Security →
#     Accessibility.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${OUTPUT:-${REPO_ROOT}/docs/sample-tape/master.mov}"
PROC_NAME="${PROC_NAME:-Warp}"
DURATION_S="${DURATION_S:-14}"
ACTIVATE_DELAY_S="${WARP_ACTIVATE_DELAY_S:-2}"
DEFAULT_CMD="cd ${REPO_ROOT} && WARP_OSS_LOG_PATH=/tmp/warp-taper-demo.log DEMO_MCP_LOG_DIR=/tmp/warp-taper-demo-mcp DEMO_DURATION_MS=8000 ./scripts/demo-rotation/target/release/warp-oss"
DEMO_COMMAND="${DEMO_COMMAND:-${DEFAULT_CMD}}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "record-warp-demo.sh: macOS only." >&2
    exit 1
fi

mkdir -p "$(dirname "${OUTPUT}")"
rm -f "${OUTPUT}"

# Make sure the demo binary exists; build it lazily if not.
DEMO_BIN="${REPO_ROOT}/scripts/demo-rotation/target/release/warp-oss"
if [[ ! -x "${DEMO_BIN}" ]]; then
    echo "==> building scripts/demo-rotation (one-time)"
    (cd "${REPO_ROOT}/scripts/demo-rotation" && cargo build --release >/dev/null)
fi

echo "==> starting ScreenCaptureKit recording (${DURATION_S}s) of \"${PROC_NAME}\"'s window"
swift "${REPO_ROOT}/scripts/record-window.swift" "${PROC_NAME}" "${DURATION_S}" "${OUTPUT}" 2>/tmp/record-warp-demo.recorder.log &
REC_PID=$!

# Wait a moment so the recorder is capturing before we drive Warp.
sleep "${ACTIVATE_DELAY_S}"

echo "==> activating Warp + typing demo command"
swift "${REPO_ROOT}/scripts/drive-warp.swift" "${DEMO_COMMAND}" 2>/tmp/record-warp-demo.driver.log

echo "==> letting the command run; waiting for recorder to finish"
wait "${REC_PID}"

if [[ ! -s "${OUTPUT}" ]]; then
    echo "record-warp-demo.sh: recorder produced no bytes at ${OUTPUT}." >&2
    echo "  recorder stderr: $(cat /tmp/record-warp-demo.recorder.log 2>/dev/null | tail -5)" >&2
    exit 1
fi

BYTES="$(wc -c <"${OUTPUT}" | tr -d ' ')"
echo "wrote ${OUTPUT} (${BYTES} bytes)"

# Browsers (Chrome / Firefox / Edge) don't reliably play .mov inline on
# GitHub's rendered README, even from raw URLs. Remux the H.264 stream
# into an .mp4 container (no re-encode) so the README's bare-URL embed
# works in every browser.
MP4_OUTPUT="${OUTPUT%.mov}.mp4"
if [[ "${MP4_OUTPUT}" != "${OUTPUT}" ]]; then
    echo "==> remuxing to ${MP4_OUTPUT} for cross-browser playback"
    swift "${REPO_ROOT}/scripts/mov-to-mp4.swift" "${OUTPUT}" "${MP4_OUTPUT}" >/dev/null
    MP4_BYTES="$(wc -c <"${MP4_OUTPUT}" | tr -d ' ')"
    echo "wrote ${MP4_OUTPUT} (${MP4_BYTES} bytes)"
fi

echo
echo "Embed in README.md (bare URL — GitHub renders an inline player):"
echo "    https://github.com/david-engelmann/warp-taper/raw/main/docs/sample-tape/master.mp4"
