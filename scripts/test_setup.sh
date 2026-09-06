#!/usr/bin/env bash
# Offline regression test for scripts/setup.sh's fresh-checkout preparation.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$TMP/bin"
cat > "$TMP/bin/rclone" <<'EOF'
#!/usr/bin/env bash
if [ "${1:-}" = "obscure" ]; then
    printf 'obscured-test-password\n'
else
    exit 1
fi
EOF
chmod +x "$TMP/bin/rclone"

cp "$ROOT/.env.template" "$TMP/.env"
cp "$ROOT/docker-compose.yml" "$TMP/docker-compose.yml"
sed -i \
    -e 's/^PLEX_TOKEN=.*/PLEX_TOKEN=plex-token/' \
    -e 's/^FRONTEND_BACKEND_API_KEY=.*/FRONTEND_BACKEND_API_KEY=frontend-key/' \
    -e 's/^NZBDAV_WEBDAV_PASS=.*/NZBDAV_WEBDAV_PASS=webdav-password/' \
    -e 's/^NZBDAV_RCLONE_RC_PASS=.*/NZBDAV_RCLONE_RC_PASS=rc-password/' \
    -e 's/^NZBDAV_PROFILE_TOKEN=.*/NZBDAV_PROFILE_TOKEN=profile-token/' \
    -e 's/=changeme$/=configured/g' \
    "$TMP/.env"

(
    cd "$TMP"
    PATH="$TMP/bin:$PATH" \
    STACK_HOST_MOUNT_DIR="$TMP/mnt/remote/nzbdav" \
    bash -c '
        source "$1/scripts/lib/helpers.sh"
        source "$1/scripts/lib/validate.sh"
        SCRIPT_DIR="$1/scripts"
        validate_env_file
        prepare_directories
        prepare_runtime_files
        validate_directories
    ' _ "$ROOT"
)

for dir in \
    config/ca config/prowlarr config/radarr config/sonarr \
    config/nzbdav config/nzbdav-rclone/cache config/seerr config/imagemaid \
    config/plex/'Plex Media Server' config/plex-transcode media/movies media/shows usenet; do
    test -d "$TMP/$dir" || { echo "FAIL: missing directory $dir" >&2; exit 1; }
done

test -d "$TMP/mnt/remote/nzbdav"
test -s "$TMP/config/ca/ca-bundle.pem"
test -s "$TMP/config/ca/rootCA.pem"
test "$(stat -c '%a' "$TMP/config/nzbdav-rclone/rclone.conf")" = 600
grep -q '^user = usenet$' "$TMP/config/nzbdav-rclone/rclone.conf"
grep -q '^pass = obscured-test-password$' "$TMP/config/nzbdav-rclone/rclone.conf"
if grep -q 'webdav-password' "$TMP/config/nzbdav-rclone/rclone.conf"; then
    echo "FAIL: plaintext WebDAV password was written" >&2
    exit 1
fi

# The validator must reject any missing Compose credential, rather than
# allowing Compose to substitute an empty value.
cp "$TMP/.env" "$TMP/missing.env"
sed -i 's/^NZBDAV_USENET_HOST=.*/# NZBDAV_USENET_HOST=missing/' "$TMP/missing.env"
if (
    cd "$TMP"
    ENV_FILE="$TMP/missing.env" bash -c '
        source "$1/scripts/lib/helpers.sh"
        source "$1/scripts/lib/validate.sh"
        SCRIPT_DIR="$1/scripts"
        ENV_FILE="$2"
        validate_env_file
    ' _ "$ROOT" "$TMP/missing.env"
); then
    echo "FAIL: missing Compose credential was accepted" >&2
    exit 1
fi

# The bash guard: sourced from a non-bash shell (e.g. newgrp -> zsh), the lib
# must refuse loudly instead of leaking variable values to stdout.
if command -v zsh >/dev/null 2>&1; then
    if output=$(zsh -c 'source "$1/scripts/lib/helpers.sh"' _ "$ROOT" 2>&1); then
        echo "FAIL: helpers.sh was accepted under zsh" >&2
        exit 1
    fi
    case "$output" in
        *"must be sourced from bash"*) ;;
        *) echo "FAIL: zsh guard message missing: $output" >&2; exit 1 ;;
    esac
fi

echo "test_setup: all assertions passed"
