# ImageMaid maintenance

ImageMaid is a manual, profile-gated maintenance service for reclaiming unused Plex
`Cache/PhotoTranscoder` files. It is not part of the normal nine-container startup and
adds no always-on container.

| | |
|---|---|
| Image | `kometateam/imagemaid:latest` (digest-pinned in Compose) |
| Profile | `maintenance` |
| Memory / CPU | 256 MiB / 0.5 CPU |
| Network | none; no Plex API connection is required for this mode |
| Plex mount | `./config/plex/Plex Media Server` → `/plex` read/write |
| ImageMaid config/logs | `./config/imagemaid` → `/config` |

## Scope

The configured run uses:

```text
PLEX_PATH=/plex
MODE=nothing
PHOTO_TRANSCODER=True
EMPTY_TRASH=False
CLEAN_BUNDLES=False
OPTIMIZE_DB=False
```

That means it removes generated PhotoTranscoder cache files only. It does not remove
Plex metadata, empty trash, clean bundles, optimize the database, or alter media files.
ImageMaid reports the reclaimed amount as `Space Recovered: ...`, which the Fish command
passes through.

ImageMaid does **not** resize or recompress images and has no image-quality selection.
There is therefore no quality reduction to configure; poster/artwork quality is unchanged.

## Run

Run while Plex is idle and do not run it alongside another tool that writes Plex artwork:

```fish
stack-plex-image-clean
```

Equivalent one-line command from the repository root:

```bash
docker compose --profile maintenance run --rm --no-deps imagemaid
```

The operation is destructive to unused generated cache files. Back up `config/plex` before
the first run if recovery of deleted cache files is important. Do not follow it with Plex
trash-emptying; PhotoTranscoder cleanup is independent of library visibility.

## References

- [ImageMaid upstream](https://github.com/Kometa-Team/ImageMaid)
- [Plex service](plex.md)

The host path is the Plex application-support subdirectory containing `Cache/`,
`Metadata/`, and `Plug-in Support/`; mounting the parent `config/plex` directory
would leave ImageMaid unable to find `Cache/PhotoTranscoder`.
