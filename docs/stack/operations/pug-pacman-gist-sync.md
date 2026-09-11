# pug — Pacman/AUR package-list gist sync (hardened)

`pug` ([Ventto/pug](https://github.com/Ventto/pug), AUR package `pug`) is an ALPM
hook that keeps two GitHub gists in sync with the installed package lists after every
pacman transaction: `pacman -Qqen` → `pacman-list.pkg` gist, `pacman -Qqem` →
`aur-list.pkg` gist.

**Why it's hardened:** a stock pug run calls `gist` over the network from inside the
hook. On a transient network failure (`ENETUNREACH`/timeout — this WAN is flaky) the
hook exited `1` and libalpm aborted the whole transaction with
`error: command failed to execute correctly`. The hardened setup retries, then warns
and continues instead of aborting, and a daily timer keeps the gists fresh even when
no pacman transaction runs.

---

## Layout

| Path | Owned by | Role |
|------|----------|------|
| `/usr/local/bin/pug.sh` | untracked (survives upgrades) | **Hardened** script: retry + warn-and-continue + flock |
| `/etc/pacman.d/hooks/pug.hook` | untracked (survives upgrades) | Override hook → `/usr/local/bin/pug.sh --from-hook` |
| `/usr/bin/pug.sh` | `pug` package | Stock script (pristine; clobbered on upgrade — harmless) |
| `/usr/share/libalpm/hooks/pug.hook` | `pug` package | Stock hook (pristine; **shadowed** by the override) |
| `/etc/systemd/system/pug-sync.service` | untracked | Oneshot runner for the timer |
| `/etc/systemd/system/pug-sync.timer` | untracked | Daily sync so changes made outside pacman also sync |
| `/etc/pug` | pug-generated | Gist IDs (`GIST_NAT=` / `GIST_AUR=`) |
| `/root/.gist` | pug-generated | GitHub OAuth token (created by `gist --login`) |

The override shadows the stock hook because libalpm scans `HookDir`
(`/etc/pacman.d/hooks`) before the package hook dir and skips duplicate hook names:
same filename wins in `/etc`. Verified empirically — with both hooks pointing at
marker files, only the override ran.

---

## How the hardening works

### 1. Retry around every network gist call

`gist_retry` wraps the four network operations (read pacman/aur gists, update
pacman/aur gists). Defaults: 4 attempts, 2s backoff — tune with
`PUG_RETRY_ATTEMPTS` / `PUG_RETRY_DELAY`.

```sh
# Retry a gist command a few times with a short pause, to absorb transient
# network failures (e.g. ENETUNREACH, timeouts) that would otherwise abort
# the whole pacman transaction via this hook.
# Usage: gist_retry [--stdin FILE] [--output FILE] cmd [args...]
gist_retry() {
    stdin_file=""
    out_file=""
    if [ "$1" = "--stdin" ]; then
        stdin_file="$2"
        shift 2
    fi
    if [ "$1" = "--output" ]; then
        out_file="$2"
        shift 2
    fi
    attempts="${PUG_RETRY_ATTEMPTS:-4}"
    delay="${PUG_RETRY_DELAY:-2}"
    n=1
    while [ "$n" -le "$attempts" ]; do
        rc=0
        if [ -n "$stdin_file" ] && [ -n "$out_file" ]; then
            "$@" < "$stdin_file" > "$out_file" || rc=1
        elif [ -n "$stdin_file" ]; then
            "$@" < "$stdin_file" || rc=1
        elif [ -n "$out_file" ]; then
            "$@" > "$out_file" || rc=1
        else
            "$@" || rc=1
        fi
        if [ "$rc" -eq 0 ]; then
            return 0
        fi
        if [ "$n" -lt "$attempts" ]; then
            echo "${bold}${red}::${white} gist command failed (attempt ${n}/${attempts}), retrying in ${delay}s...${normal}"
            sleep "$delay"
        fi
        n=$((n + 1))
    done
    return 1
}
```

Two details matter:

- **`--output FILE` for reads** — each attempt truncates and rewrites the file, so a
  partial first attempt can't contaminate a successful retry.
- **`--stdin FILE` for updates** — naively retrying `cat list | gist -u` re-runs
  against a consumed pipe and fails every retry; re-feeding the file per attempt is
  what makes the retry actually work.

### 2. Warn and continue instead of aborting

After retries are exhausted, each failure path prints e.g.

```
:: Failed to read gist after retries, skipping gist sync (pacman continues).
```

and `exit 0` — the pacman transaction is never aborted by a gist sync failure.
Misconfiguration still fails loudly (`exit 1`): missing `/etc/pug.bkp` /
`/root/.gist.bkp` backups, failed interactive `gist --login`, nonexistent pkgdir.

### 3. flock serialization

`pug_update` takes a non-blocking `flock` on `/tmp/pug-sync.lock`, so the pacman hook
and the timer can't race each other (both write `/tmp/pacman.gist` etc.). A
concurrent run prints `:: Another gist sync is already running, skipping.` and returns.

### 4. Daily timer

`pug-sync.service`:

```ini
[Unit]
Description=Sync Pacman/AUR package lists to GitHub gists (pug)
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
Environment=TERM=xterm-256color
Environment=HOME=/root
ExecStart=/usr/local/bin/pug.sh --from-hook
```

`pug-sync.timer`:

```ini
[Unit]
Description=Daily gist sync of Pacman/AUR package lists (pug)

[Timer]
OnCalendar=daily
Persistent=true
RandomizedDelaySec=1h

[Install]
WantedBy=timers.target
```

Notes:

- `HOME=/root` is **required**: gist shells out to `` `git config --global
  gist.private` ``, which needs `$HOME` to read `~/.gitconfig`. Without it every run
  logs `fatal: $HOME not set` (and still exits 0 — the warn-and-continue masks it).
- `TERM=xterm-256color` keeps `tput` quiet for the colored output in the journal.
- `Persistent=true` catches up after downtime; `RandomizedDelaySec=1h` avoids the
  exact-hour slot.

---

## Reproduce on a fresh host

1. Install and initialize stock pug:

   ```bash
   # pug from AUR (helper of choice), gist from the official repos
   sudo /usr/bin/pug.sh        # interactive: gist --login, creates gists + /etc/pug + /root/.gist
   ```

2. Create the hardened script — copy stock to `/usr/local/bin` first (leaving
   `/usr/bin/pug.sh` pristine), then apply three edits:

   ```bash
   sudo cp /usr/bin/pug.sh /usr/local/bin/pug.sh
   ```

   a. Insert `gist_retry` (above) after the color variables at the top.

   b. In `pug_update`, replace the four gist calls and their failure handlers
      (8-space indent for the read blocks, 12-space for the nested update blocks):

      ```sh
      # reads
      if ! gist_retry --output /tmp/pacman.gist gist -r "${GIST_NAT}"; then
          echo "${bold}${red}::${white} Failed to read gist after retries, skipping gist sync (pacman continues).${normal}"
          exit 0
      fi
      # ... same for --output /tmp/aur.gist / gist -r "${GIST_AUR}"
      ```

      ```sh
      # updates
      if ! gist_retry --stdin /tmp/pacman.list gist -u "${GIST_NAT}" -f "${PACMANFILE}"; then
          echo "${bold}${red}::${white} Failed to update gist after retries, skipping gist sync (pacman continues).${normal}"
          exit 0
      fi
      # ... same for --stdin /tmp/aur.list / gist -u "${GIST_AUR}" -f "${AURFILE}"
      ```

   c. Add the flock guard at the top of `pug_update`:

      ```sh
      # Serialize with the pacman hook: skip if another sync is already running
      exec 9>/tmp/pug-sync.lock
      if ! flock -n 9; then
          echo "${bold}${cyan}::${white} Another gist sync is already running, skipping.${normal}"
          return 0
      fi
      ```

3. Override hook `/etc/pacman.d/hooks/pug.hook`:

   ```ini
   [Trigger]
   Operation = Install
   Operation = Upgrade
   Operation = Remove
   Type = Package
   Target = *

   [Action]
   Depends = coreutils
   When = PostTransaction
   Exec = /usr/local/bin/pug.sh --from-hook
   ```

4. Timer + service units (from the section above), then:

   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable --now pug-sync.timer
   sudo systemctl start pug-sync.service    # smoke test
   journalctl -u pug-sync.service
   ```

---

## Verification

```bash
# Hook runs on a real transaction and pacman exits 0
sudo pacman -U --noconfirm /var/cache/pacman/pkg/which-*.pkg.tar.zst; echo $?

# Retry + warn path, without touching the network: fake a failing gist
mkdir -p /tmp/fakebin && printf '#!/bin/sh\nexit 1\n' > /tmp/fakebin/gist && chmod +x /tmp/fakebin/gist
sudo env PATH=/tmp/fakebin:/usr/bin:/bin PUG_RETRY_ATTEMPTS=2 PUG_RETRY_DELAY=1 \
  sh -c '. /usr/local/bin/pug.sh'; echo $?    # expect retry notice, warning, exit 0

# Lock contention: hold the flock, then run a transaction — hook skips, pacman exits 0
sudo sh -c 'exec 9>/tmp/pug-sync.lock; flock -n 9; sleep 25' &
sudo pacman -U --noconfirm /var/cache/pacman/pkg/gzip-*.pkg.tar.zst

# Shadowing: point both hooks at marker files, reinstall a cached pkg,
# only the /etc override's marker appears
```

---

## Upgrade behavior

A `pug` package upgrade overwrites `/usr/bin/pug.sh` and
`/usr/share/libalpm/hooks/pug.hook` with stock versions — harmless, since the
override hook and `/usr/local/bin/pug.sh` are untracked and take effect without
re-application. Nothing to re-apply after upgrades.

---

## Related

- Symptom-to-doc link in [landmines.md](../landmines.md) ("pacman aborts …")
- Motivation: transient WAN failures (`ENETUNREACH`) on this host — the retry logic
  is what makes the hook resilient to the same failures that break direct `gist`
  runs.
