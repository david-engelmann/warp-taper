#!/usr/bin/env bash
# Assertions for the MCP-log-rotation scenario.
#
# These run after the recording finishes. Variables exported by the runner:
#   TAPE_DIR        absolute path to the bundle root
#   TAPE_LOGS       ${TAPE_DIR}/logs
#   TAPE_SESSION    ${TAPE_DIR}/logs/warp-oss.session.log  (may not exist)
#   TAPE_MCP_LOGS   ${TAPE_DIR}/logs/mcp                   (may not exist)
#
# The scenario's metadata declares `mcp_log_paths` pointing at the live MCP log
# directory; record_session copies the directory's contents into
# ${TAPE_MCP_LOGS}. By the time this runs, ${TAPE_MCP_LOGS} is a snapshot of
# what was on disk at end-of-recording.

set -uo pipefail

# Source the helpers from the warp-taper repo. We look two levels up from the
# scenario directory.
SCENARIO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCENARIO_DIR}/../.." && pwd)"
# shellcheck source=../../lib/evaluate.sh
source "${REPO_ROOT}/lib/evaluate.sh"

fail_count=0

echo "Assertions for ${SCENARIO_DIR##*/}:"
echo

# 1. The recording produced an MCP-logs snapshot. If the scenario was driven
#    correctly, the directory should be non-empty.
if [[ -d "${TAPE_MCP_LOGS}" ]] && [[ -n "$(ls -A "${TAPE_MCP_LOGS}" 2>/dev/null)" ]]; then
    echo "  ✓ MCP log snapshot captured ($(ls "${TAPE_MCP_LOGS}" | wc -l | tr -d ' ') files)"
else
    echo "  ✗ MCP log snapshot is empty or missing"
    fail_count=$((fail_count + 1))
fi

# 2. At least one rotated file (.log.1) exists. This is the headline proof
#    that rotation actually fired during the session.
shopt -s nullglob
rotated_files=()
for f in "${TAPE_MCP_LOGS}"/*.log.[0-9]*; do
    rotated_files+=("$f")
done
shopt -u nullglob

if (( ${#rotated_files[@]} > 0 )); then
    echo "  ✓ ${#rotated_files[@]} rotated log file(s) present:"
    for f in "${rotated_files[@]}"; do
        echo "      $(basename "${f}") ($(wc -c <"${f}" | tr -d ' ') bytes)"
    done
else
    echo "  ✗ no rotated log files found (rotation may not have fired — drive more volume)"
    fail_count=$((fail_count + 1))
fi

# 3. The rotation event sidecar (added by the advanced-rotation work in
#    PR #10882) should also be present once that lands. Treat this as an
#    informational signal only — its absence doesn't fail the assertion,
#    since the basic-rotation PR doesn't ship the event sidecar yet.
shopt -s nullglob
event_sidecars=("${TAPE_MCP_LOGS}"/*.log.rotations.jsonl)
shopt -u nullglob
if (( ${#event_sidecars[@]} > 0 )); then
    echo "  ✓ rotation event sidecar present ($(basename "${event_sidecars[0]}")) — advanced rotation is in effect"
else
    echo "  ⓘ no rotation event sidecar (expected for basic-rotation build; advanced sidecar lands in PR #10882)"
fi

# 4. Optional: assert that warp-oss didn't emit any rotation-failure WARNs
#    during the session.
if [[ -f "${TAPE_SESSION}" ]]; then
    if grep -qE 'SimpleLogger: rotation failed' "${TAPE_SESSION}"; then
        echo "  ✗ session log contains 'rotation failed' WARN — rotation errored at runtime"
        echo "      (see logs/warp-oss.session.log)"
        fail_count=$((fail_count + 1))
    else
        echo "  ✓ no rotation-failure WARNs in session log"
    fi
fi

echo
if (( fail_count == 0 )); then
    echo "result: pass"
    exit 0
else
    echo "result: ${fail_count} assertion(s) failed"
    exit 1
fi
