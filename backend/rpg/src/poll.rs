//! The poll loop (spec §9.1, poll-loop semantics finalized 2026-09-08): the
//! one owner of unattended play. Each cycle runs, in order:
//!
//! 1. **content sync** — `SyncPipeline::run`: mirror Plex/Sonarr/Radarr plus
//!    provider enrichment into Postgres (sync owns the catalog),
//! 2. **game tick** — `run_game_tick` with the stack clients: watch-award
//!    detection (phase 1), order reveals (phase 2), achievement evaluation
//!    (phase 3).
//!
//! Cadence is a fixed 5-minute interval from cycle start (jitter/back-off
//! deliberately not implemented in V1 — the stack is small and rate limits
//! are not a factor). The first tick of a `tokio::time::interval` fires
//! immediately, so a fresh process syncs right away (the §9.3 "catch up
//! then settle" shape). Any sync or phase-1 failure is logged and
//! non-fatal — the loop never exits due to poll failures. Ctrl-C signals
//! the loop to finish its current cycle and exit before the process
//! returns.

use crate::config::ProbeConfig;
use crate::enrichment::MetadataCache;
use crate::game::{run_game_tick, GameTickReport};
use crate::persistence::PostgresContentStore;
use crate::pipeline::{SyncPipeline, SyncRunResult};
use crate::providers::{FanartClient, OmdbClient, TmdbClient, TvdbClient};
use crate::stack::{PlexClient, RadarrClient, SonarrClient};
use crate::Result;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::sync::watch;

/// Spec §9.1: 5 minutes between full poll cycles (the seeded
/// `poll_interval_seconds` setting default; F-25).
pub const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Spec §6.4.10 settings default `provider_cache_ttl_seconds` = 86400.
const PROVIDER_CACHE_TTL_SECONDS: u64 = 86_400;

/// Store handle shared between the HTTP server and the poll loop. The pool
/// inside makes clones cheap and contention-free (F-19).
pub type StoreHandle = std::sync::Arc<PostgresContentStore>;

/// The stack clients one poll cycle talks to. Constructed once at startup;
/// `with_base_url` constructors exist for tests and mocks.
pub struct PollStack {
    pub plex: PlexClient,
    pub sonarr: SonarrClient,
    pub radarr: RadarrClient,
    pub tmdb: TmdbClient,
    pub tvdb: TvdbClient,
    pub omdb: OmdbClient,
    pub fanart: FanartClient,
}

impl PollStack {
    /// Builds the real stack clients from the probed environment.
    pub fn from_config(config: &ProbeConfig) -> Self {
        Self {
            plex: PlexClient::new(&config.plex_url, &config.plex_token),
            sonarr: SonarrClient::new(&config.sonarr_url, &config.sonarr_api_key),
            radarr: RadarrClient::new(&config.radarr_url, &config.radarr_api_key),
            tmdb: TmdbClient::new(&config.tmdb_api_key),
            tvdb: TvdbClient::new(&config.tvdb_api_key),
            omdb: OmdbClient::new(&config.omdb_api_key),
            fanart: FanartClient::new(&config.fanart_api_key),
        }
    }
}

/// What one poll cycle did. Either half may fail independently: a sync
/// failure does not stop the tick (phases 2–3 advance on known state,
/// §9.1); a tick failure (store-level) is retried next interval.
#[derive(Debug)]
pub struct PollCycleSummary {
    pub sync: Result<SyncRunResult>,
    pub tick: Result<GameTickReport>,
}

/// Runs one full poll cycle: content sync first (the catalog phase 1
/// matches against), then the game tick with the stack clients. A sync
/// failure is returned in the summary (logged by the caller) and must not
/// stop the tick — phases 2–3 advance on known state (§9.1).
pub async fn run_poll_cycle(
    store: &PostgresContentStore,
    stack: &PollStack,
) -> PollCycleSummary {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("host clock is after the epoch")
        .as_secs();

    let sync = {
        // V1: a fresh cache per cycle. Cache rows persist in Postgres via
        // the pipeline; rebuilding the in-memory layer keeps the loop
        // stateless across cycles.
        let mut cache = MetadataCache::default();
        SyncPipeline::new(
            &stack.plex,
            &stack.sonarr,
            &stack.radarr,
            &stack.tmdb,
            &stack.tvdb,
            &stack.omdb,
            &stack.fanart,
            &mut cache,
            PROVIDER_CACHE_TTL_SECONDS,
            // V1 seam: the TMDb keyword → parent-genre mapping table (§4.6)
            // has no storage yet; an empty map degrades to no sub-genres.
            BTreeMap::new(),
        )
        .run(store, now)
        .await
    };

    let tick = run_game_tick(store, Some(&stack.plex)).await;

    PollCycleSummary { sync, tick }
}

/// The one-line-per-cycle log (§9.1 observability): auditable from logs
/// alone — sync rows plus every phase count. A failed sync is reported
/// inline; the tick still ran.
pub fn format_cycle_log(summary: &PollCycleSummary) -> String {
    let sync_part = match &summary.sync {
        Ok(sync) => format!(
            "sync +{} content/{} cache rows ({} stack, {} provider failures)",
            sync.persistence.content_rows,
            sync.persistence.provider_cache_rows,
            sync.prepared.outcome.stack_failures.len(),
            sync.prepared.outcome.provider_failures.len(),
        ),
        Err(error) => format!("sync failed: {error}"),
    };
    match &summary.tick {
        Ok(tick) => format!(
            "poll: {sync_part}; tick: {} watches awarded, {} orders created, {} skips granted, {} achievements unlocked",
            tick.watches_awarded,
            tick.orders.orders_created.len(),
            tick.orders.skips_granted,
            tick.achievements.as_ref().map_or(0, |a| a.unlocked.len()),
        ),
        Err(error) => format!("poll: {sync_part}; tick failed: {error}"),
    }
}

/// Runs the poll loop until `shutdown` flips to `true`, then returns the
/// number of completed cycles. The shutdown receiver is checked between
/// cycles only — a cycle in flight finishes (no mid-write abandonment).
/// Cycle failures are logged and non-fatal; the loop itself never exits on
/// them. F-24: every cycle counts (run vs failed) so the exit line is a
/// real health signal instead of a success-only count.
pub async fn run_poll_loop(
    store: StoreHandle,
    stack: PollStack,
    interval: Duration,
    mut shutdown: watch::Receiver<bool>,
) -> usize {
    let mut ticker = tokio::time::interval(interval);
    // A long cycle (slow providers) must not bank missed ticks and replay
    // them in a burst — skip to the next scheduled slot.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut cycles_run = 0_usize;
    let mut cycles_failed = 0_usize;
    loop {
        if *shutdown.borrow() {
            break;
        }
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => continue,
        }
        if *shutdown.borrow() {
            break;
        }
        let summary = run_poll_cycle(&store, &stack).await;
        // Every cycle gets one structured line, including a failed tick. The
        // next interval retries a failed cycle; the loop itself stays alive.
        println!("{}", format_cycle_log(&summary));
        cycles_run += 1;
        if summary.sync.is_err() || summary.tick.is_err() {
            cycles_failed += 1;
        }
    }
    if cycles_failed > 0 {
        println!(
            "poll loop stopped after {cycles_run} cycle(s), {cycles_failed} with failures"
        );
    }
    cycles_run
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_interval_matches_the_spec() {
        assert_eq!(POLL_INTERVAL, Duration::from_secs(300));
    }

    #[test]
    fn cycle_log_line_carries_every_phase_count() {
        let summary = PollCycleSummary {
            sync: Ok(SyncRunResult {
                prepared: crate::pipeline::PreparedSync {
                    outcome: crate::sync::EnrichedSyncOutcome::default(),
                    plan: crate::persistence::ContentPersistencePlan {
                        content: Vec::new(),
                        provider_cache: Vec::new(),
                        stack_failures: Vec::new(),
                        provider_failures: Vec::new(),
                        warnings: Vec::new(),
                    },
                },
                persistence: crate::persistence::PersistenceSummary {
                    content_rows: 4,
                    provider_cache_rows: 9,
                },
            }),
            tick: Ok(GameTickReport {
                watches_awarded: 2,
                orders: crate::persistence::OrderRefreshSummary {
                    orders_created: Vec::new(),
                    skips_granted: 1,
                    skipped_genres: Vec::new(),
                },
                achievements: Some(crate::persistence::EvaluationSummary {
                    evaluated: 101,
                    unlocked: vec!["first_watch".to_owned(), "second_watch".to_owned()],
                    not_evaluable: 7,
                }),
            }),
        };
        let line = format_cycle_log(&summary);
        assert!(line.contains("+4 content/9 cache rows"), "{line}");
        assert!(line.contains("2 watches awarded"), "{line}");
        assert!(line.contains("0 orders created"), "{line}");
        assert!(line.contains("1 skips granted"), "{line}");
        assert!(line.contains("2 achievements unlocked"), "{line}");
    }

    #[test]
    fn cycle_log_reports_sync_failure_inline() {
        let summary = PollCycleSummary {
            sync: Err(crate::ProbeError::HttpStatus {
                provider: "plex",
                status: 503,
            }),
            tick: Ok(GameTickReport {
                watches_awarded: 0,
                orders: crate::persistence::OrderRefreshSummary {
                    orders_created: Vec::new(),
                    skips_granted: 0,
                    skipped_genres: Vec::new(),
                },
                achievements: None,
            }),
        };
        let line = format_cycle_log(&summary);
        assert!(line.contains("sync failed"), "{line}");
        assert!(line.contains("tick: 0 watches awarded"), "{line}");
    }
}
