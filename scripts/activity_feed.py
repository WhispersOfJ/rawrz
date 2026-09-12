#!/usr/bin/env python3
"""Media-stack activity feed (TODO.md #8).

A thin RSS/JSON feed of what the *arr apps actually did — imports, upgrades,
and deletions — recorded through the *arr apps' own History API (the same
event data their webhook events carry: downloadFolderImported /
movieFileDeleted / episodeFileDeleted). No container, no listener, no open
port: this poller reads history with a cursor in .cache/activity/state.json
and re-renders three artifacts:

  .cache/activity/feed.jsonl  append-only event log (one JSON line per entry)
  .cache/activity/feed.json   the latest 50 entries, newest first
  .cache/activity/feed.xml    the same as RSS 2.0 for a reader to consume

A missed run or a down app is harmless — the cursor never moves past
records it has seen, so the next run catches up and nothing is lost or
duplicated. Run from a user timer (every 15 minutes is plenty); point an
RSS reader at feed.xml, or serve the directory with `python3 -m http.server`
when remote access is wanted (documented in docs/stack/services/bash-functions.md).

Event mapping (Radarr/Sonarr History eventTypes):
  downloadFolderImported (both), seriesFolderImported (Sonarr) -> import
  movieFileDeleted (Radarr), episodeFileDeleted (Sonarr)        -> delete
  an import for an item that already has an import on record     -> upgrade
  everything else (grabs, renames, failures, ignored)            -> skipped

Exit codes:
  0  polled cleanly (at least one app reachable; other may be down)
  1  feed/state could not be written
  2  both apps unreachable — nothing to record this run

Usage:
  python3 scripts/activity_feed.py
  python3 scripts/activity_feed.py --print 20
  python3 scripts/activity_feed.py --json   # print the rendered feed.json
  python3 scripts/activity_feed.py --state-file /tmp/feed-state.json

Install as a user timer (mirrors the arrival notifier):

  # ~/.config/systemd/user/stack-activity-feed.service
  [Unit]
  Description=Bear Cave media activity feed
  [Service]
  Type=oneshot
  EnvironmentFile=/home/bear/cave/.env
  ExecStart=/usr/bin/python3 /home/bear/cave/scripts/activity_feed.py

  # ~/.config/systemd/user/stack-activity-feed.timer
  [Unit]
  Description=Bear Cave activity feed (every 15 min)
  [Timer]
  OnCalendar=*:0/15
  Persistent=true
  [Install]
  WantedBy=timers.target

  systemctl --user daemon-reload
  systemctl --user enable --now stack-activity-feed.timer
"""

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime
from email.utils import format_datetime
from html import escape
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_STATE = ROOT / ".cache" / "activity" / "state.json"
DEFAULT_FEED = ROOT / ".cache" / "activity" / "feed.jsonl"
DEFAULT_JSON = ROOT / ".cache" / "activity" / "feed.json"
DEFAULT_RSS = ROOT / ".cache" / "activity" / "feed.xml"

API_TIMEOUT = 10  # seconds, matching STACK_API_TIMEOUT_LIGHT
MAX_PAGES = 5  # safety cap per app per run (pageSize 100)
BACKFILL_SECONDS = 24 * 3600  # first-run backfill window
RENDER_LIMIT = 50

# History eventType -> feed kind (None = skip).
IMPORT_EVENTS = {"downloadFolderImported", "seriesFolderImported"}
DELETE_EVENTS = {"movieFileDeleted", "episodeFileDeleted"}

APPS = ("radarr", "sonarr")
URLS = {
    "radarr": ("RADARR_URL", "http://localhost:7878"),
    "sonarr": ("SONARR_URL", "http://localhost:8989"),
}


# --- pure logic (unit-tested offline) ------------------------------------------


def parse_iso(ts):
    if not ts:
        return None
    try:
        s = ts
        if s.endswith("Z"):
            s = s[:-1] + "+00:00"
        return int(datetime.fromisoformat(s).timestamp())
    except (ValueError, TypeError):
        return None


def normalize_record(app, rec):
    """One History API record -> normalized dict (or None when unparseable)."""
    data = rec.get("data") or {}
    if app == "radarr":
        movie = rec.get("movie") or {}
        return {
            "id": rec.get("id"),
            "event_type": rec.get("eventType"),
            "ts": parse_iso(rec.get("date")),
            "title": movie.get("title") or rec.get("sourceTitle") or "?",
            "year": movie.get("year"),
            "movie_id": movie.get("id") or rec.get("movieId"),
            "season": None,
            "episode": None,
            "episode_id": None,
            "quality": (rec.get("quality") or {}).get("quality", {}).get("name"),
            "source": data.get("releaseGroup"),
        }
    series = rec.get("series") or {}
    episode = rec.get("episode") or {}
    return {
        "id": rec.get("id"),
        "event_type": rec.get("eventType"),
        "ts": parse_iso(rec.get("date")),
        "title": series.get("title") or rec.get("sourceTitle") or "?",
        "year": series.get("year"),
        "movie_id": None,
        "season": episode.get("seasonNumber"),
        "episode": episode.get("episodeNumber"),
        "episode_id": episode.get("id") or rec.get("episodeId"),
        "series_id": series.get("id") or rec.get("seriesId"),
        "quality": (rec.get("quality") or {}).get("quality", {}).get("name"),
        "source": data.get("releaseGroup"),
    }


def item_key(app, rec):
    """Stable per-item identity for upgrade tracking (normalized records)."""
    if app == "radarr":
        mid = rec.get("movie_id")
        return "radarr:%s" % mid if mid else None
    eid = rec.get("episode_id")
    if eid:
        return "sonarr:%s" % eid
    sid = rec.get("series_id")
    return "sonarr-series:%s" % sid if sid else None


def kind_for(app, rec, last_import):
    """Feed kind for a record given the last-import state; None = skip.

    Also returns the new last_import value to store for import records
    (the record id), or None when the record should be forgotten (delete).
    """
    event = rec.get("event_type")
    key = item_key(app, rec)
    if event in IMPORT_EVENTS:
        kind = "upgrade" if (key and key in last_import) else "import"
        return kind, {key: rec.get("id")} if key else {}
    if event in DELETE_EVENTS:
        if key:
            return "delete", {key: None}
        return "delete", {}
    return None, {}


def new_records(app, state, records):
    """Records newer than the app's cursor, deduped by id (desc-sorted input)."""
    last_id = (state.get(app) or {}).get("last_id")
    seen = set()
    out = []
    for rec in records:
        rid = rec.get("id")
        if rid is None or rid in seen:
            continue
        seen.add(rid)
        if last_id is None or rid > last_id:
            out.append(rec)
    return out


def advance_cursor(state, app, records):
    """Advance the app cursor over every observed record, including skipped events."""
    ids = [rec.get("id") for rec in records if rec.get("id") is not None]
    if ids:
        state.setdefault(app, {})["last_id"] = max(
            state.get(app, {}).get("last_id") or 0, max(ids)
        )


def backfill_cutoff(records, now_ts):
    """First run without a cursor: keep only records inside the backfill window."""
    cutoff = now_ts - BACKFILL_SECONDS
    return [r for r in records if r.get("ts") is not None and r["ts"] >= cutoff]


def build_entry(app, rec, kind):
    """Feed entry dict for one record."""
    entry = {
        "ts": rec.get("ts"),
        "id": rec.get("id"),
        "app": app,
        "kind": kind,
        "event_type": rec.get("event_type"),
        "title": rec.get("title"),
        "year": rec.get("year"),
        "season": rec.get("season"),
        "episode": rec.get("episode"),
        "quality": rec.get("quality"),
        "source": rec.get("source"),
    }
    return entry


def render_text(entries, limit):
    lines = []
    for e in entries[:limit]:
        when = time.strftime("%Y-%m-%d %H:%M", time.localtime(e["ts"])) if e.get("ts") else "?"
        label = {"import": "imported", "upgrade": "upgraded", "delete": "deleted"}.get(
            e.get("kind"), e.get("kind")
        )
        title = e.get("title") or "?"
        if e.get("year"):
            title = "%s (%s)" % (title, e["year"])
        if e.get("season") is not None and e.get("episode") is not None:
            title = "%s S%02dE%02d" % (title, e["season"], e["episode"])
        extra = ""
        if e.get("quality"):
            extra = " [%s]" % e["quality"]
        lines.append(
            "%s  %s %-6s %s%s" % (when, e.get("app"), label, title, extra)
        )
    return lines


def render_json(entries, limit):
    return json.dumps({"updated": int(time.time()), "entries": entries[:limit]}, indent=2)


def render_rss(entries, limit):
    items = []
    for e in entries[:limit]:
        when = e.get("ts") or int(time.time())
        kind_label = {"import": "Imported", "upgrade": "Upgraded", "delete": "Deleted"}.get(
            e.get("kind"), e.get("kind", "Changed")
        )
        title = e.get("title") or "?"
        if e.get("year"):
            title = "%s (%s)" % (title, e["year"])
        if e.get("season") is not None and e.get("episode") is not None:
            title = "%s S%02dE%02d" % (title, e["season"], e["episode"])
        desc = "%s via %s" % (kind_label, e.get("app"))
        if e.get("quality"):
            desc += " [%s]" % e["quality"]
        if e.get("source"):
            desc += " (%s)" % e["source"]
        items.append(
            "    <item>\n"
            "      <title>%s: %s</title>\n"
            "      <description>%s</description>\n"
            "      <pubDate>%s</pubDate>\n"
            "      <guid isPermaLink=\"false\">thebearcave-%s-%s</guid>\n"
            "    </item>"
            % (
                escape(kind_label),
                escape(title),
                escape(desc),
                format_datetime(datetime.fromtimestamp(when)),
                escape(str(e.get("app"))),
                escape(str(e.get("id"))),
            )
        )
    return (
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        "<rss version=\"2.0\"><channel>\n"
        "  <title>The Bear Cave — media activity</title>\n"
        "  <link>https://github.com/WhispersOfJ/thebearcave</link>\n"
        "  <description>Imports, upgrades, and deletions from Radarr and Sonarr</description>\n"
        + "\n".join(items)
        + "\n</channel></rss>\n"
    )


# --- state ---------------------------------------------------------------------


def load_state(path):
    if path.is_file():
        try:
            return json.loads(path.read_text())
        except (json.JSONDecodeError, OSError):
            return {}
    return {}


def save_state(path, state):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")


# --- HTTP transport --------------------------------------------------------------


def fetch_json(base, path, headers, timeout=API_TIMEOUT):
    sep = "" if base.endswith("/") else "/"
    url = base + sep + path.lstrip("/")
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def history_pages(base, key, app):
    """Yield normalized history records, newest first, up to MAX_PAGES."""
    extra = ""
    if app == "radarr":
        extra = "&includeMovie=true"
    else:
        extra = "&includeSeries=true&includeEpisode=true"
    for page in range(1, MAX_PAGES + 1):
        data = fetch_json(
            base,
            "api/v3/history?page=%d&pageSize=100&sortKey=date&sortDirection=desc%s"
            % (page, extra),
            {"X-Api-Key": key},
        )
        records = (data or {}).get("records") or []
        for rec in records:
            norm = normalize_record(app, rec)
            if norm is not None:
                yield norm
        if len(records) < 100:
            return


# --- orchestration ----------------------------------------------------------------


def run(args):
    import os

    state = load_state(Path(args.state_file))
    state.setdefault("last_import", {})
    entries = []
    reachable = 0
    write_error = None

    for app in APPS:
        key_var = "%s_API_KEY" % app.upper()
        url_var = "%s_URL" % app.upper()
        key = os.environ.get(key_var, "")
        base = os.environ.get(url_var) or URLS[app][1]
        if not key:
            print("Skipping %s: %s not set" % (app, key_var), file=sys.stderr)
            continue
        try:
            records = list(history_pages(base, key, app))
        except (urllib.error.URLError, OSError, json.JSONDecodeError) as exc:
            print("Cannot reach %s (%s) — next run catches up" % (app, exc), file=sys.stderr)
            continue
        reachable += 1

        # First run (no cursor yet): seed the cursor to the newest record
        # and backfill the recent window so the first render shows
        # something without dumping the app's entire history.
        if not (state.get(app) or {}).get("last_id"):
            fresh = backfill_cutoff(records, int(time.time()))
            if records:
                state.setdefault(app, {})["last_id"] = records[0].get("id")
        else:
            fresh = new_records(app, state, records)
        # Process oldest-first so import/delete state reflects the actual
        # event chronology, then advance past skipped events as well.
        fresh.sort(key=lambda rec: (rec.get("ts") or 0, rec.get("id") or 0))
        advance_cursor(state, app, fresh)
        for rec in fresh:
            kind, updates = kind_for(app, rec, state["last_import"])
            if kind is None:
                continue
            for key, val in updates.items():
                if val is None:
                    state["last_import"].pop(key, None)
                else:
                    state["last_import"][key] = val
            entries.append(build_entry(app, rec, kind))

    if reachable == 0 and not entries:
        print("Cannot assess: neither Radarr nor Sonarr reachable", file=sys.stderr)
        return 2

    # --- render + persist -----------------------------------------------------------
    entries.sort(key=lambda e: (e.get("ts") or 0), reverse=True)
    try:
        feed_path = Path(args.feed_file)
        json_path = Path(args.json_file)
        rss_path = Path(args.rss_file)
        feed_path.parent.mkdir(parents=True, exist_ok=True)
        with feed_path.open("a", encoding="utf-8") as fh:
            for e in entries:
                fh.write(json.dumps(e, sort_keys=True) + "\n")
        json_path.write_text(render_json(entries, RENDER_LIMIT))
        rss_path.write_text(render_rss(entries, RENDER_LIMIT))
        save_state(Path(args.state_file), state)
    except OSError as exc:
        write_error = exc

    # --- report -----------------------------------------------------------------------
    if args.print_limit is not None:
        for line in render_text(entries, args.print_limit):
            print(line)
    elif args.json_out:
        print(render_json(entries, RENDER_LIMIT))
    else:
        apps_seen = sorted({e["app"] for e in entries})
        print(
            "%d new event(s) recorded (%s); feed: %s"
            % (len(entries), "+".join(apps_seen) if apps_seen else "none", feed_path)
        )

    if write_error:
        print("Failed to write feed/state: %s" % write_error, file=sys.stderr)
        return 1
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--print", dest="print_limit", type=int, metavar="N",
                        help="print the latest N entries as text")
    parser.add_argument("--json", dest="json_out", action="store_true",
                        help="print the rendered feed.json instead of the summary")
    parser.add_argument("--state-file", default=str(DEFAULT_STATE), help="cursor state path")
    parser.add_argument("--feed-file", default=str(DEFAULT_FEED), help="JSONL feed path")
    parser.add_argument("--json-file", default=str(DEFAULT_JSON), help="feed.json path")
    parser.add_argument("--rss-file", default=str(DEFAULT_RSS), help="feed.xml path")
    args = parser.parse_args(argv)
    return run(args)


if __name__ == "__main__":
    sys.exit(main())
