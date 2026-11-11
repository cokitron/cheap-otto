//! Interactive M1 Terminal Access (`shell`) and Live Job Visibility (`attach`, `logs`).
//!
//! Keeps manual interactive work on the headless MacBook (`tmux` session `manual` or raw `zsh -l`)
//! cleanly isolated from automated `otto-*` CI and agent worktree sessions while giving instant
//! visibility into running or completed `otto-*` panes and persistent log files.

use crate::bridge::{BridgeError, PeerNode, validate_safe_identifier};
use crate::command::CommandSpec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when validating interactive terminal, `tmux` attach, or log streaming plans.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TerminalError {
    /// Underlying peer or project validation error.
    #[error(transparent)]
    Bridge(#[from] BridgeError),
    /// Requested manual `tmux` session name contains unsafe shell characters.
    #[error("invalid session name '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidSessionName(String),
    /// Manual `shell` attempted to use the `otto-` prefix reserved for automated CI/agent lanes.
    #[error(
        "session name '{0}' uses reserved prefix 'otto-': use `cheap-otto attach --id ...` for automated jobs or choose a manual name like 'manual'"
    )]
    ReservedSessionPrefix(String),
    /// Task or schedule identifier contains unsafe characters.
    #[error("invalid job/task ID '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidTaskId(String),
    /// Zero tail lines were requested for `logs`.
    #[error("log line count must be at least 1")]
    ZeroLogLines,
}

/// Interactive shell mode opened on the paired headless MacBook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellMode {
    /// Attaches to or creates a persistent user `tmux` session (default: `"manual"`).
    PersistentTmux {
        /// Validated non-`otto-*` session name.
        session_name: String,
    },
    /// Opens a direct `zsh -l` login shell without `tmux`.
    PlainLogin,
}

/// Starting directory on the MacBook when opening an interactive shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellTarget {
    /// Starts inside `~/otto-workspaces/<project>` (creating it if needed).
    ProjectWorkspace(String),
    /// Starts inside the user's macOS home directory (`~`).
    HomeDirectory,
}

/// Deterministic plan for `cheap-otto shell`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveShellPlan {
    /// Resolved remote working directory (`~` or `~/otto-workspaces/<project>`).
    pub remote_dir: String,
    /// Selected terminal session mode.
    pub mode: ShellMode,
    /// Human-readable SSH PTY command preview.
    pub display_command: String,
    /// Shell-free process specification (`ssh -t ...`).
    pub ssh_spec: CommandSpec,
}

/// Deterministic plan for `cheap-otto attach`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachSessionPlan {
    /// Normalized task or schedule ID (without leading `otto-`).
    pub task_id: String,
    /// Primary `tmux` session name (`otto-<id>`).
    pub tmux_session_name: String,
    /// Fallback scheduled `tmux` session name (`otto-sched-<id>`).
    pub scheduled_tmux_session_name: String,
    /// Whether `tmux attach-session` runs in read-only mode (`-r`).
    pub read_only: bool,
    /// Human-readable SSH PTY attach command preview.
    pub display_command: String,
    /// Shell-free process specification (`ssh -t ...`).
    pub ssh_spec: CommandSpec,
}

/// Deterministic plan for `cheap-otto logs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobLogsPlan {
    /// Project workspace slug.
    pub project_name: String,
    /// Normalized task or schedule ID (without leading `otto-`).
    pub task_id: String,
    /// Primary log path (`~/.cheap-otto/logs/<project>/<id>.log`).
    pub remote_log_file: String,
    /// Fallback scheduled log path (`~/.cheap-otto/logs/<project>/sched-<id>.log`).
    pub scheduled_log_file: String,
    /// Fallback `tmux` session name if no log file exists yet.
    pub tmux_session_name: String,
    /// Number of trailing lines to display.
    pub lines: u32,
    /// Whether `tail -f` live streaming is enabled.
    pub follow: bool,
    /// Human-readable SSH log command preview.
    pub display_command: String,
    /// Shell-free process specification.
    pub ssh_spec: CommandSpec,
}

/// Normalizes a user-supplied task/job ID, stripping an optional `otto-` prefix and validating safety.
fn normalize_task_id(raw_id: &str) -> Result<String, TerminalError> {
    let trimmed = raw_id.trim();
    let stripped = trimmed.strip_prefix("otto-").unwrap_or(trimmed);
    if !validate_safe_identifier(stripped) {
        return Err(TerminalError::InvalidTaskId(raw_id.to_string()));
    }
    Ok(stripped.to_string())
}

/// Builds an interactive PTY terminal login plan (`cheap-otto shell`) for the paired MacBook.
///
/// # Errors
/// Returns [`TerminalError`] if the project name or `tmux` session name is invalid or uses the
/// reserved `otto-` automation prefix.
#[must_use = "interactive shell plan must be executed or displayed"]
pub fn build_interactive_shell_plan(
    peer: &PeerNode,
    target: &ShellTarget,
    mode: ShellMode,
) -> Result<InteractiveShellPlan, TerminalError> {
    let remote_dir = match target {
        ShellTarget::ProjectWorkspace(project) => peer.remote_project_dir(project)?,
        ShellTarget::HomeDirectory => "~".to_string(),
    };

    let validated_mode = match mode {
        ShellMode::PersistentTmux { session_name } => {
            let clean = if session_name.trim().is_empty() {
                "manual".to_string()
            } else {
                session_name.trim().to_string()
            };
            if !validate_safe_identifier(&clean) {
                return Err(TerminalError::InvalidSessionName(session_name));
            }
            if clean.starts_with("otto-") {
                return Err(TerminalError::ReservedSessionPrefix(clean));
            }
            ShellMode::PersistentTmux {
                session_name: clean,
            }
        }
        ShellMode::PlainLogin => ShellMode::PlainLogin,
    };

    let prep_dir = match target {
        ShellTarget::ProjectWorkspace(_) => format!("mkdir -p {remote_dir} && cd {remote_dir}"),
        ShellTarget::HomeDirectory => "cd ~".to_string(),
    };

    let shell_launch = match &validated_mode {
        ShellMode::PersistentTmux { session_name } => format!(
            "if command -v tmux >/dev/null 2>&1; then exec tmux new-session -A -s {session_name}; else exec zsh -l; fi"
        ),
        ShellMode::PlainLogin => "exec zsh -l".to_string(),
    };

    let remote_script = format!(
        "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; {prep_dir} && {shell_launch}"
    );
    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let ssh_spec = CommandSpec::new(
        "Open interactive PTY terminal session on paired MacBook",
        "ssh",
        ["-t", "-p", &port_str, &dest, &remote_script],
    );
    let display_command = ssh_spec.display_command();

    Ok(InteractiveShellPlan {
        remote_dir,
        mode: validated_mode,
        display_command,
        ssh_spec,
    })
}

/// Builds a live `tmux` pane attachment plan (`cheap-otto attach`) for a delegated or scheduled job.
///
/// Automatically resolves both `otto-<id>` (delegated worktree) and `otto-sched-<id>` (scheduled run).
///
/// # Errors
/// Returns [`TerminalError::InvalidTaskId`] if `raw_id` contains unsafe characters.
#[must_use = "attach session plan must be executed or displayed"]
pub fn build_attach_session_plan(
    peer: &PeerNode,
    raw_id: &str,
    read_only: bool,
) -> Result<AttachSessionPlan, TerminalError> {
    let task_id = normalize_task_id(raw_id)?;
    let tmux_session_name = format!("otto-{task_id}");
    let scheduled_tmux_session_name = format!("otto-sched-{task_id}");
    let ro_flag = if read_only { "-r " } else { "" };

    let remote_script = format!(
        "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$PATH\"; if tmux has-session -t {tmux_session_name} 2>/dev/null; then exec tmux attach-session {ro_flag}-t {tmux_session_name}; elif tmux has-session -t {scheduled_tmux_session_name} 2>/dev/null; then exec tmux attach-session {ro_flag}-t {scheduled_tmux_session_name}; else echo 'No active or retained tmux session found for {task_id} (checked {tmux_session_name} and {scheduled_tmux_session_name}). Try `cheap-otto logs --id {task_id} --execute`.' >&2; exit 1; fi"
    );

    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let ssh_spec = CommandSpec::new(
        "Attach interactive terminal to remote MacBook otto tmux session",
        "ssh",
        ["-t", "-p", &port_str, &dest, &remote_script],
    );
    let display_command = ssh_spec.display_command();

    Ok(AttachSessionPlan {
        task_id,
        tmux_session_name,
        scheduled_tmux_session_name,
        read_only,
        display_command,
        ssh_spec,
    })
}

/// Builds a persistent log inspection or live tailing plan (`cheap-otto logs`) for a job.
///
/// Checks `~/.cheap-otto/logs/<project>/<id>.log`, then `sched-<id>.log`, and finally falls back
/// to `tmux capture-pane` if an older session has no log file on disk.
///
/// # Errors
/// Returns [`TerminalError`] if `project_name` or `raw_id` is invalid, or if `lines == 0`.
#[must_use = "job logs plan must be executed or displayed"]
pub fn build_job_logs_plan(
    peer: &PeerNode,
    project_name: &str,
    raw_id: &str,
    lines: u32,
    follow: bool,
) -> Result<JobLogsPlan, TerminalError> {
    if lines == 0 {
        return Err(TerminalError::ZeroLogLines);
    }
    let _ = peer.remote_project_dir(project_name)?;
    let clean_project = project_name.trim().to_string();
    let task_id = normalize_task_id(raw_id)?;

    let log_dir = format!("~/.cheap-otto/logs/{clean_project}");
    let remote_log_file = format!("{log_dir}/{task_id}.log");
    let scheduled_log_file = format!("{log_dir}/sched-{task_id}.log");
    let tmux_session_name = format!("otto-{task_id}");
    let follow_flag = if follow { " -f" } else { "" };

    let remote_script = format!(
        "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$PATH\"; if [ -f {remote_log_file} ]; then tail -n {lines}{follow_flag} {remote_log_file}; elif [ -f {scheduled_log_file} ]; then tail -n {lines}{follow_flag} {scheduled_log_file}; elif tmux has-session -t {tmux_session_name} 2>/dev/null; then tmux capture-pane -pt {tmux_session_name} -S -{lines}; else echo 'No log file or tmux session found for {task_id} in project {clean_project}.' >&2; exit 1; fi"
    );

    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let ssh_spec = if follow {
        CommandSpec::new(
            "Stream live job log output from paired MacBook",
            "ssh",
            ["-t", "-p", &port_str, &dest, &remote_script],
        )
    } else {
        CommandSpec::new(
            "Read job log output from paired MacBook",
            "ssh",
            ["-p", &port_str, &dest, &remote_script],
        )
    };
    let display_command = ssh_spec.display_command();

    Ok(JobLogsPlan {
        project_name: clean_project,
        task_id,
        remote_log_file,
        scheduled_log_file,
        tmux_session_name,
        lines,
        follow,
        display_command,
        ssh_spec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_persistent_manual_tmux_shell_in_project_workspace() -> Result<(), TerminalError> {
        let peer = PeerNode::new("otto-mac", "100.101.102.103", "admin", 22)?;
        let plan = build_interactive_shell_plan(
            &peer,
            &ShellTarget::ProjectWorkspace("cheap-otto".to_string()),
            ShellMode::PersistentTmux {
                session_name: "manual".to_string(),
            },
        )?;

        assert_eq!(plan.remote_dir, "~/otto-workspaces/cheap-otto");
        assert_eq!(plan.ssh_spec.program, "ssh");
        assert_eq!(plan.ssh_spec.args[0], "-t");
        assert!(plan.ssh_spec.args[4].contains("tmux new-session -A -s manual"));
        assert!(plan.ssh_spec.args[4].contains("cd ~/otto-workspaces/cheap-otto"));
        Ok(())
    }

    #[test]
    fn builds_plain_login_shell_in_home_directory() -> Result<(), TerminalError> {
        let peer = PeerNode::new("otto-mac", "192.168.1.42", "admin", 22)?;
        let plan = build_interactive_shell_plan(
            &peer,
            &ShellTarget::HomeDirectory,
            ShellMode::PlainLogin,
        )?;

        assert_eq!(plan.remote_dir, "~");
        assert!(plan.ssh_spec.args[4].contains("cd ~ && exec zsh -l"));
        Ok(())
    }

    #[test]
    fn rejects_reserved_otto_prefix_for_manual_shell() -> Result<(), BridgeError> {
        let peer = PeerNode::new("otto-mac", "192.168.1.42", "admin", 22)?;
        let err = build_interactive_shell_plan(
            &peer,
            &ShellTarget::HomeDirectory,
            ShellMode::PersistentTmux {
                session_name: "otto-ci".to_string(),
            },
        )
        .unwrap_err();

        assert_eq!(
            err,
            TerminalError::ReservedSessionPrefix("otto-ci".to_string())
        );
        Ok(())
    }

    #[test]
    fn builds_attach_and_logs_plans_and_strips_optional_otto_prefix() -> Result<(), TerminalError> {
        let peer = PeerNode::new("otto-mac", "100.101.102.103", "admin", 22)?;
        let attach = build_attach_session_plan(&peer, "otto-fix-auth", true)?;
        assert_eq!(attach.task_id, "fix-auth");
        assert_eq!(attach.tmux_session_name, "otto-fix-auth");
        assert_eq!(attach.scheduled_tmux_session_name, "otto-sched-fix-auth");
        assert!(attach.read_only);
        assert!(attach.ssh_spec.args[4].contains("attach-session -r -t otto-fix-auth"));

        let logs = build_job_logs_plan(&peer, "cheap-otto", "nightly-ci", 120, true)?;
        assert_eq!(
            logs.remote_log_file,
            "~/.cheap-otto/logs/cheap-otto/nightly-ci.log"
        );
        assert_eq!(
            logs.scheduled_log_file,
            "~/.cheap-otto/logs/cheap-otto/sched-nightly-ci.log"
        );
        assert!(logs.ssh_spec.args[4].contains("tail -n 120 -f"));
        Ok(())
    }

    #[test]
    fn rejects_zero_log_lines_and_unsafe_task_ids() -> Result<(), BridgeError> {
        let peer = PeerNode::new("otto-mac", "192.168.1.42", "admin", 22)?;
        assert_eq!(
            build_job_logs_plan(&peer, "cheap-otto", "valid-id", 0, false).unwrap_err(),
            TerminalError::ZeroLogLines
        );
        assert_eq!(
            build_attach_session_plan(&peer, "../escape", false).unwrap_err(),
            TerminalError::InvalidTaskId("../escape".to_string())
        );
        Ok(())
    }
}
