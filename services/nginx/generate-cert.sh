#!/usr/bin/env bash
# Generate an uncommitted LAN CA and a certificate covering the M3 hostnames.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
CERT_DIR="${NGINX_CERT_DIR:-$ROOT_DIR/config/nginx/certs}"
mkdir -p "$CERT_DIR"

if [ -s "$CERT_DIR/rawrz-ca.crt" ] && [ -s "$CERT_DIR/rawrz.lan.crt" ] && [ -s "$CERT_DIR/rawrz.lan.key" ]; then
    echo "M3 certificate already exists in $CERT_DIR"
    exit 0
fi

umask 077
openssl req -x509 -newkey rsa:2048 -nodes -days 825 \
    -subj "/CN=RAWRZ LAN CA" \
    -keyout "$CERT_DIR/rawrz-ca.key" -out "$CERT_DIR/rawrz-ca.crt" \
    >/dev/null 2>&1

cat > "$CERT_DIR/server.cnf" <<'EOF'
[req]
distinguished_name = req_distinguished_name
req_extensions = req_ext
prompt = no
[req_distinguished_name]
CN = rawrz.lan
[req_ext]
subjectAltName = @alt_names
[alt_names]
DNS.1 = rawrz.lan
DNS.2 = deck.rawrz.lan
DNS.3 = rpg.rawrz.lan
DNS.4 = seerr.rawrz.lan
DNS.5 = radarr.rawrz.lan
DNS.6 = sonarr.rawrz.lan
DNS.7 = prowlarr.rawrz.lan
DNS.8 = nzbdav.rawrz.lan
DNS.9 = plex.rawrz.lan
DNS.10 = metrics.rawrz.lan
DNS.11 = grafana.rawrz.lan
EOF

openssl req -new -newkey rsa:2048 -nodes \
    -keyout "$CERT_DIR/rawrz.lan.key" \
    -out "$CERT_DIR/rawrz.lan.csr" \
    -config "$CERT_DIR/server.cnf" >/dev/null 2>&1
openssl x509 -req -days 825 \
    -in "$CERT_DIR/rawrz.lan.csr" \
    -CA "$CERT_DIR/rawrz-ca.crt" -CAkey "$CERT_DIR/rawrz-ca.key" \
    -CAcreateserial -out "$CERT_DIR/rawrz.lan.crt" \
    -extfile "$CERT_DIR/server.cnf" -extensions req_ext >/dev/null 2>&1
rm -f "$CERT_DIR/rawrz.lan.csr" "$CERT_DIR/rawrz-ca.srl" "$CERT_DIR/server.cnf"
chmod 600 "$CERT_DIR"/*.key
chmod 644 "$CERT_DIR"/*.crt
printf 'Generated %s\n' "$CERT_DIR/rawrz.lan.crt"
printf 'Trust %s on LAN clients.\n' "$CERT_DIR/rawrz-ca.crt"
