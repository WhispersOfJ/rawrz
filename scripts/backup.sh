#!/usr/bin/env bash
# ============================================================================
# The Bear Cave — Backup Script (9-service stack)
# ============================================================================
# Creates backups of configurations, databases, and secrets.
#
# Usage:
#   ./scripts/backup.sh                    # Full backup
#   ./scripts/backup.sh --configs-only     # Only backup configs
#   ./scripts/backup.sh --secrets-only     # Only backup secrets
# ============================================================================

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Constants
BACKUP_DIR="backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="bearcave_backup_${TIMESTAMP}"

# ============================================================================
# Helper Functions
# ============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# ============================================================================
# Backup Functions
# ============================================================================

backup_configs() {
    log_info "Backing up configuration files..."

    local config_backup="$BACKUP_DIR/$BACKUP_NAME/configs"
    mkdir -p "$config_backup"

    # Backup service configs (8-service stack)
    local services=(
        "prowlarr"
        "radarr"
        "sonarr"
        "nzbdav"
        "nzbdav-rclone"
        "seerr"
    )

    for service in "${services[@]}"; do
        if [ -d "config/$service" ]; then
            mkdir -p "$config_backup/config"
            cp -r "config/$service" "$config_backup/config/"
            log_info "Backed up: config/$service"
        fi
    done

    # Backup root configs
    cp -r config "$config_backup/"
    cp docker-compose.yml "$config_backup/"
    cp .env "$config_backup/" 2>/dev/null || log_warning ".env not found"

    log_success "Configuration backup complete"
}

backup_databases() {
    log_info "Backing up databases..."

    local db_backup="$BACKUP_DIR/$BACKUP_NAME/databases"
    mkdir -p "$db_backup"

    # Plex database lives in config/plex on the slim stack
    if [ -d "config/plex" ]; then
        mkdir -p "$db_backup/plex"
        cp -r config/plex "$db_backup/plex/"
        log_info "Backed up: Plex database"
    fi

    log_success "Database backup complete"
}

backup_secrets() {
    log_info "Backing up secrets..."

    local secrets_backup="$BACKUP_DIR/$BACKUP_NAME/secrets"
    mkdir -p "$secrets_backup"

    if [ -d "secrets" ]; then
        cp -r secrets/* "$secrets_backup/"
        log_info "Backed up: secrets directory"
    fi

    if [ -f ".env" ]; then
        cp .env "$secrets_backup/"
        log_info "Backed up: .env file"
    fi

    log_success "Secrets backup complete"
}

backup_plex_metadata() {
    log_info "Backing up Plex metadata..."

    local plex_backup="$BACKUP_DIR/$BACKUP_NAME/plex-metadata"
    mkdir -p "$plex_backup"

    # The Plex metadata is in config/plex which contains:
    # - Media Server settings
    # - Library metadata
    # - Plugin data
    # - Transcode settings

    if [ -d "config/plex" ]; then
        # Use tar for efficiency with large metadata
        tar -czf "$plex_backup/plex-metadata.tar.gz" -C config plex/
        log_info "Backed up: Plex metadata archive"
    fi

    log_success "Plex metadata backup complete"
}

# ============================================================================
# Main
# ============================================================================

main() {
    cd "$(dirname "$0")/.." || exit 1

    echo ""
    echo "=========================================="
    echo "  The Bear Cave — Backup"
    echo "=========================================="
    echo ""

    # Create backup directory
    mkdir -p "$BACKUP_DIR"

    # Parse arguments
    local backup_all=true
    local backup_configs_flag=false
    local backup_secrets_flag=false

    while [[ $# -gt 0 ]]; do
        case $1 in
            --configs-only)
                backup_all=false
                backup_configs_flag=true
                shift
                ;;
            --secrets-only)
                backup_all=false
                backup_secrets_flag=true
                shift
                ;;
            *)
                shift
                ;;
        esac
    done

    # Run backups
    if [ "$backup_all" = true ] || [ "$backup_configs_flag" = true ]; then
        backup_configs
    fi

    if [ "$backup_all" = true ]; then
        backup_databases
        backup_plex_metadata
    fi

    if [ "$backup_all" = true ] || [ "$backup_secrets_flag" = true ]; then
        backup_secrets
    fi

    # Summary
    echo ""
    echo "=========================================="
    echo "  Backup Complete"
    echo "=========================================="
    echo ""
    echo "Backup location: $BACKUP_DIR/$BACKUP_NAME"
    echo ""
    echo "Contents:"
    ls -la "$BACKUP_DIR/$BACKUP_NAME/" 2>/dev/null || true
    echo ""
}

main "$@"
