-- Featured cases: one featured case per period per character (spec §6.4.7).
-- V1 cadence is weekly (§5.4), so `period` holds the ISO week label 'YYYY-Www'
-- (e.g. '2026-W37'). This migration also completes both foreign keys deferred
-- to it: watches.featured_case_id (created plain in 0007) and
-- cases.featured_case_id (created plain in 0008).

CREATE TABLE featured_cases (
  id             bigserial PRIMARY KEY,
  character_id   bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  period         text NOT NULL,                  -- ISO week label 'YYYY-Www', e.g. '2026-W37'
  content_id     bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  selection_mode text NOT NULL,                  -- 'new_arrival' | 'all_time_ranking'
  bonus_xp       bigint NOT NULL DEFAULT 10,     -- featured bonus XP (§5.1: +10)
  created_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (character_id, period)
);

-- Complete the deferred watches.featured_case_id foreign key (from 0007).
ALTER TABLE watches
  ADD CONSTRAINT watches_featured_case
  FOREIGN KEY (featured_case_id) REFERENCES featured_cases(id);

-- Complete the deferred cases.featured_case_id foreign key (from 0008).
ALTER TABLE cases
  ADD CONSTRAINT cases_featured_case
  FOREIGN KEY (featured_case_id) REFERENCES featured_cases(id);
