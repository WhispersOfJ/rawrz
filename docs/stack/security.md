# Security

The active stack is intentionally small and LAN-oriented. It has no reverse proxy,
central authentication tier, or monitoring sidecar, so host firewalling and each
application's native authentication are part of the security boundary.

## Secrets model

| Layer | Contents | Handling |
|-------|----------|----------|
| `.env` | API keys, tokens, WebDAV and Usenet credentials | Gitignored; mode `0600` |
| `secrets/` | Generated secret source files | Gitignored; directory mode `0700` |
| `config/<app>/` | Application databases and settings | Gitignored; contains credentials |
| `config/nzbdav-rclone/rclone.conf` | WebDAV remote credentials | Gitignored; password must be rclone-obscured |
| `.env.template` | Names and placeholders only | Safe to commit; never put real values here |
| `config/ca/` | Public CA bundle used by containers for outbound TLS | Do not place private keys here |

Rules:

- Never commit `.env`, `secrets/`, runtime config, or rclone credentials.
- Rotate a credential immediately if it appears in logs, chat, or version control.
- Use different values for the NzbDAV API, WebDAV, rclone RC, and provider accounts.
- Run `scripts/check_secret_manifest.py` and review `git diff --check` before merging.

## Exposed surfaces

| Surface | Exposure | Required protection |
|---------|----------|---------------------|
| Plex `:32400` | Host network / LAN | Plex account authentication and host firewall |
| NzbDAV `:3000` | LAN | API key and WebDAV credentials; do not expose directly to the Internet |
| Seerr `:5055` | LAN | Seerr authentication and Plex account controls |
| Radarr `:7878` | LAN | Native login and API key; do not publish WAN-facing |
| Sonarr `:8989` | LAN | Native login and API key; do not publish WAN-facing |
| Prowlarr `:9696` | LAN | Native login and API key; do not publish WAN-facing |
| rclone RC `:5572` | Container network only | Strong RC password; no host port is published |

The Compose file publishes only the application ports needed on the LAN. Keep
these ports blocked at the router and host firewall unless a separately reviewed
VPN or access gateway is in use. Do not assume an internal Docker network is a
substitute for application authentication.

## FUSE privileges

`nzbdav_rclone` needs `/dev/fuse` and `SYS_ADMIN` to mount WebDAV. This is the
highest-privilege container in the stack. It has no host port, uses a dedicated
mount path, and its consumers are health-gated. Do not add unrelated mounts,
privileged mode, or extra capabilities.

## TLS and CA bundles

The active stack does not terminate HTTPS. `config/ca/ca-bundle.pem` exists only
to let containers trust outbound HTTPS endpoints when a local CA is needed. Use a
VPN or a separately managed reverse proxy for encrypted remote access; do not
reintroduce the retired Traefik/TLS deployment by accident.

## CI and repository controls

- Compose variables are checked against `.env.template`.
- Shell syntax, Fish loading, Python checks, mount declarations, and queue guards
  run in CI or preflight.
- Trivy scans the images named by the active Compose file.
- Dependabot updates Docker and Python dependencies.
- Historical CVE reports and retired-service records are not runtime configuration.

## Incident response

1. Isolate the host or block the affected port.
2. Rotate the exposed key or password.
3. Inspect container logs and `git diff` without printing secret values.
4. Check the NzbDAV queue before any recreation.
5. Restore configuration from an off-host backup if integrity is uncertain.
