# Host Tools — Local-only CLI

## Purpose

System diagnostic and maintenance commands that run directly on the host.
No API dependency — pure shell. Complements the API-backed fish functions
with host-level health checks.

## Architecture

```
services/host-tools/
├── functions/
│   ├── stack-disk-free.fish
│   ├── stack-disk-health.fish
│   ├── stack-mem-pressure.fish
│   ├── stack-kernel-check.fish
│   ├── stack-reboot-check.fish
│   ├── stack-uptime-report.fish
│   ├── stack-zombie-check.fish
│   ├── stack-service-failed.fish
│   ├── stack-timer-status.fish
│   ├── stack-cron-list.fish
│   ├── stack-journal-errors.fish
│   ├── stack-journal-size.fish
│   ├── stack-firewall-status.fish
│   ├── stack-ssh-doctor.fish
│   ├── stack-git-status-all.fish
│   ├── stack-pkg-updates.fish
│   ├── stack-pkg-update.fish
│   ├── stack-pkg-history.fish
│   ├── stack-pkg-orphans.fish
│   ├── stack-pkg-clean-cache.fish
│   ├── stack-aur-audit.fish
│   └── stack-flatpak-updates.fish
├── scripts/
│   ├── install.sh
│   └── uninstall.sh
└── README.md
```

## Categories

| Category | Commands | What they check |
|----------|----------|-----------------|
| Disk | `stack-disk-free`, `stack-disk-health` | Free space, SMART health |
| Memory | `stack-mem-pressure` | Kernel PSI pressure stats |
| Kernel | `stack-kernel-check`, `stack-reboot-check` | Running vs installed, pending reboot |
| Uptime | `stack-uptime-report`, `stack-zombie-check` | Load, zombies, uptime |
| Systemd | `stack-service-failed`, `stack-timer-status`, `stack-cron-list` | Failed units, timers, crontab |
| Journal | `stack-journal-errors`, `stack-journal-size` | Error entries, journal disk usage |
| Network | `stack-firewall-status`, `stack-ssh-doctor` | nftables, SSH config health |
| Packages | `stack-pkg-updates`, `stack-pkg-update`, `stack-pkg-history`, `stack-pkg-orphans`, `stack-pkg-clean-cache`, `stack-aur-audit`, `stack-flatpak-updates` | Arch Linux package management |
| Git | `stack-git-status-all` | Git status across repos |

## Setup

```bash
bash services/host-tools/scripts/install.sh
```

## Troubleshooting

- **"command not found"**: Run the install script or check `~/.config/fish/functions/` is in `$fish_function_path`.
- **Permission denied**: Some commands (e.g., `stack-disk-health`) need root for SMART queries.
