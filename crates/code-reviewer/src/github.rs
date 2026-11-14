//! GitHub GraphQL batch poller, unified diff fetcher, and atomic inline PR review poster.
//!
//! Executes `gh` subprocesses with per-account `GH_TOKEN` isolation (`~/.code-reviewer/tokens/<user>.token`)
//! so `primary` and `secondary` accounts can operate side by side without mutating global `gh` state.

use crate::config::{WorkspaceConfig, expand_tilde, validate_repo_slug};
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

    /// Polls open Pull Requests and all review summaries, inline review threads, and comments
    /// for Code Tender in a single 1-point GraphQL request (0 AI tokens).
    pub fn poll_tender_repository(
        &self,
        account: &str,
        repo_full_name: &str,
    ) -> Result<Vec<TenderPrSnapshot>, GitHubError> {
        let (owner, name) = validate_repo_slug(repo_full_name)
            .map_err(|_| GitHubError::InvalidRepo(repo_full_name.to_string()))?;

        let query = r#"
query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    openPrs: pullRequests(states: [OPEN], first: 25, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number
        title
        isDraft
        isCrossRepository
        baseRefName
        headRefName
        headRefOid
        author { login }
        reviews(last: 20) {
          nodes {
            id
            state
            body
            submittedAt
            author { login }
            comments { totalCount }
          }
        }
        comments(last: 25) {
          nodes {
            databaseId
            author { login }
            body
            createdAt
          }
        }
        reviewThreads(last: 40) {
          nodes {
            id
            isResolved
            path
            line
            originalLine
            comments(last: 15) {
              nodes {
                databaseId
                author { login }
                body
                createdAt
              }
            }
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
        parse_graphql_tender_response(&root)
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

fn parse_graphql_tender_response(root: &Value) -> Result<Vec<TenderPrSnapshot>, GitHubError> {
    let mut out = Vec::new();
    let Some(nodes) = root["data"]["repository"]["openPrs"]["nodes"].as_array() else {
        return Ok(out);
    };

    for pr in nodes {
        let pr_number = pr["number"].as_u64().unwrap_or(0);
        if pr_number == 0 {
            continue;
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
        out.push(TenderPrSnapshot {
            pr_number,
            title: pr["title"].as_str().unwrap_or("").to_string(),
            author: pr_author,
            is_draft: pr["isDraft"].as_bool().unwrap_or(false),
            is_fork: pr["isCrossRepository"].as_bool().unwrap_or(false),
            base_ref: pr["baseRefName"].as_str().unwrap_or("main").to_string(),
            head_ref: pr["headRefName"].as_str().unwrap_or("").to_string(),
            head_sha: pr["headRefOid"].as_str().unwrap_or("").to_string(),
            feedback_items,
        });
    }

    Ok(out)
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
}
