//! `.env` engine (spec §6.5, Appendix D `env*` routes).
//!
//! Model, parse, mask, validate, and apply the Bear Cave `.env` file that the
//! compose project interpolates. The file is the single source for the stack's
//! configuration; this module is what the Env page and the apply cascade drive.
//!
//! Rules honoured here:
//! - Never log or serialize secret *values* — only masked forms or
//!   presence/length signals. `reveal` lives in the credentials milestone.
//! - Every write is atomic (temp file + rename) with a timestamped backup
//!   copy first, and it preserves the file's comments and section banners.
//! - A `.env` line is `KEY=VALUE`; section banners are `# ---- Name ----`.
//!   Inline `#` after a value is NOT stripped (compose does not strip it).

use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Keys whose value must never appear in responses/logs. Anything ending in
/// one of these suffixes — or matching `PLEX_TOKEN` — is a secret.
pub fn is_secret_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    [
        "_API_KEY",
        "_TOKEN",
        "_PASS",
        "_PASSWORD",
        "_SECRET",
        "_CLAIM",
    ]
    .iter()
    .any(|suffix| upper.ends_with(suffix))
        || upper == "PLEX_TOKEN"
        || upper == "FRONTEND_BACKEND_API_KEY"
}

/// `changeme`/empty placeholder detection — the `.env.template` placeholder
/// value and the stale state the Env page must surface.
pub fn is_stale_value(_key: &str, value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed.eq_ignore_ascii_case("changeme")
}

/// A parsed `KEY=VALUE` line plus the doc comment immediately above it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    /// Raw current value (never serialized for secret keys — see `view`).
    #[serde(skip_serializing)]
    pub value: String,
    /// Doc comment(s) above the line, joined; `None` when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

/// A `# ---- Section ---- ` block of the file.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub name: String,
    pub vars: Vec<EnvVar>,
}

/// Full parsed `.env`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvDoc {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    /// Lines that were neither banners nor assignments (blank lines, stray
    /// comments) — preserved verbatim so a write round-trips the file.
    raw: Vec<RawLine>,
}

#[derive(Debug, Clone, PartialEq)]
enum RawLine {
    Banner(String), // "# ---- Name ----"
    Blank,
    Comment(String),                      // "# ..." not a banner
    Assign { key: String, text: String }, // "KEY=VALUE"
}

const BANNER_START: &str = "# ---- ";
const BANNER_END: &str = " ----";

impl EnvDoc {
    pub fn from_path(path: impl Into<PathBuf>) -> io::Result<EnvDoc> {
        let path = path.into();
        let text = fs::read_to_string(&path)?;
        Ok(EnvDoc::parse(&text, path))
    }

    /// Parse .env text. Comments directly above an assignment (no blank line
    /// between) become the var's doc.
    pub fn parse(text: &str, path: PathBuf) -> EnvDoc {
        let mut raw = Vec::new();
        let mut sections: Vec<Section> = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_vars: Vec<EnvVar> = Vec::new();
        let mut pending_doc: Vec<String> = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(name) = parse_banner(trimmed) {
                if !current_vars.is_empty() {
                    sections.push(Section {
                        name: current_name.unwrap_or_else(|| "General".to_string()),
                        vars: std::mem::take(&mut current_vars),
                    });
                }
                current_name = Some(name);
                pending_doc.clear();
                raw.push(RawLine::Banner(line.to_string()));
            } else if trimmed.is_empty() {
                pending_doc.clear();
                raw.push(RawLine::Blank);
            } else if trimmed.starts_with('#') {
                pending_doc.push(strip_comment_marker(trimmed));
                raw.push(RawLine::Comment(line.to_string()));
            } else if let Some((key, value)) = split_assignment(trimmed) {
                let doc = if pending_doc.is_empty() {
                    None
                } else {
                    Some(pending_doc.join(" "))
                };
                current_vars.push(EnvVar {
                    key: key.clone(),
                    value,
                    doc,
                });
                pending_doc.clear();
                raw.push(RawLine::Assign {
                    key,
                    text: line.to_string(),
                });
            } else {
                // Unparseable line — keep it so round-trips don't lose content.
                pending_doc.clear();
                raw.push(RawLine::Comment(line.to_string()));
            }
        }
        if !current_vars.is_empty() {
            sections.push(Section {
                name: current_name.unwrap_or_else(|| "General".to_string()),
                vars: current_vars,
            });
        }

        EnvDoc {
            path,
            sections: sections
                .into_iter()
                .filter(|section| !section.vars.is_empty())
                .collect(),
            raw,
        }
    }

    /// All vars across sections, in file order.
    pub fn vars(&self) -> impl Iterator<Item = &EnvVar> {
        self.sections.iter().flat_map(|section| section.vars.iter())
    }

    pub fn get(&self, key: &str) -> Option<&EnvVar> {
        self.vars().find(|var| var.key == key)
    }

    pub fn value(&self, key: &str) -> Option<&str> {
        self.get(key).map(|var| var.value.as_str())
    }

    /// Sections with secret values masked and stale flags computed — the
    /// wire shape for `GET /env`.
    pub fn view(&self) -> EnvView {
        EnvView {
            sections: self
                .sections
                .iter()
                .map(|section| SectionView {
                    name: section.name.clone(),
                    vars: section
                        .vars
                        .iter()
                        .map(|var| VarView {
                            key: var.key.clone(),
                            doc: var.doc.clone(),
                            secret: is_secret_key(&var.key),
                            stale: is_stale_value(&var.key, &var.value),
                            set: !var.value.trim().is_empty(),
                            masked: if var.value.trim().is_empty() {
                                String::new()
                            } else {
                                format!("•••• ({} chars)", var.value.trim().len())
                            },
                        })
                        .collect(),
                })
                .collect(),
            note: None,
            path: self.path.display().to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Stateful handle (shared per Runtime)
// ---------------------------------------------------------------------------

/// Draft edits staged but not yet applied: `values` is key → desired value;
/// `remove` is keys whose lines should be deleted.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Draft {
    pub values: std::collections::BTreeMap<String, String>,
    pub remove: Vec<String>,
}

/// Per-runtime env engine state: where the `.env` lives, where backups go,
/// and the staged draft. Cheap to clone (inner draft is `Arc`d).
#[derive(Clone)]
pub struct EnvState {
    pub path: PathBuf,
    pub backup_dir: PathBuf,
    draft: std::sync::Arc<tokio::sync::Mutex<Draft>>,
}

impl EnvState {
    pub fn new(path: impl Into<PathBuf>, backup_dir: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            backup_dir: backup_dir.into(),
            draft: Default::default(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(
            std::env::var("CAVE_DECK_ENV_PATH").unwrap_or_else(|_| "/app/.env".into()),
            std::env::var("CAVE_DECK_ENV_BACKUP_DIR")
                .unwrap_or_else(|_| "/config/env-backups".into()),
        )
    }

    /// Load and parse the current `.env`. Missing file is an error the caller
    /// maps to a helpful 200-with-note (dev/stub) or 500.
    pub fn load(&self) -> io::Result<EnvDoc> {
        EnvDoc::from_path(&self.path)
    }

    /// View of the current file; `None` note when the file is missing.
    pub fn view(&self) -> Result<EnvView, String> {
        match self.load() {
            Ok(doc) => Ok(doc.view()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(EnvView {
                sections: Vec::new(),
                note: Some(format!(
                    "no .env file at {} — mount the stack .env or set CAVE_DECK_ENV_PATH",
                    self.path.display()
                )),
                path: self.path.display().to_string(),
            }),
            Err(error) => Err(format!("read {}: {error}", self.path.display())),
        }
    }

    pub async fn draft_snapshot(&self) -> Draft {
        self.draft.lock().await.clone()
    }

    pub async fn set_draft(&self, draft: Draft) {
        *self.draft.lock().await = draft;
    }

    pub async fn clear_draft(&self) {
        let mut draft = self.draft.lock().await;
        draft.values.clear();
        draft.remove.clear();
    }

    /// Compute the diff of the current file against the staged draft.
    pub async fn diff(&self) -> Result<Vec<Change>, String> {
        let doc = self
            .load()
            .map_err(|error| format!("read {}: {error}", self.path.display()))?;
        let draft = self.draft.lock().await.clone();
        let values: Vec<(String, String)> = draft
            .values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Ok(diff(&doc, &values, &draft.remove))
    }

    /// Apply the staged draft atomically (backup + temp + rename). Returns the
    /// backup path and the diff that was applied.
    pub async fn apply_draft(&self) -> Result<(PathBuf, Vec<Change>), String> {
        let doc = self
            .load()
            .map_err(|error| format!("read {}: {error}", self.path.display()))?;
        let draft = self.draft.lock().await.clone();
        let values: Vec<(String, String)> = draft
            .values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let changes = diff(&doc, &values, &draft.remove);
        if changes.is_empty() {
            return Err("draft matches the current .env — nothing to apply".to_string());
        }
        let backup = apply(&doc, &changes, &self.backup_dir)
            .map_err(|error| format!("apply {}: {error}", self.path.display()))?;
        Ok((backup, changes))
    }
}

fn parse_banner(trimmed: &str) -> Option<String> {
    if let Some(rest) = trimmed.strip_prefix(BANNER_START) {
        if let Some(name) = rest.strip_suffix(BANNER_END) {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn strip_comment_marker(line: &str) -> String {
    line.trim_start_matches('#').trim().to_string()
}

fn split_assignment(trimmed: &str) -> Option<(String, String)> {
    let (key, value) = trimmed.split_once('=')?;
    let key = key.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((key.to_string(), value.trim().to_string()))
}

/// Wire shape of `GET /env` — never contains raw secret values.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvView {
    pub sections: Vec<SectionView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionView {
    pub name: String,
    pub vars: Vec<VarView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VarView {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub secret: bool,
    pub stale: bool,
    pub set: bool,
    pub masked: String,
}

/// Validation issue for one var (spec §6.5 "validated inputs per var type").
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub key: String,
    pub code: &'static str,
    pub message: String,
    /// `error` blocks apply; `warning` shows but does not block.
    pub severity: &'static str,
}

/// Per-key type validation. Rules are derived from `.env.template` semantics:
/// `_URL` must parse, `_PORT` must be 1–65535, key-like values must be
/// non-empty once placeholders are gone, etc. Stale/placeholder values are
/// warnings (a fresh template is full of `changeme`), never errors.
pub fn validate_key(key: &str, value: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    let upper = key.to_ascii_uppercase();
    let value = value.trim();

    if is_stale_value(key, value) {
        issues.push(Issue {
            key: key.to_string(),
            code: "stale_value",
            message: if value.is_empty() {
                "empty — set a value or leave it unset".to_string()
            } else {
                "placeholder value 'changeme' — replace before relying on it".to_string()
            },
            severity: "warning",
        });
        return issues;
    }

    if upper.ends_with("_URL") || upper == "PLEX_URL" {
        if !(value.starts_with("http://") || value.starts_with("https://")) {
            issues.push(Issue {
                key: key.to_string(),
                code: "invalid_url",
                message: "must be an http(s):// URL".to_string(),
                severity: "error",
            });
        }
    } else if upper.ends_with("_PORT") {
        match value.parse::<u16>() {
            Ok(1..=65535) => {}
            _ => issues.push(Issue {
                key: key.to_string(),
                code: "invalid_port",
                message: "must be an integer 1–65535".to_string(),
                severity: "error",
            }),
        }
    } else if upper == "PUID" || upper == "PGID" {
        if value.parse::<u32>().is_err() {
            issues.push(Issue {
                key: key.to_string(),
                code: "invalid_uid",
                message: "must be a positive integer".to_string(),
                severity: "error",
            });
        }
    } else if upper == "HOST_IP" {
        if value.parse::<std::net::IpAddr>().is_err() {
            issues.push(Issue {
                key: key.to_string(),
                code: "invalid_ip",
                message: "must be an IPv4 or IPv6 address".to_string(),
                severity: "error",
            });
        }
    } else if is_secret_key(key) {
        if value.contains(char::is_whitespace) {
            issues.push(Issue {
                key: key.to_string(),
                code: "invalid_secret",
                message: "secrets must not contain whitespace".to_string(),
                severity: "error",
            });
        } else if value.len() < 8 {
            issues.push(Issue {
                key: key.to_string(),
                code: "weak_secret",
                message: "shorter than 8 characters — likely a placeholder".to_string(),
                severity: "warning",
            });
        }
    }

    issues
}

/// Validate every var in the doc; errors only (warnings returned separately
/// so a draft can show them without blocking).
pub fn validate_doc(doc: &EnvDoc) -> Vec<Issue> {
    doc.vars()
        .flat_map(|var| validate_key(&var.key, &var.value))
        .filter(|issue| issue.severity == "error")
        .collect()
}

// ---------------------------------------------------------------------------
// Draft + diff
// ---------------------------------------------------------------------------

/// A single proposed change.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub key: String,
    pub section: String,
    pub kind: ChangeKind,
    pub old: Option<String>,
    pub new: Option<String>,
    /// Services that read this var (blast radius, §6.5 step 1).
    pub consumers: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Changed,
    Removed,
}

/// Diff `current` against `draft` (map of key → desired value; missing key =
/// unchanged, empty value with `remove` = delete the line).
pub fn diff(current: &EnvDoc, draft: &[(String, String)], remove: &[String]) -> Vec<Change> {
    let mut changes = Vec::new();
    let draft_map: std::collections::HashMap<&str, &str> = draft
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    for section in &current.sections {
        for var in &section.vars {
            let key = var.key.as_str();
            if remove.contains(&key.to_string()) {
                changes.push(Change {
                    key: var.key.clone(),
                    section: section.name.clone(),
                    kind: ChangeKind::Removed,
                    old: Some(var.value.clone()),
                    new: None,
                    consumers: consumers_of(key),
                });
            } else if let Some(desired) = draft_map.get(key) {
                if *desired != var.value {
                    changes.push(Change {
                        key: var.key.clone(),
                        section: section.name.clone(),
                        kind: ChangeKind::Changed,
                        old: Some(var.value.clone()),
                        new: Some((*desired).to_string()),
                        consumers: consumers_of(key),
                    });
                }
            }
        }
    }
    // Added keys (in draft but not in current).
    let existing: std::collections::HashSet<&str> =
        current.vars().map(|var| var.key.as_str()).collect();
    for (key, value) in draft {
        if !existing.contains(key.as_str()) && !value.trim().is_empty() {
            changes.push(Change {
                key: key.clone(),
                section: "General".into(),
                kind: ChangeKind::Added,
                old: None,
                new: Some(value.clone()),
                consumers: consumers_of(key),
            });
        }
    }
    changes.sort_by(|a, b| a.key.cmp(&b.key));
    changes
}

// ---------------------------------------------------------------------------
// Apply — atomic write with backup
// ---------------------------------------------------------------------------

/// Write `changes` into the file at `current.path`.
///
/// 1. timestamped backup copy written first;
/// 2. temp file in the same directory, fsync, rename over the original.
///
/// Comments, banners, blank lines, and untouched assignments survive.
pub fn apply(
    current: &EnvDoc,
    changes: &[Change],
    backup_dir: impl AsRef<Path>,
) -> io::Result<PathBuf> {
    let backup_dir = backup_dir.as_ref();
    fs::create_dir_all(backup_dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_path = backup_dir.join(format!("env.backup.{stamp}"));
    fs::copy(&current.path, &backup_path)?;

    let mut lines = current.raw.clone();
    let mut apply_map: std::collections::HashMap<&str, &Change> = std::collections::HashMap::new();
    for change in changes {
        apply_map.insert(change.key.as_str(), change);
    }

    for line in lines.iter_mut() {
        if let RawLine::Assign { key, text } = line {
            if let Some(change) = apply_map.get(key.as_str()) {
                match change.kind {
                    ChangeKind::Removed => {
                        // Preserve position with a blank line.
                        *text = String::new();
                    }
                    ChangeKind::Changed | ChangeKind::Added => {
                        *text = format!("{}={}", key, change.new.as_deref().unwrap_or(""));
                    }
                }
            }
        }
    }
    // Append truly new keys (Added where the key did not exist) at the end.
    for change in changes {
        let exists = lines.iter().any(|line| match line {
            RawLine::Assign { key, .. } => key == &change.key,
            _ => false,
        });
        if !exists && change.kind == ChangeKind::Added {
            lines.push(RawLine::Assign {
                key: change.key.clone(),
                text: format!("{}={}", change.key, change.new.as_deref().unwrap_or("")),
            });
        }
    }

    let mut rendered = String::new();
    for line in &lines {
        match line {
            RawLine::Blank => rendered.push('\n'),
            RawLine::Assign { text, .. } if text.is_empty() => rendered.push('\n'),
            RawLine::Banner(text) | RawLine::Comment(text) | RawLine::Assign { text, .. } => {
                rendered.push_str(text);
                rendered.push('\n');
            }
        }
    }

    let dir = current
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let tmp = dir.join(format!(
        ".env.tmp.{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(rendered.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp, &current.path)?;
    Ok(backup_path)
}

// ---------------------------------------------------------------------------
// Usenet provider slots (flat NZBDAV_USENET_* var families)
// ---------------------------------------------------------------------------

/// One configured Usenet provider slot: the flat var family `NZBDAV_USENET_` +
/// `<PREFIX>` + `_{HOST,PORT,USER,PASS}` that docker-compose.yml interpolates
/// into the `NZBDAV_CONFIG__USENET__PROVIDERS` JSON.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSlot {
    /// `primary`, `backup`, or the dormant slot name (e.g. `eweka`).
    pub nickname: String,
    /// True when compose wires this slot today (delete-guarded). Grounded in
    /// docker-compose.yml's provider JSON — primary + backup are always wired;
    /// extra slots are dormant until re-added to compose.
    pub wired: bool,
    /// Enabled = vars exist, are non-placeholder, and host/port parse.
    pub enabled: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    /// Never a raw secret — presence/length signal only.
    pub pass_masked: String,
    pub pass_set: bool,
    /// Consumers of this slot's vars (blast radius).
    pub consumers: Vec<String>,
}

/// Known provider slots. `prefix` is the infix between `NZBDAV_USENET_` and
/// `_{HOST,PORT,USER,PASS}` (`""` for primary). `wired` mirrors the compose
/// JSON today: primary + backup are always interpolated; anything else is
/// dormant until its object is re-added to docker-compose.yml.
const PROVIDER_SLOTS: &[(&str, &str, bool)] = &[
    ("primary", "", true),
    ("backup", "BACKUP", true),
    ("eweka", "EWEKA", false),
];

fn provider_var(slot_prefix: &str, suffix: &str) -> String {
    if slot_prefix.is_empty() {
        format!("NZBDAV_USENET_{suffix}")
    } else {
        format!("NZBDAV_USENET_{slot_prefix}_{suffix}")
    }
}

/// Parse provider slots out of the current `.env`. Every slot in the registry
/// is listed (wired or dormant) so the panel shows the full provider picture;
/// missing vars read as disabled.
pub fn provider_slots(doc: &EnvDoc) -> Vec<ProviderSlot> {
    PROVIDER_SLOTS
        .iter()
        .map(|(nickname, prefix, wired)| {
            let host = doc.value(&provider_var(prefix, "HOST")).map(str::trim);
            let port = doc
                .value(&provider_var(prefix, "PORT"))
                .and_then(|v| v.trim().parse::<u16>().ok());
            let user = doc.value(&provider_var(prefix, "USER")).map(str::trim);
            let pass = doc.value(&provider_var(prefix, "PASS")).map(str::trim);
            let vars_present = host.map(|v| !v.is_empty()).unwrap_or(false)
                && user.map(|v| !v.is_empty()).unwrap_or(false)
                && pass.map(|v| !v.is_empty()).unwrap_or(false);
            let enabled = *wired
                && vars_present
                && host.is_some_and(|v| !is_stale_value("", v))
                && port.is_some();
            let consumers = vec!["nzbdav".to_string(), "cave-deck".to_string()];
            ProviderSlot {
                nickname: (*nickname).to_string(),
                wired: *wired,
                enabled,
                host: host.map(str::to_string),
                port,
                user: user.map(str::to_string),
                pass_masked: pass.map_or_else(String::new, |v| {
                    if v.is_empty() {
                        String::new()
                    } else {
                        format!("•••• ({} chars)", v.len())
                    }
                }),
                pass_set: pass.is_some_and(|v| !v.is_empty()),
                consumers,
            }
        })
        .collect()
}

/// True when a slot name is compose-wired (delete-guard: removing these vars
/// would break the compose interpolation, so DELETE refuses them).
pub fn provider_wired(nickname: &str) -> bool {
    PROVIDER_SLOTS
        .iter()
        .any(|(name, _, wired)| *wired && name == &nickname)
}

/// Keys belonging to a slot's flat var family, for draft remove/rewrite.
pub fn provider_keys(nickname: &str) -> Option<[String; 4]> {
    let (_, prefix, _) = PROVIDER_SLOTS
        .iter()
        .find(|(name, _, _)| name == &nickname)?;
    Some([
        provider_var(prefix, "HOST"),
        provider_var(prefix, "PORT"),
        provider_var(prefix, "USER"),
        provider_var(prefix, "PASS"),
    ])
}

// ---------------------------------------------------------------------------
// Consumer registry (blast radius)
// ---------------------------------------------------------------------------

/// Services that read a given var, grounded in `docker-compose.yml` env
/// interpolation (`services/bash-functions` reads keys through the compose
/// service env). The cave-deck service itself reads almost everything, so it
/// is appended to every consumer list.
pub fn consumers_of(key: &str) -> Vec<String> {
    let upper = key.to_ascii_uppercase();
    let mut consumers: Vec<&str> = if upper == "PUID" || upper == "PGID" || upper == "TZ" {
        vec![
            "prowlarr",
            "radarr",
            "sonarr",
            "nzbdav",
            "nzbdav_rclone",
            "seerr",
            "plex",
            "unpackerr",
        ]
    } else if upper.ends_with("_URL") || upper == "HOST_IP" {
        vec!["cave-deck"]
    } else if upper.starts_with("NZBDAV_") {
        let mut list = vec!["nzbdav"];
        if upper.contains("WEBDAV_PASS") || upper.contains("RCLONE_RC_PASS") {
            list.push("nzbdav_rclone");
        }
        list
    } else if upper == "RADARR_API_KEY" || upper == "SONARR_API_KEY" {
        vec!["nzbdav", "unpackerr"]
    } else if upper == "PROWLARR_API_KEY" {
        vec!["nzbdav"]
    } else if upper == "SEERR_API_KEY" || upper == "FRONTEND_BACKEND_API_KEY" {
        vec!["seerr"]
    } else if upper == "PLEX_TOKEN" || upper == "PLEX_CLAIM" {
        vec!["plex"]
    } else {
        vec!["cave-deck"]
    };
    consumers.push("cave-deck");
    consumers.sort_unstable();
    consumers.dedup();
    consumers.into_iter().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    const SAMPLE: &str = r#"# ---- Identity / Runtime ----
PUID=1000
PGID=1000

# ---- Plex ----
PLEX_URL=http://192.168.1.100:32400
PLEX_TOKEN=changeme

# ---- *arr API Keys ----
RADARR_API_KEY=deadbeefdeadbeefdeadbeefdeadbeef
"#;

    const SAMPLE_PROVIDERS: &str = r#"# ---- Usenet Providers ----
NZBDAV_USENET_HOST=usenet.example.com
NZBDAV_USENET_PORT=563
NZBDAV_USENET_USER=alice
NZBDAV_USENET_PASS=supersecretpass

NZBDAV_USENET_BACKUP_HOST=backup.example.com
NZBDAV_USENET_BACKUP_PORT=563
NZBDAV_USENET_BACKUP_USER=bob
NZBDAV_USENET_BACKUP_PASS=anothersecretpass

NZBDAV_USENET_EWEKA_HOST=changeme
NZBDAV_USENET_EWEKA_PORT=563
NZBDAV_USENET_EWEKA_USER=changeme
NZBDAV_USENET_EWEKA_PASS=changeme
"#;

    #[test]
    fn parses_sections_vars_and_docs() {
        let doc = EnvDoc::parse(SAMPLE, PathBuf::from("test.env"));
        assert_eq!(doc.sections.len(), 3);
        assert_eq!(doc.sections[0].name, "Identity / Runtime");
        assert_eq!(doc.sections[0].vars.len(), 2);
        let plex = &doc.sections[1];
        assert_eq!(plex.vars[0].key, "PLEX_URL");
        assert_eq!(plex.vars[0].doc, None);
        assert_eq!(plex.vars[1].doc, None);
        assert_eq!(
            doc.value("RADARR_API_KEY"),
            Some("deadbeefdeadbeefdeadbeefdeadbeef")
        );
    }

    #[test]
    fn view_masks_secrets_and_flags_stale() {
        let doc = EnvDoc::parse(SAMPLE, PathBuf::from("test.env"));
        let view = doc.view();
        let plex = view
            .sections
            .iter()
            .find(|section| section.name == "Plex")
            .unwrap();
        let token = plex
            .vars
            .iter()
            .find(|var| var.key == "PLEX_TOKEN")
            .unwrap();
        assert!(token.secret);
        assert!(token.stale); // 'changeme' placeholder
        assert!(token.set);
        assert!(!token.masked.contains("changeme"));
        let view = doc.view();
        let key = view
            .sections
            .iter()
            .flat_map(|section| section.vars.iter())
            .find(|var| var.key == "RADARR_API_KEY")
            .unwrap();
        assert!(key.secret);
        assert!(!key.stale);
        assert!(key.set);
        assert!(key.masked.contains("••••"));
        assert!(!key.masked.contains("deadbeef"));
    }

    #[test]
    fn secret_key_detection() {
        assert!(is_secret_key("RADARR_API_KEY"));
        assert!(is_secret_key("NZBDAV_WEBDAV_PASS"));
        assert!(is_secret_key("PLEX_TOKEN"));
        assert!(!is_secret_key("HOST_IP"));
        assert!(!is_secret_key("TZ"));
        assert!(!is_secret_key("RADARR_URL"));
    }

    #[test]
    fn validates_by_type() {
        assert!(validate_key("RADARR_URL", "not a url")
            .iter()
            .any(|i| i.code == "invalid_url"));
        assert!(validate_key("NZBDAV_USENET_PORT", "70000")
            .iter()
            .any(|i| i.code == "invalid_port"));
        assert!(validate_key("HOST_IP", "999.1.1.1")
            .iter()
            .any(|i| i.code == "invalid_ip"));
        assert!(validate_key("PLEX_TOKEN", "changeme")
            .iter()
            .any(|i| i.code == "stale_value"));
        assert!(validate_key("RADARR_API_KEY", "deadbeefdeadbeefdeadbeefdeadbeef").is_empty());
        assert!(validate_key("RADARR_API_KEY", "short")
            .iter()
            .any(|i| i.code == "weak_secret"));
    }

    #[test]
    fn diff_detects_changed_added_removed() {
        let doc = EnvDoc::parse(SAMPLE, PathBuf::from("test.env"));
        let changes = diff(
            &doc,
            &[
                ("PLEX_URL".to_string(), "http://10.0.0.5:32400".to_string()),
                (
                    "RADARR_API_KEY".to_string(),
                    "newkeynewkeynewkeynewkeynewkeynewkey".to_string(),
                ),
                ("SEERR_API_KEY".to_string(), "seerrkey123".to_string()),
            ],
            &["PLEX_CLAIM".to_string()],
        );
        assert!(changes
            .iter()
            .any(|c| c.key == "PLEX_URL" && c.kind == ChangeKind::Changed));
        assert!(changes
            .iter()
            .any(|c| c.key == "RADARR_API_KEY" && c.kind == ChangeKind::Changed));
        assert!(changes
            .iter()
            .any(|c| c.key == "SEERR_API_KEY" && c.kind == ChangeKind::Added));
        // PLEX_CLAIM isn't in the sample, so removal of a missing key yields nothing.
        assert!(!changes.iter().any(|c| c.key == "PLEX_CLAIM"));

        let removed = diff(&doc, &[], &["PLEX_URL".to_string()]);
        assert!(removed
            .iter()
            .any(|c| c.key == "PLEX_URL" && c.kind == ChangeKind::Removed));
        let radarr = changes.iter().find(|c| c.key == "RADARR_API_KEY").unwrap();
        assert!(radarr.consumers.contains(&"nzbdav".to_string()));
    }

    #[test]
    fn apply_round_trips_and_backs_up() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, SAMPLE).unwrap();
        let doc = EnvDoc::from_path(&path).unwrap();
        let backup = dir.path().join("backups");
        let changes = diff(
            &doc,
            &[("PLEX_URL".to_string(), "http://10.0.0.5:32400".to_string())],
            &[],
        );
        apply(&doc, &changes, &backup).unwrap();

        let rewritten = EnvDoc::from_path(&path).unwrap();
        assert_eq!(rewritten.value("PLEX_URL"), Some("http://10.0.0.5:32400"));
        // Unchanged values survive; banners survive.
        assert_eq!(rewritten.value("PUID"), Some("1000"));
        assert_eq!(rewritten.sections.len(), 3);
        assert_eq!(rewritten.sections[0].name, "Identity / Runtime");
        let backups = fs::read_dir(&backup).unwrap().count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn provider_slots_parse_flat_families() {
        let doc = EnvDoc::parse(SAMPLE_PROVIDERS, PathBuf::from("test.env"));
        let slots = provider_slots(&doc);
        assert_eq!(slots.len(), 3);

        let primary = slots.iter().find(|s| s.nickname == "primary").unwrap();
        assert!(primary.wired);
        assert!(primary.enabled);
        assert_eq!(primary.host.as_deref(), Some("usenet.example.com"));
        assert_eq!(primary.port, Some(563));
        assert_eq!(primary.user.as_deref(), Some("alice"));
        assert!(primary.pass_set);
        assert!(primary.pass_masked.contains("••••"));
        assert!(!primary.pass_masked.contains("supersecretpass"));

        let backup = slots.iter().find(|s| s.nickname == "backup").unwrap();
        assert!(backup.wired);
        assert!(backup.enabled);
        assert_eq!(backup.host.as_deref(), Some("backup.example.com"));

        // Dormant slot: vars present but stale/placeholder → disabled.
        let eweka = slots.iter().find(|s| s.nickname == "eweka").unwrap();
        assert!(!eweka.wired);
        assert!(!eweka.enabled);
        assert_eq!(eweka.host.as_deref(), Some("changeme"));
        assert!(eweka.pass_set); // 'changeme' counts as set but not enabled
    }

    #[test]
    fn provider_wired_guards_compose_slots() {
        assert!(provider_wired("primary"));
        assert!(provider_wired("backup"));
        assert!(!provider_wired("eweka"));
        assert!(!provider_wired("nope"));
        assert_eq!(
            provider_keys("primary").unwrap(),
            [
                "NZBDAV_USENET_HOST".to_string(),
                "NZBDAV_USENET_PORT".to_string(),
                "NZBDAV_USENET_USER".to_string(),
                "NZBDAV_USENET_PASS".to_string(),
            ]
        );
        assert_eq!(
            provider_keys("backup").unwrap()[0],
            "NZBDAV_USENET_BACKUP_HOST".to_string()
        );
        assert_eq!(provider_keys("nope"), None);
    }

    #[test]
    fn consumers_grounded_in_compose() {
        assert_eq!(
            consumers_of("SONARR_API_KEY"),
            vec!["cave-deck", "nzbdav", "unpackerr"]
        );
        assert_eq!(
            consumers_of("NZBDAV_RCLONE_RC_PASS"),
            vec!["cave-deck", "nzbdav", "nzbdav_rclone"]
        );
        assert!(consumers_of("PUID").contains(&"plex".to_string()));
    }
}
