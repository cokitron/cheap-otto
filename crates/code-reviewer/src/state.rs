//! Persistent PR review ledger, observation compliance tracker, trigger state machine,
//! and agent-actionable Markdown/GitHub formatters.

use crate::config::repo_fs_slug;
use crate::diff::{InlinePlacement, ParsedDiffMap};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

static ATOMIC_TMP_SEQ: AtomicU64 = AtomicU64::new(0);
static IN_PROCESS_SPINLOCK_MUTEX: Mutex<()> = Mutex::new(());

/// Invisible HTML marker embedded in every GitHub comment/review written by `code-reviewer`.
/// Prevents infinite self-trigger loops when the human developer and reviewer share an account.
pub const BOT_COMMENT_MARKER: &str = "<!-- otto-code-reviewer:v1 -->";

/// Sub-marker identifying the live pickup/completion status banner comment on a PR timeline.
pub const STATUS_BANNER_MARKER: &str = "<!-- otto-code-reviewer:status-banner -->";

#[derive(Debug, Error)]
pub enum StateError {
    #[error("failed to read state file {path}: {source}")]
    IoRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write state file {path}: {source}")]
    IoWrite {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse state JSON at {path}: {source}")]
    JsonParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to serialize state JSON: {0}")]
    JsonSerialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

impl Severity {
    pub const fn is_blocking(self) -> bool {
        matches!(self, Self::Critical | Self::High)
    }

    pub const fn badge(self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceTag {
    Reproduced,
    Verified,
    Plausible,
}

impl ConfidenceTag {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reproduced => "reproduced",
            Self::Verified => "verified",
            Self::Plausible => "plausible",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    Open,
    FixedInCode,
    AcceptedRationale,
    Superseded,
}

impl ObservationStatus {
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::FixedInCode => "FIXED_IN_CODE",
            Self::AcceptedRationale => "ACCEPTED_RATIONALE",
            Self::Superseded => "SUPERSEDED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InternalVerdict {
    Pass,
    ChangesNeeded,
}

impl InternalVerdict {
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::ChangesNeeded => "CHANGES_NEEDED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceabilityCriterion {
    pub id: String,
    pub requirement: String,
    pub code_evidence: String,
    pub test_evidence: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationRecord {
    /// Stable per-PR observation identifier (`OBS-001`, `OBS-002`, ...).
    pub obs_code: String,
    pub brain_id: String,
    pub severity: Severity,
    pub confidence: ConfidenceTag,
    pub path: String,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
    pub title: String,
    pub claim: String,
    pub fix_guidance: String,
    #[serde(default)]
    pub suggested_code: Option<String>,
    pub status: ObservationStatus,
    pub created_in_run: u32,
    pub created_sha: String,
    #[serde(default)]
    pub updated_sha: Option<String>,
    #[serde(default)]
    pub resolution_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedPrState {
    pub repo: String,
    pub pr_number: u64,
    pub title: String,
    pub author: String,
    pub base_ref: String,
    pub head_ref: String,
    pub initial_review_done: bool,
    pub run_count: u32,
    #[serde(default)]
    pub initial_summary: Option<String>,
    #[serde(default)]
    pub latest_summary: Option<String>,
    #[serde(default)]
    pub last_reviewed_sha: Option<String>,
    /// ISO-8601 timestamp cursor of the latest human PR comment or review reply processed.
    #[serde(default)]
    pub last_processed_reply_at: Option<String>,
    #[serde(default)]
    pub last_verdict: Option<InternalVerdict>,
    #[serde(default)]
    pub gdrive_doc_id: Option<String>,
    #[serde(default)]
    pub gdrive_doc_url: Option<String>,
    /// GitHub issue comment ID showing the live "⏳ Triggered & Running" banner for an in-flight run.
    #[serde(default)]
    pub active_status_comment_id: Option<u64>,
    /// Deduplication key (`<head_sha>:<latest_reply_ts>`) so retries update the same banner instead of spamming duplicates.
    #[serde(default)]
    pub active_status_key: Option<String>,
    /// Last `<head_sha>:<latest_reply_ts>` key dispatched by the poller (used by the retry circuit breaker so transient failures never loop AI tokens).
    #[serde(default)]
    pub last_attempt_key: Option<String>,
    /// Number of dispatch attempts recorded for `last_attempt_key` before a completed review.
    #[serde(default)]
    pub attempts_for_key: u32,
    #[serde(default)]
    pub traceability: Vec<TraceabilityCriterion>,
    #[serde(default)]
    pub observations: Vec<ObservationRecord>,
    #[serde(default)]
    pub merged_processed: bool,
    pub updated_at: DateTime<Utc>,
}

impl TrackedPrState {
    pub fn new(
        repo: &str,
        pr_number: u64,
        title: &str,
        author: &str,
        base_ref: &str,
        head_ref: &str,
    ) -> Self {
        Self {
            repo: repo.to_string(),
            pr_number,
            title: title.to_string(),
            author: author.to_string(),
            base_ref: base_ref.to_string(),
            head_ref: head_ref.to_string(),
            initial_review_done: false,
            run_count: 0,
            initial_summary: None,
            latest_summary: None,
            last_reviewed_sha: None,
            last_processed_reply_at: None,
            last_verdict: None,
            gdrive_doc_id: None,
            gdrive_doc_url: None,
            active_status_comment_id: None,
            active_status_key: None,
            last_attempt_key: None,
            attempts_for_key: 0,
            traceability: Vec::new(),
            observations: Vec::new(),
            merged_processed: false,
            updated_at: Utc::now(),
        }
    }

    pub fn next_obs_code(&self) -> String {
        format!("OBS-{:03}", self.observations.len() + 1)
    }

    pub fn open_observations(&self) -> Vec<&ObservationRecord> {
        self.observations
            .iter()
            .filter(|o| o.status.is_open())
            .collect()
    }

    pub fn has_blocking_open_observations(&self) -> bool {
        self.observations
            .iter()
            .any(|o| o.status.is_open() && o.severity.is_blocking())
    }

    pub fn compute_verdict(&self) -> InternalVerdict {
        if self.has_blocking_open_observations() {
            InternalVerdict::ChangesNeeded
        } else {
            InternalVerdict::Pass
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoPollCursor {
    #[serde(default)]
    pub last_poll_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_merged_at_cursor: Option<String>,
    #[serde(default)]
    pub living_doc_gdrive_id: Option<String>,
    #[serde(default)]
    pub living_doc_gdrive_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerStore {
    /// Round-robin counter used to intercalate lead AI provider (`claude` <-> `agy`) across parallel PR sessions.
    #[serde(default)]
    pub provider_dispatch_counter: u64,
    /// Keyed by `<owner>/<repo>`.
    #[serde(default)]
    pub repo_cursors: BTreeMap<String, RepoPollCursor>,
    /// Keyed by `<owner>/<repo>#<pr_number>`.
    #[serde(default)]
    pub pull_requests: BTreeMap<String, TrackedPrState>,
}

impl LedgerStore {
    pub fn pr_key(repo: &str, pr_number: u64) -> String {
        format!("{}#{}", repo.to_ascii_lowercase(), pr_number)
    }

    pub fn load_or_default(state_dir: &Path) -> Result<Self, StateError> {
        let path = state_dir.join("state.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path).map_err(|source| StateError::IoRead {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|source| StateError::JsonParse { path, source })
    }

    /// Writes `state.json` atomically via PID-isolated `.tmp.<pid>` + rename.
    pub fn save_atomic(&self, state_dir: &Path) -> Result<(), StateError> {
        fs::create_dir_all(state_dir).map_err(|source| StateError::IoWrite {
            path: state_dir.to_path_buf(),
            source,
        })?;
        let final_path = state_dir.join("state.json");
        let seq = ATOMIC_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp_path = state_dir.join(format!("state.json.tmp.{}.{}", std::process::id(), seq));
        let payload = serde_json::to_string_pretty(self)?;
        fs::write(&tmp_path, payload).map_err(|source| StateError::IoWrite {
            path: tmp_path.clone(),
            source,
        })?;
        fs::rename(&tmp_path, &final_path).map_err(|source| StateError::IoWrite {
            path: final_path,
            source,
        })
    }

    /// Concurrent-safe read-modify-write transaction on `state.json` across parallel async PR workers and HTTP threads.
    pub fn with_exclusive_lock<R, F>(state_dir: &Path, mutate_fn: F) -> Result<R, StateError>
    where
        F: FnOnce(&mut Self) -> R,
    {
        let _in_proc = IN_PROCESS_SPINLOCK_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let locks_dir = state_dir.join("locks");
        fs::create_dir_all(&locks_dir).map_err(|source| StateError::IoWrite {
            path: locks_dir.clone(),
            source,
        })?;
        let lock_path = locks_dir.join("ledger-write.lock");
        let _guard = acquire_file_spinlock(&lock_path, Duration::from_secs(10));

        let mut store = Self::load_or_default(state_dir)?;
        let out = mutate_fn(&mut store);
        store.save_atomic(state_dir)?;
        Ok(out)
    }
}

pub struct FileSpinLockGuard {
    path: PathBuf,
}

impl Drop for FileSpinLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn acquire_file_spinlock(lock_path: &Path, timeout: Duration) -> FileSpinLockGuard {
    if let Some(parent) = lock_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let start = Instant::now();
    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_path)
        {
            Ok(mut f) => {
                let _ = writeln!(f, "{}", std::process::id());
                let _ = f.flush();
                return FileSpinLockGuard {
                    path: lock_path.to_path_buf(),
                };
            }
            Err(_) => {
                if let Ok(raw) = fs::read_to_string(lock_path) {
                    let pid_str = raw.lines().next().unwrap_or("").trim();
                    // Only reclaim if a valid non-empty PID was written AND that OS process is gone.
                    if !pid_str.is_empty() && !is_process_alive_str(pid_str) {
                        let _ = fs::remove_file(lock_path);
                        continue;
                    }
                }
                if start.elapsed() >= timeout {
                    let _ = fs::remove_file(lock_path);
                    return FileSpinLockGuard {
                        path: lock_path.to_path_buf(),
                    };
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

pub fn is_process_alive(pid: u32) -> bool {
    is_process_alive_str(&pid.to_string())
}

pub fn is_process_alive_str(pid_str: &str) -> bool {
    if pid_str.is_empty() || pid_str.parse::<u32>().is_err() {
        return false;
    }
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", pid_str])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid_str), "/NH"])
            .stdin(Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(pid_str))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainLiveState {
    pub runner: String,
    /// `pending`, `running`, `done`, `failed`, or `skipped`.
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveWorkerState {
    pub pid: u32,
    pub repo: String,
    pub pr_number: u64,
    pub title: String,
    pub head_sha: String,
    pub mode: String,
    pub reviewer_account: String,
    pub lead_runner: String,
    pub alt_runner: String,
    /// `starting`, `worktree`, `specialist_brains`, `adversarial_verifier`, `re_review`, `drive_sync`, `posting_github`, `completed`, `error`.
    pub phase: String,
    pub started_at_utc: String,
    pub updated_at_utc: String,
    #[serde(default)]
    pub brains: BTreeMap<String, BrainLiveState>,
}

impl ActiveWorkerState {
    pub fn file_path(state_dir: &Path, repo: &str, pr_number: u64) -> PathBuf {
        state_dir
            .join("workers")
            .join(format!("{}__pr-{}.json", repo_fs_slug(repo), pr_number))
    }

    pub fn save(&mut self, state_dir: &Path) {
        self.updated_at_utc = Utc::now().to_rfc3339();
        let path = Self::file_path(state_dir, &self.repo, self.pr_number);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let seq = ATOMIC_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!("tmp.{}.{}", std::process::id(), seq));
        if let Ok(json) = serde_json::to_string_pretty(self)
            && fs::write(&tmp, json).is_ok()
        {
            let _ = fs::rename(&tmp, &path);
        }
    }

    pub fn remove(state_dir: &Path, repo: &str, pr_number: u64) {
        let _ = fs::remove_file(Self::file_path(state_dir, repo, pr_number));
    }

    pub fn is_pr_worker_running(state_dir: &Path, repo: &str, pr_number: u64) -> Option<Self> {
        let path = Self::file_path(state_dir, repo, pr_number);
        let raw = fs::read_to_string(&path).ok()?;
        let w: Self = serde_json::from_str(&raw).ok()?;
        if w.phase != "completed" && w.phase != "error" && is_process_alive(w.pid) {
            Some(w)
        } else {
            let _ = fs::remove_file(&path);
            None
        }
    }

    pub fn list_active(state_dir: &Path) -> Vec<Self> {
        let dir = state_dir.join("workers");
        let Ok(entries) = fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut active = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&p)
                && let Ok(w) = serde_json::from_str::<Self>(&raw)
            {
                if w.phase != "completed" && w.phase != "error" && is_process_alive(w.pid) {
                    active.push(w);
                } else {
                    let _ = fs::remove_file(&p);
                }
            }
        }
        active.sort_by_key(|w| w.pr_number);
        active
    }
}

/// Human comment/reply surfaced on a PR since the previous review run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanReplyItem {
    pub author: String,
    pub created_at: String,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub body: String,
}

/// Snapshot of an open PR observed during polling.
#[derive(Debug, Clone)]
pub struct OpenPrSnapshot {
    pub pr_number: u64,
    pub title: String,
    pub author: String,
    pub is_draft: bool,
    pub base_ref: String,
    pub head_ref: String,
    pub head_sha: String,
    pub latest_committer: Option<String>,
    pub human_replies: Vec<HumanReplyItem>,
}

/// Deterministic decision returned by the Internal CI trigger state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerDecision {
    SkipDraft,
    UpToDate,
    InitialFullReview {
        head_sha: String,
    },
    ReReview {
        previous_sha: String,
        head_sha: String,
        sha_changed: bool,
        new_replies: Vec<HumanReplyItem>,
    },
}

/// Evaluates whether an open PR needs an Initial Full Review, a Stateful Re-Review (push or answers), or nothing.
pub fn evaluate_pr_trigger(
    existing: Option<&TrackedPrState>,
    snapshot: &OpenPrSnapshot,
) -> TriggerDecision {
    if snapshot.is_draft {
        return TriggerDecision::SkipDraft;
    }

    let Some(tracked) = existing else {
        return TriggerDecision::InitialFullReview {
            head_sha: snapshot.head_sha.clone(),
        };
    };

    if !tracked.initial_review_done {
        return TriggerDecision::InitialFullReview {
            head_sha: snapshot.head_sha.clone(),
        };
    }

    let prev_sha = tracked.last_reviewed_sha.clone().unwrap_or_default();
    let sha_changed = prev_sha != snapshot.head_sha;

    let cursor = tracked.last_processed_reply_at.as_deref().unwrap_or("");
    let new_replies: Vec<HumanReplyItem> = snapshot
        .human_replies
        .iter()
        .filter(|r| !r.body.contains(BOT_COMMENT_MARKER))
        .filter(|r| r.created_at.as_str() > cursor)
        .cloned()
        .collect();

    if sha_changed || !new_replies.is_empty() {
        TriggerDecision::ReReview {
            previous_sha: prev_sha,
            head_sha: snapshot.head_sha.clone(),
            sha_changed,
            new_replies,
        }
    } else {
        TriggerDecision::UpToDate
    }
}

/// Inline comment ready for GitHub's `POST /repos/{owner}/{repo}/pulls/{number}/reviews` API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubInlineCommentPayload {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_side: Option<String>,
    pub line: u32,
    pub side: String,
    pub body: String,
}

/// Renders the atomic GitHub review payload (`body` + hunk-validated `comments[]`) for a run.
pub fn build_github_review_artifacts(
    pr: &TrackedPrState,
    head_sha: &str,
    is_re_review: bool,
    diff_map: &ParsedDiffMap,
    newly_surfaced_codes: &[String],
    adjudication_notes: &[(String, ObservationStatus, String)],
) -> (String, Vec<GitHubInlineCommentPayload>) {
    let mut inline_comments = Vec::new();
    let mut out_of_hunk = Vec::new();

    for obs in &pr.observations {
        if !obs.status.is_open() {
            continue;
        }
        // On re-reviews, only attach fresh inline diff comments for newly introduced observations
        // (existing open observations are summarized in the compliance scorecard so we don't duplicate threads!).
        let should_post_inline = !is_re_review
            || newly_surfaced_codes
                .iter()
                .any(|code| code == &obs.obs_code);

        if !should_post_inline {
            continue;
        }

        let comment_body = format_inline_observation_body(obs);
        match diff_map.classify(&obs.path, obs.start_line, obs.end_line) {
            InlinePlacement::SingleLine { path, line } => {
                inline_comments.push(GitHubInlineCommentPayload {
                    path,
                    start_line: None,
                    start_side: None,
                    line,
                    side: "RIGHT".to_string(),
                    body: comment_body,
                });
            }
            InlinePlacement::MultiLine {
                path,
                start_line,
                line,
            } => {
                inline_comments.push(GitHubInlineCommentPayload {
                    path,
                    start_line: Some(start_line),
                    start_side: Some("RIGHT".to_string()),
                    line,
                    side: "RIGHT".to_string(),
                    body: comment_body,
                });
            }
            InlinePlacement::OutOfHunk { .. } => {
                out_of_hunk.push(obs);
            }
        }
    }

    let verdict = pr.compute_verdict();
    let open_obs = pr.open_observations();
    let short_sha = &head_sha[..head_sha.len().min(7)];
    let mode_label = if is_re_review {
        format!("Re-Review Compliance Check (Run #{})", pr.run_count)
    } else {
        "Initial Multi-Lens Review (Run #1)".to_string()
    };

    let mut summary = String::new();
    summary.push_str(BOT_COMMENT_MARKER);
    summary.push('\n');
    summary.push_str(&format!(
        "## Internal CI Code Review — `{}`\n\n",
        verdict.badge()
    ));
    summary.push_str(&format!(
        "- **Mode:** {} | **Pinned Commit:** `{}` | **Open Observations:** `{}` (`{}` blocking)\n",
        mode_label,
        short_sha,
        open_obs.len(),
        open_obs.iter().filter(|o| o.severity.is_blocking()).count()
    ));
    if let Some(url) = &pr.gdrive_doc_url {
        summary.push_str(&format!(
            "- **Agent Handoff Doc (Google Drive):** [Open Living PR Review Doc]({})\n",
            url
        ));
    }
    summary.push('\n');

    if let Some(exec_summary) = pr.latest_summary.as_ref().or(pr.initial_summary.as_ref()) {
        summary.push_str("### Summary\n");
        summary.push_str(exec_summary.trim());
        summary.push_str("\n\n");
    }

    if is_re_review && !adjudication_notes.is_empty() {
        summary.push_str("### Compliance & Answer Adjudication (vs Initial Review)\n\n");
        summary.push_str("| Observation | Outcome | Verification / Adjudication Note |\n");
        summary.push_str("| :--- | :--- | :--- |\n");
        for (code, status, note) in adjudication_notes {
            summary.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                code,
                status.label(),
                note.replace('|', "\\|").replace('\n', " ")
            ));
        }
        summary.push('\n');
    }

    if !pr.traceability.is_empty() {
        summary.push_str("### Ticket Traceability\n\n");
        summary.push_str("| ID | Criterion | Code | Tests | Status |\n");
        summary.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
        for c in &pr.traceability {
            summary.push_str(&format!(
                "| `{}` | {} | {} | {} | `{}` |\n",
                c.id,
                c.requirement.replace('|', "\\|"),
                c.code_evidence.replace('|', "\\|"),
                c.test_evidence.replace('|', "\\|"),
                c.status
            ));
        }
        summary.push('\n');
    }

    if !open_obs.is_empty() {
        summary.push_str("### Active Open Observations\n\n");
        summary.push_str("| ID | Severity | Brain | Location | Title |\n");
        summary.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
        for obs in &open_obs {
            let loc = match (obs.start_line, obs.end_line) {
                (Some(s), Some(e)) if s != e => format!("`{}:{}-{}`", obs.path, s, e),
                (_, Some(e)) => format!("`{}:{}`", obs.path, e),
                _ => format!("`{}`", obs.path),
            };
            summary.push_str(&format!(
                "| `{}` | **{}** (`{}`) | `{}` | {} | {} |\n",
                obs.obs_code,
                obs.severity.badge(),
                obs.confidence.as_str(),
                obs.brain_id,
                loc,
                obs.title.replace('|', "\\|")
            ));
        }
        summary.push('\n');
    }

    if !out_of_hunk.is_empty() {
        summary.push_str("### Cross-File / Out-of-Diff Observations\n\n");
        for obs in out_of_hunk {
            let loc = match obs.end_line {
                Some(line) => format!("`{}:{}`", obs.path, line),
                None => format!("`{}`", obs.path),
            };
            summary.push_str(&format!(
                "- **`{}` [{} | `{}`]** {} — {}\n  - **Fix:** {}\n",
                obs.obs_code,
                obs.severity.badge(),
                obs.brain_id,
                loc,
                obs.claim,
                obs.fix_guidance
            ));
        }
        summary.push('\n');
    }

    (summary, inline_comments)
}

fn format_inline_observation_body(obs: &ObservationRecord) -> String {
    let mut out = String::new();
    out.push_str(BOT_COMMENT_MARKER);
    out.push('\n');
    out.push_str(&format!(
        "**`{}` [{} · `{}` · `{}`]** — {}\n\n",
        obs.obs_code,
        obs.severity.badge(),
        obs.confidence.as_str(),
        obs.brain_id,
        obs.title
    ));
    out.push_str(obs.claim.trim());
    out.push_str("\n\n**Actionable Fix:**\n");
    out.push_str(obs.fix_guidance.trim());
    if let Some(suggestion) = &obs.suggested_code
        && !suggestion.trim().is_empty()
    {
        out.push_str("\n\n```suggestion\n");
        out.push_str(suggestion.trim_end_matches('\n'));
        out.push_str("\n```");
    }
    out
}

/// Renders the complete agent-actionable Markdown document synced to Google Drive (`gdocs` + `gdrive`)
/// and saved locally on M1 so any coding agent (Claude, Antigravity, Codex) can read it and fix every issue.
pub fn render_agent_drive_document(pr: &TrackedPrState) -> String {
    let verdict = pr.compute_verdict();
    let open_obs = pr.open_observations();
    let resolved_obs: Vec<&ObservationRecord> = pr
        .observations
        .iter()
        .filter(|o| !o.status.is_open())
        .collect();
    let sha = pr.last_reviewed_sha.as_deref().unwrap_or("unreviewed");

    let mut md = String::new();
    md.push_str(&format!(
        "# [{}] {} — PR #{}: {}\n\n",
        verdict.badge(),
        pr.repo,
        pr.pr_number,
        pr.title
    ));
    md.push_str("## Machine & Agent Metadata\n\n");
    md.push_str(&format!("- **Repository:** `{}`\n", pr.repo));
    md.push_str(&format!("- **PR Number:** `#{}`\n", pr.pr_number));
    md.push_str(&format!(
        "- **Branch:** `{}` -> `{}`\n",
        pr.head_ref, pr.base_ref
    ));
    md.push_str(&format!("- **Author:** `{}`\n", pr.author));
    md.push_str(&format!("- **Last Reviewed Commit SHA:** `{}`\n", sha));
    md.push_str(&format!("- **Review Run Count:** `{}`\n", pr.run_count));
    md.push_str(&format!(
        "- **Internal CI Verdict:** `{}`\n",
        verdict.badge()
    ));
    md.push_str(&format!(
        "- **Open Observations Remaining:** `{}`\n",
        open_obs.len()
    ));
    md.push_str(&format!(
        "- **Updated At (UTC):** `{}`\n\n",
        pr.updated_at.to_rfc3339()
    ));

    md.push_str("## Agent Instructions (Claude Code / Antigravity / Codex)\n\n");
    md.push_str(
        "If you are an AI coding agent assigned to address this PR review:\n\
         1. Work through **Section 1 (Open Observations — Agent Action Queue)** in severity order (`CRITICAL` -> `HIGH` -> `MEDIUM` -> `LOW`).\n\
         2. Verify each fix with the repository's configured local verification gate before pushing.\n\
         3. If an observation reflects an intentional architectural decision or tracked deferral, reply on the PR citing the invariant or follow-up issue `#N`; the next automatic re-review will adjudicate your answer.\n\
         4. Do **not** regress items already listed in **Section 2 (Resolved & Accepted Ledger)**.\n\n",
    );

    if let Some(initial) = &pr.initial_summary {
        md.push_str("## Initial Review Baseline Summary\n\n");
        md.push_str(initial.trim());
        md.push_str("\n\n");
    }

    if pr.run_count > 1
        && let Some(latest) = &pr.latest_summary
    {
        md.push_str(&format!(
            "## Latest Re-Review Summary (Run #{})\n\n",
            pr.run_count
        ));
        md.push_str(latest.trim());
        md.push_str("\n\n");
    }

    md.push_str("## 1. Open Observations — Agent Action Queue\n\n");
    if open_obs.is_empty() {
        md.push_str("_Zero open observations. All checks and observations are satisfied at the current commit SHA._\n\n");
    } else {
        for obs in open_obs {
            let loc = match (obs.start_line, obs.end_line) {
                (Some(s), Some(e)) if s != e => format!("{}:{}-{}", obs.path, s, e),
                (_, Some(e)) => format!("{}:{}", obs.path, e),
                _ => obs.path.clone(),
            };
            md.push_str(&format!(
                "### {} — [{} | {} | {}] {}\n\n",
                obs.obs_code,
                obs.severity.badge(),
                obs.confidence.as_str(),
                obs.brain_id,
                obs.title
            ));
            md.push_str(&format!("- **Location:** `{}`\n", loc));
            md.push_str(&format!(
                "- **First Flagged:** Run #{} (`{}`)\n",
                obs.created_in_run,
                &obs.created_sha[..obs.created_sha.len().min(7)]
            ));
            md.push_str(&format!("- **Problem:** {}\n", obs.claim.trim()));
            md.push_str(&format!(
                "- **Required Fix / Verification:** {}\n",
                obs.fix_guidance.trim()
            ));
            if let Some(note) = &obs.resolution_note
                && !note.trim().is_empty()
            {
                md.push_str(&format!("- **Latest Re-Review Note:** {}\n", note.trim()));
            }
            if let Some(code) = &obs.suggested_code
                && !code.trim().is_empty()
            {
                md.push_str("\n```ts\n");
                md.push_str(code.trim_end_matches('\n'));
                md.push_str("\n```\n");
            }
            md.push('\n');
        }
    }

    md.push_str("## 2. Resolved & Accepted Compliance Ledger\n\n");
    if resolved_obs.is_empty() {
        md.push_str("_No resolved observations yet._\n\n");
    } else {
        md.push_str("| ID | Severity | Brain | Location | Status | Resolution Note |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");
        for obs in resolved_obs {
            let loc = match obs.end_line {
                Some(e) => format!("`{}:{}`", obs.path, e),
                None => format!("`{}`", obs.path),
            };
            md.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | `{}` | {} |\n",
                obs.obs_code,
                obs.severity.badge(),
                obs.brain_id,
                loc,
                obs.status.label(),
                obs.resolution_note
                    .as_deref()
                    .unwrap_or("-")
                    .replace('|', "\\|")
                    .replace('\n', " ")
            ));
        }
        md.push('\n');
    }

    if !pr.traceability.is_empty() {
        md.push_str("## 3. Ticket-to-Code Traceability Matrix\n\n");
        md.push_str("| Criterion | Requirement | Code Evidence | Test Evidence | Status |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
        for c in &pr.traceability {
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | `{}` |\n",
                c.id,
                c.requirement.replace('|', "\\|"),
                c.code_evidence.replace('|', "\\|"),
                c.test_evidence.replace('|', "\\|"),
                c.status
            ));
        }
        md.push('\n');
    }

    md
}

pub struct StatusBannerContext<'a> {
    pub run_number: u32,
    pub mode_label: &'a str,
    pub head_sha: &'a str,
    pub reviewer_account: &'a str,
    pub runners_label: &'a str,
    pub started_at_utc: &'a str,
}

pub struct CompletedBannerOutcome<'a> {
    pub verdict: InternalVerdict,
    pub open_count: usize,
    pub duration_secs: u64,
    pub review_url: &'a str,
    pub drive_url: Option<&'a str>,
}

/// Formats an upfront queue notice when multiple open PRs trigger in the same poll batch,
/// upgraded in place to `⏳ Triggered & Running` as soon as its turn begins.
pub fn format_queued_status_banner(
    ctx: &StatusBannerContext<'_>,
    position: usize,
    total_in_batch: usize,
) -> String {
    let short_sha = &ctx.head_sha[..ctx.head_sha.len().min(7)];
    format!(
        "{BOT_COMMENT_MARKER}\n\
         {STATUS_BANNER_MARKER}\n\
         ### 🗂️ Internal CI Code Review — Queued on `otto-mac` (M1)\n\
         - **Run:** `#{run}` (`{mode}`)\n\
         - **Pinned Commit:** `{sha}`\n\
         - **Reviewer Identity:** `{reviewer}`\n\
         - **Queue Position:** `#{pos}` of `{total}` in current poll batch (`{queued_at}`)\n\n\
         _Caught by `otto-mac` — this banner will switch to **⏳ Running** automatically as soon as the preceding PR finishes._",
        run = ctx.run_number,
        mode = ctx.mode_label,
        sha = short_sha,
        reviewer = ctx.reviewer_account,
        pos = position,
        total = total_in_batch,
        queued_at = ctx.started_at_utc,
    )
}

/// Formats the immediate pickup comment posted on GitHub the moment an async M1 worker session starts.
pub fn format_running_status_banner(ctx: &StatusBannerContext<'_>) -> String {
    let short_sha = &ctx.head_sha[..ctx.head_sha.len().min(7)];
    format!(
        "{BOT_COMMENT_MARKER}\n\
         {STATUS_BANNER_MARKER}\n\
         ### ⏳ Internal CI Code Review — Triggered & Running (`otto-mac` M1)\n\
         - **Run:** `#{run}` (`{mode}`)\n\
         - **Pinned Commit:** `{sha}`\n\
         - **Reviewer Identity:** `{reviewer}` • **AI Runners:** `{runners}`\n\
         - **Started At (UTC):** `{started}`\n\n\
         _Running in an independent async session on `otto-mac` (parallel multi-lens wave). Full review + inline comments will post automatically once verification finishes._",
        run = ctx.run_number,
        mode = ctx.mode_label,
        sha = short_sha,
        reviewer = ctx.reviewer_account,
        runners = ctx.runners_label,
        started = ctx.started_at_utc,
    )
}

/// Updates the pickup banner comment in place once the atomic PR review has been posted.
pub fn format_completed_status_banner(
    ctx: &StatusBannerContext<'_>,
    outcome: &CompletedBannerOutcome<'_>,
) -> String {
    let short_sha = &ctx.head_sha[..ctx.head_sha.len().min(7)];
    let mins = outcome.duration_secs / 60;
    let secs = outcome.duration_secs % 60;
    let duration_str = if mins > 0 {
        format!("{}m {:02}s", mins, secs)
    } else {
        format!("{}s", secs)
    };

    let mut out = format!(
        "{BOT_COMMENT_MARKER}\n\
         {STATUS_BANNER_MARKER}\n\
         ### ✅ Internal CI Code Review — Completed (`otto-mac` M1)\n\
         - **Run:** `#{run}` (`{mode}`)\n\
         - **Reviewed Commit:** `{sha}` • **Verdict:** **`{verdict}`** (`{open}` open observation(s)) • **Duration:** `{dur}`\n\
         - **Published Review:** {review_url}",
        run = ctx.run_number,
        mode = ctx.mode_label,
        sha = short_sha,
        verdict = outcome.verdict.badge(),
        open = outcome.open_count,
        dur = duration_str,
        review_url = outcome.review_url,
    );

    if let Some(drive) = outcome.drive_url
        && !drive.is_empty()
    {
        out.push_str(&format!(
            "\n- **Google Drive Agent Handoff:** [Open PR Review Doc]({})",
            drive
        ));
    }

    out
}

/// Updates the pickup banner comment if a newer commit was pushed while M1 was mid-analysis.
pub fn format_superseded_status_banner(
    ctx: &StatusBannerContext<'_>,
    new_head_sha: &str,
) -> String {
    let old_sha = &ctx.head_sha[..ctx.head_sha.len().min(7)];
    let new_sha = &new_head_sha[..new_head_sha.len().min(7)];
    format!(
        "{BOT_COMMENT_MARKER}\n\
         {STATUS_BANNER_MARKER}\n\
         ### 🔄 Internal CI Code Review — Superseded by Newer Push (`otto-mac` M1)\n\
         - **Interrupted Run:** `#{run}` at `{old_sha}`\n\
         - **New Head Commit:** `{new_sha}` — queued automatically for the next `2m` poll tick so inline comments align with the latest diff.",
        run = ctx.run_number,
    )
}

/// Updates the pickup banner comment if a transient CLI/network error defers completion to the next 2m poll tick.
pub fn format_interrupted_status_banner(ctx: &StatusBannerContext<'_>, reason: &str) -> String {
    let short_sha = &ctx.head_sha[..ctx.head_sha.len().min(7)];
    let clean_reason: String = reason.chars().take(180).collect();
    format!(
        "{BOT_COMMENT_MARKER}\n\
         {STATUS_BANNER_MARKER}\n\
         ### ⚠️ Internal CI Code Review — Queued for Retry (`otto-mac` M1)\n\
         - **Run:** `#{run}` at `{sha}`\n\
         - **Status:** Transient interruption (`{reason}`) — `otto-mac` will resume automatically on the next `2m` poll tick using this banner.",
        run = ctx.run_number,
        sha = short_sha,
        reason = clean_reason.replace('\n', " "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_banners_carry_anti_loop_markers_and_never_retrigger() {
        let ctx = StatusBannerContext {
            run_number: 1,
            mode_label: "INITIAL_FULL — 7 specialist lenses + adversarial verifier",
            head_sha: "8521991abcdef",
            reviewer_account: "alice-dev",
            runners_label: "claude ⇄ agy",
            started_at_utc: "2026-11-11T17:05:12Z",
        };
        let queued = format_queued_status_banner(&ctx, 2, 3);
        assert!(queued.contains(BOT_COMMENT_MARKER));
        assert!(queued.contains(STATUS_BANNER_MARKER));
        assert!(queued.contains("Queued on `otto-mac` (M1)"));

        let running = format_running_status_banner(&ctx);
        assert!(running.contains(BOT_COMMENT_MARKER));
        assert!(running.contains(STATUS_BANNER_MARKER));
        assert!(running.contains("Triggered & Running"));
        assert!(running.contains("`8521991`"));

        let completed = format_completed_status_banner(
            &ctx,
            &CompletedBannerOutcome {
                verdict: InternalVerdict::Pass,
                open_count: 2,
                duration_secs: 245,
                review_url: "https://github.com/acme-org/web-app/pull/42#pullrequestreview-1",
                drive_url: Some("https://docs.google.com/document/d/abc/edit"),
            },
        );
        assert!(completed.contains(BOT_COMMENT_MARKER));
        assert!(completed.contains("Completed (`otto-mac` M1)"));
        assert!(completed.contains("4m 05s"));

        // Verify evaluate_pr_trigger ignores status banner comments from the same account
        let mut tracked = TrackedPrState::new(
            "acme-org/web-app",
            42,
            "feat: test",
            "alice-dev",
            "main",
            "feat/test",
        );
        tracked.initial_review_done = true;
        tracked.run_count = 1;
        tracked.last_reviewed_sha = Some("8521991abcdef".to_string());

        let snap = OpenPrSnapshot {
            pr_number: 42,
            title: "feat: test".to_string(),
            author: "alice-dev".to_string(),
            is_draft: false,
            base_ref: "main".to_string(),
            head_ref: "feat/test".to_string(),
            head_sha: "8521991abcdef".to_string(),
            latest_committer: Some("alice-dev".to_string()),
            human_replies: vec![HumanReplyItem {
                author: "alice-dev".to_string(),
                created_at: "2026-11-11T17:05:12Z".to_string(),
                path: None,
                line: None,
                body: running,
            }],
        };
        assert_eq!(
            evaluate_pr_trigger(Some(&tracked), &snap),
            TriggerDecision::UpToDate
        );
    }

    #[test]
    fn trigger_machine_handles_initial_push_and_non_bot_replies() {
        let mut snap = OpenPrSnapshot {
            pr_number: 17,
            title: "docs: correct CI status".to_string(),
            author: "alice-dev".to_string(),
            is_draft: false,
            base_ref: "main".to_string(),
            head_ref: "docs/ci-status".to_string(),
            head_sha: "aaaa1111".to_string(),
            latest_committer: Some("alice-dev".to_string()),
            human_replies: vec![],
        };

        // 1. Fresh unseen PR triggers InitialFullReview
        assert_eq!(
            evaluate_pr_trigger(None, &snap),
            TriggerDecision::InitialFullReview {
                head_sha: "aaaa1111".to_string()
            }
        );

        let mut tracked = TrackedPrState::new(
            "acme-org/web-app",
            17,
            &snap.title,
            &snap.author,
            &snap.base_ref,
            &snap.head_ref,
        );
        tracked.initial_review_done = true;
        tracked.run_count = 1;
        tracked.last_reviewed_sha = Some("aaaa1111".to_string());
        tracked.last_processed_reply_at = Some("2026-11-11T10:00:00Z".to_string());

        // 2. Bot's own review comment (carrying BOT_COMMENT_MARKER) NEVER loops!
        snap.human_replies.push(HumanReplyItem {
            author: "alice-dev".to_string(),
            created_at: "2026-11-11T10:01:00Z".to_string(),
            path: None,
            line: None,
            body: format!("{}\n## Internal CI Code Review", BOT_COMMENT_MARKER),
        });
        assert_eq!(
            evaluate_pr_trigger(Some(&tracked), &snap),
            TriggerDecision::UpToDate
        );

        // 3. Human reply from same author (without marker!) triggers ReReview even without new push
        snap.human_replies.push(HumanReplyItem {
            author: "alice-dev".to_string(),
            created_at: "2026-11-11T10:05:00Z".to_string(),
            path: Some("CLAUDE.md".to_string()),
            line: Some(27),
            body: "Intentional deferral tracked in issue #34.".to_string(),
        });
        assert!(matches!(
            evaluate_pr_trigger(Some(&tracked), &snap),
            TriggerDecision::ReReview {
                sha_changed: false,
                ..
            }
        ));

        // 4. Subsequent push triggers ReReview with sha_changed = true
        snap.head_sha = "bbbb2222".to_string();
        assert!(matches!(
            evaluate_pr_trigger(Some(&tracked), &snap),
            TriggerDecision::ReReview {
                sha_changed: true,
                ..
            }
        ));
    }

    #[test]
    fn exclusive_ledger_lock_preserves_concurrent_async_pr_updates() {
        let tmp_dir =
            std::env::temp_dir().join(format!("code-reviewer-lock-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        std::thread::scope(|s| {
            for pr_num in 1..=6u64 {
                let dir_ref = &tmp_dir;
                s.spawn(move || {
                    LedgerStore::with_exclusive_lock(dir_ref, |store| {
                        store.provider_dispatch_counter += 1;
                        let key = LedgerStore::pr_key("acme-org/web-app", pr_num);
                        store.pull_requests.insert(
                            key,
                            TrackedPrState::new(
                                "acme-org/web-app",
                                pr_num,
                                &format!("PR #{}", pr_num),
                                "bob-reviewer",
                                "main",
                                &format!("feat/{}", pr_num),
                            ),
                        );
                    })
                    .unwrap();
                });
            }
        });

        let loaded = LedgerStore::load_or_default(&tmp_dir).unwrap();
        assert_eq!(loaded.provider_dispatch_counter, 6);
        assert_eq!(loaded.pull_requests.len(), 6);
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
