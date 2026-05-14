# lib/deploy.sh — sourced by bin/warp-taper. Runs scenario setup + launches warp-oss.

# deploy_with_setup <scenario_dir> <warp_source> <log_path>
#
# Runs the scenario's `setup.sh` if present (fixture seeding, ensuring redaction
# state, etc.), then launches `warp-oss` in the background. The launched
# process's PID is written to `${REPO_ROOT}/.recording/warp-oss.pid` so the
# record + evaluate stages can find it.
deploy_with_setup() {
    local scenario_dir="$1"
    local warp_source="$2"
    local log_path="$3"

    local binary="${warp_source}/target/debug/warp-oss"
    if [[ ! -x "${binary}" ]]; then
        echo "error: warp-oss binary not found at ${binary}" >&2
        echo "       did the build stage succeed?" >&2
        return 1
    fi

    {
        echo "warp-taper :: deploy"
        echo "    scenario:  ${scenario_dir}"
        echo "    binary:    ${binary}"
        echo "    started:   $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo
    } | tee -a "${log_path}"

    # Run the scenario's setup if it provides one. Setup scripts are responsible
    # for whatever fixture state the scenario assumes (e.g. seeding
    # ~/.warp/.mcp.json, toggling a Privacy setting via plist, clearing a log dir).
    if [[ -f "${scenario_dir}/setup.sh" ]]; then
        echo "running setup.sh..." | tee -a "${log_path}"
        if (cd "${scenario_dir}" && bash setup.sh) 2>&1 | tee -a "${log_path}"; then
            echo "setup: ok" | tee -a "${log_path}"
        else
            echo "setup: FAILED" | tee -a "${log_path}"
            return 1
        fi
    else
        echo "no setup.sh; skipping" | tee -a "${log_path}"
    fi

    # Quit any already-running Warp / warp-oss to make sure the recording is
    # against the freshly built binary.
    osascript -e 'quit app "Warp"' 2>/dev/null || true
    pkill -f 'target/debug/warp-oss' 2>/dev/null || true
    sleep 1

    local state_dir="${REPO_ROOT}/.recording"
    mkdir -p "${state_dir}"

    # Launch warp-oss in the background, redirecting its own stdout/stderr so
    # they don't pollute the parent terminal. Save the PID for later teardown.
    nohup "${binary}" >"${state_dir}/warp-oss.stdout" 2>"${state_dir}/warp-oss.stderr" &
    local warp_pid=$!
    echo "${warp_pid}" >"${state_dir}/warp-oss.pid"
    echo "launched warp-oss (pid=${warp_pid})" | tee -a "${log_path}"

    # Give the app a beat to come up before the record stage starts framing things.
    sleep 2
}
