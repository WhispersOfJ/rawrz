Hey, I'm WhispersOfJ

Self-hosted media infrastructure, developer tooling, and AI-assisted automation.

🎬 thebearcave
A slim, robust self-hosted media stack — 8 always-on Docker Compose services, one `docker compose up -d`.
Usenet-only acquisition via NzbDAV (no torrents, no debrid), streamed through an rclone FUSE mount into a Plex server with VAAPI hardware transcoding. Slimmed from 29 services down to a deliberately minimal core for stability and headroom.

Prowlarr → Radarr / Sonarr → NzbDAV (Usenet + WebDAV) → rclone FUSE mount → Plex
with Seerr for requests and Unpackerr for automatic extraction.

→ Repository · → Live architecture docs

🛠️ arrfleet-toolkit
Claude Code skills for managing a self-hosted Arr-stack: config sync, TRaSH-Guides profiles, request-manager integration, download-client orchestration, path validation, secrets, health checks, and compose lifecycle.

→ Repository

🧠 Metacacharr (archived)
A metadata cache proxy for Plex — cached TMDB/TVDB lookups, warm-up jobs, image caching, and Prometheus metrics. Retired in the slim-down; kept as reference.

→ Repository

🧰 Operations tooling
The active operational surface is a set of Bash functions: queue management, Plex maintenance (scans, trash, Butler), backlog checks, mount health checks, and a manual, profile-gated ImageMaid cleanup — all validated in CI with ShellCheck, actionlint, and compose tests.

What I work with
Infrastructure: Docker Compose, rclone (FUSE), GitHub Actions CI/CD, Trivy, CodeQL
Languages: C# (.NET), Python, Bash
Media stack: Plex, Radarr, Sonarr, Prowlarr, NzbDAV, Seerr, Unpackerr
AI tools: Claude Code, custom agent skills, AI-assisted operations

Built with Freebuff — AI pair programming for systems engineering.
