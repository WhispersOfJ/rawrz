-- Character state (spec §6.4.2): singleton per character (character_id is the
-- primary key). xp is cumulative; level is stored explicitly and recomputed by
-- the RPG logic on every XP change against the §5.2 level table. Streak
-- columns implement the §5.1 day-streak (local host-TZ calendar dates).

CREATE TABLE character_state (
  character_id         bigint PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
  xp                   bigint NOT NULL DEFAULT 0,
  level                int NOT NULL DEFAULT 1,
  total_watches        int NOT NULL DEFAULT 0,
  episode_watches      int NOT NULL DEFAULT 0,
  movie_watches        int NOT NULL DEFAULT 0,
  current_streak_days  int NOT NULL DEFAULT 0,
  best_streak_days     int NOT NULL DEFAULT 0,
  streak_last_watch_date date,
  genres_accessed      int NOT NULL DEFAULT 1,
  last_level_up_at     timestamptz
);
