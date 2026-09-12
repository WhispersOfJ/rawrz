#!/usr/bin/env python3
"""Fail when a running container is serving a stale inode for a bind-mounted
config file.

The landmine (docs/stack/landmines.md — "Bind-mounted files can be stale"): editing a
single-file bind in place with `sed -i` or `vim` writes a *new inode* on the
host, but the container can keep the *old* inode open and silently serve stale
content until restarted. `docker compose config` and `docker inspect` both
report the mount as healthy — the divergence is only visible by comparing the
*inode* the container actually has open against the host file's current inode.

Scope: only **single-file** binds are checked. Directory binds are immune —
the directory's own inode is stable, and edits change child inodes which the
container sees through the same mount. The defect is specific to per-file
binds where the host path is a regular file.

For every running container's single-file bind mount, this check runs
`stat -c %i` on the host path and `docker exec <c> stat -c %i` on the
container path, then flags any mismatch (or a container stat that errors —
the file is missing inside but present on host, or vice versa). The fix is a
plain `docker compose restart <service>`.

Run by scripts/preflight.sh (docker-gated) and locally via
`python3 scripts/check_bind_mount_staleness.py`. Exit 0 = every running
container's single-file binds match the host inode; 1 = a stale inode or
docker unavailable.

Usage:
  python3 scripts/check_bind_mount_staleness.py
  python3 scripts/check_bind_mount_staleness.py --offline   # CI: exit 0
"""

import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROBE_TIMEOUT = 10


def run(cmd, timeout=None):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def compose(args):
    return run(["docker", "compose", *args])


def host_inode(path):
    """Return the host inode for a regular file, or None if missing/not a file."""
    proc = run(["stat", "-c", "%i", path])
    if proc.returncode != 0:
        return None
    try:
        return int(proc.stdout.strip())
    except ValueError:
        return None


def container_inode(name, path):
    """Return the in-container inode, or None if stat failed/missing/stat absent.

    A None return covers: file missing inside the container, the `stat`
    binary absent from the image, or a probe timeout. Each is a stale-handle
    symptom worth reporting, so callers distinguish None from a real inode.
    """
    try:
        proc = run(["docker", "exec", name, "stat", "-c", "%i", path],
                   timeout=PROBE_TIMEOUT)
    except subprocess.TimeoutExpired:
        return None
    if proc.returncode != 0:
        return None
    try:
        return int(proc.stdout.strip())
    except ValueError:
        return None


def _file_hash(path):
    """SHA-256 of a file's bytes, or None if unreadable."""
    try:
        return hashlib.sha256(Path(path).read_bytes()).hexdigest()
    except OSError:
        return None


def container_content_hash(name, path):
    """Content hash of a file as served inside the container.

    Fallback for minimal images that ship no `stat`/`cat`/`ls` — content is
    copied through Docker and compared instead. `docker cp` reads through the
    same bind layer the app serves, so a byte mismatch proves the container
    is holding stale content even though its inode is unprobeable. Returns
    the hex digest, or None if the copy failed.
    """
    with tempfile.NamedTemporaryFile(delete=False) as tmp:
        tmp_path = tmp.name
    try:
        cp = run(["docker", "cp", f"{name}:{path}", tmp_path],
                 timeout=PROBE_TIMEOUT)
        if cp.returncode != 0:
            return None
        return _file_hash(tmp_path)
    except subprocess.TimeoutExpired:
        return None
    finally:
        try:
            Path(tmp_path).unlink()
        except OSError:
            pass


def single_file_binds(services, project_root):
    """Yield (service, container_name, host_path, container_path) for per-file binds.

    A bind is per-file when the host source is a regular file (not a
    directory, not a device). Directory binds are skipped — their inode is
    stable and child edits propagate.
    """
    # Map service -> running container name via `docker compose ps`.
    names = {}
    ps = compose(["ps", "--format", "json"])
    if ps.returncode == 0:
        for line in ps.stdout.splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                p = json.loads(line)
            except json.JSONDecodeError:
                continue
            if p.get("Service") and p.get("Name"):
                names[p["Service"]] = p["Name"]

    for svc, sconf in services.items():
        name = names.get(svc)
        if not name:
            continue  # not running — nothing to compare
        for v in sconf.get("volumes", []):
            if not isinstance(v, dict) or v.get("type") != "bind":
                continue
            src = v.get("source", "")
            dst = v.get("target", "")
            if not src or not dst:
                continue
            # Resolve relative sources against the compose project root.
            host = Path(src)
            if not host.is_absolute():
                host = (project_root / src).resolve()
            # Only regular files are inode-relevant.
            try:
                if not host.is_file():
                    continue
            except OSError:
                continue
            yield svc, name, str(host), dst


def main():
    if "--offline" in sys.argv:
        print("OK (offline mode — live bind-mount inode check skipped)")
        return 0

    cfg_proc = compose(["config", "--format", "json"])
    if cfg_proc.returncode != 0:
        print("CHECK FAILED: `docker compose config` errored:")
        print("  docker unavailable?" if not cfg_proc.stderr.strip()
              else f"  {cfg_proc.stderr.strip().splitlines()[-1]}")
        return 1

    cfg = json.loads(cfg_proc.stdout)
    services = cfg["services"]
    project_root = Path(cfg.get("name", str(ROOT)))  # fall back to script dir
    # compose config doesn't emit the project dir; resolve relative sources
    # against the repo root (where `docker compose` was invoked).
    project_root = ROOT

    stale = []
    checked = 0
    for svc, name, host_path, dst in single_file_binds(services, project_root):
        checked += 1
        h = host_inode(host_path)
        if h is None:
            # Host file vanished since compose config resolved it — odd but
            # not a stale-inode symptom; skip rather than false-positive.
            continue
        c = container_inode(name, dst)
        if c is not None:
            # Fast path: in-container stat works — compare inodes directly.
            if h != c:
                stale.append((svc, name, host_path, dst, "inode", h, c))
            continue
        # Distroless fallback: no stat in the image. Compare content bytes
        # via docker cp — a mismatch proves the container holds stale content.
        host_hash = _file_hash(host_path)
        if host_hash is None:
            continue
        c_hash = container_content_hash(name, dst)
        if c_hash is None:
            stale.append((svc, name, host_path, dst, "unprobeable",
                          host_hash, "docker cp failed"))
        elif host_hash != c_hash:
            stale.append((svc, name, host_path, dst, "content", host_hash, c_hash))

    print()
    if stale:
        for svc, name, host_path, dst, mode, h, c in stale:
            print(f"=== STALE BIND: {svc} ({name}) [{mode}] ===")
            print(f"  host      {host_path} {mode}={h}")
            print(f"  container {dst} {mode}={c}")
            print("  Fix: docker compose restart <service> (re-establishes the bind).")
        print()
        print(f"CHECK FAILED: {len(stale)} stale bind-mount(s): "
              f"{', '.join(s for s, _, _, _, _, _, _ in stale)}")
        return 1

    print(f"OK: all {checked} single-file bind mount(s) across running "
          "containers match the host (inode or content — no stale handles).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
