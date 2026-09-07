//! In-memory audit trail (spec §6.4: every reveal is audit-logged).
//!
//! M2: reveals (and other sensitive reads) append to a bounded ring buffer and
//! a tracing event. Persistence into SQLite lands with the event-history
//! milestone; until then `recent()` backs the UI badge and the red-path tests.
//! Secrets are never stored here — only the key name and event kind.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CAPACITY: usize = 200;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub kind: &'static str,
    /// The var/route target, e.g. `RADARR_API_KEY`. Never a secret value.
    pub target: String,
    pub at: u64,
}

static TRAIL: Mutex<VecDeque<AuditEvent>> = Mutex::new(VecDeque::new());

/// Append an audit event (bounded ring). Never call with secret *values* —
/// `target` is a key name only.
pub fn record(kind: &'static str, target: impl Into<String>) {
    let event = AuditEvent {
        kind,
        target: target.into(),
        at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    tracing::info!(kind = %event.kind, target = %event.target, "audit event");
    let mut trail = TRAIL.lock().expect("audit trail");
    if trail.len() >= CAPACITY {
        trail.pop_front();
    }
    trail.push_back(event);
}

/// Snapshot of recent events, newest first.
pub fn recent(limit: usize) -> Vec<AuditEvent> {
    let trail = TRAIL.lock().expect("audit trail");
    trail.iter().rev().take(limit).cloned().collect()
}

/// For tests: clear the trail so assertions start from a known state.
pub fn clear() {
    TRAIL.lock().expect("audit trail").clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_returns_newest_first() {
        clear();
        record("cred.reveal", "UNIT_A");
        record("cred.reveal", "UNIT_B");
        let events = recent(10);
        let mine = events
            .iter()
            .filter(|e| e.target == "UNIT_A" || e.target == "UNIT_B")
            .collect::<Vec<_>>();
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].target, "UNIT_B"); // newest first among mine
        assert!(mine.iter().all(|e| e.kind == "cred.reveal"));
    }

    #[test]
    fn ring_is_bounded() {
        clear();
        for i in 0..(CAPACITY + 50) {
            record("test", format!("UNIT_KEY_{i}"));
        }
        let events = recent(1000);
        let mine = events
            .iter()
            .filter(|e| e.target.starts_with("UNIT_KEY_"))
            .count();
        assert_eq!(mine, CAPACITY);
        assert!(events.iter().any(|e| e.target == "UNIT_KEY_249"));
        assert!(!events.iter().any(|e| e.target == "UNIT_KEY_0"));
    }
}
