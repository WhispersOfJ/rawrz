#!/usr/bin/env bash
# Install the nightly Docker disk-reclaim cron entry into the current user's
# crontab (no root needed — the stack runs as this user).
#
# The entry runs `stack-disk-reclaim -y --aggressive` every night at 04:00
# from the target repo's bash functions (sourcing bearcave-bash.sh first so
# BEARCAVE_REPO_DIR, .env, and the guarded docker wrapper are in place).
# Aggressive mode removes every image not referenced by docker-compose.yml
# (plus dangling volumes/build cache/stopped containers) — cache-only images
# are re-pullable, and compose-referenced + container-backed images are
# protected. See docs/stack/services/bash-functions.md → "Nightly maintenance".
#
# Refuses to install when the target repo cannot actually run the command
# (missing function, missing .env, missing script, gone repo) — this avoids
# silently wiring a cron entry that fails every night.
#
# Usage:
#   scripts/install-nightly-reclaim-cron.sh                 # install (idempotent)
#   scripts/install-nightly-reclaim-cron.sh --repo DIR      # target another checkout
#   scripts/install-nightly-reclaim-cron.sh --remove        # remove the entry
#   scripts/install-nightly-reclaim-cron.sh --check         # report installed state
#   scripts/install-nightly-reclaim-cron.sh -h|--help
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

SCHEDULE="0 4 * * *"
LOG_FILE='"'$HOME'/.stack-disk-reclaim.log"'
MARKER="# thebearcave: nightly docker disk reclaim (stack-disk-reclaim -y --aggressive)"
TOKEN="stack-disk-reclaim -y --aggressive"

usage() {
    cat <<'EOF'
Install/remove the nightly stack-disk-reclaim cron entry (04:00 daily).

usage: install-nightly-reclaim-cron.sh [options]

options:
  --repo DIR   target checkout (default: the repo this script lives in)
  --remove     remove the installed nightly entry
  --check      report whether the entry is installed (exit 0/1)
  -h, --help   show this help
EOF
}

repo="$ROOT"
action="install"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) repo="${2:?--repo needs a path}"; shift 2 ;;
        --remove) action="remove"; shift ;;
        --check) action="check"; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown option: $1 (see --help)" >&2; exit 2 ;;
    esac
done

if ! command -v crontab >/dev/null 2>&1; then
    echo "error: crontab not found — is cron installed?" >&2
    exit 1
fi

# Backstop cleanup: never leave a temp crontab file behind on early exit
# (e.g. a failed `crontab` write under set -e).
#
# Must always return 0: bash uses the EXIT trap's own last command status as
# the script's exit status, so a failing test here would corrupt exit codes
# of branches that never touched a temp file (idempotent install, --check).
_tmp=""
_rm_tmp() {
    if [ -z "$_tmp" ]; then
        return 0
    fi
    rm -f "$_tmp"
}
trap _rm_tmp EXIT

# ---------------------------------------------------------------------------
# Fail-closed validation: the entry must be able to run once installed.
# ---------------------------------------------------------------------------
if [ "$action" = "install" ]; then
    problems=""
    [ -d "$repo" ]                        || problems="$problems\n  repo missing: $repo"
    [ -f "$repo/.env" ]                   || problems="$problems\n  .env missing (compose config resolution): $repo/.env"
    [ -f "$repo/scripts/reclaim_docker_disk.py" ] || problems="$problems\n  reclaim script missing: $repo/scripts/reclaim_docker_disk.py"
    [ -f "$repo/services/bash-functions/bearcave-bash.sh" ] || problems="$problems\n  loader missing: $repo/services/bash-functions/bearcave-bash.sh"
    if ! grep -q '^stack-disk-reclaim()' "$repo/services/bash-functions/functions/stack-disk.sh" 2>/dev/null; then
        problems="$problems\n  stack-disk-reclaim not defined in $repo/services/bash-functions/functions/stack-disk.sh (checkout is older than #106)"
    fi
    if [ -n "$problems" ]; then
        printf 'error: target repo cannot run the nightly reclaim:%b\n' "$problems" >&2
        exit 1
    fi
fi

current="$(crontab -l 2>/dev/null || true)"

# Find a real installed *entry* — a crontab line that starts with the
# literal schedule and contains the command token. The marker comment also
# contains the token, so matching the token alone would falsely report
# "installed" from a marker left by hand-editing (or from a retargeted entry
# for a different checkout).
installed_entry="$(printf '%s\n' "$current" \
    | awk -v s="$SCHEDULE " 'index($0, s) == 1' \
    | grep -F "$TOKEN" | head -1 || true)"

case "$action" in
    check)
        if [ -n "$installed_entry" ]; then
            printf 'installed: %s\n' "$TOKEN"
            printf '%s\n' "$installed_entry"
            exit 0
        fi
        echo "not installed"
        exit 1
        ;;
    remove)
        if [ -z "$installed_entry" ]; then
            echo "no nightly reclaim entry installed — nothing to remove"
            exit 0
        fi
        _tmp="$(mktemp)"
        printf '%s\n' "$current" \
            | grep -Fv -e "$MARKER" -e "$TOKEN" > "$_tmp" \
            || true
        crontab "$_tmp"
        echo "removed nightly Docker disk reclaim: $MARKER"
        exit 0
        ;;
esac

entry="$SCHEDULE bash -lc 'source \"$repo/services/bash-functions/bearcave-bash.sh\" && stack-disk-reclaim -y --aggressive' >> $LOG_FILE 2>&1"

if [ -n "$installed_entry" ]; then
    if [ "$installed_entry" = "$entry" ]; then
        echo "nightly Docker disk reclaim already installed:"
        printf '  %s\n' "$installed_entry"
        exit 0
    fi
    # Same schedule, different target checkout — rewrite the entry instead of
    # silently keeping the old path (--repo's advertised purpose).
    _tmp="$(mktemp)"
    printf '%s\n' "$current" \
        | grep -Fv -e "$MARKER" -e "$installed_entry" > "$_tmp" \
        || true
    printf '%s\n' "$MARKER" >> "$_tmp"
    printf '%s\n' "$entry" >> "$_tmp"
    crontab "$_tmp"
    echo "retargeted nightly Docker disk reclaim:"
    printf '  old: %s\n' "$installed_entry"
    printf '  new: %s\n' "$entry"
    echo "  log: $HOME/.stack-disk-reclaim.log"
    exit 0
fi

_tmp="$(mktemp)"
printf '%s\n' "$current" > "$_tmp"
printf '%s\n' "$MARKER" >> "$_tmp"
printf '%s\n' "$entry" >> "$_tmp"
crontab "$_tmp"

echo "installed nightly Docker disk reclaim:"
printf '  %s\n' "$MARKER"
printf '  %s\n' "$entry"
echo "  log: $HOME/.stack-disk-reclaim.log"
echo "verify: crontab -l | grep stack-disk-reclaim   |   tail -f \"$HOME/.stack-disk-reclaim.log\""
echo "remove: scripts/install-nightly-reclaim-cron.sh --remove"
