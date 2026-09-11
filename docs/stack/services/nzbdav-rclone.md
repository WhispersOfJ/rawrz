# nzbdav_rclone (FUSE sidecar)

`nzbdav_rclone` mounts InfiniDysk’s WebDAV tree at `/mnt/remote/nzbdav`. It is the
mount owner for Radarr, Sonarr, Plex, and Unpackerr.

| | |
|---|---|
| Image | `rclone/rclone:1.75.0` |
| Network | `bearcave` |
| Published ports | none; RC API is internal on `:5572` |
| Config | `config/nzbdav-rclone/rclone.conf` and `cache/` |
| Healthcheck | `mountpoint -q /mnt/remote/nzbdav` |
| Privileges | `/dev/fuse`, `SYS_ADMIN`, AppArmor unconfined as required by FUSE |
| Depends on | `nzbdav` healthy |

## Mount contract

The sidecar mounts `/mnt/remote` as `rshared`, clears stale FUSE state before startup,
and uses a full VFS cache capped at 300 GiB (with a 10 GiB min-free-space guard so it
can never fill the disk). The container has a 4 GiB memory cap because the in-memory
VFS metadata cache peaks near the old 3 GiB cap during large library-analysis passes;
the content cache itself is disk-backed. The mount is tuned for a large, mostly immutable media tree: 32 MiB
read chunks, 32 MiB read-ahead, fast fingerprints, 30-second attribute caching,
and a six-hour directory cache.
Consumers receive
`/mnt/remote/nzbdav` with `rslave` propagation.

`scripts/setup.sh` creates `config/nzbdav-rclone/rclone.conf` from
`services/nzbdav-rclone/rclone.conf.template` and derives its credentials from
`.env`. Existing non-placeholder configs are preserved. The file must contain:

```ini
[nzbdav]
type = webdav
url = http://nzbdav:8080/
vendor = other
user = usenet
pass = <rclone-obscured WebDAV password>
```

Generate the password with `rclone obscure`; never put the plaintext password in the
config file or repository.

## Direct backend route (no frontend proxy)

The remote targets InfiniDysk's **internal WebDAV backend** at `nzbdav:8080` rather
than the published `:3000` frontend, because the Node frontend proxies every streamed
byte and media reads are the stack's highest-bandwidth path. `:8080` is reachable only
inside the `bearcave` network (it is not a published port) — never expose it to the
host or untrusted networks. Host-side WebDAV consumers (probes, checks) keep using the
`:3000` frontend.

## Recovery

If the mount is unavailable:

```bash
docker compose restart nzbdav nzbdav_rclone
docker exec nzbdav_rclone mountpoint -q /mnt/remote/nzbdav
docker compose restart radarr sonarr plex unpackerr
```

Never scan Plex while this mount is down. A scan during a dead FUSE window can create
red trash cans and mark known media as deleted.
