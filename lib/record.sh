# lib/record.sh — sourced by bin/warp-taper. Screen + log capture during the scenario.

# record_session <scenario_dir> <tape_dir> <log_path>
#
# Two simultaneous capture streams:
#
#   AUD (audience): screen recording via `screencapture -v`. Captures whatever
#                   the user does in the foreground while the scenario runs.
#                   Lands at <tape_dir>/master.mov.
#
#   SBD (soundboard): direct log tailing of warp.log (and optionally MCP server
#                     logs declared in metadata.yaml). The capture is the
#                     ground truth of what Warp's runtime actually observed —
#                     higher fidelity than the screen for most assertions.
#
# The scenario.md file is shown to the user before recording starts so they
# know which clicks to perform. After they hit Return, recording begins. They
# perform the steps, then hit Return again to stop.
record_session() {
    local scenario_dir="$1"
    local tape_dir="$2"
    local log_path="$3"

    mkdir -p "${tape_dir}/patches" "${tape_dir}/logs"

    {
        echo "warp-taper :: record"
        echo "    scenario:  ${scenario_dir}"
        echo "    tape:      ${tape_dir}"
        echo "    started:   $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo
    } | tee -a "${log_path}"

    # Show the scenario steps so the user knows what to do.
    if [[ -f "${scenario_dir}/scenario.md" ]]; then
        echo
        echo "================ scenario.md ================"
        cat "${scenario_dir}/scenario.md"
        echo "============================================="
        echo
    fi

    # Resolve warp.log path. We tail whatever warp.log the running binary
    # writes to; for warp-oss this is ~/Library/Logs/warp-oss.log.
    local warp_log="${HOME}/Library/Logs/warp-oss.log"
    if [[ ! -f "${warp_log}" ]]; then
        touch "${warp_log}"
    fi

    # Start a tail in background, capturing only NEW lines emitted during the
    # session. We seek to current EOF so the SBD recording reflects this run,
    # not the entire historical log.
    local pre_recording_offset
    pre_recording_offset="$(wc -c <"${warp_log}" | tr -d ' ')"
    echo "SBD: tailing ${warp_log} from byte offset ${pre_recording_offset}" | tee -a "${log_path}"

    # Prompt for AUD (screen recording) start.
    echo
    echo "AUD: about to start screen recording."
    echo "     A region picker will appear. Select the Warp window."
    echo "     Hit Enter when ready..."
    read -r

    # `screencapture -v` opens an interactive recording session. The user
    # selects a region, the recording starts, and stops on Ctrl+C or Esc.
    # We background it so the user can ALSO follow scenario steps in
    # parallel.
    local master="${tape_dir}/master.mov"
    rm -f "${master}"
    screencapture -v -V 600 "${master}" &
    local screencap_pid=$!
    echo "AUD: screencapture started (pid=${screencap_pid}, output=${master})" | tee -a "${log_path}"

    echo
    echo "Recording. Perform the scenario steps now."
    echo "Stop the recording by pressing Esc on the screen overlay or Ctrl+C in this terminal."
    echo "When done, hit Enter here to finish the session."
    read -r

    # Stop screen capture if still running.
    if kill -0 "${screencap_pid}" 2>/dev/null; then
        kill -INT "${screencap_pid}" 2>/dev/null || true
        wait "${screencap_pid}" 2>/dev/null || true
    fi

    # Capture the SBD slice — bytes appended to warp.log since recording started.
    local post_recording_offset
    post_recording_offset="$(wc -c <"${warp_log}" | tr -d ' ')"
    if [[ "${post_recording_offset}" -gt "${pre_recording_offset}" ]]; then
        dd if="${warp_log}" bs=1 \
           skip="${pre_recording_offset}" \
           count=$(( post_recording_offset - pre_recording_offset )) \
           2>/dev/null \
           >"${tape_dir}/logs/warp-oss.session.log"
        echo "SBD: captured $(( post_recording_offset - pre_recording_offset )) bytes from warp-oss.log" \
            | tee -a "${log_path}"
    else
        echo "SBD: warp-oss.log did not grow during recording" | tee -a "${log_path}"
    fi

    # Capture MCP server logs if the scenario points at them via
    # metadata.yaml's `mcp_log_paths:` list (one path per line).
    if [[ -f "${scenario_dir}/metadata.yaml" ]] \
        && grep -q '^mcp_log_paths:' "${scenario_dir}/metadata.yaml"; then
        mkdir -p "${tape_dir}/logs/mcp"
        awk '
            /^mcp_log_paths:/ { in_list = 1; next }
            /^[a-zA-Z]/        { in_list = 0 }
            in_list && /^  - / { sub(/^  - /, ""); print }
        ' "${scenario_dir}/metadata.yaml" \
        | while IFS= read -r mcp_log; do
            # Expand ~ since YAML doesn't do tilde-expansion.
            local expanded="${mcp_log/#\~/${HOME}}"
            if [[ -f "${expanded}" ]]; then
                cp "${expanded}" "${tape_dir}/logs/mcp/$(basename "${expanded}")"
                echo "SBD: copied MCP log ${expanded}" | tee -a "${log_path}"
            fi
        done
    fi

    echo "record: ok" | tee -a "${log_path}"
}

# patch <tape_dir> <name>
#
# Take a single screenshot ("patch") at a named point in the scenario.
# Called from a scenario's assertions.sh or interactively while recording.
patch() {
    local tape_dir="$1"
    local name="$2"
    local out="${tape_dir}/patches/${name}.png"
    screencapture -i "${out}"
    echo "patch: ${out}"
}
