# lib/bundle.sh — sourced by bin/warp-taper. Emits the PR-ready README per tape.

# bundle_tape <scenario_dir> <tape_dir> <eval_status>
#
# Reads the scenario's metadata.yaml and the tape's captured artifacts,
# composes a README.md inside the tape that's directly pasteable into a
# GitHub PR / issue comment. The README references all the artifacts via
# relative paths so dragging the entire tape directory into a comment
# (or unzipping it into one) preserves the layout.
bundle_tape() {
    local scenario_dir="$1"
    local tape_dir="$2"
    local eval_status="$3"

    # Pull the scenario metadata. We accept that YAML parsing here is
    # bash-grade; metadata.yaml is small and well-formed by convention.
    local title ticket expected
    title="$(awk -F': ' '/^title:/    {sub(/^title: */, "");    print; exit}' "${scenario_dir}/metadata.yaml" || echo '<no title>')"
    ticket="$(awk -F': ' '/^ticket:/   {sub(/^ticket: */, "");   print; exit}' "${scenario_dir}/metadata.yaml" || echo '<no ticket>')"
    expected="$(awk -F': ' '/^expected:/{sub(/^expected: */, ""); print; exit}' "${scenario_dir}/metadata.yaml" || echo '')"

    local branch head
    branch="$(git -C "${WARP_SOURCE}" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '<unknown>')"
    head="$(git -C "${WARP_SOURCE}" rev-parse --short HEAD 2>/dev/null || echo '<unknown>')"

    local readme="${tape_dir}/README.md"
    {
        echo "# Tape: ${title}"
        echo
        if [[ -n "${ticket}" ]]; then
            echo "**Ticket:** ${ticket}  "
        fi
        echo "**Branch / head:** \`${branch}\` @ \`${head}\`  "
        echo "**Recorded:** $(date -u +%Y-%m-%dT%H:%M:%SZ)  "
        echo "**Evaluation:** \`${eval_status}\`"
        echo
        if [[ -n "${expected}" ]]; then
            echo "## Expected behavior"
            echo
            echo "${expected}"
            echo
        fi

        if [[ -f "${scenario_dir}/scenario.md" ]]; then
            echo "## Scenario (setlist)"
            echo
            sed 's/^/> /' "${scenario_dir}/scenario.md"
            echo
        fi

        echo "## Artifacts"
        echo
        if [[ -f "${tape_dir}/master.mov" ]]; then
            echo "- [master.mov](master.mov) — AUD recording of the session"
        fi
        if compgen -G "${tape_dir}/patches/*.png" >/dev/null; then
            echo "- patches/ — named stills:"
            for png in "${tape_dir}"/patches/*.png; do
                echo "    - [$(basename "${png}")](patches/$(basename "${png}"))"
            done
        fi
        if [[ -f "${tape_dir}/logs/warp-oss.session.log" ]]; then
            local sbd_bytes
            sbd_bytes="$(wc -c <"${tape_dir}/logs/warp-oss.session.log" | tr -d ' ')"
            echo "- [logs/warp-oss.session.log](logs/warp-oss.session.log) — SBD: ${sbd_bytes} bytes of warp-oss.log captured during the session"
        fi
        if compgen -G "${tape_dir}/logs/mcp/*" >/dev/null; then
            echo "- logs/mcp/ — MCP server logs at end of session:"
            for f in "${tape_dir}"/logs/mcp/*; do
                echo "    - [$(basename "${f}")](logs/mcp/$(basename "${f}"))"
            done
        fi
        echo

        echo "## Stages"
        echo
        for stage_log in "${tape_dir}"/stages/*.log; do
            [[ -f "${stage_log}" ]] || continue
            echo "<details><summary>$(basename "${stage_log}")</summary>"
            echo
            echo '```text'
            tail -50 "${stage_log}"
            echo '```'
            echo "</details>"
            echo
        done

        if [[ -f "${tape_dir}/stages/04-evaluate.log" ]]; then
            echo "## Assertion summary"
            echo
            echo '```text'
            grep -E '^\s*[✓✗] ' "${tape_dir}/stages/04-evaluate.log" || echo "(no assertions emitted)"
            echo '```'
            echo
        fi

        echo "---"
        echo
        echo "_Recorded by [warp-taper](https://github.com/david-engelmann/warp-taper)._"
    } >"${readme}"

    # Copy the scenario's metadata in next to the README so the bundle is
    # self-contained (re-bundling later doesn't need the original scenario dir).
    cp "${scenario_dir}/metadata.yaml" "${tape_dir}/metadata.yaml" 2>/dev/null || true

    echo "bundle: wrote ${readme}"
}
