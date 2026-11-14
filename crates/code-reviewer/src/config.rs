//! Configuration schema, watched repository registry, and dual-account (`primary` / `secondary`) routing.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config at {path}: {source}")]
    IoRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write config at {path}: {source}")]
    IoWrite {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse TOML config at {path}: {source}")]
    TomlParse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    #[error("failed to serialize TOML config: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("invalid repository full name '{0}': expected '<owner>/<repo>'")]
    InvalidRepoName(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountPolicy {
    /// Dynamically swap between `primary` and `secondary` so committer != reviewer.
    Swap,
    /// Force a specific GitHub account (used when only one account has organization access).
    Forced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewEventPolicy {
    /// Always submit `COMMENT` reviews (with inline diff comments + explicit verdict header).
    CommentOnly,
    /// Submit `APPROVE` / `REQUEST_CHANGES` when reviewer != PR author, falling back to `COMMENT` on self-PRs.
    FormalVerdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliRunner {
    Claude,
    Agy,
}

impl CliRunner {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Agy => "agy",
        }
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::Claude => Self::Agy,
            Self::Agy => Self::Claude,
        }
    }

    pub fn from_slug(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("claude") {
            Some(Self::Claude)
        } else if raw.eq_ignore_ascii_case("agy") || raw.eq_ignore_ascii_case("antigravity") {
            Some(Self::Agy)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountsConfig {
    pub primary: String,
    pub secondary: String,
    pub tokens_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    pub primary_runner: CliRunner,
    pub verifier_runner: CliRunner,
    #[serde(default = "default_true")]
    pub allow_single_runner_fallback: bool,
    /// Intercalate (`claude` <-> `agy`) across parallel PR sessions and across specialist brains to split subscription usage 50/50.
    #[serde(default = "default_true")]
    pub intercalate_providers: bool,
    /// Maximum number of Pull Requests reviewed concurrently in independent async worker sessions (`1..=12`).
    #[serde(default = "default_max_parallel_prs")]
    pub max_parallel_prs: usize,
    /// Maximum concurrent specialist brains per PR (`1..=8`; `8` runs all 7 specialist lenses in a single simultaneous wave).
    #[serde(default = "default_max_parallel_brains")]
    pub max_parallel_brains_per_pr: usize,
    /// Fast Claude model alias/ID (`"sonnet"`, `"opus"`, `"haiku"`, etc.).
    #[serde(default = "default_claude_model")]
    pub claude_model: String,
    /// Optional Antigravity model override (`""` uses CLI default).
    #[serde(default)]
    pub agy_model: String,
    /// Reasoning effort passed to `claude` and `agy` (`"low"`, `"medium"`, `"high"`).
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String,
    /// When `true`, the 2-minute poller ONLY executes pure GraphQL polling (0 AI tokens ever) until a PR is manually triggered or `poll_only_mode` is set back to `false`.
    #[serde(default)]
    pub poll_only_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenderDeliveryMode {
    /// Automatically commit fixes, push to the PR branch, reply on review threads, and optionally resolve them.
    #[default]
    AutoPush,
    /// Stage commits/diff patch & draft replies locally so an operator can review & push in 1 click from the Code Tender UI.
    StagePatchOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenderConfig {
    /// Enable the autonomous Code Tender feedback-handling subsystem.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// When `true`, `tend-poll` only performs 0-AI GraphQL polling without spawning AI fix agents.
    #[serde(default)]
    pub poll_only_mode: bool,
    /// Whether verified fixes are pushed automatically or staged as a one-click patch in the UI.
    #[serde(default)]
    pub delivery_mode: TenderDeliveryMode,
    /// Intercalate (`claude` <-> `agy`) across tender sessions and parallel fixer packets.
    #[serde(default = "default_true")]
    pub intercalate_providers: bool,
    /// Maximum number of PRs tended concurrently in independent async worker sessions (`1..=12`).
    #[serde(default = "default_tender_parallel_prs")]
    pub max_parallel_prs: usize,
    /// Maximum concurrent domain fixer workers per PR (`1..=8`).
    #[serde(default = "default_tender_parallel_fixers")]
    pub max_parallel_fixers_per_pr: usize,
    /// Maximum autonomous Code Tender fix cycles on the same PR before requiring manual confirmation (`1..=10`).
    #[serde(default = "default_tender_max_cycles")]
    pub max_autonomous_cycles_per_pr: u32,
    /// Resolve GitHub inline review threads automatically when Code Tender pushes a verified fix or answer.
    #[serde(default = "default_true")]
    pub resolve_threads_on_fix: bool,
    /// Ignore clean `PASS` Multi-Lens Code Reviewer summaries unless open/unresolved review threads remain.
    #[serde(default = "default_true")]
    pub ignore_clean_pass_reviews: bool,
    /// Restrict automatic tending to PRs authored by these GitHub logins (`["*"]` allows any non-fork PR author; empty defaults to `[accounts.primary, accounts.secondary]`).
    #[serde(default)]
    pub allowed_authors: Vec<String>,
}

impl Default for TenderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_only_mode: false,
            delivery_mode: TenderDeliveryMode::AutoPush,
            intercalate_providers: true,
            max_parallel_prs: default_tender_parallel_prs(),
            max_parallel_fixers_per_pr: default_tender_parallel_fixers(),
            max_autonomous_cycles_per_pr: default_tender_max_cycles(),
            resolve_threads_on_fix: true,
            ignore_clean_pass_reviews: true,
            allowed_authors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleDriveConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub root_folder_id: String,
    pub living_docs_folder_id: String,
    pub gdrive_bin: String,
    pub gdocs_bin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedRepoConfig {
    pub full_name: String,
    #[serde(default = "default_branch_main")]
    pub default_branch: String,
    #[serde(default = "default_lang_en")]
    pub language: String,
    pub account_policy: AccountPolicy,
    #[serde(default)]
    pub forced_account: Option<String>,
    pub review_event_policy: ReviewEventPolicy,
    #[serde(default)]
    pub gdrive_folder_id: Option<String>,
    /// Optional local app checkout directory (e.g. `~/apps/web-app`) so Code Tender inherits local dependencies, context docs, and build caches.
    #[serde(default)]
    pub local_repo_path: Option<String>,
    #[serde(default)]
    pub local_check_cmd: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub tender_enabled: bool,
    /// Per-repository allowlist of PR author GitHub logins eligible for Code Tender.
    /// When non-empty, overrides global `tender.allowed_authors` and blocks tending any PR authored by someone else.
    #[serde(default)]
    pub tender_allowed_authors: Vec<String>,
    /// Optional per-repository allowlist of PR author GitHub logins eligible for internal `poll` reviews (`[]` = all authors).
    #[serde(default)]
    pub review_allowed_authors: Vec<String>,
    /// When `true`, internal `poll` skips cross-repository fork PRs (`isCrossRepository == true`).
    #[serde(default)]
    pub skip_fork_prs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(default = "default_state_dir")]
    pub state_dir: String,
    pub accounts: AccountsConfig,
    pub agents: AgentsConfig,
    #[serde(default)]
    pub tender: TenderConfig,
    pub gdrive: GoogleDriveConfig,
    #[serde(default)]
    pub repos: Vec<WatchedRepoConfig>,
}

fn default_true() -> bool {
    true
}

fn default_max_parallel_prs() -> usize {
    8
}

fn default_max_parallel_brains() -> usize {
    8
}

fn default_tender_parallel_prs() -> usize {
    4
}

fn default_tender_parallel_fixers() -> usize {
    4
}

fn default_tender_max_cycles() -> u32 {
    3
}

fn default_claude_model() -> String {
    "sonnet".to_string()
}

fn default_reasoning_effort() -> String {
    "medium".to_string()
}

fn default_branch_main() -> String {
    "main".to_string()
}

fn default_lang_en() -> String {
    "en".to_string()
}

fn default_state_dir() -> String {
    "~/.code-reviewer".to_string()
}

impl WorkspaceConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let effective_path = if path.exists() {
            path.to_path_buf()
        } else if let Some(parent) = path.parent() {
            let example = parent.join("repos.example.toml");
            if example.exists() {
                example
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };

        let raw = fs::read_to_string(&effective_path).map_err(|source| ConfigError::IoRead {
            path: effective_path.clone(),
            source,
        })?;
        toml::from_str(&raw).map_err(|source| ConfigError::TomlParse {
            path: effective_path,
            source: Box::new(source),
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigError::IoWrite {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let body = toml::to_string_pretty(self)?;
        fs::write(path, body).map_err(|source| ConfigError::IoWrite {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Selects which GitHub account (`primary` or `secondary`) performs the review.
    ///
    /// - Under `AccountPolicy::Forced`, always returns `forced_account` (or `accounts.primary`).
    /// - Under `AccountPolicy::Swap`, inspects `latest_committer` (falling back to `pr_author`):
    ///   if `primary` committed, `secondary` reviews; if `secondary` committed, `primary` reviews.
    pub fn select_reviewer_account<'a>(
        &'a self,
        repo: &'a WatchedRepoConfig,
        pr_author: &str,
        latest_committer: Option<&str>,
    ) -> &'a str {
        match repo.account_policy {
            AccountPolicy::Forced => repo
                .forced_account
                .as_deref()
                .unwrap_or(self.accounts.primary.as_str()),
            AccountPolicy::Swap => {
                let actor = latest_committer.unwrap_or(pr_author);
                if actor.eq_ignore_ascii_case(&self.accounts.primary) {
                    self.accounts.secondary.as_str()
                } else {
                    self.accounts.primary.as_str()
                }
            }
        }
    }

    /// Selects which GitHub account (`primary` or `secondary`) Code Tender pushes commits and replies as.
    ///
    /// Preserves dual-account separation by matching `pr_author` when possible (falling back to
    /// `forced_account` or `accounts.primary`), so Multi-Lens Reviewer continues reviewing with the peer account.
    pub fn select_tender_account<'a>(
        &'a self,
        repo: &'a WatchedRepoConfig,
        pr_author: &str,
    ) -> &'a str {
        if pr_author.eq_ignore_ascii_case(&self.accounts.secondary) {
            return self.accounts.secondary.as_str();
        }
        if pr_author.eq_ignore_ascii_case(&self.accounts.primary) {
            return self.accounts.primary.as_str();
        }
        repo.forced_account
            .as_deref()
            .unwrap_or(self.accounts.primary.as_str())
    }

    /// Returns `true` when `pr_author` is permitted for automatic Code Tender handling.
    ///
    /// - Rejects empty or `"unknown"` author logins (fail-closed).
    /// - When `repo.tender_allowed_authors` is non-empty, enforces that per-repository allowlist strictly
    ///   (overriding global `tender.allowed_authors` and default accounts).
    /// - Empty `tender.allowed_authors` safely defaults to `[accounts.primary, accounts.secondary]` (plus
    ///   `repo.forced_account` when set) so Code Tender never unexpectedly pushes to external contributors' PRs.
    pub fn is_tender_author_allowed(&self, repo: &WatchedRepoConfig, pr_author: &str) -> bool {
        let author = pr_author.trim();
        if author.is_empty() || author.eq_ignore_ascii_case("unknown") {
            return false;
        }
        if !repo.tender_allowed_authors.is_empty() {
            if repo.tender_allowed_authors.iter().any(|a| a.trim() == "*") {
                return true;
            }
            return repo
                .tender_allowed_authors
                .iter()
                .any(|a| a.trim().eq_ignore_ascii_case(author));
        }
        if self.tender.allowed_authors.iter().any(|a| a.trim() == "*") {
            return true;
        }
        if !self.tender.allowed_authors.is_empty() {
            return self
                .tender
                .allowed_authors
                .iter()
                .any(|a| a.trim().eq_ignore_ascii_case(author));
        }
        author.eq_ignore_ascii_case(&self.accounts.primary)
            || author.eq_ignore_ascii_case(&self.accounts.secondary)
            || repo
                .forced_account
                .as_deref()
                .is_some_and(|f| f.trim().eq_ignore_ascii_case(author))
    }

    /// Returns `true` when `pr_author` is permitted for automated `code-reviewer poll` runs on `repo`.
    pub fn is_review_author_allowed(&self, repo: &WatchedRepoConfig, pr_author: &str) -> bool {
        let _ = self;
        if repo.review_allowed_authors.is_empty()
            || repo.review_allowed_authors.iter().any(|a| a.trim() == "*")
        {
            return true;
        }
        let author = pr_author.trim();
        if author.is_empty() || author.eq_ignore_ascii_case("unknown") {
            return false;
        }
        repo.review_allowed_authors
            .iter()
            .any(|a| a.trim().eq_ignore_ascii_case(author))
    }

    /// Selects the GitHub Pull Request Review API `event` string (`"COMMENT"`, `"APPROVE"`, or `"REQUEST_CHANGES"`).
    ///
    /// Guarantees GitHub never returns `HTTP 422 Unprocessable Entity` for self-review.
    pub fn select_github_review_event(
        repo: &WatchedRepoConfig,
        reviewer_account: &str,
        pr_author: &str,
        has_blocking_findings: bool,
    ) -> &'static str {
        if repo.review_event_policy == ReviewEventPolicy::CommentOnly
            || reviewer_account.eq_ignore_ascii_case(pr_author)
        {
            "COMMENT"
        } else if has_blocking_findings {
            "REQUEST_CHANGES"
        } else {
            "APPROVE"
        }
    }

    pub fn upsert_repo(&mut self, mut entry: WatchedRepoConfig) -> Result<(), ConfigError> {
        entry.full_name = normalize_repo_input(&entry.full_name)?;
        if let Some(existing) = self
            .repos
            .iter_mut()
            .find(|r| r.full_name.eq_ignore_ascii_case(&entry.full_name))
        {
            *existing = entry;
        } else {
            self.repos.push(entry);
        }
        Ok(())
    }

    pub fn remove_repo(&mut self, full_name: &str) -> bool {
        let before = self.repos.len();
        self.repos
            .retain(|r| !r.full_name.eq_ignore_ascii_case(full_name));
        self.repos.len() != before
    }
}

/// Accepts `owner/repo`, `https://github.com/owner/repo(.git)`, `git@github.com:owner/repo(.git)`,
/// or `ssh://git@github.com/owner/repo(.git)` and returns canonical `owner/repo`.
pub fn normalize_repo_input(raw: &str) -> Result<String, ConfigError> {
    let mut s = raw.trim();
    if let Some((before_q, _)) = s.split_once('?') {
        s = before_q;
    }
    if let Some((before_hash, _)) = s.split_once('#') {
        s = before_hash;
    }
    s = s.trim_end_matches('/');

    if let Some((_, rest)) = s.split_once("://") {
        // https://github.com/owner/repo or ssh://git@github.com/owner/repo
        if let Some((_, path)) = rest.split_once('/') {
            s = path;
        }
    } else if s.contains(':') && !s.starts_with('/') {
        // git@github.com:owner/repo.git
        if let Some((_, path)) = s.split_once(':') {
            s = path;
        }
    } else if let Some(rest) = s
        .strip_prefix("github.com/")
        .or_else(|| s.strip_prefix("www.github.com/"))
    {
        s = rest;
    }

    // If a full PR/tree URL was pasted (e.g. https://github.com/owner/repo/pull/12), keep first 2 segments
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 2 {
        let owner = parts[0].trim();
        let repo = parts[1].trim();
        let repo = repo.strip_suffix(".git").unwrap_or(repo);
        let candidate = format!("{}/{}", owner, repo);
        validate_repo_slug(&candidate)?;
        return Ok(candidate);
    }

    let stripped = s.strip_suffix(".git").unwrap_or(s);
    validate_repo_slug(stripped)?;
    Ok(stripped.to_string())
}

pub fn validate_repo_slug(slug: &str) -> Result<(&str, &str), ConfigError> {
    let trimmed = slug.trim();
    let mut parts = trimmed.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) if !owner.is_empty() && !repo.is_empty() => {
            Ok((owner, repo))
        }
        _ => Err(ConfigError::InvalidRepoName(slug.to_string())),
    }
}

pub fn repo_fs_slug(full_name: &str) -> String {
    full_name.trim().replace('/', "__")
}

pub fn expand_tilde(raw: &str) -> PathBuf {
    if raw == "~" {
        return home_dir();
    }
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        return home_dir().join(rest);
    }
    PathBuf::from(raw)
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Resolves the private runtime Knowledge Base overlay root outside git-tracked directories.
///
/// - For real `code-reviewer` workspaces (`config/repos.toml` or `config/repos.example.toml` present),
///   resolves `<state_dir>/kb` (defaulting to `~/.code-reviewer/kb`) so runtime lessons/guardrails
///   survive `git checkout -B main otto-sync` and never leak private repository names into public commits.
/// - For isolated temporary directories in unit tests, resolves `<workspace_root>/.runtime-kb` so tests
///   remain hermetic and never touch the operator's live `~/.code-reviewer/kb`.
pub fn runtime_kb_root(workspace_root: &Path) -> PathBuf {
    let config_dir = workspace_root.join("config");
    if config_dir.join("repos.toml").exists() || config_dir.join("repos.example.toml").exists() {
        if let Ok(cfg) = WorkspaceConfig::load(&config_dir.join("repos.toml")) {
            return expand_tilde(&cfg.state_dir).join("kb");
        }
        return expand_tilde("~/.code-reviewer").join("kb");
    }
    workspace_root.join(".runtime-kb")
}

/// Merges a committed starter KB markdown file with a private runtime KB overlay, deduplicating headings and bullets.
pub fn merge_kb_markdown(committed: &str, runtime_overlay: &str) -> String {
    let base_trimmed = committed.trim_end();
    if runtime_overlay.trim().is_empty() {
        return if base_trimmed.is_empty() {
            String::new()
        } else {
            format!("{base_trimmed}\n")
        };
    }
    if base_trimmed.is_empty() {
        return format!("{}\n", runtime_overlay.trim_end());
    }

    let existing_norm: std::collections::BTreeSet<String> = base_trimmed
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches("- ")
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|l| !l.is_empty())
        .collect();

    let mut extra_lines = Vec::new();
    for line in runtime_overlay.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let norm = t.trim_start_matches("- ").trim().to_ascii_lowercase();
        if existing_norm.contains(&norm) {
            continue;
        }
        extra_lines.push(line);
    }

    if extra_lines.is_empty() {
        format!("{base_trimmed}\n")
    } else {
        format!("{base_trimmed}\n{}\n", extra_lines.join("\n"))
    }
}

/// Strips lines already present in `committed` before persisting operator edits to the private runtime KB overlay.
pub fn strip_committed_kb_lines(committed: &str, edited_full: &str) -> String {
    let base_norm: std::collections::BTreeSet<String> = committed
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches("- ")
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|l| !l.is_empty())
        .collect();

    let kept: Vec<&str> = edited_full
        .lines()
        .filter(|line| {
            let t = line.trim();
            if t.is_empty() {
                return false;
            }
            let norm = t.trim_start_matches("- ").trim().to_ascii_lowercase();
            !base_norm.contains(&norm)
        })
        .collect();
    if kept.is_empty() {
        String::new()
    } else {
        format!("{}\n", kept.join("\n"))
    }
}

/// Parses a GitHub Pull Request URL (`https://github.com/owner/repo/pull/123`) or
/// shorthand (`owner/repo#123`) into `(normalized_repo_slug, pr_number)`.
pub fn parse_pr_reference_input(raw: &str) -> Result<(String, u64), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(
            "enter a GitHub PR URL (`https://github.com/owner/repo/pull/123`) or `owner/repo#123`"
                .to_string(),
        );
    }

    for marker in ["/pull/", "/pulls/"] {
        if let Some((left, right)) = trimmed.split_once(marker) {
            let repo = normalize_repo_input(left).map_err(|e| e.to_string())?;
            let digits: String = right.chars().take_while(|c| c.is_ascii_digit()).collect();
            let pr = digits
                .parse::<u64>()
                .ok()
                .filter(|&n| n > 0)
                .ok_or_else(|| format!("invalid pull request number in `{trimmed}`"))?;
            return Ok((repo, pr));
        }
    }

    if let Some((left, right)) = trimmed.rsplit_once('#') {
        let repo = normalize_repo_input(left.trim()).map_err(|e| e.to_string())?;
        let digits: String = right
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let pr = digits
            .parse::<u64>()
            .ok()
            .filter(|&n| n > 0)
            .ok_or_else(|| format!("invalid pull request number in `{trimmed}`"))?;
        return Ok((repo, pr));
    }

    Err(
        "expected a GitHub PR URL (`https://github.com/owner/repo/pull/123`) or `owner/repo#123`"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> WorkspaceConfig {
        WorkspaceConfig {
            state_dir: "~/.code-reviewer".to_string(),
            accounts: AccountsConfig {
                primary: "alice-dev".to_string(),
                secondary: "bob-reviewer".to_string(),
                tokens_dir: "~/.code-reviewer/tokens".to_string(),
            },
            agents: AgentsConfig {
                primary_runner: CliRunner::Claude,
                verifier_runner: CliRunner::Agy,
                allow_single_runner_fallback: true,
                intercalate_providers: true,
                max_parallel_prs: 8,
                max_parallel_brains_per_pr: 8,
                claude_model: "sonnet".to_string(),
                agy_model: String::new(),
                reasoning_effort: "medium".to_string(),
                poll_only_mode: false,
            },
            tender: TenderConfig::default(),
            gdrive: GoogleDriveConfig {
                enabled: true,
                root_folder_id: "root123".to_string(),
                living_docs_folder_id: "docs123".to_string(),
                gdrive_bin: "gdrive".to_string(),
                gdocs_bin: "gdocs".to_string(),
            },
            repos: vec![],
        }
    }

    #[test]
    fn swap_policy_picks_opposite_reviewer() {
        let cfg = sample_config();
        let shared_repo = WatchedRepoConfig {
            full_name: "cokitron/cheap-otto".to_string(),
            default_branch: "main".to_string(),
            language: "en".to_string(),
            account_policy: AccountPolicy::Swap,
            forced_account: None,
            review_event_policy: ReviewEventPolicy::FormalVerdict,
            gdrive_folder_id: None,
            local_repo_path: None,
            local_check_cmd: None,
            enabled: true,
            tender_enabled: true,
            tender_allowed_authors: Vec::new(),
            review_allowed_authors: Vec::new(),
            skip_fork_prs: false,
        };

        assert_eq!(
            cfg.select_reviewer_account(&shared_repo, "alice-dev", None),
            "bob-reviewer"
        );
        assert_eq!(
            cfg.select_reviewer_account(&shared_repo, "bob-reviewer", None),
            "alice-dev"
        );
        assert_eq!(
            cfg.select_reviewer_account(&shared_repo, "bob-reviewer", Some("ALICE-DEV")),
            "bob-reviewer"
        );
    }

    #[test]
    fn forced_comment_only_policy_for_single_account_org_repo() {
        let cfg = sample_config();
        let org_repo = WatchedRepoConfig {
            full_name: "acme-org/web-app".to_string(),
            default_branch: "development".to_string(),
            language: "en".to_string(),
            account_policy: AccountPolicy::Forced,
            forced_account: Some("alice-dev".to_string()),
            review_event_policy: ReviewEventPolicy::CommentOnly,
            gdrive_folder_id: Some("folder123".to_string()),
            local_repo_path: Some("~/apps/web-app".to_string()),
            local_check_cmd: Some("npm run check".to_string()),
            enabled: true,
            tender_enabled: true,
            tender_allowed_authors: Vec::new(),
            review_allowed_authors: Vec::new(),
            skip_fork_prs: false,
        };

        let reviewer = cfg.select_reviewer_account(&org_repo, "alice-dev", Some("alice-dev"));
        assert_eq!(reviewer, "alice-dev");
        assert_eq!(
            WorkspaceConfig::select_github_review_event(&org_repo, reviewer, "alice-dev", true),
            "COMMENT"
        );
        assert_eq!(
            WorkspaceConfig::select_github_review_event(&org_repo, reviewer, "other-dev", false),
            "COMMENT"
        );
    }

    #[test]
    fn tender_author_allowlist_and_identity_selection_preserve_separation() {
        let mut cfg = sample_config();
        let repo = WatchedRepoConfig {
            full_name: "cokitron/cheap-otto".to_string(),
            default_branch: "main".to_string(),
            language: "en".to_string(),
            account_policy: AccountPolicy::Swap,
            forced_account: None,
            review_event_policy: ReviewEventPolicy::FormalVerdict,
            gdrive_folder_id: None,
            local_repo_path: None,
            local_check_cmd: None,
            enabled: true,
            tender_enabled: true,
            tender_allowed_authors: Vec::new(),
            review_allowed_authors: Vec::new(),
            skip_fork_prs: false,
        };

        assert!(cfg.is_tender_author_allowed(&repo, "alice-dev"));
        assert!(cfg.is_tender_author_allowed(&repo, "BOB-REVIEWER"));
        assert!(!cfg.is_tender_author_allowed(&repo, "random-stranger"));
        assert!(!cfg.is_tender_author_allowed(&repo, ""));
        assert!(!cfg.is_tender_author_allowed(&repo, "unknown"));
        assert_eq!(cfg.select_tender_account(&repo, "alice-dev"), "alice-dev");
        assert_eq!(
            cfg.select_tender_account(&repo, "bob-reviewer"),
            "bob-reviewer"
        );

        cfg.tender.allowed_authors = vec!["*".to_string()];
        assert!(cfg.is_tender_author_allowed(&repo, "random-stranger"));
        assert!(!cfg.is_tender_author_allowed(&repo, "unknown"));
    }

    #[test]
    fn per_repo_tender_and_review_author_allowlists_override_global_accounts() {
        let mut cfg = sample_config();
        cfg.tender.allowed_authors = vec!["*".to_string()];
        let oss_repo = WatchedRepoConfig {
            full_name: "cokitron/cheap-otto".to_string(),
            default_branch: "main".to_string(),
            language: "en".to_string(),
            account_policy: AccountPolicy::Forced,
            forced_account: Some("cokitron".to_string()),
            review_event_policy: ReviewEventPolicy::CommentOnly,
            gdrive_folder_id: None,
            local_repo_path: Some("~/otto-workspaces/cheap-otto".to_string()),
            local_check_cmd: None,
            enabled: false,
            tender_enabled: true,
            tender_allowed_authors: vec!["cokitron".to_string()],
            review_allowed_authors: vec!["cokitron".to_string()],
            skip_fork_prs: true,
        };

        // Per-repo `tender_allowed_authors = ["cokitron"]` strictly rejects primary, secondary, and external contributors!
        assert!(cfg.is_tender_author_allowed(&oss_repo, "cokitron"));
        assert!(cfg.is_tender_author_allowed(&oss_repo, "COKITRON"));
        assert!(!cfg.is_tender_author_allowed(&oss_repo, "alice-dev"));
        assert!(!cfg.is_tender_author_allowed(&oss_repo, "bob-reviewer"));
        assert!(!cfg.is_tender_author_allowed(&oss_repo, "external-contributor"));

        assert!(cfg.is_review_author_allowed(&oss_repo, "cokitron"));
        assert!(!cfg.is_review_author_allowed(&oss_repo, "external-contributor"));
        assert!(!cfg.is_review_author_allowed(&oss_repo, "unknown"));
    }

    #[test]
    fn merges_and_strips_runtime_kb_overlay_without_duplicating_committed_rules() {
        let committed = "# Code Tender KB\n\n- `[*]` Always run local checks.\n";
        let overlay = "- `[acme/app]` Normalize line endings.\n- `[*]` Always run local checks.\n";
        let merged = merge_kb_markdown(committed, overlay);
        assert_eq!(
            merged,
            "# Code Tender KB\n\n- `[*]` Always run local checks.\n- `[acme/app]` Normalize line endings.\n"
        );
        let stripped = strip_committed_kb_lines(committed, &merged);
        assert_eq!(stripped, "- `[acme/app]` Normalize line endings.\n");
    }

    #[test]
    fn normalizes_git_urls_and_slugs() {
        assert_eq!(
            normalize_repo_input("acme-org/web-app").unwrap(),
            "acme-org/web-app"
        );
        assert_eq!(
            normalize_repo_input("https://github.com/acme-org/web-app.git").unwrap(),
            "acme-org/web-app"
        );
        assert_eq!(
            normalize_repo_input("https://github.com/acme-org/web-app/pull/42").unwrap(),
            "acme-org/web-app"
        );
        assert_eq!(
            normalize_repo_input("git@github.com:acme-org/web-app.git").unwrap(),
            "acme-org/web-app"
        );
        assert_eq!(
            normalize_repo_input("ssh://git@github.com/acme-org/web-app.git/").unwrap(),
            "acme-org/web-app"
        );
        assert!(normalize_repo_input("not-a-repo").is_err());
        assert_eq!(
            parse_pr_reference_input("https://github.com/acme-org/web-app/pull/210/files").unwrap(),
            ("acme-org/web-app".to_string(), 210)
        );
        assert_eq!(
            parse_pr_reference_input("acme-org/web-app#42").unwrap(),
            ("acme-org/web-app".to_string(), 42)
        );
        assert!(parse_pr_reference_input("acme-org/web-app").is_err());
    }
}
