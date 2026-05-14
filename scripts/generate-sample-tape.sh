#!/usr/bin/env bash
#
# generate-sample-tape.sh
#
# Generates the artifacts checked in at docs/sample-tape/ by driving
# warp-taper end-to-end against the synthetic demo binary at
# scripts/demo-rotation/.
#
# IMPORTANT — screen capture is OFF by default.
#
# Region capture (--screencapture-region X,Y,W,H) records pixels at those
# screen coordinates regardless of which app owns them. That made an
# earlier sample tape accidentally include a private Slack window. So this
# script defaults to the no-op recorder and does NOT produce a master.mov.
# To record a real .mov, pass a CGWindowID of the window you want
# captured via WINDOW_ID; the script forwards it to warp-taper as
# --screencapture-window-id, which uses screencapture's window-scoped
# `-l<id>` flag. Nothing outside the chosen window can leak.
#
# Discover a window's ID:
#     osascript -e 'tell application "System Events" to id of front \
#         window of (first process whose name is "Warp")'
#
# Usage:
#     # default — no .mov, just the logs/stages/jsonl artifacts
#     scripts/generate-sample-tape.sh
#
#     # record a specific window
#     WINDOW_ID=12345 scripts/generate-sample-tape.sh
#
# Env overrides:
#     DEMO_DURATION_MS   total recording window (default 6000)
#     WINDOW_ID          CGWindowID to capture; enables real screen
#                        recording. Unset = no .mov is produced.
#     KEEP_TMP=1         keep the staging directory after the run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_DIR="${REPO_ROOT}/scripts/demo-rotation"
TARGET_DIR="${REPO_ROOT}/docs/sample-tape"
DURATION_MS="${DEMO_DURATION_MS:-6000}"

cd "${REPO_ROOT}"

echo "==> building warp-taper-cli (release)"
cargo build --release --bin warp-taper >/dev/null

# Stage a clean WARP_OSS_LOG location and a clean MCP log dir so the demo
# binary's output lands somewhere predictable. We pass these via env to the
# warp-taper invocation; the demo binary reads WARP_OSS_LOG_PATH and
# DEMO_MCP_LOG_DIR.
STAGE="$(mktemp -d)"
WARP_LOG="${STAGE}/warp-oss.log"
MCP_DIR="${STAGE}/mcp"
SCENARIO_DIR="${STAGE}/scenario"
TAPE_DIR="${STAGE}/tape"
mkdir -p "${MCP_DIR}" "${SCENARIO_DIR}"
: >"${WARP_LOG}"

cat >"${SCENARIO_DIR}/metadata.yaml" <<EOF
title: "LLM-driven MCP log rotation"
ticket: "warpdotdev/warp#10882 (POC) — modeled in the warp-taper demo"
expected: |
  An MCP server log rotates at the configured size threshold. A rotation
  event lands in the always-on .rotations.jsonl sidecar, and an LLM-
  generated summary lands in the opt-in .summaries.jsonl sidecar. The
  active log starts fresh at zero bytes. No user-visible errors during
  rotation.
mcp_log_paths:
  - ${MCP_DIR}
EOF

echo "==> staging dir:  ${STAGE}"
echo "==> warp log:     ${WARP_LOG}"
echo "==> mcp dir:      ${MCP_DIR}"

if [[ -n "${WINDOW_ID:-}" ]]; then
    VIDEO_FLAGS=(--screencapture-window-id "${WINDOW_ID}")
    echo "==> recording window id ${WINDOW_ID}"
else
    VIDEO_FLAGS=(--no-screencapture)
    echo "==> no WINDOW_ID set — using no-op recorder; master.mov will be empty"
fi

# The demo binary needs its env vars; pass them into the warp-taper run so
# the deployed child inherits them.
echo "==> running warp-taper"
WARP_OSS_LOG_PATH="${WARP_LOG}" \
DEMO_MCP_LOG_DIR="${MCP_DIR}" \
DEMO_DURATION_MS="${DURATION_MS}" \
"${REPO_ROOT}/target/release/warp-taper" run \
    "${SCENARIO_DIR}" \
    --warp-source "${DEMO_DIR}" \
    --tape-dir "${TAPE_DIR}" \
    --warp-log "${WARP_LOG}" \
    --duration-ms "${DURATION_MS}" \
    --branch "$(git -C "${REPO_ROOT}" rev-parse --abbrev-ref HEAD)" \
    --head "$(git -C "${REPO_ROOT}" rev-parse --short HEAD)" \
    "${VIDEO_FLAGS[@]}" || true

if [[ ! -d "${TAPE_DIR}" ]]; then
    echo "warp-taper did not produce a tape directory at ${TAPE_DIR}" >&2
    exit 1
fi

echo "==> copying tape -> ${TARGET_DIR}"
rm -rf "${TARGET_DIR}"
mkdir -p "${TARGET_DIR}"
cp -R "${TAPE_DIR}/." "${TARGET_DIR}/"

# Drop master.mov unless a WINDOW_ID-driven capture produced one with real
# bytes. A 0-byte file is misleading and we don't commit empty .mov files.
if [[ -f "${TARGET_DIR}/master.mov" ]]; then
    if [[ ! -s "${TARGET_DIR}/master.mov" ]] || [[ -z "${WINDOW_ID:-}" ]]; then
        rm -f "${TARGET_DIR}/master.mov"
    fi
fi

# Sanitize transient absolute paths in the stage logs + README so the
# committed artifacts don't change byte-for-byte every run. Replaces the
# tmpdir with `<sample-tape>` and absolute paths under the demo dir with
# `<demo>`.
echo "==> sanitizing paths"
sanitize_one() {
    local f="$1"
    [[ -f "${f}" ]] || return 0
    sed -i.bak \
        -e "s|${STAGE}|<sample-tape>|g" \
        -e "s|/private<sample-tape>|<sample-tape>|g" \
        -e "s|${DEMO_DIR}|<demo>|g" \
        "${f}"
    rm -f "${f}.bak"
}
while IFS= read -r f; do
    sanitize_one "${f}"
done < <(
    find "${TARGET_DIR}/stages" -maxdepth 1 -type f -name '*.log' 2>/dev/null
    find "${TARGET_DIR}/logs" -type f 2>/dev/null
    [[ -f "${TARGET_DIR}/README.md" ]] && echo "${TARGET_DIR}/README.md"
)

if [[ -z "${KEEP_TMP:-}" ]]; then
    rm -rf "${STAGE}"
else
    echo "==> KEEP_TMP=1 — staging dir preserved at ${STAGE}"
fi

echo
echo "Sample tape regenerated at ${TARGET_DIR}"
ls -la "${TARGET_DIR}"
