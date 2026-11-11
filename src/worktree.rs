//! Pillar 2 (Option 1): Parallel Agent Worktree Farm (Delegate to MacBook & Pull Back to Windows).
//!
//! Enables a developer coding on their Windows PC to offload parallel tasks (bug fixes, refactors,
//! test generation, spikes) to isolated Git worktrees on their headless MacBook over SSH.
//!
//! Each delegated task runs in its own branch (`otto/<task-id>`), its own directory
//! (`~/otto-workspaces/<project>/.otto-worktrees/<task-id>`), and its own background `tmux` session
//! (`otto-<task-id>`), then gets fetched back into the Windows Git repo via `cheap-otto pull --id <task-id>`.

use crate::bridge::{BridgeError, PeerNode, validate_safe_identifier};
use crate::command::CommandSpec;
use crate::harness::{HarnessError, HarnessRunConfig, ReasoningEffort};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when building or validating parallel worktree agent plans.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorktreeError {
    /// Underlying peer or project validation error.
    #[error(transparent)]
    Bridge(#[from] BridgeError),
    /// Task description was blank.
    #[error("task prompt cannot be empty")]
    EmptyPrompt,
    /// Task identifier contains path traversal or shell metacharacters.
    #[error("invalid task ID '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidTaskId(String),
    /// Unrecognized `--agent` runner string.
    #[error("unsupported agent runner '{0}': expected 'claude', 'agy', 'kiro', or 'custom'")]
    UnsupportedAgent(String),
    /// Invalid `--model` or `--session-id` argument.
    #[error("{0}")]
    InvalidHarnessOption(String),
}

/// Coding agent CLI invoked inside the remote MacBook Git worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRunner {
    /// Claude Code CLI (`claude -p "<prompt>" --dangerously-skip-permissions`).
    ClaudeCode,
    /// Antigravity CLI (`agy`).
    Antigravity,
    /// Kiro CLI (`kiro-cli`).
    KiroCli,
    /// Custom shell command / script execution.
    CustomShell,
}

impl AgentRunner {
    /// Parses a CLI flag string (`claude`, `agy`, `kiro`, `custom`) into an [`AgentRunner`].
    ///
    /// # Errors
    /// Returns [`WorktreeError::UnsupportedAgent`] if the value is unrecognized.
    pub fn parse(raw: &str) -> Result<Self, WorktreeError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Self::ClaudeCode),
            "agy" | "antigravity" => Ok(Self::Antigravity),
            "kiro" | "kiro-cli" => Ok(Self::KiroCli),
            "custom" | "shell" => Ok(Self::CustomShell),
            other => Err(WorktreeError::UnsupportedAgent(other.to_string())),
        }
    }

    /// Canonical short identifier (`"claude"`, `"agy"`, `"kiro"`, `"custom"`).
    #[must_use = "returns the canonical harness slug"]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Antigravity => "agy",
            Self::KiroCli => "kiro",
            Self::CustomShell => "custom",
        }
    }

    /// Formats the headless invocation command for human-readable display.
    #[must_use = "returns the shell command to invoke the agent"]
    pub fn build_agent_command(self, escaped_prompt: &str) -> String {
        match self {
            Self::ClaudeCode => {
                format!("claude -p \"{escaped_prompt}\" --dangerously-skip-permissions")
            }
            Self::Antigravity => {
                format!("agy -p \"{escaped_prompt}\" --dangerously-skip-permissions")
            }
            Self::KiroCli => format!(
                "kiro-cli chat --no-interactive --trust-all-tools --trust-workspace \"{escaped_prompt}\""
            ),
            Self::CustomShell => escaped_prompt.to_string(),
        }
    }

    /// Formats a POSIX single-quoted invocation string safe from remote variable expansion.
    #[must_use = "returns the single-quoted remote agent invocation"]
    pub fn build_remote_safe_command(self, raw_prompt: &str) -> String {
        let quoted = posix_single_quote(raw_prompt);
        match self {
            Self::ClaudeCode => format!("claude -p {quoted} --dangerously-skip-permissions"),
            Self::Antigravity => format!("agy -p {quoted} --dangerously-skip-permissions"),
            Self::KiroCli => format!(
                "kiro-cli chat --no-interactive --trust-all-tools --trust-workspace {quoted}"
            ),
            Self::CustomShell => raw_prompt.to_string(),
        }
    }
}

/// Wraps an arbitrary string in POSIX single quotes (`'...'`), escaping embedded single quotes as `'\''`.
#[must_use = "returns POSIX single-quoted string safe for remote shell interpolation"]
pub fn posix_single_quote(input: &str) -> String {
    let mut quoted = String::with_capacity(input.len() + 2);
    quoted.push('\'');
    for ch in input.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

/// Execution plan for delegating a parallel coding task to an isolated Git worktree on the MacBook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedWorktreePlan {
    /// Target repository slug.
    pub project_name: String,
    /// Sanitized task identifier (`<task-id>`).
    pub task_id: String,
    /// Remote worktree branch (`otto/<task-id>`).
    pub branch_name: String,
    /// Remote worktree filesystem path on the MacBook.
    pub remote_worktree_dir: String,
    /// Detached `tmux` session name (`otto-<task-id>`).
    pub tmux_session_name: String,
    /// Persistent log file path on the MacBook (`~/.cheap-otto/logs/<project>/<task-id>.log`).
    #[serde(default)]
    pub remote_log_file: String,
    /// Persistent `stream-json` NDJSON event log (`~/.cheap-otto/events/<project>/<task-id>.ndjson`).
    #[serde(default)]
    pub remote_events_file: String,
    /// Selected coding agent runner.
    pub agent: AgentRunner,
    /// Optional model override (`sonnet`, `gemini-4-carbon-high`, etc.).
    #[serde(default)]
    pub model: Option<String>,
    /// Optional reasoning effort override (`low`..`max`).
    #[serde(default)]
    pub effort: Option<ReasoningEffort>,
    /// Optional bound session/conversation ID for follow-up queries (`cheap-otto ask`).
    #[serde(default)]
    pub session_id: Option<String>,
    /// Verification command run prior to committing changes.
    pub verify_command: String,
    /// Whether the arbiter should evict idle local LLM weights before running verification.
    pub evict_idle_llm_for_headroom: bool,
    /// Human-readable command lines for dry-run inspection.
    pub execution_steps: Vec<String>,
    /// Shell-free process specifications executed when `--execute` is passed.
    #[serde(default)]
    pub execution_specs: Vec<CommandSpec>,
}

/// Plan for pulling a completed MacBook worktree branch back into the Windows Git repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreePullPlan {
    /// Target repository slug.
    pub project_name: String,
    /// Sanitized task identifier (`<task-id>`).
    pub task_id: String,
    /// Remote and local branch name (`otto/<task-id>`).
    pub branch_name: String,
    /// Human-readable `git fetch` command.
    pub git_fetch_command: String,
    /// Human-readable `git diff` command.
    pub inspect_diff_command: String,
    /// Human-readable `git merge --no-ff` command.
    pub merge_command: String,
    /// Human-readable remote worktree cleanup command.
    pub cleanup_remote_worktree_command: String,
    /// Shell-free `git fetch` specification.
    #[serde(default)]
    pub fetch_spec: Option<CommandSpec>,
    /// Shell-free `git diff --stat` specification.
    #[serde(default)]
    pub diff_spec: Option<CommandSpec>,
    /// Shell-free `git merge --no-ff` specification.
    #[serde(default)]
    pub merge_spec: Option<CommandSpec>,
    /// Shell-free SSH command to prune the remote MacBook worktree.
    #[serde(default)]
    pub cleanup_remote_worktree_spec: Option<CommandSpec>,
}

/// Converts a free-form task description into a deterministic, URL/branch-safe slug (`max 28 chars`).
#[must_use = "returns the sanitized task identifier"]
pub fn slugify_task_prompt(prompt: &str) -> String {
    let mut slug = String::with_capacity(28);
    let mut last_was_dash = false;

    for ch in prompt.trim().chars() {
        if slug.len() >= 28 {
            break;
        }
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "task-worktree".to_string()
    } else {
        slug
    }
}

/// Builds a complete plan to delegate a coding task from Windows to an isolated Git worktree
/// on the MacBook, run the agent + verification command inside a background `tmux` session,
/// and commit the resulting changes to `otto/<task-id>`.
///
/// # Errors
/// Returns [`WorktreeError`] if `project_name`, `prompt`, or `custom_task_id` is invalid.
#[must_use = "delegated worktree plan must be executed or displayed"]
pub fn build_delegate_plan(
    peer: &PeerNode,
    project_name: &str,
    prompt: &str,
    custom_task_id: Option<&str>,
    agent: AgentRunner,
    verify_command: &str,
) -> Result<DelegatedWorktreePlan, WorktreeError> {
    let cfg = HarnessRunConfig {
        runner: agent,
        model: None,
        effort: None,
        session_id: None,
        resume_existing: false,
        stream_json: matches!(agent, AgentRunner::ClaudeCode | AgentRunner::Antigravity),
    };
    build_delegate_plan_with_harness(
        peer,
        project_name,
        prompt,
        custom_task_id,
        &cfg,
        verify_command,
    )
}

/// Builds a worktree delegation plan with explicit [`HarnessRunConfig`] (`--model`, `--effort`,
/// `--session-id`, and `stream-json` telemetry teeing into `~/.cheap-otto/events/`).
///
/// # Errors
/// Returns [`WorktreeError`] if `project_name`, `prompt`, `custom_task_id`, or `harness` is invalid.
#[must_use = "delegated worktree plan must be executed or displayed"]
pub fn build_delegate_plan_with_harness(
    peer: &PeerNode,
    project_name: &str,
    prompt: &str,
    custom_task_id: Option<&str>,
    harness: &HarnessRunConfig,
    verify_command: &str,
) -> Result<DelegatedWorktreePlan, WorktreeError> {
    let remote_repo_dir = peer.remote_project_dir(project_name)?;
    let trimmed_prompt = prompt.trim();
    if trimmed_prompt.is_empty() {
        return Err(WorktreeError::EmptyPrompt);
    }

    let task_id = match custom_task_id {
        Some(id) if !id.trim().is_empty() => {
            let trimmed_id = id.trim();
            if !validate_safe_identifier(trimmed_id) {
                return Err(WorktreeError::InvalidTaskId(id.to_string()));
            }
            trimmed_id.to_string()
        }
        _ => slugify_task_prompt(trimmed_prompt),
    };

    let safe_agent_cmd = harness
        .build_remote_command(trimmed_prompt)
        .map_err(|e| match e {
            HarnessError::Worktree(w) => w,
            other => WorktreeError::InvalidHarnessOption(other.to_string()),
        })?;

    let clean_project = project_name.trim().to_string();
    let branch_name = format!("otto/{task_id}");
    let remote_worktree_dir = format!("{remote_repo_dir}/.otto-worktrees/{task_id}");
    let tmux_session_name = format!("otto-{task_id}");
    let remote_log_dir = format!("~/.cheap-otto/logs/{clean_project}");
    let remote_events_dir = format!("~/.cheap-otto/events/{clean_project}");
    let remote_log_file = format!("{remote_log_dir}/{task_id}.log");
    let remote_events_file = format!("{remote_events_dir}/{task_id}.ndjson");
    let remote_rc_file = format!("{remote_log_dir}/.{task_id}.rc");
    let agent_rc_file = format!("{remote_log_dir}/.{task_id}.agent_rc");

    let verify_cmd = if verify_command.trim().is_empty() {
        "cargo test".to_string()
    } else {
        verify_command.trim().to_string()
    };

    let ssh_dest = peer.ssh_destination();
    let port = peer.ssh_port;
    let port_str = port.to_string();
    let git_remote_url = format!(
        "ssh://{}@{}:{}/{}",
        peer.ssh_user, peer.host, peer.ssh_port, remote_repo_dir
    );

    let execution_steps = vec![
        format!(
            "ssh -p {port} {ssh_dest} \"mkdir -p {remote_repo_dir} {remote_log_dir} {remote_events_dir} && git -C {remote_repo_dir} init\""
        ),
        format!("git push {git_remote_url} HEAD:refs/heads/otto-base/{task_id} --force"),
        format!(
            "ssh -p {port} {ssh_dest} \"git -C {remote_repo_dir} worktree add -B {branch_name} {remote_worktree_dir} refs/heads/otto-base/{task_id}\""
        ),
        format!(
            "ssh -p {port} {ssh_dest} \"tmux new-session -d -s {tmux_session_name} 'cd {remote_worktree_dir} && ({safe_agent_cmd} && {verify_cmd} && git add -A && git commit -m \\\"otto({task_id}): completed delegated task\\\") 2>&1 | tee -a {remote_log_file}' \\; set-option -t {tmux_session_name} remain-on-exit on\""
        ),
    ];

    let remote_init_script = format!(
        "mkdir -p {remote_repo_dir} {remote_log_dir} {remote_events_dir} && git -C {remote_repo_dir} init"
    );
    let push_refspec = format!("HEAD:refs/heads/otto-base/{task_id}");
    let remote_worktree_script = format!(
        "git -C {remote_repo_dir} worktree add -B {branch_name} {remote_worktree_dir} refs/heads/otto-base/{task_id}"
    );

    let agent_stage = if harness.stream_json {
        format!(
            "rm -f {agent_rc_file}; {{ {safe_agent_cmd}; printf '%s' \"$?\" > {agent_rc_file}; }} | tee -a {remote_events_file}; test \"$(cat {agent_rc_file} 2>/dev/null || echo 1)\" = \"0\""
        )
    } else {
        safe_agent_cmd
    };

    let inner_tmux_pipeline = format!(
        "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; cd {remote_worktree_dir} && source ~/.zshrc 2>/dev/null || true; source ~/.cargo/env 2>/dev/null || true; {{ {{ {agent_stage} && {verify_cmd} && (git diff --quiet && git diff --cached --quiet || (git add -A && git commit -m \"otto({task_id}): completed delegated task\")); printf '%s' \"$?\" > {remote_rc_file}; }} 2>&1 | tee -a {remote_log_file}; }}; rc=$(cat {remote_rc_file} 2>/dev/null || echo 1); rm -f {remote_rc_file} {agent_rc_file}; printf '\\n[OTTO_STATUS exit=%s]\\n' \"$rc\" | tee -a {remote_log_file}; exit \"$rc\""
    );
    let remote_tmux_script = format!(
        "tmux kill-session -t {tmux_session_name} 2>/dev/null || true; tmux new-session -d -s {tmux_session_name} {} && tmux set-option -t {tmux_session_name} remain-on-exit on",
        posix_single_quote(&inner_tmux_pipeline)
    );

    let execution_specs = vec![
        CommandSpec::new(
            "Initialize base Git repository, logs, and event directories on MacBook",
            "ssh",
            ["-p", &port_str, &ssh_dest, &remote_init_script],
        ),
        CommandSpec::new(
            "Push current Windows HEAD snapshot to MacBook base ref",
            "git",
            ["push", &git_remote_url, &push_refspec, "--force"],
        ),
        CommandSpec::new(
            "Create isolated Git worktree for delegated task",
            "ssh",
            ["-p", &port_str, &ssh_dest, &remote_worktree_script],
        ),
        CommandSpec::new(
            "Launch detached tmux agent + verification session on MacBook",
            "ssh",
            ["-p", &port_str, &ssh_dest, &remote_tmux_script],
        ),
    ];

    Ok(DelegatedWorktreePlan {
        project_name: clean_project,
        task_id,
        branch_name,
        remote_worktree_dir,
        tmux_session_name,
        remote_log_file,
        remote_events_file,
        agent: harness.runner,
        model: harness.model.clone(),
        effort: harness.effort,
        session_id: harness.session_id.clone(),
        verify_command: verify_cmd,
        evict_idle_llm_for_headroom: true,
        execution_steps,
        execution_specs,
    })
}

/// Builds the plan to fetch a completed MacBook worktree branch (`otto/<task-id>`) back into
/// the local Windows Git repository.
///
/// # Errors
/// Returns [`WorktreeError`] if `project_name` or `task_id` is invalid.
#[must_use = "pull plan must be executed or displayed"]
pub fn build_pull_plan(
    peer: &PeerNode,
    project_name: &str,
    task_id: &str,
) -> Result<WorktreePullPlan, WorktreeError> {
    let remote_repo_dir = peer.remote_project_dir(project_name)?;
    let trimmed_id = task_id.trim();
    if !validate_safe_identifier(trimmed_id) {
        return Err(WorktreeError::InvalidTaskId(task_id.to_string()));
    }

    let branch_name = format!("otto/{trimmed_id}");
    let remote_worktree_dir = format!("{remote_repo_dir}/.otto-worktrees/{trimmed_id}");
    let git_remote_url = format!(
        "ssh://{}@{}:{}/{}",
        peer.ssh_user, peer.host, peer.ssh_port, remote_repo_dir
    );
    let refspec = format!("refs/heads/{branch_name}:refs/heads/{branch_name}");
    let diff_range = format!("HEAD..{branch_name}");
    let port_str = peer.ssh_port.to_string();
    let ssh_dest = peer.ssh_destination();
    let cleanup_script = format!(
        "tmux kill-session -t otto-{trimmed_id} 2>/dev/null || true; git -C {remote_repo_dir} worktree remove --force {remote_worktree_dir}"
    );

    let git_fetch_command = format!("git fetch {git_remote_url} {refspec}");
    let inspect_diff_command = format!("git diff {diff_range}");
    let merge_command = format!("git merge --no-ff {branch_name}");
    let cleanup_remote_worktree_command =
        format!("ssh -p {port_str} {ssh_dest} \"{cleanup_script}\"");

    let fetch_spec = CommandSpec::new(
        "Fetch completed MacBook worktree branch into Windows Git repo",
        "git",
        ["fetch", &git_remote_url, &refspec],
    );
    let diff_spec = CommandSpec::new(
        "Summarize incoming changes from delegated worktree branch",
        "git",
        ["diff", "--stat", &diff_range],
    );
    let merge_spec = CommandSpec::new(
        "Merge delegated worktree branch into current branch",
        "git",
        ["merge", "--no-ff", &branch_name],
    );
    let cleanup_remote_worktree_spec = CommandSpec::new(
        "Remove completed worktree directory on MacBook",
        "ssh",
        ["-p", &port_str, &ssh_dest, &cleanup_script],
    );

    Ok(WorktreePullPlan {
        project_name: project_name.trim().to_string(),
        task_id: trimmed_id.to_string(),
        branch_name,
        git_fetch_command,
        inspect_diff_command,
        merge_command,
        cleanup_remote_worktree_command,
        fetch_spec: Some(fetch_spec),
        diff_spec: Some(diff_spec),
        merge_spec: Some(merge_spec),
        cleanup_remote_worktree_spec: Some(cleanup_remote_worktree_spec),
    })
}

/// Builds the display SSH command to list all active parallel `otto-*` `tmux` sessions and Git worktrees.
///
/// # Errors
/// Returns [`WorktreeError::Bridge`] if `project_name` is invalid.
#[must_use = "status command should be executed or displayed"]
pub fn build_jobs_status_command(
    peer: &PeerNode,
    project_name: &str,
) -> Result<String, WorktreeError> {
    let remote_repo_dir = peer.remote_project_dir(project_name)?;
    Ok(format!(
        "ssh -p {} {} \"echo '--- Active Agent tmux Sessions ---' && (tmux list-sessions 2>/dev/null | grep '^otto-' || echo 'No running sessions') && echo '' && echo '--- MacBook Git Worktrees ---' && git -C {remote_repo_dir} worktree list\"",
        peer.ssh_port,
        peer.ssh_destination()
    ))
}

/// Builds a shell-free [`CommandSpec`] to inspect active `otto-*` `tmux` sessions and Git worktrees on the MacBook.
///
/// # Errors
/// Returns [`WorktreeError::Bridge`] if `project_name` is invalid.
#[must_use = "jobs status spec should be executed via std::process::Command"]
pub fn build_jobs_status_spec(
    peer: &PeerNode,
    project_name: &str,
) -> Result<CommandSpec, WorktreeError> {
    let remote_repo_dir = peer.remote_project_dir(project_name)?;
    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let remote_script = format!(
        "echo '--- Active Agent tmux Sessions ---' && (tmux list-sessions 2>/dev/null | grep '^otto-' || echo 'No running sessions') && echo '' && echo '--- MacBook Git Worktrees ---' && (git -C {remote_repo_dir} worktree list 2>/dev/null || echo 'Remote workspace not synced yet')"
    );
    Ok(CommandSpec::new(
        "Query active agent tmux sessions and Git worktrees on MacBook",
        "ssh",
        ["-p", &port_str, &dest, &remote_script],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_prompt_into_clean_task_id() {
        let slug = slugify_task_prompt("Fix the auth token race condition & add unit tests!");
        assert_eq!(slug, "fix-the-auth-token-race-cond");
    }

    #[test]
    fn builds_isolated_worktree_delegation_plan_for_claude_code() -> Result<(), WorktreeError> {
        let peer = PeerNode::new("otto-mac", "192.168.1.42", "admin", 22)?;
        let plan = build_delegate_plan(
            &peer,
            "cheap-otto",
            "Refactor the arbiter module and add property tests",
            Some("refactor-arbiter"),
            AgentRunner::ClaudeCode,
            "cargo test --all-targets",
        )?;

        assert_eq!(plan.task_id, "refactor-arbiter");
        assert_eq!(plan.branch_name, "otto/refactor-arbiter");
        assert_eq!(
            plan.remote_worktree_dir,
            "~/otto-workspaces/cheap-otto/.otto-worktrees/refactor-arbiter"
        );
        assert_eq!(plan.tmux_session_name, "otto-refactor-arbiter");
        assert_eq!(
            plan.remote_log_file,
            "~/.cheap-otto/logs/cheap-otto/refactor-arbiter.log"
        );
        assert_eq!(plan.execution_steps.len(), 4);
        assert_eq!(plan.execution_specs.len(), 4);
        assert!(
            plan.execution_steps[2].contains("git -C ~/otto-workspaces/cheap-otto worktree add")
        );
        assert!(plan.execution_steps[3].contains("claude -p"));
        assert!(plan.execution_specs[3].args[3].contains("remain-on-exit on"));
        assert!(
            plan.execution_specs[3].args[3]
                .contains("tee -a ~/.cheap-otto/logs/cheap-otto/refactor-arbiter.log")
        );
        Ok(())
    }

    #[test]
    fn escapes_single_quotes_in_remote_safe_agent_invocation() {
        let cmd = AgentRunner::ClaudeCode.build_remote_safe_command("Fix user's $HOME path");
        assert_eq!(
            cmd,
            "claude -p 'Fix user'\\''s $HOME path' --dangerously-skip-permissions"
        );
    }

    #[test]
    fn builds_pull_plan_to_fetch_worktree_branch_back_to_windows() -> Result<(), WorktreeError> {
        let peer = PeerNode::new("otto-mac", "100.101.102.103", "admin", 22)?;
        let pull = build_pull_plan(&peer, "cheap-otto", "refactor-arbiter")?;

        assert_eq!(pull.branch_name, "otto/refactor-arbiter");
        assert_eq!(
            pull.git_fetch_command,
            "git fetch ssh://admin@100.101.102.103:22/~/otto-workspaces/cheap-otto refs/heads/otto/refactor-arbiter:refs/heads/otto/refactor-arbiter"
        );
        assert_eq!(
            pull.inspect_diff_command,
            "git diff HEAD..otto/refactor-arbiter"
        );
        assert_eq!(
            pull.merge_command,
            "git merge --no-ff otto/refactor-arbiter"
        );
        assert!(pull.fetch_spec.is_some());
        assert!(pull.merge_spec.is_some());
        Ok(())
    }

    #[test]
    fn rejects_empty_prompt_and_invalid_task_id() -> Result<(), BridgeError> {
        let peer = PeerNode::new("otto-mac", "192.168.1.42", "admin", 22)?;
        let empty_err = build_delegate_plan(
            &peer,
            "cheap-otto",
            "   ",
            None,
            AgentRunner::ClaudeCode,
            "cargo test",
        )
        .unwrap_err();
        assert_eq!(empty_err, WorktreeError::EmptyPrompt);

        let bad_id_err = build_pull_plan(&peer, "cheap-otto", "../hack").unwrap_err();
        assert_eq!(
            bad_id_err,
            WorktreeError::InvalidTaskId("../hack".to_string())
        );
        Ok(())
    }
}
