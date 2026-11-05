//! Pillar 2 (Option 1): Parallel Agent Worktree Farm (Delegate to MacBook & Pull Back to Windows).
//!
//! Enables a developer coding on their Windows PC to offload parallel tasks (bug fixes, refactors,
//! test generation, spikes) to isolated Git worktrees on their headless MacBook over SSH.
//!
//! Each delegated task runs in its own branch (`otto/<task-id>`), its own directory
//! (`~/otto-workspaces/<project>/.otto-worktrees/<task-id>`), and its own background `tmux` session
//! (`otto-<task-id>`), then gets fetched back into the Windows Git repo via `cheap-otto pull <task-id>`.

use crate::bridge::{BridgeError, PeerNode, validate_safe_identifier};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when building or validating parallel worktree agent plans.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorktreeError {
    #[error(transparent)]
    Bridge(#[from] BridgeError),
    #[error("task prompt cannot be empty")]
    EmptyPrompt,
    #[error("invalid task ID '{0}': must contain only ASCII alphanumeric, '-', or '_'")]
    InvalidTaskId(String),
    #[error("unsupported agent runner '{0}': expected 'claude', 'agy', 'kiro', or 'custom'")]
    UnsupportedAgent(String),
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

    /// Formats the headless invocation command for the given prompt inside the remote worktree.
    #[must_use = "returns the shell command to invoke the agent"]
    pub fn build_agent_command(self, escaped_prompt: &str) -> String {
        match self {
            Self::ClaudeCode => {
                format!("claude -p \"{escaped_prompt}\" --dangerously-skip-permissions")
            }
            Self::Antigravity => format!("agy --prompt \"{escaped_prompt}\""),
            Self::KiroCli => format!("kiro-cli chat --non-interactive \"{escaped_prompt}\""),
            Self::CustomShell => escaped_prompt.to_string(),
        }
    }
}

/// Execution plan for delegating a parallel coding task to an isolated Git worktree on the MacBook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedWorktreePlan {
    pub project_name: String,
    pub task_id: String,
    pub branch_name: String,
    pub remote_worktree_dir: String,
    pub tmux_session_name: String,
    pub agent: AgentRunner,
    pub verify_command: String,
    pub evict_idle_llm_for_headroom: bool,
    pub execution_steps: Vec<String>,
}

/// Plan for pulling a completed MacBook worktree branch back into the Windows Git repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreePullPlan {
    pub project_name: String,
    pub task_id: String,
    pub branch_name: String,
    pub git_fetch_command: String,
    pub inspect_diff_command: String,
    pub merge_command: String,
    pub cleanup_remote_worktree_command: String,
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

    let branch_name = format!("otto/{task_id}");
    let remote_worktree_dir = format!("{remote_repo_dir}/.otto-worktrees/{task_id}");
    let tmux_session_name = format!("otto-{task_id}");

    let verify_cmd = if verify_command.trim().is_empty() {
        "cargo test".to_string()
    } else {
        verify_command.trim().to_string()
    };

    let escaped_prompt = trimmed_prompt
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`");
    let agent_invocation = agent.build_agent_command(&escaped_prompt);

    let ssh_dest = peer.ssh_destination();
    let port = peer.ssh_port;
    let git_remote_url = format!(
        "ssh://{}@{}:{}/{}",
        peer.ssh_user, peer.host, peer.ssh_port, remote_repo_dir
    );

    let execution_steps = vec![
        // Step 1: Ensure remote base repository exists and push current Windows HEAD
        format!(
            "ssh -p {port} {ssh_dest} \"mkdir -p {remote_repo_dir} && git -C {remote_repo_dir} init\""
        ),
        format!("git push {git_remote_url} HEAD:refs/heads/otto-base/{task_id} --force"),
        // Step 2: Create isolated Git worktree for this parallel task on the MacBook
        format!(
            "ssh -p {port} {ssh_dest} \"git -C {remote_repo_dir} worktree add -B {branch_name} {remote_worktree_dir} refs/heads/otto-base/{task_id}\""
        ),
        // Step 3: Launch detached tmux session running agent -> verification -> git commit
        format!(
            "ssh -p {port} {ssh_dest} \"tmux new-session -d -s {tmux_session_name} 'cd {remote_worktree_dir} && {agent_invocation} && {verify_cmd} && git add -A && git commit -m \\\"otto({task_id}): completed delegated task\\\"'\""
        ),
    ];

    Ok(DelegatedWorktreePlan {
        project_name: project_name.trim().to_string(),
        task_id,
        branch_name,
        remote_worktree_dir,
        tmux_session_name,
        agent,
        verify_command: verify_cmd,
        evict_idle_llm_for_headroom: true,
        execution_steps,
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

    let git_fetch_command =
        format!("git fetch {git_remote_url} refs/heads/{branch_name}:refs/heads/{branch_name}");
    let inspect_diff_command = format!("git diff HEAD..{branch_name}");
    let merge_command = format!("git merge --no-ff {branch_name}");
    let cleanup_remote_worktree_command = format!(
        "ssh -p {} {} \"git -C {remote_repo_dir} worktree remove --force {remote_worktree_dir}\"",
        peer.ssh_port,
        peer.ssh_destination()
    );

    Ok(WorktreePullPlan {
        project_name: project_name.trim().to_string(),
        task_id: trimmed_id.to_string(),
        branch_name,
        git_fetch_command,
        inspect_diff_command,
        merge_command,
        cleanup_remote_worktree_command,
    })
}

/// Builds the SSH command to list all active parallel `otto-*` `tmux` sessions and Git worktrees on the MacBook.
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
        assert_eq!(plan.execution_steps.len(), 4);
        assert!(
            plan.execution_steps[2].contains("git -C ~/otto-workspaces/cheap-otto worktree add")
        );
        assert!(plan.execution_steps[3].contains("claude -p"));
        Ok(())
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
