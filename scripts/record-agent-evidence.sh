#!/usr/bin/env bash
#
# record-agent-evidence.sh
#
# End-to-end visual-anchored agent-side evidence recorder for Warp PRs
# that change agent behavior. The actual interaction (activate, OCR
# anchors, clicks, typing, key presses, waits) is defined in a recipe
# JSON consumed by scripts/warp-driver.swift, so any future PR can ship
# its own recipe under scripts/recipes/ and reuse this orchestrator.
#
# Pipeline:
#   1. Validate the target Warp/warp-oss process is running.
#   2. Start ScreenCaptureKit recording of that process's front window.
#   3. After a warmup, invoke warp-driver.swift with the recipe. The
#      driver:
#        - OCR-anchors each typing/clicking step, aborting if the
#          expected UI text isn't visible.
#        - Refuses to continue if defensive `abort_if_text` patterns
#          (universal search, shell history dropdown) are detected.
#   4. Wait for the recorder to finish. If the driver aborted, the
#      .mov captured up to that point is intentionally NOT converted —
#      we don't want to publish video of a misfire.
#   5. Optionally convert .mov → .mp4 → .gif for inline GitHub
#      embedding.
#
# Env knobs:
#     RECIPE         Path to recipe JSON (REQUIRED). Either an absolute
#                    path or a name resolved under scripts/recipes/.
#     PROC_NAME      Process name to record. Defaults to the recipe's
#                    target_process field; pass to override.
#     DURATION_S     Total capture length in seconds (default 32).
#     ACTIVATE_DELAY_S Warmup before invoking the driver (default 2).
#     OUTPUT         Destination .mov path (default
#                    docs/sample-tape/agent-evidence.mov).
#     EMIT_GIF       "1" to also produce <output>.gif on success.
#                    GIF tuning via GIF_FPS / GIF_MAX_WIDTH (6 / 720).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -z "${RECIPE:-}" ]]; then
    echo "record-agent-evidence.sh: RECIPE env var is required." >&2
    echo "  example: RECIPE=9853-mcp-fresh-conversation $0" >&2
    exit 2
fi

# Resolve RECIPE: absolute path wins; otherwise scripts/recipes/<name>.json.
if [[ "${RECIPE}" == /* && -f "${RECIPE}" ]]; then
    RECIPE_PATH="${RECIPE}"
elif [[ -f "${REPO_ROOT}/scripts/recipes/${RECIPE}.json" ]]; then
    RECIPE_PATH="${REPO_ROOT}/scripts/recipes/${RECIPE}.json"
elif [[ -f "${REPO_ROOT}/scripts/recipes/${RECIPE}" ]]; then
    RECIPE_PATH="${REPO_ROOT}/scripts/recipes/${RECIPE}"
else
    echo "record-agent-evidence.sh: cannot resolve recipe '${RECIPE}'" >&2
    exit 2
fi

PROC_NAME_FROM_RECIPE=$(/usr/bin/python3 -c "
import json, sys
print(json.load(open(sys.argv[1])).get('target_process', ''))
" "${RECIPE_PATH}")
PROC_NAME="${PROC_NAME:-${PROC_NAME_FROM_RECIPE}}"
if [[ -z "${PROC_NAME}" ]]; then
    echo "record-agent-evidence.sh: no process name (recipe lacks target_process, no PROC_NAME)" >&2
    exit 2
fi

DURATION_S="${DURATION_S:-32}"
ACTIVATE_DELAY_S="${ACTIVATE_DELAY_S:-2}"
OUTPUT="${OUTPUT:-${REPO_ROOT}/docs/sample-tape/agent-evidence.mov}"
EMIT_GIF="${EMIT_GIF:-0}"
GIF_FPS="${GIF_FPS:-8}"
GIF_MAX_WIDTH="${GIF_MAX_WIDTH:-1280}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "record-agent-evidence.sh: macOS only." >&2
    exit 1
fi

SWIFT_RECORDER="${REPO_ROOT}/scripts/record-window.swift"
SWIFT_DRIVER="${REPO_ROOT}/scripts/warp-driver.swift"
for f in "${SWIFT_RECORDER}" "${SWIFT_DRIVER}"; do
    [[ -f "${f}" ]] || { echo "missing ${f}." >&2; exit 1; }
done

if ! pgrep -x "${PROC_NAME}" >/dev/null 2>&1; then
    if ! pgrep -i "${PROC_NAME}" >/dev/null 2>&1; then
        # Fall back to macOS displayed name (handles apps whose
        # executable differs from their bundle, e.g. Stable Warp:
        # executable `stable`, displayed name `Warp`).
        if ! osascript -e "tell application \"System Events\" to exists (first process whose displayed name is \"${PROC_NAME}\")" 2>/dev/null | grep -q true; then
            echo "record-agent-evidence.sh: no running process matches '${PROC_NAME}'." >&2
            exit 1
        fi
    fi
fi

mkdir -p "$(dirname "${OUTPUT}")"
rm -f "${OUTPUT}"

REC_LOG="/tmp/record-agent-evidence.recorder.log"
DRV_LOG="/tmp/record-agent-evidence.driver.log"

# Activate the target window BEFORE starting the recorder. The
# ScreenCaptureKit content list returns only on-screen windows, so if
# the target is in another space / minimized / behind another window
# when the recorder polls for it, recording fails to start.
osascript -e "tell application \"System Events\" to set frontmost of (first process whose name is \"${PROC_NAME}\") to true" 2>/dev/null || true
sleep 1

echo "==> recipe:    ${RECIPE_PATH}"
echo "==> recording: ${PROC_NAME} → ${OUTPUT} (${DURATION_S}s)"
swift "${SWIFT_RECORDER}" "${PROC_NAME}" "${DURATION_S}" "${OUTPUT}" 2>"${REC_LOG}" &
REC_PID=$!

sleep "${ACTIVATE_DELAY_S}"

echo "==> driver:    warp-driver.swift ${RECIPE_PATH}"
set +e
swift "${SWIFT_DRIVER}" "${RECIPE_PATH}" 2>"${DRV_LOG}"
DRV_EXIT=$?
set -e

echo "==> waiting for recorder to finish"
wait "${REC_PID}"

# Driver logs always echo so the operator can see anchor decisions.
echo "----- driver log -----"
cat "${DRV_LOG}"
echo "----------------------"

if [[ "${DRV_EXIT}" -ne 0 ]]; then
    echo "record-agent-evidence.sh: driver exited ${DRV_EXIT} — NOT emitting GIF; .mov left at ${OUTPUT} for inspection." >&2
    exit "${DRV_EXIT}"
fi

if [[ ! -s "${OUTPUT}" ]]; then
    echo "record-agent-evidence.sh: recorder produced no bytes at ${OUTPUT}." >&2
    echo "  recorder stderr: $(tail -5 "${REC_LOG}" 2>/dev/null)" >&2
    exit 1
fi

BYTES="$(wc -c <"${OUTPUT}" | tr -d ' ')"
echo "wrote ${OUTPUT} (${BYTES} bytes)"

if [[ "${EMIT_GIF}" == "1" ]]; then
    MP4="${OUTPUT%.mov}.mp4"
    GIF="${OUTPUT%.mov}.gif"
    echo "==> converting .mov → .mp4 → .gif"
    swift "${REPO_ROOT}/scripts/mov-to-mp4.swift" "${OUTPUT}" "${MP4}"
    swift "${REPO_ROOT}/scripts/mp4-to-gif.swift" "${MP4}" "${GIF}" "${GIF_FPS}" "${GIF_MAX_WIDTH}"
    echo "wrote ${GIF} ($(wc -c <"${GIF}" | tr -d ' ') bytes)"
fi
