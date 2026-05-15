#!/usr/bin/env bash
#
# prime-warp-oss-auth.sh
#
# Copies the macOS keychain `User` entry from a source Warp service
# (default: dev.warp.Warp-Stable, the public Warp app) into the
# dev.warp.WarpOss service warp-oss reads on startup. After running
# this, launching the warp-oss binary will pick up the same logged-in
# Firebase user without going through the interactive sign-in flow.
#
# This is the autonomy hook for evidence capture: it lets warp-taper
# drive an authenticated warp-oss head-to-head against master, since
# every code path gated on auth (agent request building, file-based
# MCP context assembly, etc.) is what most of Warp's interesting
# behavior lives in.
#
# Privacy:
#   - The keychain value is only ever held in shell variables and
#     immediately written into another keychain entry. It is never
#     echoed, logged, or written to disk.
#   - The destination entry is created with -A ("allow any app to
#     read without prompt") so warp-oss can boot without a keychain
#     dialog. Remove it manually when you're done:
#       security delete-generic-password -s dev.warp.WarpOss -a User
#
# Env knobs:
#   SOURCE_SERVICE   Source keychain service (default dev.warp.Warp-Stable).
#   DEST_SERVICE     Destination keychain service (default dev.warp.WarpOss).
#   ACCOUNT          Keychain account name (default User).

set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "prime-warp-oss-auth.sh: macOS only." >&2
    exit 1
fi

SOURCE_SERVICE="${SOURCE_SERVICE:-dev.warp.Warp-Stable}"
DEST_SERVICE="${DEST_SERVICE:-dev.warp.WarpOss}"
ACCOUNT="${ACCOUNT:-User}"

echo "==> reading ${ACCOUNT} entry from ${SOURCE_SERVICE}"
# `-w` prints just the password value, no metadata. We capture into
# a local; the value never touches a file or another tty.
if ! VALUE="$(security find-generic-password -s "${SOURCE_SERVICE}" -a "${ACCOUNT}" -w 2>/dev/null)"; then
    echo "prime-warp-oss-auth.sh: no ${ACCOUNT} entry under ${SOURCE_SERVICE}." >&2
    echo "  Sign in to Warp at least once so the keychain entry exists." >&2
    exit 1
fi

# Remove any existing destination entry so add-generic-password won't
# error on duplicate. Tolerate "not found".
security delete-generic-password -s "${DEST_SERVICE}" -a "${ACCOUNT}" >/dev/null 2>&1 || true

echo "==> writing ${ACCOUNT} entry to ${DEST_SERVICE} (allow-all ACL)"
# -A: allow any application to read without prompting. Required so
#     warp-oss can pick up the entry without an interactive auth
#     dialog at startup. The trade-off is documented in the privacy
#     note above.
# -U: update if it somehow already exists.
# -w "${VALUE}": password value.
security add-generic-password \
    -A \
    -U \
    -s "${DEST_SERVICE}" \
    -a "${ACCOUNT}" \
    -w "${VALUE}"

# Clear the local variable; the keychain has the only copy now.
VALUE=""

echo
echo "primed: ${DEST_SERVICE} now has the same User entry as ${SOURCE_SERVICE}."
echo "        warp-oss should read it on next launch and run as that user."
echo "        clean up later with:"
echo "          security delete-generic-password -s ${DEST_SERVICE} -a ${ACCOUNT}"
