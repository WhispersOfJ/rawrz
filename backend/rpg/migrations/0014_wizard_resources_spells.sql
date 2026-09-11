-- Lantern Academy resource and spell economy (spec §5.8.4–§5.8.6).
-- This migration is additive: neutral watches, XP, orders, skips, genres, and
-- achievements remain the authoritative progression ledgers.

CREATE TABLE character_wizard_resources (
  character_id                         bigint PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
  preview_tokens                       int NOT NULL DEFAULT 0 CHECK (preview_tokens BETWEEN 0 AND 1),
  streak_wards                         int NOT NULL DEFAULT 0 CHECK (streak_wards BETWEEN 0 AND 1),
  streak_ward_last_granted_local_date  date,
  updated_at                           timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE character_resource_events (
  id               bigserial PRIMARY KEY,
  character_id     bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  resource         text NOT NULL,
  event_type       text NOT NULL,
  source           text NOT NULL,
  source_event_key text NOT NULL,
  delta            int NOT NULL,
  balance_after    int NOT NULL,
  outcome          text NOT NULL,
  metadata         jsonb NOT NULL DEFAULT '{}',
  created_at       timestamptz NOT NULL DEFAULT now(),
  UNIQUE (character_id, resource, event_type, source_event_key)
);

CREATE INDEX character_resource_events_character
  ON character_resource_events(character_id, created_at DESC);

-- Second Sight changes presentation/priority on an active order. It never
-- changes candidate ranking or completion truth.
ALTER TABLE watch_orders
  ADD COLUMN study_target boolean NOT NULL DEFAULT false;

CREATE TABLE spells (
  id            bigserial PRIMARY KEY,
  slug          text NOT NULL UNIQUE,
  display_name  text NOT NULL,
  description  text NOT NULL,
  effect_type   text NOT NULL,
  parameters    jsonb NOT NULL DEFAULT '{}',
  unlock_kind   text NOT NULL,
  unlock_target bigint,
  charge_cap    int NOT NULL DEFAULT 3 CHECK (charge_cap > 0),
  created_at    timestamptz NOT NULL DEFAULT now()
);

INSERT INTO spells
  (slug, display_name, description, effect_type, parameters, unlock_kind, unlock_target, charge_cap)
VALUES
  ('vanishing_step', 'Vanishing Step',
   'Resolve the current non-final lesson without watching it.',
   'skip_order_item', '{"finale_protected":true}', 'completed_order', 1, 3),
  ('unsealing_light', 'Unsealing Light',
   'Reveal the next legally revealable locked lesson without resolving its predecessor.',
   'reveal_order_item', '{"finale_protected":true}', 'level', 2, 3),
  ('chronicle_ward', 'Chronicle Ward',
   'Protect one future cold-gap streak transition.',
   'protect_streak_gap', '{"one_transition":true}', 'streak', 7, 3),
  ('focus_sigil', 'Focus Sigil',
   'Add ten percent to the next watch normal-mastery component.',
   'next_watch_xp', '{"percent":10}', 'achievements', 5, 3),
  ('second_sight', 'Second Sight',
   'Mark an eligible active order as the study target for its next cycle.',
   'study_target', '{"next_cycle":true}', 'genres_accessed', 3, 3)
ON CONFLICT (slug) DO NOTHING;

CREATE TABLE spell_affinities (
  character_id                    bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  spell_id                        bigint NOT NULL REFERENCES spells(id) ON DELETE CASCADE,
  selected_genre_id               bigint REFERENCES genres(id) ON DELETE RESTRICT,
  pending_genre_id                bigint REFERENCES genres(id) ON DELETE RESTRICT,
  pending_requested_at            timestamptz,
  pending_event_key               text,
  affinity_progress_points        bigint NOT NULL DEFAULT 0 CHECK (affinity_progress_points BETWEEN 0 AND 99),
  last_affinity_change_local_date date,
  updated_at                      timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (character_id, spell_id),
  CHECK (
    (pending_genre_id IS NULL AND pending_requested_at IS NULL AND pending_event_key IS NULL)
    OR
    (pending_genre_id IS NOT NULL AND pending_requested_at IS NOT NULL AND pending_event_key IS NOT NULL)
  )
);

CREATE INDEX spell_affinities_character ON spell_affinities(character_id);
CREATE INDEX spell_affinities_selected_genre ON spell_affinities(selected_genre_id)
  WHERE selected_genre_id IS NOT NULL;

CREATE TABLE character_spell_events (
  id               bigserial PRIMARY KEY,
  character_id     bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  spell_id         bigint NOT NULL REFERENCES spells(id) ON DELETE CASCADE,
  event_type       text NOT NULL,
  source           text NOT NULL,
  source_event_key text NOT NULL,
  outcome          text NOT NULL,
  reason           text,
  metadata         jsonb NOT NULL DEFAULT '{}',
  created_at       timestamptz NOT NULL DEFAULT now(),
  UNIQUE (character_id, spell_id, event_type, source_event_key)
);

CREATE INDEX character_spell_events_character
  ON character_spell_events(character_id, created_at DESC);

-- Append-only charge/affinity/cast audit. A grant row represents one charge.
-- Casts normally stamp a grant at cast time. Chronicle Ward reserves a grant
-- until the protected cold-gap transition, then stamps spent_at.
CREATE TABLE character_spell_ledger (
  id                bigserial PRIMARY KEY,
  character_id      bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  spell_id          bigint NOT NULL REFERENCES spells(id) ON DELETE CASCADE,
  entry_type        text NOT NULL CHECK (entry_type IN ('affinity', 'grant', 'overflow_noop', 'cast')),
  source            text NOT NULL,
  source_event_key  text NOT NULL,
  affinity_points   bigint NOT NULL DEFAULT 0,
  granted_at        timestamptz NOT NULL DEFAULT now(),
  spent_at          timestamptz,
  reserved_at       timestamptz,
  reserved_event_key text,
  applied_at        timestamptz,
  target_order_id   bigint REFERENCES watch_orders(id) ON DELETE SET NULL,
  target_item_id    bigint REFERENCES watch_order_items(id) ON DELETE SET NULL,
  grant_id          bigint REFERENCES character_spell_ledger(id) ON DELETE SET NULL,
  outcome           text NOT NULL,
  metadata          jsonb NOT NULL DEFAULT '{}',
  UNIQUE (character_id, spell_id, entry_type, source_event_key),
  CHECK (
    (reserved_at IS NULL AND reserved_event_key IS NULL)
    OR
    (reserved_at IS NOT NULL AND reserved_event_key IS NOT NULL)
  )
);

CREATE INDEX character_spell_ledger_character
  ON character_spell_ledger(character_id, granted_at DESC);
CREATE INDEX character_spell_ledger_available_grants
  ON character_spell_ledger(character_id, spell_id)
  WHERE entry_type = 'grant' AND spent_at IS NULL AND reserved_at IS NULL;
CREATE INDEX character_spell_ledger_pending_casts
  ON character_spell_ledger(character_id, spell_id)
  WHERE entry_type = 'cast' AND outcome = 'armed';

-- Unsealing Light has its own audit rather than overloading the ordinary
-- reveal timestamp. A reveal never resolves an item.
CREATE TABLE watch_order_spell_reveals (
  id               bigserial PRIMARY KEY,
  order_id         bigint NOT NULL REFERENCES watch_orders(id) ON DELETE CASCADE,
  item_id          bigint NOT NULL REFERENCES watch_order_items(id) ON DELETE CASCADE,
  spell_ledger_id  bigint NOT NULL REFERENCES character_spell_ledger(id) ON DELETE CASCADE,
  revealed_at      timestamptz NOT NULL DEFAULT now(),
  UNIQUE (spell_ledger_id),
  UNIQUE (order_id, item_id, spell_ledger_id)
);

CREATE INDEX watch_order_spell_reveals_item
  ON watch_order_spell_reveals(order_id, item_id);
