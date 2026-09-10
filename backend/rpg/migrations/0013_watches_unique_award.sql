-- Award-once enforcement at the database level (spec §5.7): a character
-- holds at most one awarded watch per content item. The application path
-- (advisory lock + INSERT ... NOT EXISTS) stays as the friendly guard; this
-- constraint is the backstop that makes the invariant hold even for a second
-- writer (future worker, manual SQL, second instance). V1 databases are
-- fresh and single-account, so no existing rows can violate it.
ALTER TABLE watches
  ADD CONSTRAINT watches_character_content_unique UNIQUE (character_id, content_id);
