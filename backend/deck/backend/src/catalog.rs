//! Catalog domain (spec §6.3 / Appendix A) — loads `catalog/catalog.yaml`,
//! validates it, and provides the conflict checks behind
//! `POST /api/v1/catalog/conflicts`.
//!
//! The catalog file ships with the image (`catalog/catalog.yaml`, included at
//! compile time) and may be overridden by `<config>/catalog.yaml` at runtime
//! for hot curation. Loading is fail-closed: an invalid catalog poisons the
//! global instance and every catalog route reports `catalog_invalid` until
//! the file is fixed — the GUI must never offer an entry the validator
//! would reject.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Model — serde-shaped mirror of Appendix A's entry schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub retired: bool,
    pub image: String,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvVar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_network: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose_fragment: Option<String>,
    pub mem_limit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<String>,
    pub docs: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    version: String,
    updated: String,
    entries: Vec<Entry>,
}

/// Client-facing catalog document (§D: "entries (§6.3 schema) + install-state").
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDocument {
    pub version: String,
    pub updated: String,
    pub install_state: String,
    pub entries: Vec<Entry>,
}

// ---------------------------------------------------------------------------
// Validation — the in-binary subset of catalog/validate.py's rules.
// The Python validator stays normative in CI; this guards the runtime path.
// ---------------------------------------------------------------------------

fn validate(entries: &[Entry]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    let mut host_ports: std::collections::HashMap<(u16, String), String> =
        std::collections::HashMap::new();

    for e in entries {
        if e.retired {
            return Err(format!("entry '{}' declares retired: true", e.id));
        }
        if e.id.is_empty() || e.image.is_empty() {
            return Err(format!("entry '{}' missing id/image", e.id));
        }
        if !ids.insert(e.id.clone()) {
            return Err(format!("duplicate id '{}'", e.id));
        }
        for spec in &e.ports {
            let (host, proto) = split_port_spec(spec)
                .ok_or_else(|| format!("{}: malformed port spec '{spec}'", e.id))?;
            let entry = host_ports.entry((host, proto.to_string()));
            if let std::collections::hash_map::Entry::Occupied(prev) = &entry {
                return Err(format!(
                    "host port {}/{} claimed by both '{}' and '{}'",
                    host,
                    proto,
                    prev.get(),
                    e.id
                ));
            }
            entry.or_insert_with(|| e.id.clone());
        }
        for dep in &e.dependencies {
            if !entries.iter().any(|x| x.id == *dep) {
                return Err(format!("{}: unresolved dependency '{dep}'", e.id));
            }
        }
    }
    Ok(())
}

/// `"61208:61208/tcp"` → `(61208, "tcp")`; proto defaults to tcp (§A ports).
pub fn split_port_spec(spec: &str) -> Option<(u16, &str)> {
    let (map, proto) = match spec.split_once('/') {
        Some((m, p)) => (m, p),
        None => (spec, "tcp"),
    };
    let host = map.split(':').next()?.parse().ok()?;
    Some((host, proto))
}

// ---------------------------------------------------------------------------
// Global state — the loaded catalog. Fail-closed: invalid ⇒ poisoned.
// ---------------------------------------------------------------------------

struct State {
    doc: CatalogDocument,
}

static CATALOG: OnceLock<Mutex<Option<State>>> = OnceLock::new();
// The shared catalog lives at the repository root after the M0 relocation
// (it describes the whole stack, not just the Deck).
const EMBEDDED: &str = include_str!("../../../../catalog/catalog.yaml");

fn slot() -> &'static Mutex<Option<State>> {
    CATALOG.get_or_init(|| Mutex::new(None))
}

fn parse(text: &str) -> Result<CatalogDocument, String> {
    let file: CatalogFile = serde_yaml::from_str(text).map_err(|e| format!("parse: {e}"))?;
    validate(&file.entries)?;
    Ok(CatalogDocument {
        version: file.version,
        updated: file.updated,
        install_state: "unknown".into(),
        entries: file.entries,
    })
}

/// Load the catalog from `text`. Later calls replace earlier ones; a failure
/// poisons the slot so routes report `catalog_invalid` instead of serving a
/// stale or partial catalog.
pub fn load_from_str(text: &str) -> Result<usize, String> {
    let doc = parse(text)?;
    let n = doc.entries.len();
    *slot().lock().expect("catalog slot") = Some(State { doc });
    Ok(n)
}

/// Load the default (compile-time-embedded) catalog, unless already loaded.
pub fn ensure_loaded() {
    let mut guard = slot().lock().expect("catalog slot");
    if guard.is_none() {
        match parse(EMBEDDED) {
            Ok(doc) => *guard = Some(State { doc }),
            // The embedded file is CI-validated; if it still fails, leave the
            // slot poisoned — routes must refuse, not serve garbage.
            Err(e) => tracing::error!("embedded catalog invalid: {e}"),
        }
    }
}

pub fn get() -> Option<CatalogDocument> {
    slot()
        .lock()
        .expect("catalog slot")
        .as_ref()
        .map(|s| CatalogDocument {
            version: s.doc.version.clone(),
            updated: s.doc.updated.clone(),
            install_state: s.doc.install_state.clone(),
            entries: s.doc.entries.clone(),
        })
}

pub fn entry(id: &str) -> Option<Entry> {
    slot()
        .lock()
        .expect("catalog slot")
        .as_ref()
        .and_then(|s| s.doc.entries.iter().find(|e| e.id == id))
        .cloned()
}

pub fn is_poisoned() -> bool {
    slot().lock().expect("catalog slot").is_none()
}

// ---------------------------------------------------------------------------
// Conflict checks (§6.3 "Conflict rules") — static subset behind
// POST /catalog/conflicts. Live docker/compose probes land with M4's
// install flow; the static rules are enforced unconditionally.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DraftInstall {
    pub id: String,
    #[serde(default)]
    pub ports: Option<Vec<String>>,
    #[serde(default)]
    pub volumes: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct Conflict {
    pub kind: String, // "port" | "dependency" | "catalog_invalid"
    pub message: String,
}

pub fn check_conflicts(draft: &DraftInstall) -> Vec<Conflict> {
    let mut out = Vec::new();

    let Some(cat_entry) = entry(&draft.id) else {
        return vec![Conflict {
            kind: "unknown_entry".into(),
            message: format!("no catalog entry '{}'", draft.id),
        }];
    };

    // Effective port set = entry defaults overridden by the draft's edits.
    let ports = draft
        .ports
        .clone()
        .unwrap_or_else(|| cat_entry.ports.clone());
    let mut claimed: std::collections::HashMap<(u16, String), String> =
        std::collections::HashMap::new();
    for e in get().map(|d| d.entries).unwrap_or_default() {
        if e.id == cat_entry.id {
            continue;
        }
        for spec in &e.ports {
            if let Some((host, proto)) = split_port_spec(spec) {
                claimed.insert((host, proto.to_string()), e.id.clone());
            }
        }
    }
    for spec in &ports {
        if let Some((host, proto)) = split_port_spec(spec) {
            if let Some(other) = claimed.get(&(host, proto.to_string())) {
                out.push(Conflict {
                    kind: "port".into(),
                    message: format!(
                        "host port {host}/{proto} already claimed by catalog entry '{other}'"
                    ),
                });
            }
        }
    }

    // Known-catalog dependency check; live-service dependencies need the
    // compose probe (M4).
    for dep in &cat_entry.dependencies {
        if entry(dep).is_none() {
            out.push(Conflict {
                kind: "dependency".into(),
                message: format!(
                    "dependency '{dep}' is not a catalog entry — verify it exists in the live stack"
                ),
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_loads_and_is_valid() {
        let n = load_from_str(EMBEDDED).expect("embedded catalog must validate");
        assert!(
            n >= 75,
            "catalog should carry the researched entries (got {n})"
        );
    }

    #[test]
    fn port_spec_parsing() {
        assert_eq!(split_port_spec("61208:61208"), Some((61208, "tcp")));
        assert_eq!(split_port_spec("61208:61208/udp"), Some((61208, "udp")));
        assert_eq!(split_port_spec("garbage"), None);
    }

    #[test]
    fn duplicate_ids_rejected() {
        let e = |id: &str| Entry {
            id: id.into(),
            name: id.into(),
            category: "ops".into(),
            retired: false,
            image: format!("img:{id}"),
            ports: vec![],
            volumes: vec![],
            env: vec![],
            pid_mode: None,
            host_network: None,
            devices: vec![],
            capabilities: vec![],
            dependencies: vec![],
            compose_fragment: None,
            mem_limit: "256m".into(),
            healthcheck: None,
            docs: "https://example.com".into(),
            notes: String::new(),
        };
        assert!(validate(&[e("a"), e("a")]).is_err());
        assert!(validate(&[e("a"), e("b")]).is_ok());
    }
}
