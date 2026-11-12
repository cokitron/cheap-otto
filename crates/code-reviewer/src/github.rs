//! GitHub GraphQL batch poller, unified diff fetcher, and atomic inline PR review poster.
//!
//! Executes `gh` subprocesses with per-account `GH_TOKEN` isolation (`~/.code-reviewer/tokens/<user>.token`)
//! so `primary` and `secondary` accounts can operate side by side without mutating global `gh` state.

use crate::config::{WorkspaceConfig, expand_tilde, validate_repo_slug};
use crate::state::{
    BOT_COMMENT_MARKER, GitHubInlineCommentPayload, HumanReplyItem, OpenPrSnapshot,
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
