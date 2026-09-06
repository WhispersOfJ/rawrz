#!/usr/bin/env python3
"""Offline regression tests for scripts/backup_dropbox.py.

Proves, without any network or Dropbox account, that the engine's exclusion
model and archive pipeline behave as documented:

  * default excludes drop huge trees, runtime state, secrets, media, and
    generated metadata (sqlite DB/WAL/SHM, per-app Backups/ and logs/)
  * --include-dbs lifts ONLY the metadata category — secrets/media/huge
    trees stay excluded no matter what
  * a real GNU tar run over a fake checkout mirrors the repo layout and
    produces exactly the expected member set (tar is required, so this also
    fails loudly if the exclusion syntax ever breaks)
  * snapshot filenames are UTC-stamped YYYYMMDD-HHMMSS
  * retention selection keeps the newest N and lexical order == age order

Run:  python3 scripts/test_backup_dropbox.py   (exit 0 = pass)
"""

from __future__ import annotations

import datetime
import io
import tarfile
import tempfile
import unittest
from pathlib import Path

import backup_dropbox as bdb


def _make_fake_checkout(root: Path) -> None:
    """Mirror the repo's sensitive-layout: kept settings vs excluded state."""
    files = {
        # code + docs + .git + untracked helpers (all IN)
        "docker-compose.yml": "",
        ".env.template": "",
        "scripts/some.py": "",
        "docs/ops.md": "",
        ".git/objects/ab/def123": "",
        "dropbox-uploader/helper.py": "",
        "bear-i3-config.zip": "",
        # kept per-app SETTINGS (IN)
        "config/radarr/config.xml": "<Config/>",
        "config/sonarr/config.xml": "<Config/>",
        "config/seerr/settings.json": "{}",
        # generated metadata inside kept config dirs (OUT by default)
        "config/radarr/radarr.db": "x" * 100,
        "config/radarr/radarr.db-wal": "y",
        "config/radarr/logs.db": "z",
        "config/radarr/logs/radarr.trace.txt": "t",
        "config/radarr/Backups/manual/radarr.db": "big",
        "config/sonarr/sonarr.db": "s",
        # regenerable cache/artwork/crash trees (OUT always)
        "config/radarr/MediaCover/movies/123/fanart.jpg": "art",
        "config/sonarr/MediaCover/456/poster.jpg": "art",
        "config/radarr/Sentry/1/crash.dmp": "crash",
        "config/seerr/cache/img.png": "img",
        # huge trees (OUT)
        "config/plex/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.db": "huge",
        "config/plex-transcode/cache.bin": "t",
        "config/nzbdav/state/queue.db": "q",
        "config/_backup-20260829-165619/radarr.db": "old",
        "config/nzbdav-rclone/rclone.conf": "secret",
        "config/nzbdav-rclone/cache/vfs/x": "c",
        # runtime (OUT)
        "data/vol/d": "d",
        "logs/stack.log": "l",
        "usenet/download.nzb": "n",
        "backups/some.tar": "bk",
        ".cache/db_growth/sample.json": "g",
        ".memsearch/mem": "m",
        ".worktrees/other/README.md": "w",
        # secrets (OUT)
        ".env": "DROPBOX_ACCESS_TOKEN=leak",
        "secrets/token.txt": "secret",
        "docker-compose.override.yml": "override",
        # media (OUT)
        "media/movies/Movie.mkv": "media",
    }
    for rel, content in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content)


def _archive_members(root: Path, include_dbs: bool) -> set[str]:
    """Run the engine's real tar command over a checkout and return the
    member names, proving exclusion syntax stays in sync with the model."""
    proc = bdb.start_tar(
        root, bdb.build_exclude_patterns(include_dbs=include_dbs), "gzip")
    buf = io.BytesIO()
    for chunk in bdb.iter_tar_chunks(proc):
        buf.write(chunk)
    bdb.finish_tar(proc)
    buf.seek(0)
    with tarfile.open(fileobj=buf, mode="r:gz") as tf:
        return {m.name for m in tf.getmembers() if m.isfile()}


class ExclusionModelTests(unittest.TestCase):
    def test_all_patterns_are_root_relative(self) -> None:
        for category, patterns in bdb.EXCLUDE_CATEGORIES.items():
            for pattern in patterns:
                self.assertTrue(pattern.startswith("./"),
                                f"{category} pattern not root-relative: {pattern}")

    def test_include_dbs_lifts_only_metadata(self) -> None:
        default = set(bdb.build_exclude_patterns())
        with_dbs = set(bdb.build_exclude_patterns(include_dbs=True))
        lifted = default - with_dbs
        self.assertTrue(lifted, "include_dbs lifted nothing")
        # Only metadata patterns may be lifted.
        for pattern in lifted:
            self.assertIn(pattern, bdb.EXCLUDE_CATEGORIES["metadata"])
        # Never lifted, ever.
        for pattern in bdb.EXCLUDE_CATEGORIES["secrets"] \
                + bdb.EXCLUDE_CATEGORIES["media"] \
                + bdb.EXCLUDE_CATEGORIES["huge-trees"]:
            self.assertIn(pattern, with_dbs)


class TarPipelineTests(unittest.TestCase):
    def test_default_exclusions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_fake_checkout(root)
            members = _archive_members(root, include_dbs=False)

        self.assertIn("./docker-compose.yml", members)
        self.assertIn("./.env.template", members)
        self.assertIn("./.git/objects/ab/def123", members)
        self.assertIn("./dropbox-uploader/helper.py", members)
        self.assertIn("./bear-i3-config.zip", members)
        # Kept settings ride along.
        self.assertIn("./config/radarr/config.xml", members)
        self.assertIn("./config/sonarr/config.xml", members)

        # Excluded: media / huge trees / secrets / runtime / metadata state.
        for member in (
            "./media/movies/Movie.mkv",
            "./config/plex/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.db",
            "./config/plex-transcode/cache.bin",
            "./config/nzbdav/state/queue.db",
            "./config/_backup-20260829-165619/radarr.db",
            "./config/nzbdav-rclone/rclone.conf",
            "./config/nzbdav-rclone/cache/vfs/x",
            "./.env",
            "./secrets/token.txt",
            "./docker-compose.override.yml",
            "./data/vol/d",
            "./logs/stack.log",
            "./usenet/download.nzb",
            "./backups/some.tar",
            "./.cache/db_growth/sample.json",
            "./.worktrees/other/README.md",
            "./config/radarr/radarr.db",
            "./config/radarr/radarr.db-wal",
            "./config/radarr/logs.db",
            "./config/radarr/logs/radarr.trace.txt",
            "./config/radarr/Backups/manual/radarr.db",
            "./config/sonarr/sonarr.db",
            "./config/radarr/MediaCover/movies/123/fanart.jpg",
            "./config/sonarr/MediaCover/456/poster.jpg",
            "./config/radarr/Sentry/1/crash.dmp",
            "./config/seerr/cache/img.png",
        ):
            self.assertNotIn(member, members, f"should be excluded: {member}")

    def test_include_dbs_ships_db_but_never_secrets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_fake_checkout(root)
            members = _archive_members(root, include_dbs=True)

        self.assertIn("./config/radarr/radarr.db", members)
        self.assertIn("./config/radarr/Backups/manual/radarr.db", members)
        # Still excluded under --include-dbs: secrets, media, huge trees,
        # AND the regenerable artwork/cache category (that flag ships
        # databases, not gigabytes of posters).
        for member in ("./.env", "./secrets/token.txt",
                       "./config/nzbdav-rclone/rclone.conf",
                       "./media/movies/Movie.mkv",
                       "./config/plex/Plex Media Server/Plug-in Support/"
                       "Databases/com.plexapp.plugins.library.db",
                       "./config/radarr/MediaCover/movies/123/fanart.jpg"):
            self.assertNotIn(member, members)


class SnapshotNamingTests(unittest.TestCase):
    def test_utc_stamped_filename(self) -> None:
        stem = bdb.snapshot_stem(datetime.datetime(2026, 9, 4, 7, 34, 31,
                                                   tzinfo=datetime.timezone.utc))
        self.assertEqual(stem, "thebearcave-backup-20260904-073431")
        self.assertRegex(
            bdb.snapshot_stem(),
            r"^thebearcave-backup-\d{8}-\d{6}$",
        )


class RetentionTests(unittest.TestCase):
    def test_select_doomed_keeps_newest(self) -> None:
        names = [
            "thebearcave-backup-20260901-020000.tar.gz",
            "thebearcave-backup-20260903-020000.tar.gz",
            "thebearcave-backup-20260902-020000.tar.gz",
        ]
        self.assertEqual(bdb.select_doomed(names, keep=2),
                         ["thebearcave-backup-20260901-020000.tar.gz"])
        self.assertEqual(bdb.select_doomed(names, keep=10), [])
        self.assertEqual(bdb.select_doomed(names, keep=0), [])


class SessionProtocolTests(unittest.TestCase):
    """Offline proof of the upload-session call sequence.

    The real HTTP helper is patched out; only the protocol logic runs.
    A single payload chunk must be start+finish with NO duplicate append,
    and multi-chunk streams must be start, append(n-2), finish with correct
    cursor offsets.
    """

    def _run(self, chunks: list[bytes]) -> list[tuple[str, dict, int]]:
        calls: list[tuple[str, dict, int]] = []  # (url, api_args, payload_len)

        def fake_post(url, headers, data):
            import json
            api_args = json.loads(headers["Dropbox-API-Arg"])
            calls.append((url, api_args, len(data)))
            if url == bdb.DBX_SESSION_START_URL:
                return {"session_id": "sess123"}
            if url == bdb.DBX_SESSION_FINISH_URL:
                return {"path_display": "/Backups/x.tar.gz", "size": 0}
            return {}

        import unittest.mock as mock
        with mock.patch.dict("os.environ",
                             {"DROPBOX_ACCESS_TOKEN": "tok"}), \
             mock.patch.object(bdb, "_post_with_retry", side_effect=fake_post):
            result, sent = bdb.upload_session_stream(iter(chunks),
                                                     "/Backups/x.tar.gz",
                                                     overwrite=True)
        return calls, result, sent

    def test_single_chunk_is_start_plus_finish(self) -> None:
        calls, result, sent = self._run([b"A" * 100])
        kinds = [url.rsplit("/", 1)[-1] for url, _, _ in calls]
        self.assertEqual(kinds, ["start", "finish"])
        # Finish cursor must point past the single chunk, never offset 0.
        finish = calls[-1][1]
        self.assertEqual(finish["cursor"]["offset"], 100)
        self.assertEqual(finish["commit"]["path"], "/Backups/x.tar.gz")
        self.assertEqual(sent, 100)

    def test_multi_chunk_streams_once_without_duplicates(self) -> None:
        sizes = [100, 200, 300, 400]
        chunks = [bytes([i]) * n for i, n in enumerate(sizes)]
        calls, _, sent = self._run(chunks)
        kinds = [url.rsplit("/", 1)[-1] for url, _, _ in calls]
        self.assertEqual(kinds, ["start", "append_v2", "append_v2",
                                 "finish"])
        payloads = [length for _, _, length in calls]
        # start=chunk0, appends=chunk1,chunk2, finish=chunk3 — nothing twice.
        self.assertEqual(payloads, sizes)
        # Append offsets advance correctly past each sent chunk.
        self.assertEqual(calls[1][1]["cursor"]["offset"], 100)
        self.assertEqual(calls[2][1]["cursor"]["offset"], 300)
        self.assertEqual(calls[3][1]["cursor"]["offset"], 600)
        self.assertEqual(sent, sum(sizes))

    def test_empty_stream_exits_1(self) -> None:
        import unittest.mock as mock
        with mock.patch.dict("os.environ",
                             {"DROPBOX_ACCESS_TOKEN": "tok"}):
            with self.assertRaises(SystemExit) as ctx:
                bdb.upload_session_stream(iter([]), "/Backups/x.tar.gz",
                                          overwrite=True)
        self.assertEqual(ctx.exception.code, 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
