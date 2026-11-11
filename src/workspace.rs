//! Multi-Project & Config Discovery (`sync`, `exec`, `delegate`, `jobs`, `pull`, `projects`).
//!
//! Pure, cross-platform discovery and registry logic that resolves:
//! 1. Active project names from `--project`, `git rev-parse --show-toplevel`, or working directory paths.
//! 2. Active Git branch names from `git rev-parse --abbrev-ref HEAD`.
//! 3. Hierarchical `.cheap-otto` state directories (explicit override -> cwd -> repo root -> `~/.cheap-otto`).
//! 4. Persistent multi-project tracking (`projects.json`) across Windows repositories and remote MacBook worktrees.

use crate::bridge::validate_safe_identifier;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors returned during workspace project, branch, or state discovery.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkspaceError {
    /// Explicit or discovered project folder name contains unsafe characters.
    #[error("invalid project name '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidProjectName(String),
    /// Neither `--project`, Git top-level, nor working directory yielded a usable project name.
    #[error("unable to detect project name from Git repository or current working directory")]
    UnableToDetectProject,
    /// Git branch output is empty or contains unsafe shell/ref characters.
    #[error("invalid Git branch name '{0}'")]
    InvalidBranchName(String),
}

/// Auto-detected local Git repository metadata for multi-project workflows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedWorkspace {
    /// Sanitized project identifier mapped to `~/otto-workspaces/<project_name>`.
    pub project_name: String,
    /// Active local Git branch (or `"HEAD"` when detached / outside Git).
    pub current_branch: String,
    /// Normalized local repository root path if inside a Git working tree.
    pub repo_root: Option<String>,
}

/// Persistent entry for a paired local-to-MacBook repository inside `.cheap-otto/projects.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecord {
    /// Project slug (`~/otto-workspaces/<project_name>`).
    pub project_name: String,
    /// Last known local filesystem root on the Windows workstation.
    #[serde(default)]
    pub local_path: Option<String>,
    /// Target directory on the paired headless MacBook.
    pub remote_project_dir: String,
    /// Most recently synced local Git branch.
    #[serde(default)]
    pub last_synced_branch: Option<String>,
    /// Active delegated worktree task IDs (`otto/<task-id>`).
    #[serde(default)]
    pub delegated_tasks: Vec<String>,
}

/// Multi-project workspace registry stored in `.cheap-otto/projects.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRegistry {
    /// Tracked projects ordered by discovery/registration.
    #[serde(default)]
    pub projects: Vec<ProjectRecord>,
}

impl WorkspaceRegistry {
    /// Inserts or updates a project record when `cheap-otto sync` runs.
    pub fn record_sync(
        &mut self,
        project_name: &str,
        local_path: Option<&str>,
        remote_project_dir: &str,
        branch: Option<&str>,
    ) {
        if let Some(existing) = self
            .projects
            .iter_mut()
            .find(|p| p.project_name == project_name)
        {
            if let Some(path) = local_path {
                existing.local_path = Some(path.to_string());
            }
            existing.remote_project_dir = remote_project_dir.to_string();
            if let Some(br) = branch {
                existing.last_synced_branch = Some(br.to_string());
            }
        } else {
            self.projects.push(ProjectRecord {
                project_name: project_name.to_string(),
                local_path: local_path.map(str::to_string),
                remote_project_dir: remote_project_dir.to_string(),
                last_synced_branch: branch.map(str::to_string),
                delegated_tasks: Vec::new(),
            });
        }
    }

    /// Records a newly delegated parallel worktree task ID under its project.
    pub fn record_delegated_task(
        &mut self,
        project_name: &str,
        local_path: Option<&str>,
        remote_project_dir: &str,
        task_id: &str,
    ) {
        self.record_sync(project_name, local_path, remote_project_dir, None);
        if let Some(project) = self
            .projects
            .iter_mut()
            .find(|p| p.project_name == project_name)
            && !project.delegated_tasks.iter().any(|id| id == task_id)
        {
            project.delegated_tasks.push(task_id.to_string());
        }
    }

    /// Removes a completed task ID from the active list once pulled/cleaned up.
    pub fn mark_task_pulled(&mut self, project_name: &str, task_id: &str) {
        if let Some(project) = self
            .projects
            .iter_mut()
            .find(|p| p.project_name == project_name)
        {
            project.delegated_tasks.retain(|id| id != task_id);
        }
    }
}

/// Extracts the final path component across both Windows (`\`) and POSIX (`/`) separators.
fn extract_basename(raw_path: &str) -> Option<&str> {
    let trimmed = raw_path.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return None;
    }
    let candidate = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed).trim();
    if candidate.is_empty() || candidate.ends_with(':') {
        None
    } else {
        Some(candidate)
    }
}

/// Resolves the active project name using priority:
/// 1. Explicit `--project` CLI flag (if provided).
/// 2. Basename of `git rev-parse --show-toplevel` output.
/// 3. Basename of the current working directory (`cwd`).
///
/// # Errors
/// Returns [`WorkspaceError::InvalidProjectName`] if the chosen name fails safety validation,
/// or [`WorkspaceError::UnableToDetectProject`] if no source produces a candidate name.
#[must_use = "resolved project name should be passed to bridge or worktree plans"]
pub fn resolve_project_name(
    explicit_project: Option<&str>,
    git_toplevel_stdout: Option<&str>,
    cwd: Option<&Path>,
) -> Result<String, WorkspaceError> {
    if let Some(explicit) = explicit_project {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            if !validate_safe_identifier(trimmed) {
                return Err(WorkspaceError::InvalidProjectName(explicit.to_string()));
            }
            return Ok(trimmed.to_string());
        }
    }

    if let Some(toplevel) = git_toplevel_stdout
        && let Some(base) = extract_basename(toplevel)
    {
        if !validate_safe_identifier(base) {
            return Err(WorkspaceError::InvalidProjectName(base.to_string()));
        }
        return Ok(base.to_string());
    }

    if let Some(dir) = cwd
        && let Some(os_name) = dir.file_name()
        && let Some(name) = os_name.to_str()
    {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            if !validate_safe_identifier(trimmed) {
                return Err(WorkspaceError::InvalidProjectName(trimmed.to_string()));
            }
            return Ok(trimmed.to_string());
        }
    }

    Err(WorkspaceError::UnableToDetectProject)
}

/// Parses and validates the current Git branch name from `git rev-parse --abbrev-ref HEAD`.
///
/// # Errors
/// Returns [`WorkspaceError::InvalidBranchName`] if the output is empty, starts with `'-'`,
/// contains `'..'`, ends with `'.lock'`, or includes unsafe non-ref characters.
pub fn parse_git_branch(git_branch_stdout: &str) -> Result<String, WorkspaceError> {
    let trimmed = git_branch_stdout.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('-')
        || trimmed.starts_with('/')
        || trimmed.ends_with('/')
        || trimmed.contains("..")
        || trimmed.contains("//")
        || trimmed.ends_with(".lock")
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    {
        return Err(WorkspaceError::InvalidBranchName(
            git_branch_stdout.trim().to_string(),
        ));
    }

    Ok(trimmed.to_string())
}

/// Builds a complete [`DetectedWorkspace`] from optional Git probe outputs and current directory.
///
/// # Errors
/// Returns [`WorkspaceError`] if no safe project name can be determined.
#[must_use = "detected workspace metadata should be used for sync or delegation"]
pub fn detect_workspace_from_probes(
    explicit_project: Option<&str>,
    git_toplevel_stdout: Option<&str>,
    git_branch_stdout: Option<&str>,
    cwd: Option<&Path>,
) -> Result<DetectedWorkspace, WorkspaceError> {
    let project_name = resolve_project_name(explicit_project, git_toplevel_stdout, cwd)?;
    let current_branch = git_branch_stdout
        .and_then(|raw| parse_git_branch(raw).ok())
        .unwrap_or_else(|| "HEAD".to_string());
    let repo_root = git_toplevel_stdout
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    Ok(DetectedWorkspace {
        project_name,
        current_branch,
        repo_root,
    })
}

/// Selects the active `.cheap-otto` state directory across multi-project locations:
/// 1. Explicit `--state-dir` CLI override.
/// 2. `.cheap-otto` in current working directory if `peer.json` exists there.
/// 3. `<repo_root>/.cheap-otto` if `peer.json` exists there.
/// 4. `~/.cheap-otto` global user config if `peer.json` exists there.
/// 5. `<repo_root>/.cheap-otto` when inside a Git repository, otherwise `.cheap-otto`.
#[must_use = "selected state directory path is needed to load peer.json and backups"]
pub fn select_state_dir(
    explicit_override: Option<&Path>,
    cwd_has_peer: bool,
    repo_state_dir: Option<(&Path, bool)>,
    home_state_dir: Option<(&Path, bool)>,
) -> PathBuf {
    if let Some(explicit) = explicit_override {
        return explicit.to_path_buf();
    }

    if cwd_has_peer {
        return PathBuf::from(".cheap-otto");
    }

    if let Some((repo_dir, true)) = repo_state_dir {
        return repo_dir.to_path_buf();
    }

    if let Some((home_dir, true)) = home_state_dir {
        return home_dir.to_path_buf();
    }

    if let Some((repo_dir, _)) = repo_state_dir {
        return repo_dir.to_path_buf();
    }

    PathBuf::from(".cheap-otto")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_project_name_from_windows_and_unix_git_toplevel() -> Result<(), WorkspaceError> {
        let win = resolve_project_name(None, Some("D:\\projects\\cheapotto\r\n"), None)?;
        assert_eq!(win, "cheapotto");

        let unix = resolve_project_name(None, Some("/Users/dev/work/api-backend\n"), None)?;
        assert_eq!(unix, "api-backend");

        let explicit = resolve_project_name(
            Some("custom-service"),
            Some("D:\\projects\\cheapotto"),
            None,
        )?;
        assert_eq!(explicit, "custom-service");
        Ok(())
    }

    #[test]
    fn falls_back_to_cwd_when_outside_git_repo() -> Result<(), WorkspaceError> {
        let cwd = Path::new("/tmp/my_cli_tool");
        let detected = detect_workspace_from_probes(None, None, None, Some(cwd))?;
        assert_eq!(
            detected,
            DetectedWorkspace {
                project_name: "my_cli_tool".to_string(),
                current_branch: "HEAD".to_string(),
                repo_root: None,
            }
        );
        Ok(())
    }

    #[test]
    fn parses_feature_branches_and_rejects_unsafe_git_refs() -> Result<(), WorkspaceError> {
        let branch = parse_git_branch("ticket/CC-33.1-realtime-alert\n")?;
        assert_eq!(branch, "ticket/CC-33.1-realtime-alert");

        assert_eq!(
            parse_git_branch("--upload-pack=evil"),
            Err(WorkspaceError::InvalidBranchName(
                "--upload-pack=evil".to_string()
            ))
        );
        assert_eq!(
            parse_git_branch("refs/.."),
            Err(WorkspaceError::InvalidBranchName("refs/..".to_string()))
        );
        Ok(())
    }

    #[test]
    fn rejects_unsafe_project_identifiers_and_missing_sources() {
        let bad = resolve_project_name(Some("../etc/passwd"), None, None);
        assert_eq!(
            bad,
            Err(WorkspaceError::InvalidProjectName(
                "../etc/passwd".to_string()
            ))
        );

        let none = resolve_project_name(None, Some("   "), None);
        assert_eq!(none, Err(WorkspaceError::UnableToDetectProject));
    }

    #[test]
    fn selects_global_home_peer_config_when_repo_has_no_local_peer() {
        let repo_dir = Path::new("D:/projects/app/.cheap-otto");
        let home_dir = Path::new("C:/Users/admin/.cheap-otto");

        let selected =
            select_state_dir(None, false, Some((repo_dir, false)), Some((home_dir, true)));
        assert_eq!(selected, home_dir);

        let override_dir = Path::new("D:/custom-state");
        let overridden = select_state_dir(
            Some(override_dir),
            true,
            Some((repo_dir, true)),
            Some((home_dir, true)),
        );
        assert_eq!(overridden, override_dir);
    }

    #[test]
    fn tracks_projects_and_delegated_worktrees_in_registry() {
        let mut registry = WorkspaceRegistry::default();
        registry.record_sync(
            "cheapotto",
            Some("D:/projects/cheapotto"),
            "~/otto-workspaces/cheapotto",
            Some("main"),
        );
        registry.record_delegated_task(
            "cheapotto",
            Some("D:/projects/cheapotto"),
            "~/otto-workspaces/cheapotto",
            "fix-auth-race",
        );
        registry.record_delegated_task(
            "cheapotto",
            Some("D:/projects/cheapotto"),
            "~/otto-workspaces/cheapotto",
            "fix-auth-race",
        );

        assert_eq!(registry.projects.len(), 1);
        assert_eq!(
            registry.projects[0].delegated_tasks,
            vec!["fix-auth-race".to_string()]
        );

        registry.mark_task_pulled("cheapotto", "fix-auth-race");
        assert!(registry.projects[0].delegated_tasks.is_empty());
    }
}
