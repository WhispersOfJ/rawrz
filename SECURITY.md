# Security Policy

The Bear Cave stack is LAN-oriented and intentionally slim: no reverse proxy,
no central authentication tier, and no monitoring sidecar. Host firewalling and
each application's native authentication are part of the security boundary.
The full security model — secrets handling, exposed surfaces, TLS posture, CI
controls, and incident response — is documented in [docs/stack/security.md](docs/stack/security.md).

## Reporting a vulnerability

**Do not open a public issue for security problems.** Please report privately
so the issue can be addressed before it is disclosed:

- **Preferred:** GitHub private vulnerability reporting (the **Security** tab
  → **Report a vulnerability**).
- **Alternative:** open a GitHub issue with a clearly non-exploitable,
  sanitized description and ask for a private channel.

Include, when known:

- Which service is affected (Plex, NzbDAV/rclone, Radarr/Sonarr/Prowlarr,
  Seerr/Unpackerr, CI/CD, scripts).
- Steps to reproduce and impact (what an attacker could do).
- Suggested fix, if you have one.
- **Do not include real secrets** (`.env` values, API keys, tokens,
  credentials) in the report.

## Scope

The active stack is the 9 always-on Compose services plus the manual ImageMaid
profile described in `AGENTS.md` and `docs/services/`. Historical records and
retired-service files under `archive/` and `docs/services/lifecycle.md` are
reference material, not runtime configuration.

Things we care about most:

- Exposure of `.env`, `secrets/`, `config/<app>/`, or rclone credentials.
- A service reaching the public Internet without its documented protection
  (Plex account auth, NzbDAV API/WebDAV auth, *arr login + API key).
- Credential rotation being possible but not performed after a suspected leak.

Things that are expected, not vulnerabilities:

- Ports published on the LAN per the documented port map (firewall the router;
  do not port-forward).
- rclone RC (`:5572`) bound to the container network only.
- The known operational landmines documented in `AGENTS.md` (FUSE mount
  fragility, non-persistent NzbDAV queue, etc.) — report incidents per
  `docs/security.md` incident response instead.

## Expectations

- We aim to acknowledge a report within a few days and keep you informed as we
  assess and fix it.
- We will rotate any credential involved in a confirmed exposure.
- We coordinate disclosure with you; please give us a reasonable window before
  going public.

## Supported versions

Security fixes land on `main` and are shipped via the automated
`release-please` releases. There are no LTS or backport channels — please test
against the latest release (see `docs/ci-cd.md` for the release process).
