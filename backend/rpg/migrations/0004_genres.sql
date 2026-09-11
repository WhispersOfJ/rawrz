-- Genre access + sub-genre XP (spec §6.4.3). Seeds the fixed horror-first
-- genre list finalized in spec §5.2 (2026-09-08): 10 genres matching the
-- 10-level table. genre_xp_ledger is deferred to the watches migration: its
-- source_watch_id foreign key targets watches(id), same deferred-prerequisite
-- pattern as sync_state (0002 deferred to 0003).

CREATE TABLE genres (
  id          bigserial PRIMARY KEY,
  name        text NOT NULL UNIQUE,
  list_order  int NOT NULL,
  is_opening  boolean NOT NULL DEFAULT false
);

-- The fixed genre list order (spec §5.2 cascade): horror first, then the
-- remaining genres in purchase order. One new genre becomes purchasable per
-- level, so the list length matches the level table (§5.2).
INSERT INTO genres (name, list_order, is_opening) VALUES
  ('Horror',      1, true),
  ('Thriller',    2, false),
  ('Mystery',     3, false),
  ('Sci-Fi',      4, false),
  ('Fantasy',     5, false),
  ('Documentary', 6, false),
  ('Comedy',      7, false),
  ('Drama',       8, false),
  ('Romance',     9, false),
  ('Animation',  10, false);

CREATE TABLE sub_genres (
  id          bigserial PRIMARY KEY,
  genre_id    bigint NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  name        text NOT NULL,
  UNIQUE (genre_id, name)
);

-- Sub-genres are not seeded here: they are created from the library mirror on
-- first sync or on demand as watches accumulate in new sub-genres (§6.4.11).

CREATE TABLE genre_access (
  character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  genre_id     bigint NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  accessed_at  timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (character_id, genre_id)
);

CREATE TABLE sub_genre_xp (
  character_id  bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  sub_genre_id  bigint NOT NULL REFERENCES sub_genres(id) ON DELETE CASCADE,
  xp            bigint NOT NULL DEFAULT 0,
  purchased     boolean NOT NULL DEFAULT false,
  purchased_at  timestamptz,
  PRIMARY KEY (character_id, sub_genre_id)
);
