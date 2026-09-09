pub const INITIAL_CONTENT_PROVIDER_CACHE_VERSION: &str = "0001_content_provider_cache";
pub const INITIAL_CONTENT_PROVIDER_CACHE: &str =
    include_str!("../migrations/0001_content_provider_cache.sql");
pub const SYNC_STATE_VERSION: &str = "0002_sync_state";
pub const SYNC_STATE_MIGRATION: &str = include_str!("../migrations/0002_sync_state.sql");
pub const ACCOUNTS_CHARACTERS_VERSION: &str = "0003_accounts_characters";
pub const ACCOUNTS_CHARACTERS_MIGRATION: &str =
    include_str!("../migrations/0003_accounts_characters.sql");
pub const GENRES_VERSION: &str = "0004_genres";
pub const GENRES_MIGRATION: &str = include_str!("../migrations/0004_genres.sql");
pub const CHARACTER_STATE_VERSION: &str = "0005_character_state";
pub const CHARACTER_STATE_MIGRATION: &str =
    include_str!("../migrations/0005_character_state.sql");
pub const SETTINGS_VERSION: &str = "0006_settings";
pub const SETTINGS_MIGRATION: &str = include_str!("../migrations/0006_settings.sql");
pub const WATCHES_VERSION: &str = "0007_watches";
pub const WATCHES_MIGRATION: &str = include_str!("../migrations/0007_watches.sql");
pub const CASES_VERSION: &str = "0008_cases";
pub const CASES_MIGRATION: &str = include_str!("../migrations/0008_cases.sql");
pub const FEATURED_CASES_VERSION: &str = "0009_featured_cases";
pub const FEATURED_CASES_MIGRATION: &str =
    include_str!("../migrations/0009_featured_cases.sql");
pub const ACHIEVEMENTS_VERSION: &str = "0010_achievements";
pub const ACHIEVEMENTS_MIGRATION: &str =
    include_str!("../migrations/0010_achievements.sql");

pub const MIGRATIONS: &[(&str, &str)] = &[
    (
        INITIAL_CONTENT_PROVIDER_CACHE_VERSION,
        INITIAL_CONTENT_PROVIDER_CACHE,
    ),
    (SYNC_STATE_VERSION, SYNC_STATE_MIGRATION),
    (
        ACCOUNTS_CHARACTERS_VERSION,
        ACCOUNTS_CHARACTERS_MIGRATION,
    ),
    (GENRES_VERSION, GENRES_MIGRATION),
    (CHARACTER_STATE_VERSION, CHARACTER_STATE_MIGRATION),
    (SETTINGS_VERSION, SETTINGS_MIGRATION),
    (WATCHES_VERSION, WATCHES_MIGRATION),
    (CASES_VERSION, CASES_MIGRATION),
    (FEATURED_CASES_VERSION, FEATURED_CASES_MIGRATION),
    (ACHIEVEMENTS_VERSION, ACHIEVEMENTS_MIGRATION),
];

#[cfg(test)]
mod tests {
    use super::{
        ACCOUNTS_CHARACTERS_MIGRATION, ACHIEVEMENTS_MIGRATION, CASES_MIGRATION,
        CHARACTER_STATE_MIGRATION, FEATURED_CASES_MIGRATION, GENRES_MIGRATION,
        INITIAL_CONTENT_PROVIDER_CACHE as SQL, MIGRATIONS, SETTINGS_MIGRATION,
        SYNC_STATE_MIGRATION, SYNC_STATE_VERSION, WATCHES_MIGRATION,
    };

    fn statement_containing<'a>(sql: &'a str, fragment: &str) -> &'a str {
        sql.split(';')
            .find(|statement| statement.contains(fragment))
            .unwrap_or_else(|| panic!("migration is missing statement containing {fragment:?}"))
    }

    fn contains_sql(statement: &str, fragment: &str) -> bool {
        let normalized = statement.split_whitespace().collect::<Vec<_>>().join(" ");
        let fragment = fragment.split_whitespace().collect::<Vec<_>>().join(" ");
        normalized.contains(&fragment)
    }

    #[test]
    fn exposes_a_stable_initial_migration_version() {
        assert_eq!(
            super::INITIAL_CONTENT_PROVIDER_CACHE_VERSION,
            "0001_content_provider_cache"
        );
        assert!(!SQL.trim().is_empty());
    }

    #[test]
    fn exposes_ordered_migration_catalog_with_sync_state_upgrade() {
        assert_eq!(MIGRATIONS.len(), 10);
        assert_eq!(MIGRATIONS[0].0, "0001_content_provider_cache");
        assert_eq!(MIGRATIONS[1].0, SYNC_STATE_VERSION);
        assert_eq!(MIGRATIONS[2].0, super::ACCOUNTS_CHARACTERS_VERSION);
        assert_eq!(MIGRATIONS[3].0, super::GENRES_VERSION);
        assert_eq!(MIGRATIONS[4].0, super::CHARACTER_STATE_VERSION);
        assert_eq!(MIGRATIONS[5].0, super::SETTINGS_VERSION);
        assert_eq!(MIGRATIONS[6].0, super::WATCHES_VERSION);
        assert_eq!(MIGRATIONS[7].0, super::CASES_VERSION);
        assert_eq!(MIGRATIONS[8].0, super::FEATURED_CASES_VERSION);
        assert_eq!(MIGRATIONS[9].0, super::ACHIEVEMENTS_VERSION);
        assert!(MIGRATIONS[0].0 < MIGRATIONS[1].0 && MIGRATIONS[1].0 < MIGRATIONS[2].0);
        assert!(MIGRATIONS[2].0 < MIGRATIONS[3].0 && MIGRATIONS[3].0 < MIGRATIONS[4].0);
        assert!(MIGRATIONS[4].0 < MIGRATIONS[5].0 && MIGRATIONS[5].0 < MIGRATIONS[6].0);
        assert!(MIGRATIONS[6].0 < MIGRATIONS[7].0 && MIGRATIONS[7].0 < MIGRATIONS[8].0);
        assert!(MIGRATIONS[8].0 < MIGRATIONS[9].0);
        assert!(SYNC_STATE_MIGRATION.contains("CREATE TABLE sync_state ("));
        assert!(SYNC_STATE_MIGRATION.contains("PRIMARY KEY (character_id, source)"));
    }

    #[test]
    fn creates_identity_tables_and_completes_the_deferred_sync_state_key() {
        let account_position = ACCOUNTS_CHARACTERS_MIGRATION
            .find("CREATE TABLE accounts (")
            .expect("migration is missing the accounts table");
        let character_position = ACCOUNTS_CHARACTERS_MIGRATION
            .find("CREATE TABLE characters (")
            .expect("migration is missing the characters table");
        let key_position = ACCOUNTS_CHARACTERS_MIGRATION
            .find("ALTER TABLE sync_state")
            .expect("migration is missing the sync_state foreign key");
        assert!(account_position < character_position && character_position < key_position);

        for field in [
            "pin_hash text NOT NULL",
            "pin_salts text NOT NULL",
            "created_at timestamptz NOT NULL DEFAULT now()",
            "updated_at timestamptz NOT NULL DEFAULT now()",
        ] {
            assert!(
                contains_sql(
                    statement_containing(ACCOUNTS_CHARACTERS_MIGRATION, "CREATE TABLE accounts ("),
                    field
                ),
                "accounts table is missing {field:?}"
            );
        }
        for field in [
            "account_id bigint NOT NULL REFERENCES accounts(id) ON DELETE CASCADE",
            "name text NOT NULL DEFAULT 'The Investigator'",
            "created_at timestamptz NOT NULL DEFAULT now()",
        ] {
            assert!(
                contains_sql(
                    statement_containing(
                        ACCOUNTS_CHARACTERS_MIGRATION,
                        "CREATE TABLE characters ("
                    ),
                    field
                ),
                "characters table is missing {field:?}"
            );
        }
        assert!(ACCOUNTS_CHARACTERS_MIGRATION
            .contains("FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE"));
    }

    #[test]
    fn seeds_the_fixed_genre_list_and_creates_genre_tables() {
        let genres_position = GENRES_MIGRATION
            .find("CREATE TABLE genres (")
            .expect("migration is missing the genres table");
        let sub_genres_position = GENRES_MIGRATION
            .find("CREATE TABLE sub_genres (")
            .expect("migration is missing the sub_genres table");
        let access_position = GENRES_MIGRATION
            .find("CREATE TABLE genre_access (")
            .expect("migration is missing the genre_access table");
        let xp_position = GENRES_MIGRATION
            .find("CREATE TABLE sub_genre_xp (")
            .expect("migration is missing the sub_genre_xp table");
        assert!(
            genres_position < sub_genres_position
                && sub_genres_position < access_position
                && access_position < xp_position
        );

        for field in [
            "name text NOT NULL UNIQUE",
            "list_order int NOT NULL",
            "is_opening boolean NOT NULL DEFAULT false",
        ] {
            assert!(
                contains_sql(statement_containing(GENRES_MIGRATION, "CREATE TABLE genres ("), field),
                "genres table is missing {field:?}"
            );
        }
        for field in [
            "genre_id bigint NOT NULL REFERENCES genres(id) ON DELETE CASCADE",
            "UNIQUE (genre_id, name)",
        ] {
            assert!(
                contains_sql(
                    statement_containing(GENRES_MIGRATION, "CREATE TABLE sub_genres ("),
                    field
                ),
                "sub_genres table is missing {field:?}"
            );
        }
        for field in [
            "PRIMARY KEY (character_id, genre_id)",
            "accessed_at timestamptz NOT NULL DEFAULT now()",
        ] {
            assert!(
                contains_sql(
                    statement_containing(GENRES_MIGRATION, "CREATE TABLE genre_access ("),
                    field
                ),
                "genre_access table is missing {field:?}"
            );
        }
        for field in [
            "PRIMARY KEY (character_id, sub_genre_id)",
            "xp bigint NOT NULL DEFAULT 0",
            "purchased boolean NOT NULL DEFAULT false",
            "purchased_at timestamptz",
        ] {
            assert!(
                contains_sql(
                    statement_containing(GENRES_MIGRATION, "CREATE TABLE sub_genre_xp ("),
                    field
                ),
                "sub_genre_xp table is missing {field:?}"
            );
        }

        // The finalized §5.2 genre list: horror first (the opening genre),
        // then the fixed cascade, 10 genres matching the 10-level table.
        let expected = [
            ("Horror", 1, true),
            ("Thriller", 2, false),
            ("Mystery", 3, false),
            ("Sci-Fi", 4, false),
            ("Fantasy", 5, false),
            ("Documentary", 6, false),
            ("Comedy", 7, false),
            ("Drama", 8, false),
            ("Romance", 9, false),
            ("Animation", 10, false),
        ];
        let seed = GENRES_MIGRATION
            .split("INSERT INTO genres")
            .nth(1)
            .and_then(|rest| rest.split(';').next())
            .expect("migration is missing the genre seed");
        for (name, order, opening) in expected {
            assert!(
                contains_sql(seed, &format!("'{name}', {order}, {opening}")),
                "genre seed is missing ({name}, {order}, {opening})"
            );
        }
    }

    #[test]
    fn creates_singleton_character_state_with_progression_and_streak_columns() {
        let statement =
            statement_containing(CHARACTER_STATE_MIGRATION, "CREATE TABLE character_state (");
        for field in [
            "character_id bigint PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE",
            "xp bigint NOT NULL DEFAULT 0",
            "level int NOT NULL DEFAULT 1",
            "total_watches int NOT NULL DEFAULT 0",
            "episode_watches int NOT NULL DEFAULT 0",
            "movie_watches int NOT NULL DEFAULT 0",
            "current_streak_days int NOT NULL DEFAULT 0",
            "best_streak_days int NOT NULL DEFAULT 0",
            "streak_last_watch_date date",
            "genres_accessed int NOT NULL DEFAULT 1",
            "last_level_up_at timestamptz",
        ] {
            assert!(
                contains_sql(statement, field),
                "character_state table is missing {field:?}"
            );
        }
    }

    #[test]
    fn creates_settings_key_value_table_for_character_scoped_config() {
        let statement =
            statement_containing(SETTINGS_MIGRATION, "CREATE TABLE settings (");
        for field in [
            "character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE",
            "key text NOT NULL",
            "value text NOT NULL",
            "PRIMARY KEY (character_id, key)",
        ] {
            assert!(
                contains_sql(statement, field),
                "settings table is missing {field:?}"
            );
        }
    }

    #[test]
    fn creates_watches_ledger_with_audit_flags_and_deferred_featured_fk() {
        let watches_position = WATCHES_MIGRATION
            .find("CREATE TABLE watches (")
            .expect("migration is missing the watches table");
        let ledger_position = WATCHES_MIGRATION
            .find("CREATE TABLE genre_xp_ledger (")
            .expect("migration is missing the genre_xp_ledger table");
        assert!(watches_position < ledger_position);

        let statement = statement_containing(WATCHES_MIGRATION, "CREATE TABLE watches (");
        for field in [
            "character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE",
            "content_id bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE",
            "content_type text NOT NULL",
            "completed_at timestamptz NOT NULL DEFAULT now()",
            "pct_viewed numeric NOT NULL",
            "xp_awarded bigint NOT NULL",
            "normal_xp bigint NOT NULL",
            "bonuses jsonb NOT NULL DEFAULT '[]'",
            "new_arrival boolean NOT NULL DEFAULT false",
            "featured boolean NOT NULL DEFAULT false",
            "featured_case_id bigint",
            "season_bonus boolean NOT NULL DEFAULT false",
            "series_bonus boolean NOT NULL DEFAULT false",
            "first_completion boolean NOT NULL DEFAULT false",
            "holiday_bonus jsonb",
            "via_plex boolean NOT NULL DEFAULT true",
            "via_manual boolean NOT NULL DEFAULT false",
        ] {
            assert!(
                contains_sql(statement, field),
                "watches table is missing {field:?}"
            );
        }
        // The featured_cases FK is deferred to 0009 — the column must be a
        // plain bigint here.
        assert!(
            !statement.contains("REFERENCES featured_cases"),
            "watches must not reference featured_cases before it exists"
        );
        for index in [
            "CREATE INDEX watches_character ON watches(character_id)",
            "CREATE INDEX watches_completed_at ON watches(character_id, completed_at DESC)",
            "CREATE INDEX watches_content ON watches(content_id)",
        ] {
            assert!(WATCHES_MIGRATION.contains(index), "watches migration is missing {index:?}");
        }
    }

    #[test]
    fn lands_the_deferred_genre_xp_ledger_with_watch_link() {
        let statement =
            statement_containing(WATCHES_MIGRATION, "CREATE TABLE genre_xp_ledger (");
        for field in [
            "character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE",
            "sub_genre_id bigint NOT NULL REFERENCES sub_genres(id) ON DELETE CASCADE",
            "xp_added bigint NOT NULL",
            "source_watch_id bigint REFERENCES watches(id)",
            "at timestamptz NOT NULL DEFAULT now()",
        ] {
            assert!(
                contains_sql(statement, field),
                "genre_xp_ledger table is missing {field:?}"
            );
        }
    }

    #[test]
    fn creates_case_board_with_deferred_featured_fk() {
        let statement = statement_containing(CASES_MIGRATION, "CREATE TABLE cases (");
        for field in [
            "character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE",
            "content_id bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE",
            "case_type text NOT NULL",
            "status text NOT NULL DEFAULT 'available'",
            "taken_at timestamptz",
            "completed_at timestamptz",
            "completion_watch_id bigint REFERENCES watches(id)",
            "bonus_flags jsonb NOT NULL DEFAULT '[]'",
            "featured_case_id bigint",
            "created_at timestamptz NOT NULL DEFAULT now()",
        ] {
            assert!(
                contains_sql(statement, field),
                "cases table is missing {field:?}"
            );
        }
        // The featured_cases FK is deferred to 0009 — the column must be a
        // plain bigint here.
        assert!(
            !statement.contains("REFERENCES featured_cases"),
            "cases must not reference featured_cases before it exists"
        );
        for index in [
            "CREATE INDEX cases_character ON cases(character_id)",
            "CREATE INDEX cases_status ON cases(character_id, status)",
        ] {
            assert!(CASES_MIGRATION.contains(index), "cases migration is missing {index:?}");
        }
    }

    #[test]
    fn creates_featured_cases_and_completes_both_deferred_foreign_keys() {
        let table_position = FEATURED_CASES_MIGRATION
            .find("CREATE TABLE featured_cases (")
            .expect("migration is missing the featured_cases table");
        let watches_fk_position = FEATURED_CASES_MIGRATION
            .find("ALTER TABLE watches")
            .expect("migration is missing the watches featured-case foreign key");
        let cases_fk_position = FEATURED_CASES_MIGRATION
            .find("ALTER TABLE cases")
            .expect("migration is missing the cases featured-case foreign key");
        assert!(
            table_position < watches_fk_position && table_position < cases_fk_position,
            "the deferred foreign keys must be added after the table exists"
        );

        for field in [
            "character_id bigint NOT NULL REFERENCES characters(id) ON DELETE CASCADE",
            "period text NOT NULL",
            "content_id bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE",
            "selection_mode text NOT NULL",
            "bonus_xp bigint NOT NULL DEFAULT 10",
            "created_at timestamptz NOT NULL DEFAULT now()",
            "UNIQUE (character_id, period)",
        ] {
            assert!(
                contains_sql(
                    statement_containing(FEATURED_CASES_MIGRATION, "CREATE TABLE featured_cases ("),
                    field
                ),
                "featured_cases table is missing {field:?}"
            );
        }
        assert!(FEATURED_CASES_MIGRATION.contains(
            "FOREIGN KEY (featured_case_id) REFERENCES featured_cases(id)"
        ));
        // Both deferred columns are completed: watches (0007) and cases (0008).
        assert_eq!(
            FEATURED_CASES_MIGRATION.matches("FOREIGN KEY (featured_case_id)").count(),
            2,
            "0009 must complete both deferred featured_case_id foreign keys"
        );
    }

    #[test]
    fn creates_achievement_tables_with_categories_and_progress_key() {
        let definitions_position = ACHIEVEMENTS_MIGRATION
            .find("CREATE TABLE achievements (")
            .expect("migration is missing the achievements table");
        let unlocks_position = ACHIEVEMENTS_MIGRATION
            .find("CREATE TABLE character_achievements (")
            .expect("migration is missing the character_achievements table");
        let seed_position = ACHIEVEMENTS_MIGRATION
            .find("INSERT INTO achievements")
            .expect("migration is missing the achievement seed");
        assert!(definitions_position < unlocks_position && unlocks_position < seed_position);

        for field in [
            "slug text NOT NULL UNIQUE",
            "category text NOT NULL",
            "visible boolean NOT NULL DEFAULT true",
            "kind text NOT NULL",
            "target_value bigint",
            "metadata jsonb NOT NULL DEFAULT '{}'",
        ] {
            assert!(
                contains_sql(statement_containing(ACHIEVEMENTS_MIGRATION, "CREATE TABLE achievements ("), field),
                "achievements table is missing {field:?}"
            );
        }
        for field in [
            "achievement_id bigint NOT NULL REFERENCES achievements(id) ON DELETE CASCADE",
            "PRIMARY KEY (character_id, achievement_id)",
            "progress bigint NOT NULL DEFAULT 0",
        ] {
            assert!(
                contains_sql(
                    statement_containing(ACHIEVEMENTS_MIGRATION, "CREATE TABLE character_achievements ("),
                    field
                ),
                "character_achievements table is missing {field:?}"
            );
        }
        for index in [
            "CREATE INDEX achievements_category ON achievements(category)",
            "CREATE INDEX achievements_visible ON achievements(visible)",
        ] {
            assert!(
                ACHIEVEMENTS_MIGRATION.contains(index),
                "achievements migration is missing {index:?}"
            );
        }
    }

    /// Parse a seed row's trailing metadata jsonb literal (last column,
    /// always shaped `'{...}'` in the seed).
    fn seed_row_metadata(row: &str) -> serde_json::Value {
        let start = row
            .rfind("'{")
            .expect("seed row is missing a metadata literal")
            + 1;
        let rest = &row[start..];
        let end = rest
            .find("}'")
            .expect("metadata literal is unterminated")
            + 1;
        serde_json::from_str(&rest[..end])
            .unwrap_or_else(|error| panic!("seed metadata must be valid json ({error}): {:?}", &rest[..end]))
    }

    #[test]
    fn seeds_the_full_achievement_first_cut_with_kinds_targets_and_metadata() {
        let seed = ACHIEVEMENTS_MIGRATION
            .split("VALUES")
            .nth(1)
            .and_then(|rest| rest.split(';').next())
            .expect("migration is missing the achievement seed");
        // Strip the section-comment lines between seed groups, then split rows.
        let body = seed
            .lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        let rows: Vec<&str> = body
            .split("),\n")
            .map(|row| {
                row.trim()
                    .trim_start_matches('(')
                    .trim_start_matches("\n  ")
            })
            .collect();
        assert_eq!(rows.len(), 101, "the §5.5 first-cut list has 101 achievements");

        // Every row names exactly one of the six spec categories.
        for category in [
            "'completion_milestone'",
            "'genre_coverage'",
            "'time_streak'",
            "'novelty_firsts'",
            "'themed_quirky'",
            "'combo'",
        ] {
            assert!(
                body.contains(category),
                "no seed rows for category {category}"
            );
        }

        // Slugs are unique across the seed.
        let mut slugs: Vec<&str> = rows
            .iter()
            .map(|row| row.split(',').next().unwrap_or("").trim().trim_matches('\''))
            .collect();
        assert!(!slugs.iter().any(|slug| slug.is_empty()), "seed row is missing a slug");
        let unique = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), unique, "seed slugs must be unique");

        // Counter/streak kinds carry targets, once/combo kinds do not, and
        // every row's metadata parses as json.
        for (slug, target) in [
            ("episode_100", 100),
            ("horror_homeground_25", 25),
            ("streak_legend", 30),
            ("iron_streak", 60),
        ] {
            let row = rows
                .iter()
                .find(|row| row.contains(&format!("'{slug}'")))
                .unwrap_or_else(|| panic!("seed is missing {slug}"));
            assert!(
                row.contains(&format!(", {target}, ")),
                "{slug} must carry target_value {target}"
            );
        }
        for slug in ["first_blood", "case_closed", "perfect_day"] {
            let row = rows
                .iter()
                .find(|row| row.contains(&format!("'{slug}'")))
                .unwrap_or_else(|| panic!("seed is missing {slug}"));
            assert!(row.contains(", NULL, "), "{slug} must not carry a target_value");
        }
        for slug in ["spooky_season", "rainy_day", "actors_playground"] {
            let row = rows
                .iter()
                .find(|row| row.contains(&format!("'{slug}'")))
                .unwrap_or_else(|| panic!("seed is missing {slug}"));
            let metadata = seed_row_metadata(row);
            assert!(
                metadata.get("metadata_dependent").is_some() || metadata.get("cumulative").is_some(),
                "{slug} must carry its finalized metadata flag"
            );
        }
    }

    #[test]
    fn creates_content_before_provider_cache_with_required_mirror_fields() {
        let content_position = SQL.find("CREATE TABLE content (").unwrap();
        let cache_position = SQL.find("CREATE TABLE content_provider_cache (").unwrap();
        assert!(content_position < cache_position);

        for field in [
            "source text NOT NULL",
            "source_id text NOT NULL",
            "external_id text",
            "content_type text NOT NULL",
            "metadata_blob jsonb NOT NULL DEFAULT '{}'::jsonb",
            "provider_metadata jsonb NOT NULL DEFAULT '{}'::jsonb",
            "last_synced_at timestamptz NOT NULL DEFAULT now()",
            "UNIQUE (source, source_id)",
        ] {
            assert!(
                contains_sql(statement_containing(SQL, "CREATE TABLE content ("), field),
                "content table is missing {field:?}"
            );
        }
    }

    #[test]
    fn creates_provider_cache_with_auditable_payload_and_identity_key() {
        let statement = statement_containing(SQL, "CREATE TABLE content_provider_cache (");
        for field in [
            "content_id bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE",
            "provider text NOT NULL",
            "provider_id text NOT NULL",
            "payload jsonb NOT NULL",
            "fetched_at timestamptz NOT NULL DEFAULT now()",
            "expires_at timestamptz",
            "http_status int",
            "error text",
            "UNIQUE (content_id, provider, provider_id)",
        ] {
            assert!(
                contains_sql(statement, field),
                "provider cache table is missing {field:?}"
            );
        }
    }

    #[test]
    fn creates_lookup_and_json_indexes() {
        for index in [
            "CREATE INDEX content_external_id",
            "CREATE INDEX content_genres_gin",
            "CREATE INDEX content_sub_genres_gin",
            "CREATE INDEX content_provider_metadata_gin",
            "CREATE INDEX content_provider_cache_lookup",
        ] {
            assert!(SQL.contains(index), "migration is missing {index:?}");
        }
        assert!(SQL.contains("ON content USING GIN (genres jsonb_path_ops)"));
        assert!(SQL.contains("ON content USING GIN (sub_genres jsonb_path_ops)"));
        assert!(SQL.contains("ON content_provider_cache(provider, provider_id)"));
    }
}
