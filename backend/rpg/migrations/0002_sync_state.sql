-- Incremental poll cursors for the content mirror.
-- The character schema is introduced by a later RPG migration; its foreign-key
-- constraint is added when that prerequisite exists.

CREATE TABLE sync_state (
  character_id  bigint NOT NULL,
  source        text NOT NULL,
  last_sync_at  timestamptz NOT NULL DEFAULT now(),
  cursor        text,
  PRIMARY KEY (character_id, source)
);
