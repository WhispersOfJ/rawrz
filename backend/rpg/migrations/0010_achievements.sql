-- Achievements: definition + unlock tables (spec §6.4.8) and the §5.5
-- first-cut achievement seed (the full Q8 list, 101 entries). Slugs are the
-- stable internal ids, names are display strings (tiered achievements share
-- a name), target_value applies to counter kind, and metadata carries the
-- per-achievement evaluation parameters (finalized per row below).

CREATE TABLE achievements (
  id           bigserial PRIMARY KEY,
  slug         text NOT NULL UNIQUE,             -- e.g. 'first_blood', 'horror_native'
  name         text NOT NULL,
  description  text NOT NULL,
  category     text NOT NULL,                    -- 'completion_milestone' | 'genre_coverage' | 'time_streak' | 'novelty_firsts' | 'themed_quirky' | 'combo'
  visible      boolean NOT NULL DEFAULT true,   -- visible vs hidden (§5.5)
  kind         text NOT NULL,                     -- 'once' | 'progress' | 'streak' | 'counter' | 'combo'
  target_value bigint,                           -- for counter/progress kinds: the target count
  metadata     jsonb NOT NULL DEFAULT '{}',     -- evaluation metadata (content type, genre, window, day-of-week, ...)
  created_at   timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE character_achievements (
  character_id   bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  achievement_id bigint NOT NULL REFERENCES achievements(id) ON DELETE CASCADE,
  unlocked_at    timestamptz NOT NULL DEFAULT now(),
  progress       bigint NOT NULL DEFAULT 0,     -- current progress for progress/counter kinds (e.g. 5/10)
  PRIMARY KEY (character_id, achievement_id)
);

CREATE INDEX achievements_category ON achievements(category);
CREATE INDEX achievements_visible ON achievements(visible);

-- §5.5 first-cut list, in spec order. Ambiguities marked "finalize during
-- implementation" in the spec are resolved here and in §6.4.8: Spooky Season
-- counts cumulatively across years, Rainy Day and Actor's Playground carry
-- metadata_dependent flags, and the daily-budget/fame firsts are gated on
-- their optional features.
INSERT INTO achievements (slug, name, description, category, visible, kind, target_value, metadata) VALUES
  -- §5.5.1 Completion milestones (visible)
  ('first_blood', 'First Blood', 'Complete your first episode (any show).', 'completion_milestone', true, 'once', NULL, '{"content_type":"episode"}'),
  ('case_closed', 'Case Closed', 'Complete your first movie.', 'completion_milestone', true, 'once', NULL, '{"content_type":"movie"}'),
  ('double_feature', 'Double Feature', 'Complete 2 movies in the same real day.', 'completion_milestone', true, 'once', NULL, '{"content_type":"movie","count":2,"window":"same_day"}'),
  ('episode_10', 'Episode 10', 'Complete 10 episodes total.', 'completion_milestone', true, 'counter', 10, '{"content_type":"episode"}'),
  ('episode_50', 'Episode 50', 'Complete 50 episodes total.', 'completion_milestone', true, 'counter', 50, '{"content_type":"episode"}'),
  ('episode_100', 'Episode 100', 'Complete 100 episodes total.', 'completion_milestone', true, 'counter', 100, '{"content_type":"episode"}'),
  ('silver_screen_novice', 'Silver Screen Novice', 'Complete 10 movies total.', 'completion_milestone', true, 'counter', 10, '{"content_type":"movie"}'),
  ('silver_screen_devotee', 'Silver Screen Devotee', 'Complete 50 movies total.', 'completion_milestone', true, 'counter', 50, '{"content_type":"movie"}'),
  ('one_season_under_your_belt', 'One Season Under Your Belt', 'Complete all episodes of one season of any show.', 'completion_milestone', true, 'once', NULL, '{"scope":"season"}'),
  ('two_seasons', 'Two Seasons', 'Complete all episodes of two seasons (same or different shows).', 'completion_milestone', true, 'counter', 2, '{"scope":"season"}'),
  ('series_completed', 'Series Completed', 'Finish every episode of an entire series (all seasons).', 'completion_milestone', true, 'once', NULL, '{"scope":"series"}'),
  ('collector', 'Collector', 'Complete 5 different series.', 'completion_milestone', true, 'counter', 5, '{"scope":"series"}'),
  ('completist', 'Completist', 'Complete 10 different series.', 'completion_milestone', true, 'counter', 10, '{"scope":"series"}'),
  ('backlog_burner', 'Backlog Burner', 'Complete a series where at least 3 episodes were already watched when you took the case.', 'completion_milestone', true, 'once', NULL, '{"backlog_episodes":3}'),
  ('full_slate', 'Full Slate', 'Complete at least one episode and one movie in the same real day.', 'completion_milestone', true, 'once', NULL, '{"requires":["episode","movie"],"window":"same_day"}'),
  -- §5.5.2 Completion milestones (hidden)
  ('the_quiet_100', 'The Quiet 100', 'Complete 100 episodes without ever manually refreshing the case board.', 'completion_milestone', false, 'counter', 100, '{"content_type":"episode","no_manual_refresh":true}'),
  ('ghost_completer', 'Ghost Completer', 'Complete a series where the whole series was already in your watched history when you claimed it.', 'completion_milestone', false, 'once', NULL, '{"all_pre_watched":true}'),
  ('one_click_wonder', 'One-Click Wonder', 'Complete a featured case on the same day you first saw it appear.', 'completion_milestone', false, 'once', NULL, '{"case_type":"featured","window":"same_day"}'),
  ('twice_in_a_day', 'Twice in a Day', 'Complete the final episodes of two different series in the same real day.', 'completion_milestone', false, 'once', NULL, '{"count":2,"window":"same_day"}'),
  ('slow_burn', 'Slow Burn', 'Complete a 10+ season series (or equivalent long campaign).', 'completion_milestone', false, 'once', NULL, '{"min_seasons":10}'),
  -- §5.5.3 Genre coverage (visible)
  ('first_genre_explored', 'First Genre Explored', 'Complete at least one watch in your first unlocked genre (horror, the opening genre).', 'genre_coverage', true, 'once', NULL, '{"genre":"Horror"}'),
  ('genre_explorer_3', 'Genre Explorer', 'Complete at least one watch in 3 different genres.', 'genre_coverage', true, 'counter', 3, '{"distinct_genres":3}'),
  ('genre_explorer_5', 'Genre Explorer', 'Complete at least one watch in 5 different genres.', 'genre_coverage', true, 'counter', 5, '{"distinct_genres":5}'),
  ('genre_explorer_8', 'Genre Explorer', 'Complete at least one watch in 8 different genres.', 'genre_coverage', true, 'counter', 8, '{"distinct_genres":8}'),
  ('genre_explorer_10', 'Genre Explorer', 'Complete at least one watch in 10 different genres.', 'genre_coverage', true, 'counter', 10, '{"distinct_genres":10}'),
  ('horror_homeground_10', 'Horror Homeground', 'Complete 10 horror watches (the opening genre).', 'genre_coverage', true, 'counter', 10, '{"genre":"Horror"}'),
  ('horror_homeground_25', 'Horror Homeground', 'Complete 25 horror watches.', 'genre_coverage', true, 'counter', 25, '{"genre":"Horror"}'),
  ('genre_purchase_1', 'Genre Purchase', 'Buy your first sub-genre unlock with sub-genre XP (the first purchased genre beyond horror).', 'genre_coverage', true, 'once', NULL, '{"purchases":1}'),
  ('genre_purchase_3', 'Genre Purchase', 'Buy 3 sub-genres via sub-genre XP.', 'genre_coverage', true, 'counter', 3, '{"purchases":3}'),
  ('genre_purchase_5', 'Genre Purchase', 'Buy 5 sub-genres via sub-genre XP.', 'genre_coverage', true, 'counter', 5, '{"purchases":5}'),
  ('genre_fiesta', 'Genre Fiesta', 'Complete at least one watch in a sub-genre you just bought (same poll cycle or next).', 'genre_coverage', true, 'once', NULL, '{"timing":"same_or_next_poll"}'),
  ('variety_player', 'Variety Player', 'Complete watches in at least one sub-genre for 5 different parent genres.', 'genre_coverage', true, 'counter', 5, '{"distinct_parent_genres":5}'),
  ('broad_coverage', 'Broad Coverage', 'Complete at least one watch in sub-genres across 3 different parent genres in the same real day.', 'genre_coverage', true, 'once', NULL, '{"distinct_parent_genres":3,"window":"same_day"}'),
  -- §5.5.4 Genre coverage (hidden)
  ('horror_native', 'Horror Native', 'Complete 50 horror watches before buying any other genre.', 'genre_coverage', false, 'counter', 50, '{"genre":"Horror","before_other_purchase":true}'),
  ('sub_genre_hoarder', 'Sub-genre Hoarder', 'Buy 10 sub-genres via sub-genre XP.', 'genre_coverage', false, 'counter', 10, '{"purchases":10}'),
  ('full_catalog', 'Full Catalog', 'Unlock every sub-genre the library exposes (within the current library genre set).', 'genre_coverage', false, 'once', NULL, '{"scope":"library"}'),
  ('peerless_variety', 'Peerless Variety', 'Complete watches in 15 different genres.', 'genre_coverage', false, 'counter', 15, '{"distinct_genres":15}'),
  ('depth_and_breadth', 'Depth & Breadth', 'Complete both a 10+ season series and watches in 10+ genres.', 'genre_coverage', false, 'combo', NULL, '{"min_seasons":10,"distinct_genres":10}'),
  -- §5.5.5 Time / streak (visible)
  ('back_to_back', 'Back-to-Back', 'Complete watches on 2 consecutive real days.', 'time_streak', true, 'streak', 2, '{}'),
  ('streak_starter', 'Streak Starter', 'Maintain a 3-day watch streak (a completed watch on each of 3 consecutive real days).', 'time_streak', true, 'streak', 3, '{}'),
  ('streak_builder', 'Streak Builder', 'Maintain a 5-day watch streak.', 'time_streak', true, 'streak', 5, '{}'),
  ('streak_keeper', 'Streak Keeper', 'Maintain a 7-day watch streak.', 'time_streak', true, 'streak', 7, '{}'),
  ('streak_veteran', 'Streak Veteran', 'Maintain a 14-day watch streak.', 'time_streak', true, 'streak', 14, '{}'),
  ('streak_legend', 'Streak Legend', 'Maintain a 30-day watch streak.', 'time_streak', true, 'streak', 30, '{}'),
  ('no_gap', 'No Gap', 'Maintain a 7-day streak where each day also included a new-arrival bonus watch.', 'time_streak', true, 'streak', 7, '{"require_new_arrival":true}'),
  ('weekend_warrior', 'Weekend Warrior', 'Complete at least one watch on each of 4 weekends in a row (Saturday or Sunday).', 'time_streak', true, 'counter', 4, '{"weekends":true}'),
  ('holiday_heat_1', 'Holiday Heat', 'Complete a watch during a detected holiday window and earn the holiday bonus for that window.', 'time_streak', true, 'once', NULL, '{"holiday_bonus":1}'),
  ('holiday_heat_3', 'Holiday Heat', 'Earn the holiday bonus in 3 different holiday windows.', 'time_streak', true, 'counter', 3, '{"distinct_windows":3}'),
  -- §5.5.6 Time / streak (hidden)
  ('iron_streak', 'Iron Streak', 'Maintain a 60-day watch streak.', 'time_streak', false, 'streak', 60, '{}'),
  ('unbroken', 'Unbroken', 'Maintain a 30-day streak without a single day relying solely on a re-watch (every day had a new completion).', 'time_streak', false, 'streak', 30, '{"no_rewatch_only_days":true}'),
  ('holiday_sprinter', 'Holiday Sprinter', 'Complete 3 watches across 3 different holiday windows within their respective windows.', 'time_streak', false, 'counter', 3, '{"distinct_windows":3}'),
  ('marathon_level5', 'Marathon', 'Reach level 5 while maintaining a 14-day streak through the level-up.', 'time_streak', false, 'combo', NULL, '{"level":5,"streak":14}'),
  ('seasonal_veteran', 'Seasonal Veteran', 'Earn holiday bonuses in all holiday windows the current calendar year exposes.', 'time_streak', false, 'once', NULL, '{"scope":"calendar_year"}'),
  -- §5.5.7 Novelty / firsts (visible)
  ('first_featured_case', 'First Featured Case', 'Complete your first featured case (any selection mode).', 'novelty_firsts', true, 'once', NULL, '{"case_type":"featured"}'),
  ('featured_fan', 'Featured Fan', 'Complete 5 featured cases.', 'novelty_firsts', true, 'counter', 5, '{"case_type":"featured"}'),
  ('featured_master', 'Featured Master', 'Complete 10 featured cases.', 'novelty_firsts', true, 'counter', 10, '{"case_type":"featured"}'),
  ('first_new_arrival_bonus', 'First New-Arrival Bonus', 'Earn a new-arrival bonus (watched something within its new-arrival window).', 'novelty_firsts', true, 'once', NULL, '{"new_arrival":true}'),
  ('new_arrival_habit', 'New-Arrival Habit', 'Earn new-arrival bonuses on 5 different titles.', 'novelty_firsts', true, 'counter', 5, '{"new_arrival_titles":5}'),
  ('speed_demon_48h', 'Speed Demon', 'Complete a new arrival within 48 hours of its arrival on the stack (Sonarr/Radarr import).', 'novelty_firsts', true, 'once', NULL, '{"hours":48}'),
  ('speed_demon_24h', 'Speed Demon', 'Complete a new arrival within 24 hours of arrival.', 'novelty_firsts', true, 'once', NULL, '{"hours":24}'),
  ('first_purchase', 'First Purchase', 'Buy your first sub-genre unlock (first genre purchase beyond horror).', 'novelty_firsts', true, 'once', NULL, '{"purchases":1}'),
  ('level_up_2', 'Level Up', 'Reach level 2.', 'novelty_firsts', true, 'counter', 2, '{"level":2}'),
  ('level_up_3', 'Level Up', 'Reach level 3.', 'novelty_firsts', true, 'counter', 3, '{"level":3}'),
  ('level_up_5', 'Level Up', 'Reach level 5.', 'novelty_firsts', true, 'counter', 5, '{"level":5}'),
  ('level_up_10', 'Level Up', 'Reach level 10.', 'novelty_firsts', true, 'counter', 10, '{"level":10}'),
  ('first_banked_day', 'First Banked Day', 'Use a daily investigation budget for the first time (if daily budget is enabled).', 'novelty_firsts', true, 'once', NULL, '{"enabled_feature":"daily_budget"}'),
  ('first_fame_tick', 'First Fame Tick', 'Earn your first fame/renown increment (if fame is enabled).', 'novelty_firsts', true, 'once', NULL, '{"enabled_feature":"fame"}'),
  -- §5.5.8 Novelty / firsts (hidden)
  ('instant_case', 'Instant Case', 'Take a case from the board and complete it within the same poll cycle.', 'novelty_firsts', false, 'once', NULL, '{"window":"same_poll"}'),
  ('double_speed', 'Double Speed', 'Complete two different new arrivals within 24 hours of each of their arrivals.', 'novelty_firsts', false, 'once', NULL, '{"count":2,"hours":24}'),
  ('featured_streak', 'Featured Streak', 'Complete 3 featured cases in a row (3 consecutive featured cases completed, one after another).', 'novelty_firsts', false, 'counter', 3, '{"case_type":"featured","consecutive":true}'),
  ('first_day_fighter', 'First-Day Fighter', 'On your very first day in the RPG, complete both a movie and an episode.', 'novelty_firsts', false, 'once', NULL, '{"day":1,"requires":["movie","episode"]}'),
  ('new_old_hybrid', 'New-Old Hybrid', 'Complete a new-arrival bonus on a title that was also a featured case.', 'novelty_firsts', false, 'once', NULL, '{"new_arrival":true,"featured":true}'),
  -- §5.5.9 Themed / quirky (visible)
  ('double_feature_special', 'Double Feature Special', 'Watch two movies from the same franchise/franchise-indicated pair in the same real day.', 'themed_quirky', true, 'once', NULL, '{"count":2,"window":"same_day","scope":"franchise"}'),
  ('directors_cut', 'Director''s Cut', 'Complete two movies by the same director (director metadata, where available) within 7 days.', 'themed_quirky', true, 'once', NULL, '{"count":2,"window_days":7,"scope":"director"}'),
  ('themed_night', 'Themed Night', 'Complete 3 movies/episodes in the same genre in the same real day.', 'themed_quirky', true, 'once', NULL, '{"count":3,"window":"same_day"}'),
  ('binge_builder', 'Binge Builder', 'Complete 5 episodes of the same series within 48 hours.', 'themed_quirky', true, 'counter', 5, '{"hours":48,"scope":"series"}'),
  ('weekend_binge', 'Weekend Binge', 'Complete 5 episodes of the same series over a single weekend.', 'themed_quirky', true, 'counter', 5, '{"window":"weekend","scope":"series"}'),
  ('marathon_season', 'Marathon', 'Complete an entire season worth of episodes within 7 days of starting it.', 'themed_quirky', true, 'once', NULL, '{"scope":"season","window_days":7}'),
  ('holiday_haunter', 'Holiday Haunter', 'Complete a horror watch during the Halloween window (holiday bonus earned).', 'themed_quirky', true, 'once', NULL, '{"genre":"Horror","holiday":"halloween"}'),
  ('holiday_warmth', 'Holiday Warmth', 'Complete a cozy/winter-holiday-adjacent watch during the winter holiday window (holiday bonus earned).', 'themed_quirky', true, 'once', NULL, '{"genres":["Comedy","Drama"],"holiday":"winter"}'),
  -- §5.5.10 Themed / quirky (hidden)
  ('spooky_season', 'Spooky Season', 'Complete 10 horror watches during the Halloween window (cumulative across years — finalized 2026-09-08).', 'themed_quirky', false, 'counter', 10, '{"genre":"Horror","holiday":"halloween","cumulative":true}'),
  ('franchise_head', 'Franchise Head', 'Complete the first movie of 5 different franchises (first installment of 5 franchises).', 'themed_quirky', false, 'counter', 5, '{"scope":"franchise","position":"first"}'),
  ('directors_passport', 'Director''s Passport', 'Complete movies by 5 different directors (where director metadata is available).', 'themed_quirky', false, 'counter', 5, '{"scope":"director"}'),
  ('actors_playground', 'Actor''s Playground', 'Complete two movies/episodes sharing a lead actor (where lead-actor metadata is available).', 'themed_quirky', false, 'once', NULL, '{"scope":"lead_actor","metadata_dependent":true}'),
  ('the_oldie', 'The Oldie', 'Complete a movie/episode whose release/air date is more than 20 years before the current date.', 'themed_quirky', false, 'once', NULL, '{"older_than_years":20}'),
  ('the_newcomer', 'The Newcomer', 'Complete a movie/episode whose release/air date is within the last 30 days.', 'themed_quirky', false, 'once', NULL, '{"released_within_days":30}'),
  ('tuesday_thriller', 'Tuesday Thriller', 'Complete a thriller/mystery watch on a Tuesday (day-of-week + genre quirk).', 'themed_quirky', false, 'once', NULL, '{"day_of_week":2,"genres":["Thriller","Mystery"]}'),
  ('friday_night_film', 'Friday Night Film', 'Complete a movie on a Friday night (evening local time).', 'themed_quirky', false, 'once', NULL, '{"day_of_week":5,"daypart":"evening","content_type":"movie"}'),
  ('dawn_watcher', 'Dawn Watcher', 'Complete a watch that started before 6:00 local time.', 'themed_quirky', false, 'once', NULL, '{"hour_before":6}'),
  ('night_owl', 'Night Owl', 'Complete a watch that started after 22:00 local time.', 'themed_quirky', false, 'once', NULL, '{"hour_after":22}'),
  ('rainy_day', 'Rainy Day', 'Complete a watch on a date matching a library holiday/seasonal theme not yet bonused (host weather/date permitting).', 'themed_quirky', false, 'once', NULL, '{"metadata_dependent":true}'),
  -- §5.5.11 Cross-category combo (visible)
  ('well_rounded', 'Well-Rounded', 'Complete at least one movie and at least one episode, and earn at least one new-arrival bonus, all in the same real day.', 'combo', true, 'combo', NULL, '{"window":"same_day"}'),
  ('streak_collector', 'Streak Collector', 'Maintain a 7-day streak while also completing a full series during that streak.', 'combo', true, 'combo', NULL, '{"streak":7,"scope":"series"}'),
  ('genre_plus_streak', 'Genre + Streak', 'Maintain a 7-day streak where each day included a watch in a different genre.', 'combo', true, 'combo', NULL, '{"streak":7}'),
  ('featured_plus_new', 'Featured + New', 'Complete a featured case that was also a new arrival (within its new-arrival window).', 'combo', true, 'combo', NULL, '{"featured":true,"new_arrival":true}'),
  ('horror_plus_holiday', 'Horror + Holiday', 'Complete the Halloween holiday bonus and also complete 5 horror watches in the same window.', 'combo', true, 'combo', NULL, '{"genre":"Horror","holiday":"halloween","count":5}'),
  ('first_100_plus_streak', 'First 100 + Streak', 'Reach 100 total completed episodes while maintaining a 7-day streak.', 'combo', true, 'combo', NULL, '{"episodes":100,"streak":7}'),
  -- §5.5.12 Cross-category combo (hidden)
  ('perfect_day', 'Perfect Day', 'In a single real day: at least one movie, at least one episode, at least one new-arrival bonus, at least one featured case, and at least one holiday-window bonus — all in the same day.', 'combo', false, 'combo', NULL, '{"window":"same_day"}'),
  ('silent_completer', 'Silent Completer', 'Reach 100 total episodes completed all via poll, without ever missing a day in a 30-day streak that overlaps the 100th episode.', 'combo', false, 'combo', NULL, '{"episodes":100,"streak":30}'),
  ('genre_omnivore', 'Genre Omnivore', 'Unlock and complete at least one watch in every sub-genre the library exposes, plus complete 3 full series, plus maintain a 14-day streak.', 'combo', false, 'combo', NULL, '{"scope":"library","series":3,"streak":14}'),
  ('holiday_sweep', 'Holiday Sweep', 'Earn holiday bonuses in all holiday windows for the current year, and complete a series in each of 3 different genres during those windows.', 'combo', false, 'combo', NULL, '{"scope":"calendar_year","series_genres":3}');
