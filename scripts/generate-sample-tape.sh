#!/usr/bin/env bash
#
# generate-sample-tape.sh
#
# Generates the artifacts checked in at docs/sample-tape/ by driving
# warp-taper end-to-end against the synthetic demo binary at
# scripts/demo-rotation/. Captures master.mov via screencapture without
# the interactive region picker (uses --screencapture-region), so the
# whole run is non-interactive.
#
# Usage:
#     scripts/generate-sample-tape.sh
#
# Env overrides:
#     DEMO_DURATION_MS   total recording window (default 6000)
#     REGION_RECT        screencapture -R rect (default 100,100,1200,720)
#     NO_VIDEO=1         skip screencapture; useful on CI / non-macOS
#     KEEP_TMP=1         keep the staging directory after the run

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_DIR="${REPO_ROOT}/scripts/demo-rotation"
TARGET_DIR="${REPO_ROOT}/docs/sample-tape"
# Defaults tuned to keep master.mov under a few MB on a Retina display
# (screencapture writes ~1.5 MB / sec at this size on modern macs).
DURATION_MS="${DEMO_DURATION_MS:-4000}"
REGION="${REGION_RECT:-100,100,720,480}"

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

VIDEO_FLAGS=(--screencapture-region "${REGION}")
if [[ -n "${NO_VIDEO:-}" ]]; then
    VIDEO_FLAGS=(--no-screencapture)
    echo "==> NO_VIDEO=1 — will use the no-op recorder"
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
    --branch "p9-real-sample-artifacts" \
    --head "$(git -C "${REPO_ROOT}" rev-parse --short HEAD)" \
    "${VIDEO_FLAGS[@]}" || true

if [[ ! -d "${TAPE_DIR}" ]]; then
    echo "warp-taper did not produce a tape directory at ${TAPE_DIR}" >&2
    exit 1
fi

echo "==> copying tape -> ${TARGET_DIR}"
rm -rf "${TARGET_DIR}"
mkdir -p "${TARGET_DIR}"
# Only copy what we want to commit. master.mov, logs/, stages/, README.md.
cp -R "${TAPE_DIR}/." "${TARGET_DIR}/"

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
