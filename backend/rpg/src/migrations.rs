pub const INITIAL_CONTENT_PROVIDER_CACHE: &str =
    include_str!("../migrations/0001_content_provider_cache.sql");

#[cfg(test)]
mod tests {
    use super::INITIAL_CONTENT_PROVIDER_CACHE as SQL;

    fn statement_containing(fragment: &str) -> &str {
        SQL.split(';')
            .find(|statement| statement.contains(fragment))
            .unwrap_or_else(|| panic!("migration is missing statement containing {fragment:?}"))
    }

    fn contains_sql(statement: &str, fragment: &str) -> bool {
        let normalized = statement.split_whitespace().collect::<Vec<_>>().join(" ");
        let fragment = fragment.split_whitespace().collect::<Vec<_>>().join(" ");
        normalized.contains(&fragment)
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
                contains_sql(statement_containing("CREATE TABLE content ("), field),
                "content table is missing {field:?}"
            );
        }
    }

    #[test]
    fn creates_provider_cache_with_auditable_payload_and_identity_key() {
        let statement = statement_containing("CREATE TABLE content_provider_cache (");
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
