//! Job engine (spec §5.2) — every mutation is a job.
//!
//! M3: jobs now *execute*. `spawn_with_runner` registers a job, then runs the
//! caller-supplied future on the tokio runtime, transitioning
//! queued → running → done | failed | cancelled. Progress and step logs are
//! recorded between steps (steps are the only cancellation points), and every
//! transition is broadcast on the `jobs` WS topic (§5.3).
//!
//! Cancellation is cooperative: `cancel` flips a flag the runner checks between
//! steps via [`JobHandle::cancelled`]. A single in-flight HTTP/compose step
//! cannot be interrupted mid-flight — the flag takes effect at the next step
//! boundary, which keeps every step atomic.

use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepEntry {
    pub ts: i64,
    pub message: String,
}

/// A job record. Interior state is guarded by the registry mutex; runners
/// update through [`JobHandle`] which re-locks per call.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub state: JobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    pub steps: Vec<StepEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Job {
    fn new(id: String, kind: String, target: String, params: Option<Value>) -> Self {
        let now = now_millis();
        Self {
            id,
            kind,
            target,
            state: JobState::Queued,
            params,
            progress: None,
            steps: Vec::new(),
            result: None,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

static REGISTRY: Mutex<Vec<Job>> = Mutex::new(Vec::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static CANCEL_FLAGS: Mutex<Vec<(String, Arc<AtomicBool>)>> = Mutex::new(Vec::new());
static BROADCAST: std::sync::OnceLock<broadcast::Sender<Job>> = std::sync::OnceLock::new();

/// Register a job and hand back a handle for a runner to update it.
fn register(kind: String, target: String, params: Option<Value>) -> (Job, JobHandle) {
    let id = format!("job-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
    let job = Job::new(id.clone(), kind, target, params);
    REGISTRY.lock().expect("job registry").push(job.clone());
    let cancelled = Arc::new(AtomicBool::new(false));
    CANCEL_FLAGS
        .lock()
        .expect("cancel flags")
        .push((id.clone(), cancelled.clone()));
    tracing::info!(id = %job.id, kind = %job.kind, target = %job.target, "job registered");
    let handle = JobHandle { id, cancelled };
    (job, handle)
}

/// Legacy M0/M1 registration: a job that is spawned but has no M3 runner yet.
pub fn spawn(kind: String, target: String) -> Result<Job, crate::error::ApiError> {
    let (job, _handle) = register(kind, target, None);
    let _ = BROADCAST
        .get_or_init(|| broadcast::channel::<Job>(128).0)
        .send(job.clone());
    Ok(job)
}

/// Register a job and run `runner` to completion on the tokio runtime.
pub fn spawn_with_runner<F>(kind: String, target: String, params: Option<Value>, runner: F) -> Job
where
    F: FnOnce(
            JobHandle,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send>>
        + Send
        + 'static,
{
    let (job, handle) = register(kind, target, params);
    let job_id = job.id.clone();
    let _ = BROADCAST
        .get_or_init(|| broadcast::channel::<Job>(128).0)
        .send(job.clone());
    tokio::spawn(async move {
        set_state(&job_id, JobState::Running);
        let future = runner(handle.clone());
        match future.await {
            Ok(result) => finish(&job_id, JobState::Done, None, Some(result)),
            Err(error) => {
                let cancelled = handle.cancelled.load(Ordering::Relaxed);
                let state = if cancelled {
                    JobState::Cancelled
                } else {
                    JobState::Failed
                };
                finish(&job_id, state, Some(error), None);
            }
        }
        CANCEL_FLAGS
            .lock()
            .expect("cancel flags")
            .retain(|(id, _)| id != &job_id);
    });
    job
}

pub async fn list() -> Vec<Job> {
    REGISTRY.lock().expect("job registry").clone()
}

pub fn get(id: &str) -> Option<Job> {
    REGISTRY
        .lock()
        .expect("job registry")
        .iter()
        .find(|job| job.id == id)
        .cloned()
}

/// Cooperatively cancel a queued/running job. Returns false for unknown,
/// already-finished jobs.
pub fn cancel(id: &str) -> bool {
    let flags = CANCEL_FLAGS.lock().expect("cancel flags");
    if let Some((_, flag)) = flags.iter().find(|(job_id, _)| job_id == id) {
        flag.store(true, Ordering::Relaxed);
        drop(flags);
        set_state(id, JobState::Cancelled);
        return true;
    }
    false
}

pub fn set_state(id: &str, state: JobState) {
    let mut registry = REGISTRY.lock().expect("job registry");
    if let Some(job) = registry.iter_mut().find(|job| job.id == id) {
        if job.state == JobState::Done
            || job.state == JobState::Failed
            || job.state == JobState::Cancelled
        {
            return;
        }
        job.state = state.clone();
        job.updated_at = now_millis();
        tracing::info!(id = %job.id, ?state, "job state transition");
        let snapshot = job.clone();
        drop(registry);
        let _ = BROADCAST
            .get_or_init(|| broadcast::channel::<Job>(128).0)
            .send(snapshot);
    }
}

pub fn finish(id: &str, state: JobState, error: Option<String>, result: Option<Value>) {
    let mut registry = REGISTRY.lock().expect("job registry");
    if let Some(job) = registry.iter_mut().find(|job| job.id == id) {
        job.state = state;
        job.error = error;
        job.result = result;
        job.updated_at = now_millis();
        if job.progress.is_some() {
            job.progress = Some(100);
        }
        tracing::info!(id = %job.id, ?job.state, "job finished");
        let snapshot = job.clone();
        drop(registry);
        let _ = BROADCAST
            .get_or_init(|| broadcast::channel::<Job>(128).0)
            .send(snapshot);
    }
}

/// Runner-side handle: logs steps, reports progress, checks cancellation.
#[derive(Clone)]
pub struct JobHandle {
    id: String,
    cancelled: Arc<AtomicBool>,
}

impl JobHandle {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn step(&self, message: impl Into<String>) {
        let msg: String = message.into();
        let entry = StepEntry {
            ts: now_millis(),
            message: msg.clone(),
        };
        if let Some(job) = REGISTRY
            .lock()
            .expect("job registry")
            .iter_mut()
            .find(|job| job.id == self.id)
        {
            job.steps.push(entry);
            if job.steps.len() > 200 {
                let overflow = job.steps.len() - 200;
                job.steps.drain(0..overflow);
            }
            job.updated_at = now_millis();
        }
        tracing::info!(id = %self.id, step = %msg, "job step");
    }

    pub fn progress(&self, percent: u8) {
        if let Some(job) = REGISTRY
            .lock()
            .expect("job registry")
            .iter_mut()
            .find(|job| job.id == self.id)
        {
            job.progress = Some(percent.min(100));
            job.updated_at = now_millis();
        }
    }
}

pub fn subscribe() -> broadcast::Receiver<Job> {
    BROADCAST
        .get_or_init(|| broadcast::channel::<Job>(128).0)
        .subscribe()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runner_transitions_to_done_with_result() {
        let job = spawn_with_runner("test.ok".into(), "t".into(), None, |handle| {
            Box::pin(async move {
                handle.step("working");
                handle.progress(50);
                Ok(Value::from("done"))
            })
        });
        for _ in 0..100 {
            if let Some(job) = get(&job.id) {
                if job.state == JobState::Done {
                    assert_eq!(job.result, Some(Value::from("done")));
                    assert_eq!(job.progress, Some(100));
                    assert!(job.steps.iter().any(|step| step.message == "working"));
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("job did not reach Done in time");
    }

    #[tokio::test]
    async fn runner_failure_marks_failed_with_error() {
        let job = spawn_with_runner("test.fail".into(), "t".into(), None, |_handle| {
            Box::pin(async move { Err("boom".into()) })
        });
        for _ in 0..100 {
            if let Some(job) = get(&job.id) {
                if job.state == JobState::Failed {
                    assert_eq!(job.error.as_deref(), Some("boom"));
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("job did not reach Failed in time");
    }

    #[tokio::test]
    async fn cancel_flips_flag_runner_sees() {
        let job = spawn_with_runner("test.cancel".into(), "t".into(), None, |handle| {
            Box::pin(async move {
                for _ in 0..200 {
                    if handle.cancelled() {
                        return Err("cancelled at step boundary".into());
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                }
                Ok(Value::Null)
            })
        });
        assert!(cancel(&job.id));
        for _ in 0..200 {
            if let Some(job) = get(&job.id) {
                if matches!(job.state, JobState::Cancelled | JobState::Failed) {
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("job did not terminate after cancel");
    }
}
