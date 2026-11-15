//! GitHub GraphQL batch poller, unified diff fetcher, and atomic inline PR review poster.
//!
//! Executes `gh` subprocesses with per-account `GH_TOKEN` isolation (`~/.code-reviewer/tokens/<user>.token`)
//! so `primary` and `secondary` accounts can operate side by side without mutating global `gh` state.

use crate::config::{WatchedRepoConfig, WorkspaceConfig, expand_tilde, validate_repo_slug};
use crate::state::{
    BOT_COMMENT_MARKER, BOT_TENDER_MARKER, BOT_TENDER_NEEDS_HUMAN_MARKER,
    GitHubInlineCommentPayload, HumanReplyItem, OpenPrSnapshot, TenderFeedbackItem,
    TenderFeedbackKind, TenderPrSnapshot,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitHubError {
    #[error("invalid repo '{0}'")]
    InvalidRepo(String),
    #[error(
        "missing token for GitHub account '{account}' (checked {token_path} and `gh auth token`)"
    )]
    MissingToken {
        account: String,
        token_path: PathBuf,
    },
    #[error("failed to execute `gh` CLI: {0}")]
    CommandExec(#[from] std::io::Error),
    #[error("`gh` exited with status {code}: {stderr}")]
    CommandFailed { code: i32, stderr: String },
    #[error("failed to parse GitHub JSON response: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedIssue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct DetailedOpenPr {
    pub snapshot: OpenPrSnapshot,
    pub body: String,
    pub linked_issues: Vec<LinkedIssue>,
}

#[derive(Debug, Clone)]
pub struct MergedPrEvent {
    pub pr_number: u64,
    pub title: String,
    pub body: String,
    pub merged_at: String,
    pub merge_commit_sha: String,
    pub linked_issues: Vec<LinkedIssue>,
}

pub struct AtomicPrReviewRequest<'a> {
    pub account: &'a str,
    pub repo_full_name: &'a str,
    pub pr_number: u64,
    pub commit_id: &'a str,
    pub event: &'a str,
    pub body: &'a str,
    pub comments: &'a [GitHubInlineCommentPayload],
}

#[derive(Debug, Clone)]
pub struct PostedIssueComment {
    pub id: u64,
    pub html_url: String,
}

#[derive(Debug, Clone)]
pub struct CreatedGitHubIssue {
    pub number: u64,
    pub html_url: String,
}

#[derive(Debug, Clone)]
pub struct RepoPollBatch {
    pub open_prs: Vec<DetailedOpenPr>,
    pub merged_prs: Vec<MergedPrEvent>,
}

/// Lightweight Stage-1 Pull Request header fetched during author-gated Code Tender polling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenderOpenPrHeader {
    pub pr_number: u64,
    pub title: String,
    pub author: String,
    pub is_draft: bool,
    pub is_fork: bool,
}

/// Result of a two-stage author- and fork-filtered Code Tender repository poll.
#[derive(Debug, Clone)]
pub struct TenderRepoPollOutcome {
    pub snapshots: Vec<TenderPrSnapshot>,
    pub skipped_authors: Vec<(u64, String)>,
    pub skipped_forks: Vec<u64>,
    /// Every open PR number seen at Stage 1 (eligible or not), used to mark ledger entries closed.
    pub open_pr_numbers: Vec<u64>,
    /// `true` only when Stage 1 paged through the whole open-PR connection (no page cap or errors hit).
    pub open_list_complete: bool,
}

/// Upper bound on Stage-1 pages (100 PRs each) so a pathological repo cannot stall a poll.
const TENDER_STAGE1_MAX_PAGES: usize = 10;

/// Filters Stage-1 open PR headers at poll time so Code Tender NEVER fetches review threads,
/// comments, or dispatches runs for external contributors' PRs or cross-repository forks.
///
/// When `repo_cfg.tender_allowed_authors` is explicitly configured (e.g. `["cokitron"]`),
/// even `explicit_watched_prs` cannot bypass the per-repository author allowlist.
pub fn partition_tender_open_pr_headers(
    cfg: &WorkspaceConfig,
    repo_cfg: &WatchedRepoConfig,
    headers: &[TenderOpenPrHeader],
    explicit_watched_prs: &[u64],
) -> (Vec<u64>, Vec<(u64, String)>, Vec<u64>) {
    let mut eligible = Vec::new();
    let mut skipped_authors = Vec::new();
    let mut skipped_forks = Vec::new();
    let strict_repo_allowlist = !repo_cfg.tender_allowed_authors.is_empty();

    for h in headers {
        if h.pr_number == 0 {
            continue;
        }
        if h.is_fork {
            skipped_forks.push(h.pr_number);
            continue;
        }
        let author_allowed = cfg.is_tender_author_allowed(repo_cfg, &h.author);
        let watched_override = !strict_repo_allowlist
            && explicit_watched_prs.contains(&h.pr_number)
            && !h.author.trim().is_empty()
            && !h.author.trim().eq_ignore_ascii_case("unknown");
        if !author_allowed && !watched_override {
            skipped_authors.push((h.pr_number, h.author.clone()));
            continue;
        }
        eligible.push(h.pr_number);
    }

    (eligible, skipped_authors, skipped_forks)
}

pub struct GitHubClient {
    tokens_dir: PathBuf,
}

impl GitHubClient {
    pub fn new(cfg: &WorkspaceConfig) -> Self {
        Self {
            tokens_dir: expand_tilde(&cfg.accounts.tokens_dir),
        }
    }

    pub fn resolve_token(&self, account: &str) -> Result<String, GitHubError> {
        let token_file = self.tokens_dir.join(format!("{}.token", account));
        if let Ok(raw) = fs::read_to_string(&token_file) {
            let trimmed = raw.trim().trim_start_matches('\u{feff}').trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }

        // Fallback to `gh auth token --user <account>` when running interactively.
        let output = Command::new(gh_binary())
            .args(["auth", "token", "--user", account])
            .stdin(Stdio::null())
            .output()?;
        if output.status.success() {
            let tok = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !tok.is_empty() {
                return Ok(tok);
            }
        }

        Err(GitHubError::MissingToken {
            account: account.to_string(),
            token_path: token_file,
        })
    }

    pub fn verify_account(&self, account: &str) -> Result<String, GitHubError> {
        let out = self.run_gh(account, &["api", "user", "--jq", ".login"], None)?;
        Ok(out.trim().to_string())
    }

    /// Polls both open PRs and recently merged PRs for `<owner>/<repo>` in a single 1-point GraphQL request.
    pub fn poll_repository(
        &self,
        account: &str,
        repo_full_name: &str,
    ) -> Result<RepoPollBatch, GitHubError> {
        let (owner, name) = validate_repo_slug(repo_full_name)
            .map_err(|_| GitHubError::InvalidRepo(repo_full_name.to_string()))?;

        let query = r#"
query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    openPrs: pullRequests(states: [OPEN], first: 25, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number
        title
        body
        url
        isDraft
        isCrossRepository
        baseRefName
        headRefName
        headRefOid
        author { login }
        commits(last: 1) {
          nodes {
            commit {
              author {
                user { login }
              }
            }
          }
        }
        closingIssuesReferences(first: 10) {
          nodes {
            number
            title
            body
            state
          }
        }
        comments(last: 20) {
          nodes {
            author { login }
            body
            createdAt
          }
        }
        reviewThreads(last: 30) {
          nodes {
            path
            line
            comments(last: 10) {
              nodes {
                author { login }
                body
                createdAt
              }
            }
          }
        }
      }
    }
    mergedPrs: pullRequests(states: [MERGED], first: 10, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number
        title
        body
        mergedAt
        author { login }
        mergeCommit { oid }
        closingIssuesReferences(first: 10) {
          nodes {
            number
            title
            body
            state
          }
        }
      }
    }
  }
}
"#;

        let payload = serde_json::json!({
            "query": query,
            "variables": {
                "owner": owner,
                "name": name,
            }
        });
        let raw = self.run_gh(
            account,
            &["api", "graphql", "--input", "-"],
            Some(&serde_json::to_vec(&payload)?),
        )?;
        let root: Value = serde_json::from_str(&raw)?;
        parse_graphql_poll_response(&root)
    }

    /// Fetches the full unified diff (`application/vnd.github.v3.diff`) for a PR.
    pub fn fetch_pr_diff(
        &self,
        account: &str,
        repo_full_name: &str,
        pr_number: u64,
    ) -> Result<String, GitHubError> {
        let endpoint = format!("repos/{}/pulls/{}", repo_full_name, pr_number);
        self.run_gh(
            account,
            &[
                "api",
                &endpoint,
                "-H",
                "Accept: application/vnd.github.v3.diff",
            ],
            None,
        )
    }

    /// Fetches the incremental commit-to-commit diff (`previous_sha...head_sha`) for a re-review.
    pub fn fetch_compare_diff(
        &self,
        account: &str,
        repo_full_name: &str,
        base_sha: &str,
        head_sha: &str,
    ) -> Result<String, GitHubError> {
        let endpoint = format!(
            "repos/{}/compare/{}...{}",
            repo_full_name, base_sha, head_sha
        );
        self.run_gh(
            account,
            &[
                "api",
                &endpoint,
                "-H",
                "Accept: application/vnd.github.v3.diff",
            ],
            None,
        )
    }

    /// Fetches the current `head.sha` right before posting so inline line numbers never hit a moved tip.
    pub fn fetch_current_pr_head_sha(
        &self,
        account: &str,
        repo_full_name: &str,
        pr_number: u64,
    ) -> Result<String, GitHubError> {
        let endpoint = format!("repos/{}/pulls/{}", repo_full_name, pr_number);
        let out = self.run_gh(account, &["api", &endpoint, "--jq", ".head.sha"], None)?;
        Ok(out.trim().to_string())
    }

    /// Submits an atomic GitHub Pull Request Review (`COMMENT`, `APPROVE`, or `REQUEST_CHANGES`)
    /// with hunk-validated inline diff comments pinned to `commit_id`.
    pub fn post_atomic_pr_review(
        &self,
        req: &AtomicPrReviewRequest<'_>,
    ) -> Result<String, GitHubError> {
        let endpoint = format!(
            "repos/{}/pulls/{}/reviews",
            req.repo_full_name, req.pr_number
        );
        let payload = serde_json::json!({
            "commit_id": req.commit_id,
            "event": req.event,
            "body": req.body,
            "comments": req.comments,
        });
        let out = self.run_gh(
            req.account,
            &["api", "--method", "POST", &endpoint, "--input", "-"],
            Some(&serde_json::to_vec(&payload)?),
        )?;
        let resp: Value = serde_json::from_str(&out)?;
        Ok(resp
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    /// Posts a top-level PR timeline comment (`POST /repos/{owner}/{repo}/issues/{pr_number}/comments`).
    pub fn create_issue_comment(
        &self,
        account: &str,
        repo_full_name: &str,
        pr_number: u64,
        body: &str,
    ) -> Result<PostedIssueComment, GitHubError> {
        let endpoint = format!("repos/{}/issues/{}/comments", repo_full_name, pr_number);
        let payload = serde_json::json!({ "body": body });
        let out = self.run_gh(
            account,
            &["api", "--method", "POST", &endpoint, "--input", "-"],
            Some(&serde_json::to_vec(&payload)?),
        )?;
        let resp: Value = serde_json::from_str(&out)?;
        Ok(PostedIssueComment {
            id: resp.get("id").and_then(Value::as_u64).unwrap_or(0),
            html_url: resp
                .get("html_url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Updates an existing PR timeline comment (`PATCH /repos/{owner}/{repo}/issues/comments/{comment_id}`).
    pub fn update_issue_comment(
        &self,
        account: &str,
        repo_full_name: &str,
        comment_id: u64,
        body: &str,
    ) -> Result<PostedIssueComment, GitHubError> {
        let endpoint = format!("repos/{}/issues/comments/{}", repo_full_name, comment_id);
        let payload = serde_json::json!({ "body": body });
        let out = self.run_gh(
            account,
            &["api", "--method", "PATCH", &endpoint, "--input", "-"],
            Some(&serde_json::to_vec(&payload)?),
        )?;
        let resp: Value = serde_json::from_str(&out)?;
        Ok(PostedIssueComment {
            id: resp.get("id").and_then(Value::as_u64).unwrap_or(comment_id),
            html_url: resp
                .get("html_url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Updates an existing pickup status banner comment in place when retrying, or creates a fresh banner comment.
    pub fn upsert_status_banner_comment(
        &self,
        account: &str,
        repo_full_name: &str,
        pr_number: u64,
        existing_comment_id: Option<u64>,
        body: &str,
    ) -> Result<PostedIssueComment, GitHubError> {
        if let Some(cid) = existing_comment_id
            && cid > 0
            && let Ok(updated) = self.update_issue_comment(account, repo_full_name, cid, body)
        {
            return Ok(updated);
        }
        self.create_issue_comment(account, repo_full_name, pr_number, body)
    }

    /// Two-stage author- and fork-filtered Code Tender poller (0 AI tokens):
    /// 1. Stage 1 queries lightweight open PR headers (`number`, `title`, `isDraft`, `isCrossRepository`, `author`).
    /// 2. Immediately rejects cross-repository forks (`isCrossRepository == true`) and PRs whose author
    ///    is outside `repo_cfg.tender_allowed_authors` / `cfg.tender.allowed_authors`.
    /// 3. If zero owned PRs are open, returns immediately without querying any review threads or comments.
    /// 4. Otherwise Stage 2 fetches full reviews, comments, and `reviewThreads` ONLY for the eligible owned PR numbers.
    pub fn poll_tender_repository_for_repo(
        &self,
        account: &str,
        cfg: &WorkspaceConfig,
        repo_cfg: &WatchedRepoConfig,
        explicit_watched_prs: &[u64],
    ) -> Result<TenderRepoPollOutcome, GitHubError> {
        let (owner, name) = validate_repo_slug(&repo_cfg.full_name)
            .map_err(|_| GitHubError::InvalidRepo(repo_cfg.full_name.clone()))?;

        let stage1_query = r#"
query($owner: String!, $name: String!, $after: String) {
  repository(owner: $owner, name: $name) {
    openPrs: pullRequests(states: [OPEN], first: 100, after: $after, orderBy: {field: UPDATED_AT, direction: DESC}) {
      pageInfo { hasNextPage endCursor }
      nodes {
        number
        title
        isDraft
        isCrossRepository
        author { login }
      }
    }
  }
}
"#;
        let mut headers: Vec<TenderOpenPrHeader> = Vec::new();
        let mut after: Option<String> = None;
        let mut open_list_complete = false;
        for _ in 0..TENDER_STAGE1_MAX_PAGES {
            let stage1_payload = serde_json::json!({
                "query": stage1_query,
                "variables": {
                    "owner": owner,
                    "name": name,
                    "after": after,
                }
            });
            let stage1_raw = self.run_gh(
                account,
                &["api", "graphql", "--input", "-"],
                Some(&serde_json::to_vec(&stage1_payload)?),
            )?;
            let stage1_root: Value = serde_json::from_str(&stage1_raw)?;
            headers.extend(parse_graphql_tender_headers(&stage1_root));
            match parse_graphql_tender_next_cursor(&stage1_root) {
                Some(cursor) => after = Some(cursor),
                None => {
                    // A response without `nodes` is malformed, never a proof that no PRs are open.
                    open_list_complete = !tender_stage1_has_next_page(&stage1_root)
                        && stage1_root["data"]["repository"]["openPrs"]["nodes"].is_array();
                    break;
                }
            }
        }
        let open_pr_numbers: Vec<u64> = headers
            .iter()
            .map(|h| h.pr_number)
            .filter(|&n| n > 0)
            .collect();
        let (eligible_nums, skipped_authors, skipped_forks) =
            partition_tender_open_pr_headers(cfg, repo_cfg, &headers, explicit_watched_prs);

        if eligible_nums.is_empty() {
            return Ok(TenderRepoPollOutcome {
                snapshots: Vec::new(),
                skipped_authors,
                skipped_forks,
                open_pr_numbers,
                open_list_complete,
            });
        }

        let mut snapshots = Vec::with_capacity(eligible_nums.len());
        for chunk in eligible_nums.chunks(25) {
            let mut aliased_fields = String::new();
            for &num in chunk {
                aliased_fields.push_str(&format!(
                    "    pr_{num}: pullRequest(number: {num}) {{ ...TenderPrFields }}\n"
                ));
            }
            let stage2_query = format!(
                "query($owner: String!, $name: String!) {{\n\
                   repository(owner: $owner, name: $name) {{\n\
                 {aliased_fields}\
                   }}\n\
                 }}\n\
                 fragment TenderPrFields on PullRequest {{\n\
                   number\n\
                   title\n\
                   isDraft\n\
                   isCrossRepository\n\
                   baseRefName\n\
                   headRefName\n\
                   headRefOid\n\
                   author {{ login }}\n\
                   reviews(last: 20) {{\n\
                     nodes {{\n\
                       id\n\
                       state\n\
                       body\n\
                       submittedAt\n\
                       author {{ login }}\n\
                       comments {{ totalCount }}\n\
                     }}\n\
                   }}\n\
                   comments(last: 25) {{\n\
                     nodes {{\n\
                       databaseId\n\
                       author {{ login }}\n\
                       body\n\
                       createdAt\n\
                     }}\n\
                   }}\n\
                   reviewThreads(last: 40) {{\n\
                     nodes {{\n\
                       id\n\
                       isResolved\n\
                       path\n\
                       line\n\
                       originalLine\n\
                       comments(last: 15) {{\n\
                         nodes {{\n\
                           databaseId\n\
                           author {{ login }}\n\
                           body\n\
                           createdAt\n\
                         }}\n\
                       }}\n\
                     }}\n\
                   }}\n\
                 }}\n"
            );
            let stage2_payload = serde_json::json!({
                "query": stage2_query,
                "variables": {
                    "owner": owner,
                    "name": name,
                }
            });
            let stage2_raw = self.run_gh(
                account,
                &["api", "graphql", "--input", "-"],
                Some(&serde_json::to_vec(&stage2_payload)?),
            )?;
            let stage2_root: Value = serde_json::from_str(&stage2_raw)?;
            snapshots.extend(parse_aliased_tender_prs_response(&stage2_root, chunk));
        }

        Ok(TenderRepoPollOutcome {
            snapshots,
            skipped_authors,
            skipped_forks,
            open_pr_numbers,
            open_list_complete,
        })
    }

    /// Posts an inline reply to a specific Pull Request review comment thread.
    pub fn reply_to_pull_request_review_comment(
        &self,
        account: &str,
        repo_full_name: &str,
        pr_number: u64,
        comment_id: u64,
        body: &str,
    ) -> Result<PostedIssueComment, GitHubError> {
        let endpoint = format!(
            "repos/{}/pulls/{}/comments/{}/replies",
            repo_full_name, pr_number, comment_id
        );
        let payload = serde_json::json!({ "body": body });
        let out = self.run_gh(
            account,
            &["api", "--method", "POST", &endpoint, "--input", "-"],
            Some(&serde_json::to_vec(&payload)?),
        )?;
        let resp: Value = serde_json::from_str(&out)?;
        Ok(PostedIssueComment {
            id: resp.get("id").and_then(Value::as_u64).unwrap_or(0),
            html_url: resp
                .get("html_url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Resolves a GitHub Pull Request review thread by its GraphQL node ID.
    pub fn resolve_review_thread(
        &self,
        account: &str,
        thread_node_id: &str,
    ) -> Result<(), GitHubError> {
        let query = r#"
mutation($threadId: ID!) {
  resolveReviewThread(input: { threadId: $threadId }) {
    thread { id isResolved }
  }
}
"#;
        let payload = serde_json::json!({
            "query": query,
            "variables": { "threadId": thread_node_id }
        });
        let _ = self.run_gh(
            account,
            &["api", "graphql", "--input", "-"],
            Some(&serde_json::to_vec(&payload)?),
        )?;
        Ok(())
    }

    /// Creates a follow-up GitHub Issue (`POST /repos/{owner}/{repo}/issues`) so deferred or
    /// out-of-scope review feedback can be tracked, linked in the PR thread, and resolved cleanly.
    pub fn create_github_issue(
        &self,
        account: &str,
        repo_full_name: &str,
        title: &str,
        body: &str,
    ) -> Result<CreatedGitHubIssue, GitHubError> {
        let endpoint = format!("repos/{}/issues", repo_full_name);
        let payload = serde_json::json!({
            "title": title,
            "body": body,
        });
        let out = self.run_gh(
            account,
            &["api", "--method", "POST", &endpoint, "--input", "-"],
            Some(&serde_json::to_vec(&payload)?),
        )?;
        let resp: Value = serde_json::from_str(&out)?;
        Ok(CreatedGitHubIssue {
            number: resp.get("number").and_then(Value::as_u64).unwrap_or(0),
            html_url: resp
                .get("html_url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    }

    fn run_gh(
        &self,
        account: &str,
        args: &[&str],
        stdin_bytes: Option<&[u8]>,
    ) -> Result<String, GitHubError> {
        let token = self.resolve_token(account)?;
        let mut cmd = Command::new(gh_binary());
        cmd.args(args)
            .env("GH_TOKEN", token)
            .env("GH_PROMPT_DISABLED", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if stdin_bytes.is_some() {
            cmd.stdin(Stdio::piped());
        } else {
            cmd.stdin(Stdio::null());
        }

        let mut child = cmd.spawn()?;
        if let Some(bytes) = stdin_bytes
            && let Some(mut child_in) = child.stdin.take()
        {
            child_in.write_all(bytes)?;
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(GitHubError::CommandFailed {
                code: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

pub fn gh_binary() -> PathBuf {
    let brew = PathBuf::from("/opt/homebrew/bin/gh");
    if brew.exists() {
        return brew;
    }
    PathBuf::from("gh")
}

fn parse_graphql_poll_response(root: &Value) -> Result<RepoPollBatch, GitHubError> {
    let repo_node = &root["data"]["repository"];
    let mut open_prs = Vec::new();
    if let Some(nodes) = repo_node["openPrs"]["nodes"].as_array() {
        for pr in nodes {
            let pr_number = pr["number"].as_u64().unwrap_or(0);
            if pr_number == 0 {
                continue;
            }
            let title = pr["title"].as_str().unwrap_or("").to_string();
            let body = pr["body"].as_str().unwrap_or("").to_string();
            let is_draft = pr["isDraft"].as_bool().unwrap_or(false);
            let is_fork = pr["isCrossRepository"].as_bool().unwrap_or(false);
            let base_ref = pr["baseRefName"].as_str().unwrap_or("main").to_string();
            let head_ref = pr["headRefName"].as_str().unwrap_or("").to_string();
            let head_sha = pr["headRefOid"].as_str().unwrap_or("").to_string();
            let author = pr["author"]["login"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let latest_committer = pr["commits"]["nodes"]
                .as_array()
                .and_then(|arr| arr.last())
                .and_then(|c| c["commit"]["author"]["user"]["login"].as_str())
                .map(ToString::to_string);

            let linked_issues = parse_linked_issues(&pr["closingIssuesReferences"]["nodes"]);

            let mut human_replies = Vec::new();
            if let Some(comments) = pr["comments"]["nodes"].as_array() {
                for c in comments {
                    let c_body = c["body"].as_str().unwrap_or("");
                    if c_body.contains(BOT_COMMENT_MARKER) {
                        continue;
                    }
                    human_replies.push(HumanReplyItem {
                        author: c["author"]["login"]
                            .as_str()
                            .unwrap_or("unknown")
                            .to_string(),
                        created_at: c["createdAt"].as_str().unwrap_or("").to_string(),
                        path: None,
                        line: None,
                        body: c_body.to_string(),
                    });
                }
            }
            if let Some(threads) = pr["reviewThreads"]["nodes"].as_array() {
                for t in threads {
                    let t_path = t["path"].as_str().map(ToString::to_string);
                    let t_line = t["line"].as_u64().map(|n| n as u32);
                    if let Some(t_comments) = t["comments"]["nodes"].as_array() {
                        for c in t_comments {
                            let c_body = c["body"].as_str().unwrap_or("");
                            if c_body.contains(BOT_COMMENT_MARKER) {
                                continue;
                            }
                            human_replies.push(HumanReplyItem {
                                author: c["author"]["login"]
                                    .as_str()
                                    .unwrap_or("unknown")
                                    .to_string(),
                                created_at: c["createdAt"].as_str().unwrap_or("").to_string(),
                                path: t_path.clone(),
                                line: t_line,
                                body: c_body.to_string(),
                            });
                        }
                    }
                }
            }
            human_replies.sort_by(|a, b| a.created_at.cmp(&b.created_at));

            open_prs.push(DetailedOpenPr {
                snapshot: OpenPrSnapshot {
                    pr_number,
                    title,
                    author,
                    is_draft,
                    is_fork,
                    base_ref,
                    head_ref,
                    head_sha,
                    latest_committer,
                    human_replies,
                },
                body,
                linked_issues,
            });
        }
    }

    let mut merged_prs = Vec::new();
    if let Some(nodes) = repo_node["mergedPrs"]["nodes"].as_array() {
        for pr in nodes {
            let pr_number = pr["number"].as_u64().unwrap_or(0);
            let Some(merged_at) = pr["mergedAt"].as_str() else {
                continue;
            };
            let Some(merge_sha) = pr["mergeCommit"]["oid"].as_str() else {
                continue;
            };
            merged_prs.push(MergedPrEvent {
                pr_number,
                title: pr["title"].as_str().unwrap_or("").to_string(),
                body: pr["body"].as_str().unwrap_or("").to_string(),
                merged_at: merged_at.to_string(),
                merge_commit_sha: merge_sha.to_string(),
                linked_issues: parse_linked_issues(&pr["closingIssuesReferences"]["nodes"]),
            });
        }
    }
    merged_prs.sort_by(|a, b| a.merged_at.cmp(&b.merged_at));

    Ok(RepoPollBatch {
        open_prs,
        merged_prs,
    })
}

fn parse_linked_issues(nodes_val: &Value) -> Vec<LinkedIssue> {
    let mut out = Vec::new();
    if let Some(arr) = nodes_val.as_array() {
        for item in arr {
            if let Some(number) = item["number"].as_u64() {
                out.push(LinkedIssue {
                    number,
                    title: item["title"].as_str().unwrap_or("").to_string(),
                    body: item["body"].as_str().unwrap_or("").to_string(),
                    state: item["state"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    out
}

fn parse_graphql_tender_headers(root: &Value) -> Vec<TenderOpenPrHeader> {
    let mut out = Vec::new();
    let Some(nodes) = root["data"]["repository"]["openPrs"]["nodes"].as_array() else {
        return out;
    };
    for pr in nodes {
        let pr_number = pr["number"].as_u64().unwrap_or(0);
        if pr_number == 0 {
            continue;
        }
        out.push(TenderOpenPrHeader {
            pr_number,
            title: pr["title"].as_str().unwrap_or("").to_string(),
            author: pr["author"]["login"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            is_draft: pr["isDraft"].as_bool().unwrap_or(false),
            is_fork: pr["isCrossRepository"].as_bool().unwrap_or(false),
        });
    }
    out
}

fn tender_stage1_has_next_page(root: &Value) -> bool {
    root["data"]["repository"]["openPrs"]["pageInfo"]["hasNextPage"]
        .as_bool()
        .unwrap_or(false)
}

/// Returns the `endCursor` to continue paging, or `None` when the connection is exhausted
/// (or GitHub reported more pages without a usable cursor, which the caller treats as incomplete).
fn parse_graphql_tender_next_cursor(root: &Value) -> Option<String> {
    if !tender_stage1_has_next_page(root) {
        return None;
    }
    root["data"]["repository"]["openPrs"]["pageInfo"]["endCursor"]
        .as_str()
        .filter(|c| !c.is_empty())
        .map(str::to_string)
}

fn parse_aliased_tender_prs_response(root: &Value, pr_numbers: &[u64]) -> Vec<TenderPrSnapshot> {
    let repo_node = &root["data"]["repository"];
    let mut out = Vec::new();
    for &num in pr_numbers {
        let alias = format!("pr_{num}");
        if let Some(pr_val) = repo_node.get(&alias)
            && !pr_val.is_null()
            && let Some(snap) = parse_single_tender_pr_node(pr_val)
        {
            out.push(snap);
        }
    }
    out
}

#[cfg(test)]
fn parse_graphql_tender_response(root: &Value) -> Result<Vec<TenderPrSnapshot>, GitHubError> {
    let mut out = Vec::new();
    let Some(nodes) = root["data"]["repository"]["openPrs"]["nodes"].as_array() else {
        return Ok(out);
    };
    for pr in nodes {
        if let Some(snap) = parse_single_tender_pr_node(pr) {
            out.push(snap);
        }
    }
    Ok(out)
}

fn parse_single_tender_pr_node(pr: &Value) -> Option<TenderPrSnapshot> {
    let pr_number = pr["number"].as_u64().unwrap_or(0);
    if pr_number == 0 {
        return None;
    }
    let pr_author = pr["author"]["login"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let mut feedback_items = Vec::new();

    if let Some(reviews) = pr["reviews"]["nodes"].as_array() {
        for r in reviews {
            let rev_id = r["id"].as_str().unwrap_or("");
            let submitted_at = r["submittedAt"].as_str().unwrap_or("").to_string();
            let body = r["body"].as_str().unwrap_or("").to_string();
            if submitted_at.is_empty() || body.trim().is_empty() {
                continue;
            }
            let inline_comment_count = r["comments"]["totalCount"].as_u64().unwrap_or(0) as u32;
            feedback_items.push(TenderFeedbackItem {
                feedback_id: format!("review:{}", rev_id),
                thread_node_id: None,
                reply_comment_id: None,
                kind: TenderFeedbackKind::ReviewSummary,
                author: r["author"]["login"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
                created_at: submitted_at,
                path: None,
                line: None,
                review_state: r["state"].as_str().map(ToString::to_string),
                is_resolved: false,
                inline_comment_count,
                answered_by_tender: false,
                answered_by_pr_author: false,
                latest_author_reply: None,
                body,
            });
        }
    }

    if let Some(comments) = pr["comments"]["nodes"].as_array() {
        for c in comments {
            let db_id = c["databaseId"].as_u64().unwrap_or(0);
            let created_at = c["createdAt"].as_str().unwrap_or("").to_string();
            let body = c["body"].as_str().unwrap_or("").to_string();
            if created_at.is_empty() || body.trim().is_empty() {
                continue;
            }
            feedback_items.push(TenderFeedbackItem {
                feedback_id: format!("comment:{}", db_id),
                thread_node_id: None,
                reply_comment_id: Some(db_id),
                kind: TenderFeedbackKind::IssueComment,
                author: c["author"]["login"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
                created_at,
                path: None,
                line: None,
                review_state: None,
                is_resolved: false,
                inline_comment_count: 0,
                answered_by_tender: false,
                answered_by_pr_author: false,
                latest_author_reply: None,
                body,
            });
        }
    }

    if let Some(threads) = pr["reviewThreads"]["nodes"].as_array() {
        for t in threads {
            let thread_id = t["id"].as_str().map(ToString::to_string);
            let is_resolved = t["isResolved"].as_bool().unwrap_or(false);
            let path = t["path"].as_str().map(ToString::to_string);
            let line = t["line"]
                .as_u64()
                .or_else(|| t["originalLine"].as_u64())
                .map(|n| n as u32);
            let Some(t_comments) = t["comments"]["nodes"].as_array() else {
                continue;
            };
            let valid_comments: Vec<&Value> = t_comments
                .iter()
                .filter(|c| !c["createdAt"].as_str().unwrap_or("").is_empty())
                .collect();
            let Some(&first_c) = valid_comments.first() else {
                continue;
            };
            let root_db_id = first_c["databaseId"].as_u64().unwrap_or(0);

            // Inspect the latest comment in the thread to detect if Code Tender or the PR author already answered it.
            // A `NeedsHuman` bot note (`BOT_TENDER_NEEDS_HUMAN_MARKER` / "Leaving this thread open") leaves the thread
            // awaiting input and must NOT count as answered by either Code Tender or the PR author.
            let last_c = *valid_comments.last().unwrap_or(&first_c);
            let last_author = last_c["author"]["login"].as_str().unwrap_or("unknown");
            let last_body = last_c["body"].as_str().unwrap_or("");
            let is_tender_comment = last_body.contains(BOT_TENDER_MARKER);
            let is_tender_needs_human = last_body.contains(BOT_TENDER_NEEDS_HUMAN_MARKER)
                || (is_tender_comment && last_body.contains("Leaving this thread open"));
            let answered_by_tender = is_tender_comment && !is_tender_needs_human;
            let answered_by_pr_author = !is_tender_comment
                && !last_body.contains(BOT_COMMENT_MARKER)
                && last_author.eq_ignore_ascii_case(&pr_author);
            let latest_author_reply = if answered_by_pr_author {
                Some(last_body.to_string())
            } else {
                None
            };

            // Collect external reviewer comments (including single-account BOT_COMMENT_MARKER self-reviews).
            let reviewer_comments: Vec<&Value> = valid_comments
                .iter()
                .copied()
                .filter(|c| {
                    let c_author = c["author"]["login"].as_str().unwrap_or("unknown");
                    let c_body = c["body"].as_str().unwrap_or("");
                    !c_body.contains(BOT_TENDER_MARKER)
                        && (!c_author.eq_ignore_ascii_case(&pr_author)
                            || c_body.contains(BOT_COMMENT_MARKER))
                })
                .collect();
            let Some(&first_rev) = reviewer_comments.first() else {
                continue;
            };
            let reviewer_author = first_rev["author"]["login"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let latest_reviewer_at = reviewer_comments
                .last()
                .and_then(|c| c["createdAt"].as_str())
                .unwrap_or("")
                .to_string();

            let combined_body = if valid_comments.len() == 1 {
                first_rev["body"].as_str().unwrap_or("").to_string()
            } else {
                valid_comments
                    .iter()
                    .enumerate()
                    .map(|(idx, c)| {
                        let ca = c["author"]["login"].as_str().unwrap_or("unknown");
                        let cb = c["body"].as_str().unwrap_or("");
                        if idx == 0 {
                            cb.to_string()
                        } else {
                            format!("---\n**Thread reply (@{ca}):**\n{cb}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            };

            let feedback_id = if root_db_id > 0 {
                format!("thread-comment:{}", root_db_id)
            } else {
                format!("thread:{}", thread_id.as_deref().unwrap_or("unknown"))
            };

            feedback_items.push(TenderFeedbackItem {
                feedback_id,
                thread_node_id: thread_id,
                reply_comment_id: if root_db_id > 0 {
                    Some(root_db_id)
                } else {
                    None
                },
                kind: TenderFeedbackKind::ReviewThreadComment,
                author: reviewer_author,
                created_at: latest_reviewer_at,
                path,
                line,
                review_state: None,
                is_resolved,
                inline_comment_count: 0,
                answered_by_tender,
                answered_by_pr_author,
                latest_author_reply,
                body: combined_body,
            });
        }
    }

    feedback_items.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Some(TenderPrSnapshot {
        pr_number,
        title: pr["title"].as_str().unwrap_or("").to_string(),
        author: pr_author,
        is_draft: pr["isDraft"].as_bool().unwrap_or(false),
        is_fork: pr["isCrossRepository"].as_bool().unwrap_or(false),
        base_ref: pr["baseRefName"].as_str().unwrap_or("main").to_string(),
        head_ref: pr["headRefName"].as_str().unwrap_or("").to_string(),
        head_sha: pr["headRefOid"].as_str().unwrap_or("").to_string(),
        feedback_items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_graphql_tender_threads_and_detects_author_replies() {
        let sample = serde_json::json!({
            "data": {
                "repository": {
                    "openPrs": {
                        "nodes": [{
                            "number": 9,
                            "title": "fix: null guard",
                            "isDraft": false,
                            "isCrossRepository": false,
                            "baseRefName": "main",
                            "headRefName": "fix/null-guard",
                            "headRefOid": "deadbeef1234",
                            "author": { "login": "alice-dev" },
                            "reviews": {
                                "nodes": [{
                                    "id": "PRR_1",
                                    "state": "CHANGES_REQUESTED",
                                    "body": "Please fix edge case",
                                    "submittedAt": "2026-11-12T11:00:00Z",
                                    "author": { "login": "bob-reviewer" },
                                    "comments": { "totalCount": 1 }
                                }]
                            },
                            "comments": { "nodes": [] },
                            "reviewThreads": {
                                "nodes": [
                                    {
                                        "id": "PRRT_99",
                                        "isResolved": false,
                                        "path": "src/auth.rs",
                                        "line": 42,
                                        "originalLine": 40,
                                        "comments": {
                                            "nodes": [{
                                                "databaseId": 555,
                                                "author": { "login": "bob-reviewer" },
                                                "body": "Missing empty string check",
                                                "createdAt": "2026-11-12T11:01:00Z"
                                            }]
                                        }
                                    },
                                    {
                                        "id": "PRRT_100",
                                        "isResolved": false,
                                        "path": "src/formats.ts",
                                        "line": 17,
                                        "originalLine": 17,
                                        "comments": {
                                            "nodes": [
                                                {
                                                    "databaseId": 777,
                                                    "author": { "login": "cubic-dev-ai" },
                                                    "body": "P2: Underscore mismatch",
                                                    "createdAt": "2026-11-12T11:02:00Z"
                                                },
                                                {
                                                    "databaseId": 778,
                                                    "author": { "login": "alice-dev" },
                                                    "body": "Tracked in #164.",
                                                    "createdAt": "2026-11-12T11:05:00Z"
                                                }
                                            ]
                                        }
                                    }
                                ]
                            }
                        }]
                    }
                }
            }
        });

        let parsed = parse_graphql_tender_response(&sample).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].pr_number, 9);
        // Emits 1 ReviewSummary + 2 grouped reviewThreads (not 3 separate thread comments!)
        assert_eq!(parsed[0].feedback_items.len(), 3);
        assert_eq!(parsed[0].feedback_items[0].inline_comment_count, 1);

        let open_thread = &parsed[0].feedback_items[1];
        assert_eq!(open_thread.thread_node_id.as_deref(), Some("PRRT_99"));
        assert_eq!(open_thread.reply_comment_id, Some(555));
        assert!(!open_thread.answered_by_pr_author);

        let answered_thread = &parsed[0].feedback_items[2];
        assert_eq!(answered_thread.thread_node_id.as_deref(), Some("PRRT_100"));
        assert_eq!(answered_thread.reply_comment_id, Some(777));
        assert_eq!(answered_thread.author, "cubic-dev-ai");
        assert!(answered_thread.answered_by_pr_author);
        assert_eq!(
            answered_thread.latest_author_reply.as_deref(),
            Some("Tracked in #164.")
        );
    }

    #[test]
    fn needs_human_bot_reply_under_same_author_does_not_count_as_answered() {
        let sample = serde_json::json!({
            "data": {
                "repository": {
                    "openPrs": {
                        "nodes": [{
                            "number": 224,
                            "title": "refactor: wallet",
                            "isDraft": false,
                            "isCrossRepository": false,
                            "baseRefName": "development",
                            "headRefName": "feat/wallet",
                            "headRefOid": "0ab540f",
                            "author": { "login": "alice-dev" },
                            "reviews": { "nodes": [] },
                            "comments": { "nodes": [] },
                            "reviewThreads": {
                                "nodes": [{
                                    "id": "PRRT_NH",
                                    "isResolved": false,
                                    "path": "src/wallet.ts",
                                    "line": 10,
                                    "originalLine": 10,
                                    "comments": {
                                        "nodes": [
                                            {
                                                "databaseId": 900,
                                                "author": { "login": "cubic-dev-ai" },
                                                "body": "P2: Validate origin",
                                                "createdAt": "2026-11-14T02:00:00Z"
                                            },
                                            {
                                                "databaseId": 901,
                                                "author": { "login": "alice-dev" },
                                                "body": format!("{BOT_TENDER_MARKER}\n{BOT_TENDER_NEEDS_HUMAN_MARKER}\n🛠️ **Code Tender:** Leaving this thread open for a human."),
                                                "createdAt": "2026-11-14T02:05:00Z"
                                            }
                                        ]
                                    }
                                }]
                            }
                        }]
                    }
                }
            }
        });
        let parsed = parse_graphql_tender_response(&sample).unwrap();
        let fb = &parsed[0].feedback_items[0];
        assert!(!fb.answered_by_tender);
        assert!(!fb.answered_by_pr_author);
    }

    #[test]
    fn stage1_tender_poller_filters_out_non_owner_prs_and_forks_before_fetching_threads() {
        use crate::config::{AccountPolicy, ReviewEventPolicy};

        let cfg: WorkspaceConfig = toml::from_str(
            r#"
[accounts]
primary = "alice-dev"
secondary = "cokitron"
tokens_dir = "~/.code-reviewer/tokens"
[agents]
primary_runner = "claude"
verifier_runner = "agy"
[gdrive]
enabled = false
root_folder_id = ""
living_docs_folder_id = ""
gdrive_bin = "gdrive"
gdocs_bin = "gdocs"
"#,
        )
        .unwrap();

        let repo_cfg = WatchedRepoConfig {
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
            review_allowed_authors: Vec::new(),
            skip_fork_prs: true,
        };

        let stage1_json = serde_json::json!({
            "data": {
                "repository": {
                    "openPrs": {
                        "nodes": [
                            {
                                "number": 10,
                                "title": "chore: oss git strategy",
                                "isDraft": false,
                                "isCrossRepository": false,
                                "author": { "login": "cokitron" }
                            },
                            {
                                "number": 11,
                                "title": "feat: external pr",
                                "isDraft": false,
                                "isCrossRepository": false,
                                "author": { "login": "some-other-dev" }
                            },
                            {
                                "number": 12,
                                "title": "feat: other local account",
                                "isDraft": false,
                                "isCrossRepository": false,
                                "author": { "login": "alice-dev" }
                            },
                            {
                                "number": 13,
                                "title": "feat: spoofed fork pr",
                                "isDraft": false,
                                "isCrossRepository": true,
                                "author": { "login": "cokitron" }
                            }
                        ]
                    }
                }
            }
        });

        let headers = parse_graphql_tender_headers(&stage1_json);
        assert_eq!(headers.len(), 4);

        // Even if someone passed `#11` or `#12` in `explicit_watched_prs`, strict `tender_allowed_authors = ["cokitron"]` blocks them!
        let (eligible, skipped_authors, skipped_forks) =
            partition_tender_open_pr_headers(&cfg, &repo_cfg, &headers, &[11, 12]);
        assert_eq!(eligible, vec![10]);
        assert_eq!(
            skipped_authors,
            vec![
                (11, "some-other-dev".to_string()),
                (12, "alice-dev".to_string())
            ]
        );
        assert_eq!(skipped_forks, vec![13]);

        // Verify Stage-2 aliased response parsing for matched PR #10.
        let stage2_json = serde_json::json!({
            "data": {
                "repository": {
                    "pr_10": {
                        "number": 10,
                        "title": "chore: oss git strategy",
                        "isDraft": false,
                        "isCrossRepository": false,
                        "baseRefName": "main",
                        "headRefName": "chore/oss-git-strategy-and-cubic",
                        "headRefOid": "4acacac999",
                        "author": { "login": "cokitron" },
                        "reviews": { "nodes": [] },
                        "comments": { "nodes": [] },
                        "reviewThreads": { "nodes": [] }
                    }
                }
            }
        });
        let snaps = parse_aliased_tender_prs_response(&stage2_json, &eligible);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].pr_number, 10);
        assert_eq!(snaps[0].author, "cokitron");
    }

    #[test]
    fn stage1_cursor_continues_only_while_has_next_page_with_cursor() {
        let more = serde_json::json!({"data": {"repository": {"openPrs": {
            "pageInfo": {"hasNextPage": true, "endCursor": "abc"}, "nodes": []
        }}}});
        assert_eq!(
            parse_graphql_tender_next_cursor(&more),
            Some("abc".to_string())
        );

        let last = serde_json::json!({"data": {"repository": {"openPrs": {
            "pageInfo": {"hasNextPage": false, "endCursor": "zzz"}, "nodes": []
        }}}});
        assert_eq!(parse_graphql_tender_next_cursor(&last), None);
        assert!(!tender_stage1_has_next_page(&last));

        // More pages reported without a cursor must stay "has next page" so the poll is flagged incomplete.
        let broken = serde_json::json!({"data": {"repository": {"openPrs": {
            "pageInfo": {"hasNextPage": true, "endCursor": null}, "nodes": []
        }}}});
        assert_eq!(parse_graphql_tender_next_cursor(&broken), None);
        assert!(tender_stage1_has_next_page(&broken));
    }
}
