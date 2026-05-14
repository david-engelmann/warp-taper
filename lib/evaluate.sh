# lib/evaluate.sh — sourced by bin/warp-taper. Runs the scenario's assertions.

# evaluate_assertions <scenario_dir> <tape_dir> <log_path>
#
# Runs the scenario's `assertions.sh` if present. The script is expected to
# exit 0 for "scenario verified" and non-zero for "did not verify."
#
# The script runs with these variables exported so it can introspect the
# captured artifacts without re-deriving paths:
#
#   TAPE_DIR        absolute path to the tape bundle root
#   TAPE_LOGS       ${TAPE_DIR}/logs
#   TAPE_PATCHES    ${TAPE_DIR}/patches
#   TAPE_SBD        ${TAPE_DIR}/logs/warp-oss.session.log  (if it exists)
#   TAPE_MCP_LOGS   ${TAPE_DIR}/logs/mcp                   (if non-empty)
#   WARP_SOURCE     resolved warp checkout (used by some assertions)
#
# Assertions should write a short human-readable summary to stdout so
# bundle_tape can paste it into README.md.
evaluate_assertions() {
    local scenario_dir="$1"
    local tape_dir="$2"
    local log_path="$3"

    {
        echo "warp-taper :: evaluate"
        echo "    scenario:  ${scenario_dir}"
        echo "    tape:      ${tape_dir}"
        echo "    started:   $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo
    } | tee -a "${log_path}"

    if [[ ! -f "${scenario_dir}/assertions.sh" ]]; then
        echo "no assertions.sh; skipping (manual review only)" | tee -a "${log_path}"
        return 0
    fi

    export TAPE_DIR="${tape_dir}"
    export TAPE_LOGS="${tape_dir}/logs"
    export TAPE_PATCHES="${tape_dir}/patches"
    export TAPE_SBD="${tape_dir}/logs/warp-oss.session.log"
    export TAPE_MCP_LOGS="${tape_dir}/logs/mcp"
    export WARP_SOURCE

    if (cd "${scenario_dir}" && bash assertions.sh) 2>&1 | tee -a "${log_path}"; then
        echo "evaluate: pass" | tee -a "${log_path}"
        return 0
    else
        echo "evaluate: FAIL" | tee -a "${log_path}"
        return 1
    fi
}

# Helper functions that scenario `assertions.sh` files can use.
# A scenario sources this lib (or invokes warp-taper as a wrapper); these
# helpers keep assertion scripts terse.

# assert_file_exists <path> <description>
assert_file_exists() {
    local path="$1"
    local desc="${2:-${path}}"
    if [[ -e "${path}" ]]; then
        echo "  ✓ ${desc}"
        return 0
    else
        echo "  ✗ ${desc} (missing)"
        return 1
    fi
}

# assert_log_contains <log_path> <pattern> <description>
assert_log_contains() {
    local log_path="$1"
    local pattern="$2"
    local desc="${3:-log contains '${pattern}'}"
    if [[ -f "${log_path}" ]] && grep -qE "${pattern}" "${log_path}"; then
        echo "  ✓ ${desc}"
        return 0
    else
        echo "  ✗ ${desc}"
        return 1
    fi
}

# assert_log_lacks <log_path> <pattern> <description>
assert_log_lacks() {
    local log_path="$1"
    local pattern="$2"
    local desc="${3:-log lacks '${pattern}'}"
    if [[ ! -f "${log_path}" ]] || ! grep -qE "${pattern}" "${log_path}"; then
        echo "  ✓ ${desc}"
        return 0
    else
        echo "  ✗ ${desc}"
        return 1
    fi
}
