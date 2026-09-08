pub const INITIAL_CONTENT_PROVIDER_CACHE_VERSION: &str = "0001_content_provider_cache";
pub const INITIAL_CONTENT_PROVIDER_CACHE: &str =
    include_str!("../migrations/0001_content_provider_cache.sql");
pub const SYNC_STATE_VERSION: &str = "0002_sync_state";
pub const SYNC_STATE_MIGRATION: &str = include_str!("../migrations/0002_sync_state.sql");
pub const ACCOUNTS_CHARACTERS_VERSION: &str = "0003_accounts_characters";
pub const ACCOUNTS_CHARACTERS_MIGRATION: &str =
    include_str!("../migrations/0003_accounts_characters.sql");

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
];

#[cfg(test)]
mod tests {
    use super::{
        ACCOUNTS_CHARACTERS_MIGRATION, INITIAL_CONTENT_PROVIDER_CACHE as SQL, MIGRATIONS,
        SYNC_STATE_MIGRATION, SYNC_STATE_VERSION,
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
        assert_eq!(MIGRATIONS.len(), 3);
        assert_eq!(MIGRATIONS[0].0, "0001_content_provider_cache");
        assert_eq!(MIGRATIONS[1].0, SYNC_STATE_VERSION);
        assert_eq!(MIGRATIONS[2].0, super::ACCOUNTS_CHARACTERS_VERSION);
        assert!(MIGRATIONS[0].0 < MIGRATIONS[1].0 && MIGRATIONS[1].0 < MIGRATIONS[2].0);
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
