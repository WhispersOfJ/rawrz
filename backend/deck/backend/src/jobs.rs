//! Job engine (spec §5.2) — M0 in-memory registry.
//!
//! Every mutating action becomes a job: queued, progress-reported over WS topic
//! `jobs`, audit-logged. Jobs are the only path to mutation; handlers return
//! `202 {jobId}`. Persistence into SQLite lands in M1.

use crate::error::ApiError;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub state: JobState,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

static REGISTRY: Mutex<Vec<Job>> = Mutex::new(Vec::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Spawn a job. M0: registers it queued; real executors arrive per milestone.
pub fn spawn(kind: String, target: String) -> Result<Job, ApiError> {
    let job = Job {
        id: format!("job-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed)),
        kind,
        target,
        state: JobState::Queued,
    };
    REGISTRY.lock().expect("job registry").push(job.clone());
    tracing::info!(id = %job.id, kind = %job.kind, target = %job.target, "job spawned");
    Ok(job)
}

pub async fn list() -> Vec<Job> {
    REGISTRY.lock().expect("job registry").clone()
}

/// Transition an existing job's state (used by M2's env-apply executor and
/// the job engine milestone). No-op when the id is unknown.
pub fn transition(id: &str, state: JobState) {
    let mut registry = REGISTRY.lock().expect("job registry");
    if let Some(job) = registry.iter_mut().find(|job| job.id == id) {
        job.state = state;
        tracing::info!(id = %job.id, kind = %job.kind, ?state, "job state transition");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_registers_and_lists() {
        let j = spawn("container.restart".into(), "sonarr".into()).expect("spawn");
        assert!(j.id.starts_with("job-"));
        assert_eq!(j.state, JobState::Queued);
        let jobs = list().await;
        let job = jobs.iter().find(|job| job.id == j.id).unwrap();
        assert_eq!(job.kind, "container.restart");
        assert_eq!(job.target, "sonarr");
    }

    #[tokio::test]
    async fn transition_updates_state() {
        let j = spawn("env.apply".into(), "env".into()).expect("spawn");
        assert_eq!(j.state, JobState::Queued);
        transition(&j.id, JobState::Done);
        let jobs = list().await;
        let job = jobs.iter().find(|job| job.id == j.id).unwrap();
        assert_eq!(job.state, JobState::Done);
    }
}
