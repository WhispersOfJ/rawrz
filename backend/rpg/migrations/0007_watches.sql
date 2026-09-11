-- Watch records + genre XP ledger (spec §6.4.5, §6.4.3). `watches` is the
-- RPG's own awarded-completion ledger (Plex stays authoritative for watch
-- state). `genre_xp_ledger` lands here rather than with the other genre
-- tables (0004): its source_watch_id foreign key targets watches(id) — the
-- deferred-prerequisite pattern used for sync_state (0002 → 0003).

CREATE TABLE watches (
  id                  bigserial PRIMARY KEY,
  character_id        bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  content_id          bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  content_type        text NOT NULL,               -- 'movie' | 'episode'
  completed_at        timestamptz NOT NULL DEFAULT now(),
  pct_viewed          numeric NOT NULL,
  xp_awarded          bigint NOT NULL,
  normal_xp           bigint NOT NULL,
  bonuses             jsonb NOT NULL DEFAULT '[]',
  new_arrival         boolean NOT NULL DEFAULT false,
  new_arrival_at      timestamptz,
  featured            boolean NOT NULL DEFAULT false,
  -- Deferred FK: featured_cases is created in 0009 — the constraint is
  -- added there (ALTER TABLE watches ADD CONSTRAINT watches_featured_case).
  featured_case_id    bigint,
  season_bonus        boolean NOT NULL DEFAULT false,
  series_bonus        boolean NOT NULL DEFAULT false,
  first_completion    boolean NOT NULL DEFAULT false,
  holiday_bonus       jsonb,
  via_plex            boolean NOT NULL DEFAULT true,
  via_manual          boolean NOT NULL DEFAULT false,
  created_at          timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX watches_character ON watches(character_id);
CREATE INDEX watches_completed_at ON watches(character_id, completed_at DESC);
CREATE INDEX watches_content ON watches(content_id);

-- Sub-genre XP audit trail (§6.4.3): one row per XP addition, optionally
-- linked to the watch that generated it.
CREATE TABLE genre_xp_ledger (
  id              bigserial PRIMARY KEY,
  character_id    bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  sub_genre_id    bigint NOT NULL REFERENCES sub_genres(id) ON DELETE CASCADE,
  xp_added        bigint NOT NULL,
  source_watch_id bigint REFERENCES watches(id),
  at              timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX genre_xp_ledger_character ON genre_xp_ledger(character_id);
CREATE INDEX genre_xp_ledger_sub_genre ON genre_xp_ledger(sub_genre_id);
CREATE INDEX genre_xp_ledger_source_watch ON genre_xp_ledger(source_watch_id)
  WHERE source_watch_id IS NOT NULL;
