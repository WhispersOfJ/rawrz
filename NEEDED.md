# NEEDED.md — Arch Linux host prerequisites for The Bear Cave

Everything a freshly reinstalled Arch host needs installed to **run** the stack
and to **diagnose it when it is not running**. Compiled from the repo's
host-touching surfaces: `docker-compose.yml`, `services/bash-functions/`,
`services/host-tools/`, `scripts/`, `tests/`, `.pre-commit-config.yaml`, and the
CI workflows under `.github/workflows/`.

Repo status of each package was checked against the official Arch package
repos (`core`/`extra`) and the AUR on 2026-09-04. Almost everything is in the
official repos; only the AUR helper itself (`yay`/`paru`) comes from the AUR.

The stack itself is containerized — the host does **not** need Plex, Radarr,
Sonarr, Seerr, NzbDAV, Unpackerr, Prowlarr, or rclone-as-a-service
installed. Host packages exist for three reasons: the Docker runtime, the
repo's host-side CLI/scripts (`stack-*`, python gates, cron/timers), and
diagnosis.

## 1. TL;DR

```bash
# Official repos (core/extra)
sudo pacman -S --needed \
  docker docker-compose docker-buildx \
  git openssh sudo ca-certificates \
  fish python python-requests curl jq sqlite openssl \
  rclone \
  pacman-contrib smartmontools nftables cronie \
  ruff shellcheck actionlint gitleaks pre-commit yamllint trivy \
  github-cli

# AUR (pick one helper first: yay or paru)
sudo pacman -S --needed --asdeps base-devel git
git clone https://aur.archlinux.org/yay.git /tmp/yay && cd /tmp/yay && makepkg -si
# then, if wanted, the optional extras in section 4 (§4.1 has the desired set)
```

Then do the non-package steps in section 6 (docker group + service, `.env`,
host-tools install, cron/timers).

## 2. What depends on what

| Repo surface | Host dependency | Why |
|---|---|---|
| 9-container compose deployment | `docker` + `docker-compose` (v2 CLI plugin) + `docker-buildx` | `docker compose up -d`, pinned images, any local build |
| `services/host-tools/` (fish CLI, `install.sh`) | `fish` | every `stack-*` host function is a `.fish` file symlinked into `~/.config/fish/functions/` |
| `services/bash-functions/` (operational CLI) | `bash`, `curl`, `python`, `docker`, `jq`, `sqlite` | sourced from `~/.bashrc`; calls host-published APIs, embeds `python3` renderers, reads the Plex DB read-only |
| `services/bash-functions/waybar/` toggle | `jq` (+ `swaymsg` only if you use the sway/waybar desktop integration) | `stack-tui-toggle.sh` builds JSON with `jq` |
| `scripts/*.py`, `tests/*` | `python` (stdlib only — see §5) | DB-size gates, maintenance digest, arrival notifier, activity feed, prune/verify scripts |
| `scripts/setup.sh` secret generation and trust bootstrap | `openssl`, `ca-certificates` | random secret material and the initial public CA bundle |
| `scripts/setup.sh` external mount preparation | `sudo` | creates `/mnt/remote/nzbdav` when `/mnt` is root-owned |
| `stack-plex-markers` | `sqlite` CLI | read-only queries against the Plex DB |
| `rclone obscure` setup step (docs/quick-start) | `rclone` host binary | generating the obscured WebDAV password for `config/nzbdav-rclone/rclone.conf`; also handy to probe the WebDAV remote independently of the container |
| `scripts/backup_dropbox.py` (docs/operations/dropbox-backup.md) | `python-requests` | streaming single-tar Dropbox snapshot (tar → chunked upload-session API) |
| `stack-disk-health` | `smartmontools` | SMART queries (`smartctl`, needs root) |
| `stack-pkg-clean-cache`, `stack-pkg-orphans` | `pacman-contrib` | `paccache -rk` cache vacuuming, `pacman -Qtd` orphan plumbing |
| `stack-pkg-update`, `stack-aur-audit` | AUR helper: `yay` or `paru` (AUR) | `yay -Syu`, AUR security audit |
| `stack-firewall-status` | `nftables` | reads active `nft` ruleset |
| Nightly reclaim cron (04:00) + `stack-cron-list` | `cronie` | `crontab -l`/user crontab entries |
| `stack-flatpak-updates` | `flatpak` | only if you actually use Flatpaks |
| `stack-ssh-doctor`, off-host backup copies | `openssh` | SSH config health, `scp` backups off-host |
| Pre-push hook + CI-parity gates | `ruff`, `shellcheck`, `actionlint`, `gitleaks`, `pre-commit`, `yamllint`, `trivy` | see §3 |
| Plex VAAPI transcoding | kernel GPU driver + `/dev/dri` (not a package) | see §6 |

## 3. Dev / CI-parity tools

Needed to reproduce the repo's gates locally (pre-push hook runs
`scripts/preflight.sh`, which runs ruff + compose validation + the secret-drift
guard; pre-commit hooks add gitleaks/ruff/shellcheck/actionlint/yamllint checks).

All are in the official repos now:

| Package | Repo | Used by |
|---|---|---|
| `ruff` | extra | `scripts/preflight.sh`, `.pre-commit-config.yaml`, `validate.yml` |
| `shellcheck` | extra | pre-commit, `validate.yml` |
| `actionlint` | extra | pre-commit, `validate.yml` (workflow lint) |
| `gitleaks` | extra | `.pre-commit-config.yaml` (secret scanning) |
| `pre-commit` | extra | `.pre-commit-config.yaml` (hook runner) |
| `github-cli` | extra | PR workflow per docs/worktree-lifecycle.md (`gh pr create` / `gh pr merge`) |
| `yamllint` | extra | `quality.yml`, `nightly-healthcheck.yml` |
| `trivy` | extra | `trivy-scan.yml` (CVE/IaC scans) |

CI pins exact versions (`ruff==0.8.6`, `pre-commit==4.6.2`,
`yamllint==1.37.1`, actionlint `v1.7.12`). The distro packages track newer
versions and are fine for local gating; only install the pip/go-pinned
versions if you must reproduce a CI result exactly:

```bash
python -m pip install --user --break-system-packages 'ruff==0.8.6' 'pre-commit==4.6.2' 'yamllint==1.37.1'
# or: uv tool install ruff==0.8.6
# actionlint alternative: go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12
```

> Current host note: `trivy` (/usr/local/bin) and `ruff` (~/.local/bin)
> predate their official-repo packaging — on the reinstall prefer
> `extra/trivy` and `extra/ruff` and drop the manual installs.

## 4. Optional — beyond the minimum

Sections 1–3 are enough to run and diagnose the stack. This section adds
packages that make **management, security, and maintenance** more efficient
(§4.1) plus purely situational diagnostics (§4.2). Everything here is in the
official `extra` repo (verified 2026-09-04) except the AUR helpers. The
additions are deliberately CLI-first — consistent with the post-slim-down
posture, which retired containerized observability in favor of the host-side
`stack-*` surface (see [docs/services/lifecycle.md](docs/services/lifecycle.md)).

### 4.1 Desired — management, security, maintenance

```bash
sudo pacman -S --needed lazydocker btop nvtop ncdu dive restic lnav arch-audit lynis nvme-cli lsof
```

| Package | Why it earns its place |
|---|---|
| `lazydocker` | Terminal UI over the compose project: per-container logs, stats, restart/recreate from one screen instead of aliasing `docker compose` incantations |
| `btop` | Live CPU/mem/disk/network + PSI pressure — the same signal `stack-mem-pressure` reads, with a process view for spotting a wedged import or scan |
| `nvtop` | GPU utilization including video engines — confirms Plex VAAPI transcoding is actually using `/dev/dri` rather than silently falling back to CPU |
| `ncdu` | Interactive disk-usage TUI for `config/` — spot `radarr.db`/`sonarr.db`/`logs.db` bloat (landmine 9) and `backups/` size before the prune gates or disk reclaim have to |
| `dive` | Inspect image layer sizes to find bloat before `stack-disk-reclaim` has to remove it; pairs with the weekly dependabot image re-pins |
| `restic` | Encrypted, incremental, off-host backups of `config/`, `secrets/`, `.env` — replaces manual `scp` copies; the backup doc warns same-disk backups protect against mistakes, not disk failure |
| `lnav` | Log navigator over `docker compose logs` exports, `~/.stack-disk-reclaim.log`, and the prune logs the maintenance digest verifies |
| `arch-audit` | Makes `stack-aur-audit` fully functional — it checks AUR/foreign packages against the Arch CVE tracker instead of falling back to `pacman -Qmq` |
| `lynis` | Post-reinstall host hardening audit — the security baseline *around* the stack (open ports, file permissions, config) |
| `nvme-cli` | NVMe SMART/firmware/log access if the boot or data disk is NVMe — deeper than the SMART summary `stack-disk-health` prints |
| `lsof` | See which process holds a bind-mounted or FUSE-backed file open — the tool behind the bind-mount staleness and stale-mount landmines (1, 2) |

### 4.2 Situational diagnostics

| Package | When |
|---|---|
| `bind` (provides `dig`/`host`/`nslookup`; `iproute2`'s `ss` covers local sockets and ships in base) | DNS troubleshooting — the Debian/Ubuntu name `bind-tools` is `bind` on Arch |
| `mtr` | Combined ping+traceroute to the Usenet provider host when downloads stall or history shows provider timeouts |
| `openbsd-netcat` | Raw port probes (`nc -zv HOST PORT`) when curl isn't enough |
| `libva-utils` + `intel-media-driver` (Intel) or `libva-mesa-driver` (AMD) | Host-side `vainfo` sanity check to isolate "GPU broken" from "Plex broken"; the Plex container ships its own VAAPI userspace, so this is diagnostic-only |
| `flatpak` | You use Flatpak apps and want `stack-flatpak-updates` |
| `paru` (AUR, alternative to `yay`) | You prefer paru as the AUR helper |
| Node.js, Go toolchain | **not needed** — see §5 |

## 5. Node / Go / pip — nothing is required

Deliberately checked for this: the active stack has **zero** tracked
`package.json`, `requirements.txt`, `go.mod`, `Pipfile`, or `pyproject.toml`
outside `archive/`, and every `scripts/*.py` and `tests/**/*.py` imports only
the Python standard library. So:

- **No Node.js / npm** — the TypeScript sources under `archive/`
  (arr-dashboard, landing-page) are retired reference material, not part of the
  running stack.
- **No Go toolchain** — only needed if you `go install` actionlint instead of
  using `extra/actionlint`.
- **No pip packages / venv** — the python gates run on the system `python`
  directly (`core/python`, currently 3.14.x, matching the repo's Python 3.14
  baseline). pip/uv are only for pinning the exact CI lint versions (§3).

## 6. Runtime prerequisites that are NOT packages

1. **Kernel `fuse` module + `/dev/fuse`** — `nzbdav_rclone` mounts NzbDAV's
   WebDAV tree via FUSE (`/dev/fuse:/dev/fuse:rwm` in compose). Verify:
   `ls -l /dev/fuse` and `zgrep FUSE /proc/modules`. No host FUSE userspace
   package is required — the mount lives inside the container.
2. **Kernel GPU driver + `/dev/dri`** — Plex hardware transcoding maps
   `/dev/dri` into the container. Intel: `i915` in the kernel; AMD: `amdgpu`.
   Verify: `ls /dev/dri` shows `card*`/`renderD128`. The host user should be
   in the `render` (and `video`) groups for any host-side GPU access.
3. **Docker daemon running + user in `docker` group:**
   ```bash
   sudo systemctl enable --now docker
   sudo usermod -aG docker "$USER"   # re-login after
   ```
4. **`.env`** — `cp .env.template .env`, then real values. `PUID`/`PGID` should
   match your host user, `HOST_IP` the LAN address. Never commit `.env` or
   `secrets/`.
5. **Host firewall** — the stack publishes directly on host ports with no
   reverse proxy. Allow inbound TCP `3000` (NzbDAV), `5055` (Seerr),
   `7878` (Radarr), `8989` (Sonarr), `9696` (Prowlarr),
   `32400` (Plex); Plex on host network also uses its companion discovery/DLNA
   ports if those features are enabled.
6. **Directories** — the repo layout with `config/` (per-service state,
   gitignored), `media/movies`, `media/shows` (bind-mounted into Plex), and
   `secrets/` (gitignored) must exist; `./scripts/setup.sh` creates what it
   needs.
7. **Repo-local installs (no packages involved):**
   ```bash
   bash services/host-tools/scripts/install.sh          # symlink fish stack-* CLI
   # add to ~/.bashrc:
   #   [ -f "$HOME/cave/services/bash-functions/bearcave-bash.sh" ] && \
   #       source "$HOME/cave/services/bash-functions/bearcave-bash.sh"
   ./scripts/install-git-hooks.sh                        # pre-push preflight gate
   ./scripts/install-nightly-reclaim-cron.sh             # 04:00 docker reclaim (needs cronie)
   ```
8. **User timers and cron (systemd is in base, `cronie` from §1)** —
   maintenance digest (05:10 daily), `stack-arrival-notify` (15 min),
   `stack-activity-feed` (15 min), and the monthly sonarr prune (03:30 on the
   1st) are run as user timers/cron entries. Schedules and ready-to-install
   unit contents are documented in the script docstrings
   (`scripts/maintenance_digest.py`, `scripts/arrival_notifier.py`,
   `scripts/activity_feed.py`) and in
   [docs/services/bash-functions.md](docs/services/bash-functions.md).

## 7. Diagnosis: which tool for which failure

| Symptom | First command | Needs |
|---|---|---|
| Container down / crash-looping | `docker compose ps`, `docker compose logs <svc>`, `docker stats` | docker |
| Service up but API unresponsive | `curl -sS http://HOST_IP:<port>` per service; JSON through `jq` | curl, jq |
| Port unreachable vs service dead | `ss -tlnp \| grep <port>`; `nc -zv HOST_IP <port>` | iproute2, openbsd-netcat |
| DNS resolution issues | `dig @1.1.1.1 <host>` | bind |
| *arr DB bloat / integrity (landmine 9) | `python3 scripts/check_radarr_db_size.py`, `sqlite3 config/radarr/radarr.db 'PRAGMA integrity_check;'` | python, sqlite |
| FUSE mount stale / missing files | `docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav`, `python3 scripts/check_mount_drift.py` | docker, python |
| Plex won't transcode | `ls -l /dev/dri`, `vainfo` (host) to isolate GPU vs Plex | intel-media-driver/libva-mesa-driver + libva-utils (optional) |
| Host health (disk/SMART/PSI/zombies/failed units) | `stack-disk-free`, `stack-disk-health`, `stack-mem-pressure`, `stack-service-failed`, `stack-journal-errors` (fish CLI) | smartmontools, cronie (systemd is base) |
| Config/secret drift, retired residue | `./scripts/preflight.sh`, `stack-config-drift`, `stack-audit-residue` | python, docker, ruff |
| Everything above but host-only tooling | `tests/health/run-all.sh`, `tests/bash/test_bash_functions.sh --offline`, `tests/integration/test_pipeline.sh --dry-run` | bash, docker, python, curl |

## 8. Post-install verification

```bash
for c in docker fish python3 curl jq sqlite3 rclone openssl git gh smartctl nft paccache sudo; do
  command -v $c >/dev/null && echo "OK   $c" || echo "MISS $c"
done
ls -l /dev/fuse && ls /dev/dri          # FUSE + GPU nodes
# /dev/fuse must exist; a renderD* node is required because Compose maps /dev/dri for Plex
id -nG | grep -qw docker && echo "docker group OK"
# inside the checkout:
docker compose config --quiet && echo "compose OK"
./tests/health/run-all.sh               # after `docker compose up -d`
```

If any `stack-*` command reports `command not found` after install, re-run
`services/host-tools/scripts/install.sh` and confirm
`~/.config/fish/functions/` is on `$fish_function_path`.
