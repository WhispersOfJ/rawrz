# ============================================================================
# stack-arr-3.sh — arr commands (part 3) + summary commands
# ============================================================================
# desc: arr command triggers, backlog status, import lists, prowlarr indexers
# ============================================================================

# stack-arr <radarr|sonarr> <rss-sync|search-missing|unstick|unstick-importing>
stack-arr() {
# complete: <arr-app> rss-sync|search-missing|unstick|unstick-importing
    if [ "$#" -lt 2 ]; then
        echo "Usage: stack-arr <radarr|sonarr> <rss-sync|search-missing|unstick|unstick-importing>" >&2
        return 1
    fi
    local app
    app="$(__stack_arr_app "$1")" || { echo "Invalid app: $1 (use radarr or sonarr)" >&2; return 1; }
    local cmd="$2"

    # Map commands to Arr API command names
    local api_cmd
    case "$cmd" in
        rss-sync)          api_cmd="RssSync" ;;
        search-missing)    api_cmd="MissingEpisodeSearch" ;;
        unstick)           api_cmd="RefreshMonitoredDownloads" ;;
        unstick-importing) api_cmd="ManualImport" ;;
        *)
            echo "Unknown command: $cmd" >&2
            return 1 ;;
    esac

    local url key
    url="$(__arr_api_url "$app")" || { echo "Cannot determine URL for $app" >&2; return 1; }
    key="$(__arr_api_key "$app")" || { echo "Cannot determine API key for $app" >&2; return 1; }

    if __stack_curl "$STACK_API_TIMEOUT_MUTATE" -sf -X POST "$url/api/v3/command" \
        -H "X-Api-Key: $key" \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"$api_cmd\"}" 2>/dev/null >/dev/null; then
        fmt_success "$api_cmd triggered on $app."
    else
        fmt_error "Failed to trigger $api_cmd on $app."
        return 1
    fi
}

# stack-backlog-status — Every app wanted/missing backlog
stack-backlog-status() {
    fmt_heading "Backlog Status"
    echo ""
    local app url key total missing
    for app in radarr sonarr; do
        url="$(__arr_api_url "$app")"
        key="$(__arr_api_key "$app")" || continue

        if [ "$app" = radarr ]; then
            total="$(__stack_curl "$STACK_API_TIMEOUT_LIGHT" -sf "$url/api/v3/movie?pageSize=1" -H "X-Api-Key: $key" 2>/dev/null \
                | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("totalRecords", "?"))' 2>/dev/null)"
            missing="$(__stack_curl "$STACK_API_TIMEOUT_LIGHT" -sf "$url/api/v3/movie?pageSize=1000" -H "X-Api-Key: $key" 2>/dev/null \
                | python3 -c 'import sys,json; d=json.load(sys.stdin); print(sum(1 for i in d if i.get("monitored") and not i.get("hasFile") and i.get("isAvailable")))' 2>/dev/null)"
            echo "  $app: $total monitored, $missing released+missing"
        else
            total="$(__stack_curl "$STACK_API_TIMEOUT_LIGHT" -sf "$url/api/v3/series?pageSize=1" -H "X-Api-Key: $key" 2>/dev/null \
                | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("totalRecords", "?"))' 2>/dev/null)"
            missing="$(__stack_curl "$STACK_API_TIMEOUT_LIGHT" -sf "$url/api/v3/missing?pageSize=1" -H "X-Api-Key: $key" 2>/dev/null \
                | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("totalRecords", "?"))' 2>/dev/null)"
            echo "  $app: $total series, $missing aired episodes missing"
        fi
    done
}

# stack-command-queue-summary — Backlog across every arr app at once
stack-command-queue-summary() {
    fmt_heading "Command Queue Summary"
    echo ""
    local app url key count
    for app in radarr sonarr; do
        url="$(__arr_api_url "$app")"
        key="$(__arr_api_key "$app")" || continue
        count="$(__stack_curl "$STACK_API_TIMEOUT_LIGHT" -sf "$url/api/v3/command?pageSize=50" -H "X-Api-Key: $key" 2>/dev/null \
            | python3 -c "import sys,json; d=json.load(sys.stdin); recs=d.get('records',d) if isinstance(d,dict) else d; print(len([c for c in recs if c.get('status')=='queued']))" 2>/dev/null)"
        echo "  $app: $count queued commands"
    done
}

# stack-import-lists <radarr|sonarr> — configured import lists and enabled state
stack-import-lists() {
# complete: radarr|sonarr
    if [ "$#" -ne 1 ]; then
        echo "Usage: stack-import-lists <radarr|sonarr>" >&2
        return 1
    fi
    local app
    app="$(__stack_arr_app "$1")" || { echo "Invalid app: $1" >&2; return 1; }

    local url key result
    url="$(__arr_api_url "$app")"
    key="$(__arr_api_key "$app")" || return 1
    fmt_heading "$app — Import Lists"
    echo ""

    result="$(__stack_curl "$STACK_API_TIMEOUT_LIGHT" -sf "$url/api/v3/importlist" -H "X-Api-Key: $key" 2>/dev/null)"
    if [ $? -ne 0 ]; then
        fmt_error "Cannot reach $app"
        return 1
    fi

    echo "$result" | python3 -c "
import sys, json
data = json.load(sys.stdin)
items = data if isinstance(data, list) else data.get('records', [])
if not items:
    print('  No import lists configured.')
else:
    for lst in items:
        name = lst.get('name', '?')
        enabled = '✓' if lst.get('enabled') else '✗'
        ltype = lst.get('listType', '?')
        print(f'  [{enabled}] {name} ({ltype})')
" 2>/dev/null
}

# stack-radarr-health — Radarr DB integrity (quality profiles + size)
# Wraps the two preflight scripts so the checks run in one command.
stack-radarr-health() {
    fmt_heading "Radarr Health"
    echo ""
    local repo="$BEARCAVE_REPO_DIR"
    if [ -z "$repo" ]; then
        local self="${BASH_SOURCE[0]}"
        local real
        real="$(readlink -f "$self" 2>/dev/null || echo "$self")"
        repo="$(dirname "$(dirname "$(dirname "$(dirname "$real")")")")"
    fi

    local db="$repo/config/radarr/radarr.db"
    if [ ! -f "$db" ]; then
        echo "  radarr.db  $(fmt_status_dot "missing")  ($repo/config/radarr/radarr.db)"
        return 1
    fi

    # Each guard is read-only; run them in order and summarize.
    python3 "$repo/scripts/check_radarr_profiles.py"
    local profiles=$?
    python3 "$repo/scripts/check_radarr_db_size.py"
    local size=$?

    echo ""
    if [ "$profiles" -eq 0 ] && [ "$size" -eq 0 ]; then
        fmt_success "Radarr healthy — profiles and DB size OK."
    else
        fmt_error "Radarr needs attention — see diagnostics above."
    fi
}

# stack-radarr-prune [-y|--yes] [--dry-run] — prune radarr.db MediaInfo bloat
# Stops radarr (when running), backs up radarr.db + logs.db, prunes MediaInfo
# blobs and old history, vacuums, and verifies via scripts/prune_radarr_db.py,
# then resumes radarr. Requires an explicit flag — refuses with no args.
stack-radarr-prune() {
# complete: -y|--yes|--dry-run
    if [ "$#" -eq 0 ]; then
        echo "Usage: stack-radarr-prune [-y|--yes] [--dry-run]" >&2
        return 1
    fi
    local assume_yes=false dry_run=false arg
    for arg in "$@"; do
        case "$arg" in
            -y|--yes) assume_yes=true ;;
            --dry-run) dry_run=true ;;
            -h|--help)
                echo "Usage: stack-radarr-prune [-y|--yes] [--dry-run]" >&2
                echo "Prune radarr.db bloat: backup, prune, vacuum, verify (see docs/stack/services/radarr.md)." >&2
                return 0
                ;;
            *)
                echo "Unknown option: $arg (usage: stack-radarr-prune [-y|--yes] [--dry-run])" >&2
                return 1
                ;;
        esac
    done

    local repo="${BEARCAVE_REPO_DIR:-}"
    if [ -z "$repo" ]; then
        local self="${BASH_SOURCE[0]}"
        local real
        real="$(readlink -f "$self" 2>/dev/null || echo "$self")"
        repo="$(dirname "$(dirname "$(dirname "$(dirname "$real")")")")"
    fi

    local db="$repo/config/radarr/radarr.db"
    if [ ! -f "$db" ]; then
        echo "  radarr.db  $(fmt_status_dot "missing")  ($repo/config/radarr/radarr.db)"
        return 1
    fi

    local running=false
    if command -v docker >/dev/null 2>&1 \
        && docker ps --filter "name=^/radarr$" --format '{{.Names}}' | grep -qx radarr; then
        running=true
    fi

    # VACUUM needs an exclusive lock; stop radarr first unless dry-running.
    local stopped=false
    if [ "$running" = true ] && [ "$dry_run" = false ]; then
        if [ "$assume_yes" != true ]; then
            printf 'Stop radarr while its DB is vacuumed? [y/N] '
            local reply
            read -r reply
            case "$reply" in
                y|Y) ;;
                *) echo "Aborted."; return 1 ;;
            esac
        fi
        fmt_heading "Stopping radarr"
        docker compose -f "$repo/docker-compose.yml" stop radarr \
            || { fmt_error "could not stop radarr"; return 1; }
        stopped=true
    fi

    local -a pargs=()
    [ "$assume_yes" = true ] && pargs+=(--yes)
    [ "$dry_run" = true ] && pargs+=(--dry-run)
    python3 "$repo/scripts/prune_radarr_db.py" "${pargs[@]}"
    local rc=$?

    if [ "$stopped" = true ]; then
        fmt_heading "Starting radarr"
        docker compose -f "$repo/docker-compose.yml" start radarr >/dev/null 2>&1 \
            || fmt_warning "radarr did not restart cleanly — run stack-restart-all"
    fi

    echo ""
    if [ "$rc" -eq 0 ]; then
        fmt_success "Radarr DB maintenance complete."
    else
        fmt_error "Radarr DB maintenance reported problems (exit $rc)."
    fi
    return "$rc"
}

# stack-sonarr-prune [-y|--yes] [--dry-run] — prune sonarr.db MediaInfo bloat
# Sonarr analogue of stack-radarr-prune (AGENTS.md landmine #9, EpisodeFiles
# table): stops sonarr (when running), backs up sonarr.db + logs.db, prunes
# MediaInfo blobs and old history, vacuums, and verifies via
# scripts/prune_sonarr_db.py, then resumes sonarr. Requires an explicit flag —
# refuses with no args.
stack-sonarr-prune() {
# complete: -y|--yes|--dry-run
    if [ "$#" -eq 0 ]; then
        echo "Usage: stack-sonarr-prune [-y|--yes] [--dry-run]" >&2
        return 1
    fi
    local assume_yes=false dry_run=false arg
    for arg in "$@"; do
        case "$arg" in
            -y|--yes) assume_yes=true ;;
            --dry-run) dry_run=true ;;
            -h|--help)
                echo "Usage: stack-sonarr-prune [-y|--yes] [--dry-run]" >&2
                echo "Prune sonarr.db bloat: backup, prune, vacuum, verify (see docs/stack/services/sonarr.md)." >&2
                return 0
                ;;
            *)
                echo "Unknown option: $arg (usage: stack-sonarr-prune [-y|--yes] [--dry-run])" >&2
                return 1
                ;;
        esac
    done

    local repo="${BEARCAVE_REPO_DIR:-}"
    if [ -z "$repo" ]; then
        local self="${BASH_SOURCE[0]}"
        local real
        real="$(readlink -f "$self" 2>/dev/null || echo "$self")"
        repo="$(dirname "$(dirname "$(dirname "$(dirname "$real")")")")"
    fi

    local db="$repo/config/sonarr/sonarr.db"
    if [ ! -f "$db" ]; then
        echo "  sonarr.db  $(fmt_status_dot "missing")  ($repo/config/sonarr/sonarr.db)"
        return 1
    fi

    local running=false
    if command -v docker >/dev/null 2>&1 \
        && docker ps --filter "name=^/sonarr$" --format '{{.Names}}' | grep -qx sonarr; then
        running=true
    fi

    # VACUUM needs an exclusive lock; stop sonarr first unless dry-running.
    local stopped=false
    if [ "$running" = true ] && [ "$dry_run" = false ]; then
        if [ "$assume_yes" != true ]; then
            printf 'Stop sonarr while its DB is vacuumed? [y/N] '
            local reply
            read -r reply
            case "$reply" in
                y|Y) ;;
                *) echo "Aborted."; return 1 ;;
            esac
        fi
        fmt_heading "Stopping sonarr"
        docker compose -f "$repo/docker-compose.yml" stop sonarr \
            || { fmt_error "could not stop sonarr"; return 1; }
        stopped=true
    fi

    local -a pargs=()
    [ "$assume_yes" = true ] && pargs+=(--yes)
    [ "$dry_run" = true ] && pargs+=(--dry-run)
    python3 "$repo/scripts/prune_sonarr_db.py" "${pargs[@]}"
    local rc=$?

    if [ "$stopped" = true ]; then
        fmt_heading "Starting sonarr"
        docker compose -f "$repo/docker-compose.yml" start sonarr >/dev/null 2>&1 \
            || fmt_warning "sonarr did not restart cleanly — run stack-restart-all"
    fi

    echo ""
    if [ "$rc" -eq 0 ]; then
        fmt_success "Sonarr DB maintenance complete."
    else
        fmt_error "Sonarr DB maintenance reported problems (exit $rc)."
    fi
    return "$rc"
}

# stack-prowlarr-indexers — Every indexer enabled state + priority
stack-prowlarr-indexers() {
    local url="${PROWLARR_URL:-http://localhost:9696}"
    local key="${PROWLARR_API_KEY:-}"

    fmt_heading "Prowlarr Indexers"
    echo ""

    local result
    result="$(__stack_curl "$STACK_API_TIMEOUT_LIGHT" -sf "$url/api/v1/indexer" -H "X-Api-Key: $key" 2>/dev/null)"
    if [ $? -ne 0 ]; then
        fmt_error "Cannot reach Prowlarr"
        return 1
    fi

    echo "$result" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for idx in data:
    name = idx.get('name', '?')
    enabled = '✓' if idx.get('enable') else '✗'
    priority = idx.get('priority', '?')
    print(f'  [{enabled}] {name:<30s} priority={priority}')
" 2>/dev/null
}
