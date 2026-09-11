-- Cases: the player-driven case board (spec §6.4.6). One row per case card
-- offered to a character: movie cases, series campaigns, and featured cases.
-- featured_case_id's foreign key to featured_cases is deferred to 0009, which
-- creates that table (ALTER TABLE cases ADD CONSTRAINT cases_featured_case) —
-- the same deferred-prerequisite pattern as watches.featured_case_id (0007).

CREATE TABLE cases (
  id                  bigserial PRIMARY KEY,
  character_id        bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  content_id          bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  case_type           text NOT NULL,               -- 'movie_case' | 'series_campaign' | 'featured'
  status              text NOT NULL DEFAULT 'available',  -- 'available' | 'taken' | 'in_progress' | 'completed'
  taken_at            timestamptz,
  completed_at        timestamptz,
  -- The watch row that completed this case (for movie_case: the movie
  -- watch, for series_campaign: the watch that completed the final episode).
  completion_watch_id bigint REFERENCES watches(id),
  bonus_flags         jsonb NOT NULL DEFAULT '[]',
  -- Deferred FK: featured_cases is created in 0009 — the constraint is added
  -- there (ALTER TABLE cases ADD CONSTRAINT cases_featured_case).
  featured_case_id    bigint,
  created_at          timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX cases_character ON cases(character_id);
CREATE INDEX cases_status ON cases(character_id, status);
