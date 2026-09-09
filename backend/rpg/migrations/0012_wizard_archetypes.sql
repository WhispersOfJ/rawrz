-- Lantern Academy archetype state (spec §5.8.1–§5.8.3, finalized 2026-09-10).
-- This migration is additive: neutral progression ledgers remain authoritative.

CREATE TABLE wizard_archetypes (
  id                 bigserial PRIMARY KEY,
  slug               text NOT NULL UNIQUE,
  display_name       text NOT NULL,
  description        text NOT NULL,
  portrait_key       text NOT NULL,
  primary_effect     jsonb NOT NULL DEFAULT '{}',
  secondary_effect   jsonb,
  unlock_kind        text NOT NULL,
  unlock_target      bigint,
  strengths          jsonb NOT NULL DEFAULT '[]',
  weaknesses         jsonb NOT NULL DEFAULT '[]',
  created_at         timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE character_archetypes (
  character_id            bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  archetype_id            bigint NOT NULL REFERENCES wizard_archetypes(id) ON DELETE CASCADE,
  unlocked_at             timestamptz NOT NULL DEFAULT now(),
  unlock_source_event_key text NOT NULL,
  PRIMARY KEY (character_id, archetype_id)
);

CREATE INDEX character_archetypes_character ON character_archetypes(character_id);
CREATE INDEX character_archetypes_archetype ON character_archetypes(archetype_id);

CREATE TABLE character_archetype_events (
  id               bigserial PRIMARY KEY,
  character_id     bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  archetype_id     bigint REFERENCES wizard_archetypes(id) ON DELETE CASCADE,
  event_type       text NOT NULL,
  source           text NOT NULL,
  source_event_key text NOT NULL,
  outcome          text NOT NULL,
  reason           text,
  metadata         jsonb NOT NULL DEFAULT '{}',
  created_at       timestamptz NOT NULL DEFAULT now(),
  UNIQUE (character_id, event_type, source_event_key)
);

CREATE INDEX character_archetype_events_character
  ON character_archetype_events(character_id, created_at DESC);

ALTER TABLE characters
  ADD COLUMN active_archetype_id bigint,
  ADD COLUMN pending_archetype_id bigint,
  ADD COLUMN pending_archetype_requested_at timestamptz,
  ADD COLUMN pending_archetype_event_key text,
  ADD COLUMN archetype_selected_local_date date;

INSERT INTO wizard_archetypes
  (slug, display_name, description, portrait_key, primary_effect, secondary_effect,
   unlock_kind, unlock_target, strengths, weaknesses)
VALUES
  ('lantern_scholar', 'Lantern Scholar', 'A steady foundation for curious study.',
   'lantern-scholar', '{"kind":"baseline"}', NULL, 'bootstrap', NULL,
   '["reliable neutral route"]', '[]'),
  ('ember_adept', 'Ember Adept', 'A brisk path for students who favor films.',
   'ember-adept', '{"kind":"xp_modifier","component":"movie_normal_xp","percent":10}',
   '{"kind":"xp_modifier","component":"episode_normal_xp","percent":-10}',
   'level', 2, '["movie mastery"]', '["episode mastery"]'),
  ('veil_cartographer', 'Veil Cartographer', 'A mapmaker who learns to glimpse the sealed path.',
   'veil-cartographer', '{"kind":"resource","resource":"preview_token","cap":1}',
   '{"kind":"affinity_modifier","component":"movie","percent":-10}',
   'completed_order', 1, '["order preview"]', '["movie affinity"]'),
  ('rune_forger', 'Rune Forger', 'A meticulous scholar of visible accomplishments.',
   'rune-forger', '{"kind":"display_modifier","component":"visible_achievement_progress","percent":10}',
   '{"kind":"xp_modifier","component":"normal_xp","percent":-10}',
   'achievements', 3, '["achievement clarity"]', '["normal mastery"]'),
  ('star_shepherd', 'Star Shepherd', 'A patient keeper of continuity across difficult weeks.',
   'star-shepherd', '{"kind":"resource","resource":"streak_ward","cap":1}',
   '{"kind":"affinity_modifier","component":"episode","percent":-10}',
   'streak', 7, '["streak resilience"]', '["episode affinity"]'),
  ('moonlit_mediator', 'Moonlit Mediator', 'A bridge-builder among many disciplines.',
   'moonlit-mediator', '{"kind":"affinity_modifier","component":"matching_discipline","percent":10}',
   '{"kind":"xp_modifier","component":"horror_normal_xp","percent":-10}',
   'genres_accessed', 3, '["discipline affinity"]', '["Shadowcraft mastery"]')
ON CONFLICT (slug) DO NOTHING;

-- Backfill existing characters without touching neutral progression history.
UPDATE characters
SET active_archetype_id = (
  SELECT id FROM wizard_archetypes WHERE slug = 'lantern_scholar'
)
WHERE active_archetype_id IS NULL;

INSERT INTO character_archetypes (character_id, archetype_id, unlock_source_event_key)
SELECT c.id, a.id, 'bootstrap:lantern_scholar'
FROM characters c
JOIN wizard_archetypes a ON a.slug = 'lantern_scholar'
ON CONFLICT (character_id, archetype_id) DO NOTHING;

ALTER TABLE characters
  ADD CONSTRAINT characters_active_archetype
  FOREIGN KEY (active_archetype_id) REFERENCES wizard_archetypes(id) ON DELETE RESTRICT;

ALTER TABLE characters
  ALTER COLUMN active_archetype_id SET NOT NULL;

ALTER TABLE characters
  ADD CONSTRAINT characters_pending_archetype_fields
  CHECK (
    (pending_archetype_id IS NULL
      AND pending_archetype_requested_at IS NULL
      AND pending_archetype_event_key IS NULL)
    OR
    (pending_archetype_id IS NOT NULL
      AND pending_archetype_requested_at IS NOT NULL
      AND pending_archetype_event_key IS NOT NULL)
  );

ALTER TABLE characters
  ADD CONSTRAINT characters_pending_archetype
  FOREIGN KEY (pending_archetype_id) REFERENCES wizard_archetypes(id) ON DELETE RESTRICT;
