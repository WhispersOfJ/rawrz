-- Movie / TV RPG V1 content mirror and provider cache.
-- The application owns the stack mirror; this migration never writes to Plex or *arr services.

CREATE TABLE content (
  id                bigserial PRIMARY KEY,
  source            text NOT NULL,
  source_id         text NOT NULL,
  external_id       text,
  external_id_type  text,
  title             text NOT NULL,
  year              int,
  content_type      text NOT NULL,
  parent_id         bigint REFERENCES content(id) ON DELETE SET NULL,
  season_number     int,
  episode_number    int,
  runtime_seconds   int,
  release_date      date,
  first_air_date    date,
  status            text,
  summary           text,
  rating            numeric,
  rating_source     text,
  poster_url        text,
  fanart_url        text,
  section_key       text,
  section_title     text,
  genres            jsonb NOT NULL DEFAULT '[]'::jsonb,
  sub_genres        jsonb NOT NULL DEFAULT '[]'::jsonb,
  metadata_blob     jsonb NOT NULL DEFAULT '{}'::jsonb,
  provider_metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  last_synced_at    timestamptz NOT NULL DEFAULT now(),
  last_enriched_at  timestamptz,
  UNIQUE (source, source_id)
);

CREATE INDEX content_external_id
  ON content(external_id)
  WHERE external_id IS NOT NULL;

CREATE INDEX content_type_type ON content(content_type);

CREATE INDEX content_parent
  ON content(parent_id)
  WHERE parent_id IS NOT NULL;

CREATE INDEX content_genres_gin
  ON content USING GIN (genres jsonb_path_ops);

CREATE INDEX content_sub_genres_gin
  ON content USING GIN (sub_genres jsonb_path_ops);

CREATE INDEX content_section ON content(section_key);

CREATE INDEX content_provider_metadata_gin
  ON content USING GIN (provider_metadata jsonb_path_ops);

CREATE TABLE content_provider_cache (
  id           bigserial PRIMARY KEY,
  content_id   bigint NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  provider     text NOT NULL,
  provider_id  text NOT NULL,
  payload      jsonb NOT NULL,
  fetched_at   timestamptz NOT NULL DEFAULT now(),
  expires_at   timestamptz,
  http_status  int,
  error        text,
  UNIQUE (content_id, provider, provider_id)
);

CREATE INDEX content_provider_cache_lookup
  ON content_provider_cache(provider, provider_id);
