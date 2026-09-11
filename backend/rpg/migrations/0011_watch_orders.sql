-- Mystery watch orders (spec §5.7, §6.4.13). One active order per genre per
-- character; item resolution is DERIVED from the watches ledger (the ≥95%
-- award — never stored here), and skip rewards are an audited grant ledger
-- (balance = unspent rows), mirroring the genre_xp_ledger audit pattern.

CREATE TABLE watch_orders (
  id           bigserial PRIMARY KEY,
  character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  genre_id     bigint NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  cycle_number int NOT NULL,                     -- 1, 2, 3… per (character, genre)
  status       text NOT NULL DEFAULT 'active',   -- 'active' | 'completed'
  created_at   timestamptz NOT NULL DEFAULT now(),
  completed_at timestamptz,
  UNIQUE (character_id, genre_id, cycle_number)
);

CREATE TABLE watch_order_items (
  id           bigserial PRIMARY KEY,
  order_id     bigint NOT NULL REFERENCES watch_orders(id) ON DELETE CASCADE,
  position     int NOT NULL,                     -- 1-based within the order
  content_id   bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  revealed_at  timestamptz NOT NULL DEFAULT now(),  -- item 1 at creation, others at reveal
  skipped_at   timestamptz,                      -- stamped when a skip is spent here
  UNIQUE (order_id, position)
);

-- No completion column: an item is resolved when a `watches` row exists for
-- its content_id, or when skipped_at IS NOT NULL. One threshold, one ledger.

CREATE TABLE skip_grants (
  id              bigserial PRIMARY KEY,
  character_id    bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  source_order_id bigint NOT NULL REFERENCES watch_orders(id) ON DELETE CASCADE,
  earned_at       timestamptz NOT NULL DEFAULT now(),
  spent_at        timestamptz,
  spent_item_id   bigint REFERENCES watch_order_items(id)
);

CREATE INDEX watch_orders_character ON watch_orders(character_id);
CREATE INDEX watch_order_items_order ON watch_order_items(order_id, position);
CREATE INDEX skip_grants_character ON skip_grants(character_id) WHERE spent_at IS NULL;
CREATE INDEX watch_order_items_content ON watch_order_items(content_id);
