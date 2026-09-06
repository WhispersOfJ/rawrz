//! Job engine (spec §5.2) — M0 in-memory registry.
//!
//! Every mutating action becomes a job: queued, progress-reported over WS topic
//! `jobs`, audit-logged. Jobs are the only path to mutation; handlers return
//! `202 {jobId}`. Persistence into SQLite lands in M1.

use crate::error::ApiError;
use serde::Serialize;
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

/// Spawn a job. M0: registers it queued; real executors arrive per milestone.
pub fn spawn(kind: String, target: String) -> Result<Job, ApiError> {
    let job = Job {
        id: format!("job-{}", REGISTRY.lock().expect("job registry").len() + 1),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_registers_and_lists() {
        let before = list().await.len();
        let j = spawn("container.restart".into(), "sonarr".into()).expect("spawn");
        assert!(j.id.starts_with("job-"));
        assert_eq!(j.state, JobState::Queued);
        let after = list().await;
        assert_eq!(after.len(), before + 1);
    }
}
