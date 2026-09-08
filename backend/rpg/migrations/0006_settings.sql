-- Settings / config (spec §6.4.10): the RPG's toggleable/tunable feature
-- surface. Created here (not last) because character-creation bootstrap seeds
-- the V1 defaults right after character_state exists (§6.4.11 placement note).
-- Rows are seeded per character by the application at character creation, not
-- by this migration.

CREATE TABLE settings (
  character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  key          text NOT NULL,
  value        text NOT NULL,
  PRIMARY KEY (character_id, key)
);
