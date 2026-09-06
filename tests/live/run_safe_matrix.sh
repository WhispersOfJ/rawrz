#!/usr/bin/env bash
# ============================================================================
# tests/live/run_safe_matrix.sh — D11 safe-command matrix runner
# ============================================================================
# Executes every enabled row of safe_command_registry.yaml once per shell
# (bash/zsh/fish) through the Cave-Scripts port loaders, against the live
# stack. This is the plan §4.2 boundary enforcement: only registry rows touch
# the stack during development, and every row's argv is exactly what runs.
#
# Per-surface preflight: a command whose surface is down is SKIPped (SKIP is
# never a failure); a command that runs and fails its rc/contract gate is FAIL.
# Pending rows (M3 families before their implementing PR) are listed as SKIP
# with reason — visible, not executed.
#
# Usage:
#   tests/live/run_safe_matrix.sh                 # full matrix, all shells
#   tests/live/run_safe_matrix.sh --list          # print rows, execute nothing
#   tests/live/run_safe_matrix.sh --shell fish    # one shell only
#   tests/live/run_safe_matrix.sh --family btrfs  # one family only
#   tests/live/run_safe_matrix.sh --skip-parity   # don't chain the parity harness
#
# Exit code: 0 when no row FAILED; 1 on any FAIL (SKIP always fine).
# ============================================================================
set -uo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[PASS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[SKIP]${NC} $1"; }
log_fail()    { echo -e "${RED}[FAIL]${NC} $1"; }

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIVE_DIR="$REPO_DIR/tests/live"
REGISTRY="$LIVE_DIR/safe_command_registry.yaml"
SUBMODULE="$REPO_DIR/services/cave-scripts"
BASH_LOADER="$SUBMODULE/bash/cave-scripts.sh"
ZSH_LOADER="$SUBMODULE/zsh/cave-scripts.zsh"
FISH_LOADER="$SUBMODULE/fish/cave-scripts.fish"
PARITY="$SUBMODULE/tests/live/test_live_parity.sh"

SHELL_FILTER=""   # bash | zsh | fish
FAMILY_FILTER=""
SKIP_PARITY=0
LIST_ONLY=0

while [ $# -gt 0 ]; do
    case "$1" in
        --list)        LIST_ONLY=1; shift ;;
        --shell)       SHELL_FILTER="$2"; shift 2 ;;
        --family)      FAMILY_FILTER="$2"; shift 2 ;;
        --skip-parity) SKIP_PARITY=1; shift ;;
        -h|--help)     sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown flag: $1 (see --help)" >&2; exit 2 ;;
    esac
done

for f in "$REGISTRY" "$BASH_LOADER" "$ZSH_LOADER" "$FISH_LOADER"; do
    [ -f "$f" ] || { log_fail "missing $f"; exit 1; }
done
command -v python3 >/dev/null || { log_fail "python3 required"; exit 1; }
python3 -c 'import yaml' 2>/dev/null || { log_fail "PyYAML required (pip install pyyaml)"; exit 1; }

REGISTRY_CLI=(python3 "$LIVE_DIR/registry.py" "$REGISTRY")

if [ "$LIST_ONLY" -eq 1 ]; then
    exec "${REGISTRY_CLI[@]}" --list
fi

# ---------------------------------------------------------------------------
# Registry sanity: no duplicate rows, no cross-section duplicates, every name
# known to spec/functions.yaml in the pinned submodule. Fails hard on drift.
# ---------------------------------------------------------------------------
if ! "${REGISTRY_CLI[@]}" --verify "$SUBMODULE/spec/functions.yaml"; then
    log_fail "registry drift against spec/functions.yaml — fix safe_command_registry.yaml first"
    exit 1
fi

# ---------------------------------------------------------------------------
# Per-surface preflight probes (light, unauthenticated where possible).
# ---------------------------------------------------------------------------
SURFACE_STATE=()
surface_up() { # $1 = surface -> 0 if usable
    local s="$1"
    for st in "${SURFACE_STATE[@]}"; do
        [ "${st%%=*}" = "$s" ] && { [ "${st#*=}" = "up" ]; return; }
    done
    local up=0
    case "$s" in
        plex)     curl -sf --max-time 5 http://localhost:32400/identity >/dev/null 2>&1 && up=1 ;;
        arr)      curl -sf --max-time 5 http://localhost:7878/ping >/dev/null 2>&1 && up=1 ;;
        nzbdav)   curl -sf --max-time 5 http://localhost:3000/healthz >/dev/null 2>&1 && up=1 ;;
        seerr)    curl -sf --max-time 5 http://localhost:5055/api/v1/status >/dev/null 2>&1 && up=1 ;;
        docker)   docker ps >/dev/null 2>&1 && up=1 ;;
        host)     up=1 ;;
        btrfs)    up=1 ;;
        hyprland) [ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ] && up=1 ;;
        *)        up=0 ;;
    esac
    SURFACE_STATE+=("$s=$([ "$up" -eq 1 ] && echo up || echo down)")
    [ "$up" -eq 1 ]
}

# ---------------------------------------------------------------------------
# Loaders — same conventions as the submodule's live parity harness: clean env
# for the timeout budgets, STACK_COLOR=false, loaders self-resolve the .env.
# ---------------------------------------------------------------------------
run_bash() { # $1 = command string
    bash -c "
        unset CAVE_SCRIPTS_DIR STACK_API_TIMEOUT_LIGHT STACK_API_TIMEOUT_MUTATE STACK_API_TIMEOUT_HEAVY STACK_DOCKER_TIMEOUT
        export STACK_COLOR=false
        source '$BASH_LOADER' >/dev/null 2>&1
        $1
    " 2>&1
}
run_zsh() {
    zsh -f -c "
        unset CAVE_SCRIPTS_DIR STACK_API_TIMEOUT_LIGHT STACK_API_TIMEOUT_MUTATE STACK_API_TIMEOUT_HEAVY STACK_DOCKER_TIMEOUT
        export STACK_COLOR=false
        source '$ZSH_LOADER' >/dev/null 2>&1
        $1
    " 2>&1
}
run_fish() {
    env -u CAVE_SCRIPTS_DIR -u STACK_API_TIMEOUT_LIGHT -u STACK_API_TIMEOUT_MUTATE \
        -u STACK_API_TIMEOUT_HEAVY -u STACK_DOCKER_TIMEOUT fish --no-config -N -c "
        set -e STACK_API_TIMEOUT_LIGHT STACK_API_TIMEOUT_MUTATE STACK_API_TIMEOUT_HEAVY STACK_DOCKER_TIMEOUT
        set -gx STACK_COLOR false
        source '$FISH_LOADER' >/dev/null 2>&1
        $1
    " 2>&1
}

# timeout(1) executes a child process, not a shell function — export the
# runners (and the vars they close over) so `timeout bash -c 'run_bash …'`
# can reach them.
export BASH_LOADER ZSH_LOADER FISH_LOADER
export -f run_bash run_zsh run_fish

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# row field: rc gate comes from the registry via registry.py --row-rc

passed=0; failed=0; skipped=0

run_row() { # $1 = name, $2 = family, $3 = args-string, $4 = surface, $5 = contract-substr, $6 = timeout-secs, $7 = xfail
    local name="$1" family="$2" argstr="$3" surface="$4" expect="$5" tmo="$6" xfail="$7"
    local shell
    for shell in bash zsh fish; do
        [ -n "$SHELL_FILTER" ] && [ "$shell" != "$SHELL_FILTER" ] && continue

        # surface gate first: every listed surface must be up (SKIP never fails)
        local s down=0
        IFS=',' read -ra surfs <<< "$surface"
        for s in "${surfs[@]}"; do surface_up "$s" || down=1; done
        if [ "$down" -eq 1 ]; then
            log_warning "$name [$shell]: surface $surface down — SKIP"
            skipped=$((skipped+1)); return
        fi

        # decision: run / pending — enforced from the registry itself
        local verdict
        verdict="$("${REGISTRY_CLI[@]}" --resolve "$name")" || {
            log_fail "$name: registry lookup failed"; failed=$((failed+1)); return; }
        case "$verdict" in
            run) ;;
            pending)
                log_warning "$name [$shell]: pending (family not landed yet) — SKIP"
                skipped=$((skipped+1)); return ;;
            *)
                log_fail "$name [$shell]: unexpected verdict '$verdict'"; failed=$((failed+1)); return ;;
        esac

        local rc_ok rc out
        rc_ok="$("${REGISTRY_CLI[@]}" --row-rc "$name")"
        case "$shell" in
            bash) out="$(timeout "$tmo" bash -c 'run_bash "$@"' _ "$name $argstr")"; rc=$? ;;
            zsh)  out="$(timeout "$tmo" bash -c 'run_zsh "$@"'  _ "$name $argstr")"; rc=$? ;;
            fish) out="$(timeout "$tmo" bash -c 'run_fish "$@"' _ "$name $argstr")"; rc=$? ;;
        esac
        printf '%s\n' "$out" > "$WORK/last.$shell"

        local ok=1 why=""
        if [ "$rc" -eq 124 ]; then
            ok=0 why="timed out after ${tmo}s"
        else
            local rc_hit=0 code
            IFS=',' read -ra codes <<< "$rc_ok"
            for code in "${codes[@]}"; do [ "$rc" = "$code" ] && rc_hit=1; done
            if [ "$rc_hit" -ne 1 ]; then
                ok=0 why="rc $rc (allowed: $rc_ok)"
            elif [ -n "$expect" ] && ! grep -qF -- "$expect" "$WORK/last.$shell"; then
                ok=0 why="output missing contract string: $expect"
            fi
        fi

        if [ "$ok" -eq 1 ]; then
            if [ -n "$xfail" ]; then
                log_warning "$name [$shell]: KNOWN-ISSUE NOW PASSES — remove known_issue from the registry row"
                failed=$((failed+1))   # an un-xfailed defect must be re-triaged
            else
                log_success "$name [$shell] rc=$rc"
                passed=$((passed+1))
            fi
        else
            if [ -n "$xfail" ]; then
                log_warning "$name [$shell]: known issue ($why) — tracked in the registry"
                skipped=$((skipped+1))
            else
                log_fail "$name [$shell]: $why"
                head -5 "$WORK/last.$shell" | sed 's/^/    /'
                failed=$((failed+1))
            fi
        fi
    done
}

echo ""
echo "=========================================="
echo "  Cave-Scripts LIVE Safe-Command Matrix"
echo "  (D11 boundary: registry rows only)"
echo "=========================================="
echo ""

# ---------------------------------------------------------------------------
# Iterate rows in registry order. Argv strings are taken verbatim from the
# registry (asserted quote-free at verify time; none of the current rows
# contain quoting).
# ---------------------------------------------------------------------------
mapfile -t ROWS < <("${REGISTRY_CLI[@]}" --rows)
for rowline in "${ROWS[@]}"; do
    IFS=$'\t' read -r name family argstr surface expect tmo xfail <<< "$rowline"
    [ "$argstr" = "-" ] && argstr=""      # '-' = empty (see registry.py --rows)
    [ "$expect" = "-" ] && expect=""
    [ "$xfail"  = "-" ] && xfail=""
    [ -n "$FAMILY_FILTER" ] && [ "$family" != "$FAMILY_FILTER" ] && continue
    run_row "$name" "$family" "$argstr" "$surface" "$expect" "${tmo:-120}" "${xfail:-}"
done

# ---------------------------------------------------------------------------
# Parity harness chain (the submodule's byte-identity gates) — same live tier,
# complementary scope. Optional via --skip-parity for row-only debugging.
# ---------------------------------------------------------------------------
if [ "$SKIP_PARITY" -eq 0 ] && [ -f "$PARITY" ]; then
    echo ""
    log_info "chaining the three-shell byte-parity harness..."
    if bash "$PARITY"; then
        passed=$((passed+1))
    else
        log_fail "parity harness reported failures"
        failed=$((failed+1))
    fi
fi

echo ""
echo "=========================================="
echo "  LIVE MATRIX: $passed PASSED, $failed FAILED, $skipped SKIP"
echo "=========================================="
[ "$failed" -eq 0 ]
