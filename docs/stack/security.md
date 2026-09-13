# Security

The active stack is LAN-oriented and now has an nginx internal TLS ingress. Native
application authentication remains the security boundary for Seerr, Plex, Radarr,
Sonarr, Prowlarr, and NzbDAV; unified RAWRZ authentication is a later milestone.

## Secrets model

| Layer | Contents | Handling |
|-------|----------|----------|
| `.env` | API keys, tokens, WebDAV and Usenet credentials | Gitignored; mode `0600` |
| `secrets/` | Generated secret source files | Gitignored; directory mode `0700` |
| `config/<app>/` | Application databases and settings | Gitignored; contains credentials |
| `config/nginx/certs/` | Generated LAN CA, certificate, and private keys | Gitignored; private keys mode `0600` |
| `.env.template` | Names and placeholders only | Safe to commit; never put real values here |
| `config/ca/` | CA bundle for outbound TLS trust | Do not place private keys here |

Never commit runtime configuration, certificates, private keys, or credentials.
Keep the LAN-only firewall boundary and do not expose the internal CA or ingress to
an untrusted network.

## Ingress guardrails

- nginx redirects HTTP to HTTPS and rejects unknown TLS hosts.
- Only explicit GET/HEAD routes are cacheable; all other routes bypass cache.
- Requests with cookies or `Authorization` bypass cache.
- Upstream `Set-Cookie`, `private`, and `no-store` responses are not cached.
- Playback, mutation, request-approval, session, and webhook routes are not in the
  allowlist.
- Direct service ports remain available as a rollback path during migration.

The current ingress does not provide unified authentication. Existing services must
still be configured with their native login/API controls.

## TLS and client trust

Run `./services/nginx/generate-cert.sh` on the stack host. Install only
`config/nginx/certs/rawrz-ca.crt` into trusted LAN clients; never distribute the CA
private key. The certificate covers the planned `*.rawrz.lan` names.

## FUSE privileges

`nzbdav_rclone` needs `/dev/fuse` and `SYS_ADMIN` to mount WebDAV. Keep its narrow
mount and dependency chain; do not add privileged mode or unrelated capabilities.

## CI and incident response

The M3 workflow runs nginx syntax, Compose, YAML, ShellCheck, and deterministic
fixture-backed route/cache acceptance. For an incident, block the ingress ports or
stop only nginx and use the unchanged direct service ports while investigating.
Rotate exposed credentials and check the NzbDAV queue before service recreation.
