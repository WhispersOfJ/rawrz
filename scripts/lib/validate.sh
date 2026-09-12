#!/usr/bin/env bash
# Validation functions for The Bear Cave setup (9-service slim stack).
# Sources: scripts/lib/helpers.sh

validate_env_file() {
    log_info "Validating .env file..."

    if [ ! -f "$ENV_FILE" ]; then
        log_error ".env file not found. Run: cp .env.template .env"
        return 1
    fi

    # Derive this list from Compose so a newly added ${VAR} cannot silently
    # bypass first-boot validation. Compose otherwise substitutes an empty
    # value, leaving services unable to authenticate or reach a provider.
    local required_vars=("HOST_IP" "PLEX_TOKEN")
    while IFS= read -r var; do
        required_vars+=("$var")
    done < <(grep -oE '\$\{[A-Z_][A-Z_0-9]*' docker-compose.yml \
        | sed 's/^\${//' | sort -u)

    local missing=()
    local placeholder=()

    for var in "${required_vars[@]}"; do
        local value
        value=$(grep -E "^${var}=" "$ENV_FILE" | cut -d'=' -f2- | tr -d '"' || true)

        if [ -z "$value" ]; then
            missing+=("$var")
        elif [ "$value" = "changeme" ] || [[ "$value" == \$\{* ]]; then
            placeholder+=("$var")
        fi
    done

    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing required variables:"
        for var in "${missing[@]}"; do
            echo "  - $var"
        done
        return 1
    fi

    if [ ${#placeholder[@]} -gt 0 ]; then
        log_error "Variables still set to placeholder values:"
        for var in "${placeholder[@]}"; do
            echo "  - $var"
        done
        return 1
    fi

    log_success ".env file validation passed"
    return 0
}

validate_docker() {
    log_info "Validating Docker installation..."

    check_dependency "docker"
    if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
        log_error "Required command 'docker-compose' or 'docker compose' not found."
        return 1
    fi

    if ! docker info &> /dev/null; then
        if getent group docker >/dev/null 2>&1 \
            && ! id -nG | tr ' ' '\n' | grep -qx docker; then
            log_error "Current session lacks Docker group access; log in again or run: newgrp docker"
        else
            log_error "Docker daemon is not running or current user lacks permissions"
        fi
        return 1
    fi

    log_success "Docker validation passed"
    return 0
}

validate_host_runtime() {
    log_info "Validating Linux runtime prerequisites..."

    if [ "$(uname -s)" != "Linux" ]; then
        log_error "The stack requires a Linux host for FUSE and Plex host networking"
        return 1
    fi
    if [ ! -c /dev/fuse ]; then
        log_error "Missing /dev/fuse; load the kernel FUSE module before starting the stack"
        return 1
    fi
    if [ ! -d /dev/dri ] || ! find /dev/dri -maxdepth 1 -name 'renderD*' -print -quit | grep -q .; then
        log_error "Missing a /dev/dri/renderD* node required by Plex VAAPI"
        return 1
    fi

    log_success "Linux runtime prerequisites passed"
    return 0
}

validate_compose() {
    log_info "Validating docker-compose.yml..."

    if [ ! -f "docker-compose.yml" ]; then
        log_error "docker-compose.yml not found"
        return 1
    fi

    if ! docker compose config --quiet 2>/dev/null; then
        log_error "docker-compose.yml validation failed"
        return 1
    fi

    log_success "docker-compose.yml validation passed"
    return 0
}

required_runtime_directories() {
    printf '%s\n' \
        "config/ca" \
        "config/prowlarr" \
        "config/radarr" \
        "config/sonarr" \
        "config/nzbdav" \
        "config/nzbdav-rclone" \
        "config/nzbdav-rclone/cache" \
        "config/seerr" \
        "config/redis" \
        "config/imagemaid" \
        "config/plex" \
        "config/plex/Plex Media Server" \
        "config/plex-transcode" \
        "media/movies" \
        "media/shows" \
        "usenet"
}

prepare_directories() {
    log_info "Preparing bind-mount directories..."

    local missing_mount=""
    while IFS= read -r dir; do
        if ! mkdir -p "$dir"; then
            missing_mount="$dir"
            break
        fi
    done < <(required_runtime_directories)

    if [ -n "$missing_mount" ]; then
        log_error "Cannot create required directory: $missing_mount"
        return 1
    fi

    # This directory is outside the repository because it is the host-side
    # mount propagation point shared by rclone and the consumer containers.
    local mount_dir="${STACK_HOST_MOUNT_DIR:-/mnt/remote/nzbdav}"
    if ! mkdir -p "$mount_dir"; then
        if command -v sudo >/dev/null 2>&1 && sudo mkdir -p "$mount_dir"; then
            log_info "Created host mount directory with sudo: $mount_dir"
        else
            log_error "Cannot create host mount directory: $mount_dir"
            log_error "Create it with appropriate permissions, then run setup again."
            return 1
        fi
    fi

    log_success "Bind-mount directories prepared"
    return 0
}

prepare_ca_bundle() {
    local ca_dir="config/ca"
    local ca_bundle="$ca_dir/ca-bundle.pem"
    local root_ca="$ca_dir/rootCA.pem"
    local source=""

    if [ -f "$ca_bundle" ] && [ -f "$root_ca" ]; then
        return 0
    fi

    for candidate in \
        "$ca_bundle" \
        /etc/ssl/certs/ca-certificates.crt \
        /etc/ssl/cert.pem \
        /etc/pki/tls/certs/ca-bundle.crt; do
        if [ -f "$candidate" ]; then
            source="$candidate"
            break
        fi
    done

    if [ -z "$source" ]; then
        log_error "No system CA bundle found to initialize config/ca"
        return 1
    fi

    mkdir -p "$ca_dir"
    [ -f "$ca_bundle" ] || install -m 0644 "$source" "$ca_bundle"
    # rootCA.pem is the append-only Node trust path. A public bundle is a safe
    # default on a fresh host; replace it later if a private CA is required.
    [ -f "$root_ca" ] || install -m 0644 "$source" "$root_ca"
    log_success "CA trust files prepared from $source"
}

prepare_rclone_config() {
    local config="config/nzbdav-rclone/rclone.conf"
    local template="$SCRIPT_DIR/../services/nzbdav-rclone/rclone.conf.template"
    local user
    local password
    local obscured
    local tmp

    [ -f "$config" ] && ! grep -q 'REPLACE_WITH_RCLONE_ENCODED_PASSWORD' "$config" && return 0
    if [ ! -f "$template" ]; then
        log_error "Missing rclone config template: $template"
        return 1
    fi

    user=$(grep -E '^NZBDAV_WEBDAV_USER=' "$ENV_FILE" | cut -d'=' -f2- | tr -d '"' || true)
    password=$(grep -E '^NZBDAV_WEBDAV_PASS=' "$ENV_FILE" | cut -d'=' -f2- | tr -d '"' || true)
    if [ -z "$user" ] || [ -z "$password" ]; then
        log_error "NZBDAV_WEBDAV_USER and NZBDAV_WEBDAV_PASS are required to create rclone.conf"
        return 1
    fi
    if ! command -v rclone >/dev/null 2>&1; then
        log_error "Required command 'rclone' not found to create rclone.conf"
        return 1
    fi
    if ! obscured=$(rclone obscure "$password"); then
        log_error "rclone could not obscure NZBDAV_WEBDAV_PASS"
        return 1
    fi

    tmp="$(mktemp)"
    awk -v user="$user" -v pass="$obscured" '{
        if ($0 ~ /^user = /) print "user = " user
        else if ($0 ~ /^pass = /) print "pass = " pass
        else print
    }' "$template" > "$tmp"
    install -m 0600 "$tmp" "$config"
    rm -f "$tmp"
    log_success "rclone WebDAV config prepared"
}

prepare_runtime_files() {
    prepare_ca_bundle && prepare_rclone_config
}

validate_directories() {
    log_info "Validating directory structure..."

    local missing=()
    local dir
    while IFS= read -r dir; do
        [ -d "$dir" ] || missing+=("$dir")
    done < <(required_runtime_directories)

    if [ ! -d "${STACK_HOST_MOUNT_DIR:-/mnt/remote/nzbdav}" ]; then
        missing+=("${STACK_HOST_MOUNT_DIR:-/mnt/remote/nzbdav}")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing required directories:"
        for dir in "${missing[@]}"; do
            echo "  - $dir"
        done
        return 1
    fi

    for file in config/ca/ca-bundle.pem config/ca/rootCA.pem config/nzbdav-rclone/rclone.conf; do
        if [ ! -f "$file" ]; then
            log_error "Missing required bind-mount file: $file"
            return 1
        fi
    done

    log_success "Directory structure validation passed"
    return 0
}
