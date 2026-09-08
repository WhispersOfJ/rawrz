-- V1 accounts + characters (spec §6.4.1): single PIN-gated account, single
-- investigator character. Completes the sync_state foreign key that
-- 0002_sync_state deferred until this prerequisite existed.

CREATE TABLE accounts (
  id          bigserial PRIMARY KEY,
  pin_hash    text NOT NULL,        -- hashed PIN (argon2 or similar - hashing crate finalized during implementation)
  pin_salts   text NOT NULL,        -- salt/params storage needed to verify the PIN
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE characters (
  id          bigserial PRIMARY KEY,
  account_id  bigint NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  name        text NOT NULL DEFAULT 'The Investigator',
  created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX characters_account ON characters(account_id);

-- Backfill the sync_state.character_id foreign key deferred by 0002_sync_state.
ALTER TABLE sync_state
  ADD CONSTRAINT sync_state_character
  FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE;
