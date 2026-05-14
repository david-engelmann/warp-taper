# lib/build.sh — sourced by bin/warp-taper. Builds warp-oss in WARP_SOURCE.

# build_warp_oss <warp_source_dir> <log_path>
#
# Runs `cargo build -p warp` (yielding the warp-oss binary as a side effect)
# in the provided warp source checkout. Output is captured to `log_path`.
#
# The warp app crate is named `warp` (per its `Cargo.toml`); the `warp-oss`
# binary it produces is what we actually launch in the deploy stage.
build_warp_oss() {
    local warp_source="$1"
    local log_path="$2"

    if [[ ! -d "${warp_source}" ]]; then
        echo "error: WARP_SOURCE is not a directory: ${warp_source}" >&2
        return 1
    fi
    if [[ ! -f "${warp_source}/Cargo.toml" ]]; then
        echo "error: WARP_SOURCE does not look like a warp checkout (no Cargo.toml): ${warp_source}" >&2
        return 1
    fi

    {
        echo "warp-taper :: build"
        echo "    warp_source: ${warp_source}"
        echo "    started at:  $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "    branch:      $(git -C "${warp_source}" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '<unknown>')"
        echo "    head:        $(git -C "${warp_source}" rev-parse --short HEAD 2>/dev/null || echo '<unknown>')"
        echo
    } | tee -a "${log_path}"

    # We build the binary, not the whole workspace, to keep the build window
    # small. `--bin warp-oss` would be even tighter but is package-scoped under
    # warp, so we let cargo pick.
    if cargo build --manifest-path "${warp_source}/Cargo.toml" --bin warp-oss 2>&1 | tee -a "${log_path}"; then
        echo "build: ok" | tee -a "${log_path}"
        return 0
    else
        echo "build: FAILED" | tee -a "${log_path}"
        return 1
    fi
}
