//! Pillar 1 (Option 5): Cross-OS Dev Bridge (Windows + Headless MacBook as One Machine).
//!
//! Connects a primary Windows workstation to a spare MacBook over LAN or Tailscale SSH
//! with zero daemon setup. Provides deterministic planning and command generation for:
//! 1. **Peer Pairing (`pair`):** SSH profile (`~/.ssh/config`), LAN vs. Tailscale transport detection.
//! 2. **Workspace Sync (`sync`):** Fast Git-aware project mirroring (`D:\projects\app` -> `~/otto-workspaces/app`)
//!    that automatically excludes heavy OS-specific build artifacts (`target/`, `node_modules/`).
//! 3. **Cross-OS Execution (`exec`):** Run native macOS / ARM64 commands (`cargo test`, `npm test`, `swift build`)
//!    inside the synced MacBook workspace directly from a Windows terminal.
//! 4. **Localhost Port Bridge (`tunnel`):** Forward remote MacBook dev server ports (`3000`, `5173`, `8080`)
//!    to `127.0.0.1` on the Windows PC.

use crate::command::CommandSpec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when configuring or validating the Cross-OS Dev Bridge.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BridgeError {
    /// Host contains invalid or shell-unsafe characters.
    #[error("invalid host '{0}': host must be a valid IPv4, hostname, or .local/.ts.net address")]
    InvalidHost(String),
    /// SSH username contains invalid characters.
    #[error("invalid SSH user '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidSshUser(String),
    /// Project name or SSH alias contains path traversal or shell metacharacters.
    #[error("invalid project name '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidProjectName(String),
    /// Port `0` was requested.
    #[error("invalid port {0}: port must be between 1 and 65535")]
    InvalidPort(u16),
    /// Empty remote command string was passed to `exec`.
    #[error("remote command cannot be empty")]
    EmptyCommand,
}

/// Network transport used to reach the headless MacBook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeTransport {
    /// Tailscale WireGuard private mesh (`100.x.y.z` or `.ts.net`).
    Tailscale,
    /// Local Wi-Fi / Gigabit LAN (`192.168.x.x`, `10.x.x.x`, or `.local` mDNS).
    LocalLan,
}

/// Paired headless MacBook profile saved in `.cheap-otto/peer.json` on the Windows PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerNode {
    /// Short SSH host alias (e.g., `"otto-mac"`).
    pub alias: String,
    /// Reachable IPv4, `.local` mDNS hostname, or `.ts.net` MagicDNS name.
    pub host: String,
    /// Remote macOS login username.
    pub ssh_user: String,
    /// Remote SSH daemon port (default `22`).
    pub ssh_port: u16,
    /// Base workspace directory on the MacBook (`~/otto-workspaces`).
    pub remote_workspace_root: String,
    /// Detected network transport tier.
    pub transport: BridgeTransport,
}

/// Plan for syncing the active Windows project directory to the MacBook over SSH + Git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSyncPlan {
    /// Sanitized repository folder name.
    pub project_name: String,
    /// Auto-detected local Git branch being mirrored (if known).
    #[serde(default)]
    pub source_branch: Option<String>,
    /// Target directory on the MacBook (`~/otto-workspaces/<project_name>`).
    pub remote_project_dir: String,
    /// Local Git remote alias (`peer.alias`).
    pub git_remote_name: String,
    /// Full `ssh://` Git push URL.
    pub git_remote_url: String,
    /// Heavy/OS-specific build paths inherently skipped by Git push.
    pub excluded_paths: Vec<String>,
    /// Whether uncommitted manual edits on the MacBook are forcibly overwritten.
    #[serde(default)]
    pub force_overwrite: bool,
    /// Human-readable command lines for dry-run display.
    pub sync_commands: Vec<String>,
    /// Shell-free process specifications executed when `--execute` is passed.
    #[serde(default)]
    pub sync_specs: Vec<CommandSpec>,
}

/// Plan for forwarding ports from the headless MacBook back to `127.0.0.1` on Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortTunnelPlan {
    /// Forwarded localhost TCP ports.
    pub ports: Vec<u16>,
    /// Human-readable `ssh -N -L ...` command.
    pub ssh_tunnel_command: String,
    /// Rendered `~/.ssh/config` `Host` block.
    pub ssh_config_snippet: String,
    /// Shell-free process specification for launching the port tunnel directly.
    #[serde(default)]
    pub tunnel_spec: Option<CommandSpec>,
}

/// Validates a safe identifier (username, alias, or project folder name) to prevent shell injection.
pub(crate) fn validate_safe_identifier(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('-')
        && !trimmed.contains("..")
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Idempotently merges or updates a managed `cheap-otto` `Host` block inside `~/.ssh/config`.
///
/// # Errors
/// Returns [`BridgeError::InvalidProjectName`] if `alias` is not a safe identifier.
#[must_use = "merged SSH config text should be written back to ~/.ssh/config"]
pub fn merge_ssh_config_block(
    existing_config: &str,
    alias: &str,
    snippet: &str,
) -> Result<String, BridgeError> {
    if !validate_safe_identifier(alias) {
        return Err(BridgeError::InvalidProjectName(alias.to_string()));
    }
    let clean_alias = alias.trim();
    let begin_marker = format!("# --- BEGIN cheap-otto ({clean_alias}) ---");
    let end_marker = format!("# --- END cheap-otto ({clean_alias}) ---");
    let managed_block = format!(
        "{begin_marker}\n{}\n{end_marker}\n",
        snippet.trim_end_matches(['\r', '\n'])
    );

    if let (Some(start_idx), Some(end_rel_idx)) = (
        existing_config.find(&begin_marker),
        existing_config.find(&end_marker),
    ) && end_rel_idx >= start_idx
    {
        let after_end = end_rel_idx + end_marker.len();
        let trailing = existing_config[after_end..]
            .strip_prefix("\r\n")
            .or_else(|| existing_config[after_end..].strip_prefix('\n'))
            .unwrap_or(&existing_config[after_end..]);
        let mut updated =
            String::with_capacity(existing_config.len().saturating_add(managed_block.len()));
        updated.push_str(&existing_config[..start_idx]);
        updated.push_str(&managed_block);
        updated.push_str(trailing);
        return Ok(updated);
    }

    let mut appended = String::with_capacity(
        existing_config
            .len()
            .saturating_add(managed_block.len() + 2),
    );
    appended.push_str(existing_config);
    if !appended.is_empty() && !appended.ends_with('\n') {
        appended.push('\n');
    }
    if !appended.is_empty() && !appended.ends_with("\n\n") {
        appended.push('\n');
    }
    appended.push_str(&managed_block);
    Ok(appended)
}

impl PeerNode {
    /// Creates and validates a paired headless MacBook profile.
    ///
    /// # Errors
    /// Returns [`BridgeError`] if `host`, `ssh_user`, `alias`, or `ssh_port` is invalid.
    pub fn new(
        alias: &str,
        host: &str,
        ssh_user: &str,
        ssh_port: u16,
    ) -> Result<Self, BridgeError> {
        let trimmed_host = host.trim();
        if trimmed_host.is_empty()
            || trimmed_host.starts_with('-')
            || !trimmed_host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(BridgeError::InvalidHost(host.to_string()));
        }

        if !validate_safe_identifier(ssh_user) {
            return Err(BridgeError::InvalidSshUser(ssh_user.to_string()));
        }

        let resolved_alias = if alias.trim().is_empty() {
            "otto-mac"
        } else {
            alias.trim()
        };
        if !validate_safe_identifier(resolved_alias) {
            return Err(BridgeError::InvalidProjectName(alias.to_string()));
        }

        if ssh_port == 0 {
            return Err(BridgeError::InvalidPort(ssh_port));
        }

        let transport = if trimmed_host.starts_with("100.") || trimmed_host.ends_with(".ts.net") {
            BridgeTransport::Tailscale
        } else {
            BridgeTransport::LocalLan
        };

        Ok(Self {
            alias: resolved_alias.to_string(),
            host: trimmed_host.to_string(),
            ssh_user: ssh_user.trim().to_string(),
            ssh_port,
            remote_workspace_root: "~/otto-workspaces".to_string(),
            transport,
        })
    }

    /// Returns the `user@host` SSH target string.
    #[must_use = "returns the formatted SSH destination"]
    pub fn ssh_destination(&self) -> String {
        format!("{}@{}", self.ssh_user, self.host)
    }

    /// Returns the remote directory path for a given project on the MacBook.
    ///
    /// # Errors
    /// Returns [`BridgeError::InvalidProjectName`] if `project_name` contains unsafe characters.
    pub fn remote_project_dir(&self, project_name: &str) -> Result<String, BridgeError> {
        if !validate_safe_identifier(project_name) {
            return Err(BridgeError::InvalidProjectName(project_name.to_string()));
        }
        Ok(format!(
            "{}/{}",
            self.remote_workspace_root,
            project_name.trim()
        ))
    }

    /// Builds a fast, non-interactive SSH probe command (`BatchMode=yes`, `ConnectTimeout=3`).
    #[must_use = "probe spec should be executed to collect remote telemetry"]
    pub fn build_ssh_probe_spec(&self, description: &str, remote_command: &str) -> CommandSpec {
        let port_str = self.ssh_port.to_string();
        let dest = self.ssh_destination();
        CommandSpec::new(
            description,
            "ssh",
            [
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=3",
                "-p",
                &port_str,
                &dest,
                remote_command,
            ],
        )
    }

    /// Builds a deterministic Git + SSH workspace sync plan from Windows to the MacBook.
    ///
    /// # Errors
    /// Returns [`BridgeError::InvalidProjectName`] if `project_name` is invalid.
    #[must_use = "workspace sync plan should be executed or displayed"]
    pub fn build_sync_plan(&self, project_name: &str) -> Result<WorkspaceSyncPlan, BridgeError> {
        self.build_sync_plan_for_branch(project_name, None)
    }

    /// Builds a deterministic Git + SSH workspace sync plan annotated with an optional local branch.
    ///
    /// By default (`force_overwrite = false`), refuses to clobber uncommitted manual edits inside
    /// `~/otto-workspaces/<project>` on the MacBook.
    ///
    /// # Errors
    /// Returns [`BridgeError::InvalidProjectName`] if `project_name` is invalid.
    #[must_use = "workspace sync plan should be executed or displayed"]
    pub fn build_sync_plan_for_branch(
        &self,
        project_name: &str,
        source_branch: Option<&str>,
    ) -> Result<WorkspaceSyncPlan, BridgeError> {
        self.build_sync_plan_with_options(project_name, source_branch, false)
    }

    /// Builds a deterministic Git + SSH workspace sync plan with explicit dirty-worktree handling.
    ///
    /// When `force_overwrite` is `false`, step 1 verifies `git status --porcelain` is empty on the
    /// MacBook before pushing so manual edits made via `cheap-otto shell` are never lost silently.
    ///
    /// # Errors
    /// Returns [`BridgeError::InvalidProjectName`] if `project_name` is invalid.
    #[must_use = "workspace sync plan should be executed or displayed"]
    pub fn build_sync_plan_with_options(
        &self,
        project_name: &str,
        source_branch: Option<&str>,
        force_overwrite: bool,
    ) -> Result<WorkspaceSyncPlan, BridgeError> {
        let remote_project_dir = self.remote_project_dir(project_name)?;
        let clean_project = project_name.trim().to_string();
        let git_remote_name = self.alias.clone();
        let git_remote_url = format!(
            "ssh://{}@{}:{}/~/otto-workspaces/{}",
            self.ssh_user, self.host, self.ssh_port, clean_project
        );

        let excluded_paths = vec![
            "target/".to_string(),
            "node_modules/".to_string(),
            ".git/index.lock".to_string(),
            ".cheap-otto/".to_string(),
            ".DS_Store".to_string(),
        ];

        let port_str = self.ssh_port.to_string();
        let dest = self.ssh_destination();
        let remote_init_script = if force_overwrite {
            format!(
                "mkdir -p {remote_project_dir} && git -C {remote_project_dir} init && git -C {remote_project_dir} config receive.denyCurrentBranch updateInstead && if git -C {remote_project_dir} rev-parse --verify HEAD >/dev/null 2>&1; then git -C {remote_project_dir} reset --hard HEAD && git -C {remote_project_dir} clean -fd; fi"
            )
        } else {
            format!(
                "mkdir -p {remote_project_dir} && git -C {remote_project_dir} init && git -C {remote_project_dir} config receive.denyCurrentBranch updateInstead && if [ -n \"$(git -C {remote_project_dir} status --porcelain 2>/dev/null)\" ]; then echo 'Refusing to overwrite uncommitted manual changes in {remote_project_dir}; commit/stash via cheap-otto shell or pass --force.' >&2; exit 2; fi"
            )
        };
        let remote_checkout_script =
            format!("git -C {remote_project_dir} checkout -f -B otto-sync refs/heads/otto-sync");

        let sync_commands = vec![
            format!("ssh -p {port_str} {dest} \"{remote_init_script}\""),
            format!(
                "git remote add {git_remote_name} {git_remote_url} 2>$null; git remote set-url {git_remote_name} {git_remote_url}"
            ),
            format!("git push {git_remote_name} HEAD:refs/heads/otto-sync --force"),
            format!("ssh -p {port_str} {dest} \"{remote_checkout_script}\""),
        ];

        let sync_specs = vec![
            CommandSpec::new(
                "Initialize remote Git workspace on MacBook (checking for uncommitted manual edits)",
                "ssh",
                ["-p", &port_str, &dest, &remote_init_script],
            ),
            CommandSpec::new(
                "Register paired MacBook Git remote (if not already present)",
                "git",
                ["remote", "add", &git_remote_name, &git_remote_url],
            )
            .with_allow_failure(true),
            CommandSpec::new(
                "Ensure paired MacBook Git remote URL is up to date",
                "git",
                ["remote", "set-url", &git_remote_name, &git_remote_url],
            ),
            CommandSpec::new(
                "Push current HEAD commit to MacBook otto-sync branch",
                "git",
                [
                    "push",
                    &git_remote_name,
                    "HEAD:refs/heads/otto-sync",
                    "--force",
                ],
            ),
            CommandSpec::new(
                "Check out synced working tree on MacBook",
                "ssh",
                ["-p", &port_str, &dest, &remote_checkout_script],
            ),
        ];

        Ok(WorkspaceSyncPlan {
            project_name: clean_project,
            source_branch: source_branch.map(str::to_string),
            remote_project_dir,
            git_remote_name,
            git_remote_url,
            excluded_paths,
            force_overwrite,
            sync_commands,
            sync_specs,
        })
    }

    /// Builds a remote execution command string for human-readable display.
    ///
    /// # Errors
    /// Returns [`BridgeError`] if `project_name` is invalid or `command` is empty.
    #[must_use = "remote exec command should be executed or displayed"]
    pub fn build_remote_exec(
        &self,
        project_name: &str,
        command: &str,
    ) -> Result<String, BridgeError> {
        let remote_dir = self.remote_project_dir(project_name)?;
        let trimmed_cmd = command.trim();
        if trimmed_cmd.is_empty() {
            return Err(BridgeError::EmptyCommand);
        }
        let escaped_cmd = trimmed_cmd.replace('"', "\\\"");
        Ok(format!(
            "ssh -t -p {} {} \"cd {remote_dir} && source ~/.zshrc 2>/dev/null || true; source ~/.cargo/env 2>/dev/null || true; {escaped_cmd}\"",
            self.ssh_port,
            self.ssh_destination()
        ))
    }

    /// Builds a shell-free [`CommandSpec`] to execute `command` inside the synced MacBook workspace.
    ///
    /// # Errors
    /// Returns [`BridgeError`] if `project_name` is invalid or `command` is empty.
    #[must_use = "remote exec spec should be executed via std::process::Command"]
    pub fn build_remote_exec_spec(
        &self,
        project_name: &str,
        command: &str,
    ) -> Result<CommandSpec, BridgeError> {
        let remote_dir = self.remote_project_dir(project_name)?;
        let trimmed_cmd = command.trim();
        if trimmed_cmd.is_empty() {
            return Err(BridgeError::EmptyCommand);
        }
        let port_str = self.ssh_port.to_string();
        let dest = self.ssh_destination();
        let remote_script = format!(
            "cd {remote_dir} && source ~/.zshrc 2>/dev/null || true; source ~/.cargo/env 2>/dev/null || true; {trimmed_cmd}"
        );
        Ok(CommandSpec::new(
            "Execute command inside synced MacBook workspace",
            "ssh",
            ["-t", "-p", &port_str, &dest, &remote_script],
        ))
    }

    /// Builds a localhost port-forwarding tunnel plan for the given ports.
    ///
    /// # Errors
    /// Returns [`BridgeError::InvalidPort`] if any port is `0`.
    #[must_use = "port tunnel plan should be displayed or executed"]
    pub fn build_port_tunnel(&self, ports: &[u16]) -> Result<PortTunnelPlan, BridgeError> {
        for &port in ports {
            if port == 0 {
                return Err(BridgeError::InvalidPort(0));
            }
        }

        let port_str = self.ssh_port.to_string();
        let dest = self.ssh_destination();
        let mut forward_flags = String::new();
        let mut ssh_config_forwards = String::new();
        let mut tunnel_args = Vec::with_capacity(4 + ports.len().saturating_mul(2));
        tunnel_args.push("-N".to_string());
        tunnel_args.push("-p".to_string());
        tunnel_args.push(port_str);

        for &port in ports {
            let bind_expr = format!("127.0.0.1:{port}:127.0.0.1:{port}");
            forward_flags.push_str(&format!(" -L {bind_expr}"));
            ssh_config_forwards.push_str(&format!(
                "  LocalForward 127.0.0.1:{port} 127.0.0.1:{port}\n"
            ));
            tunnel_args.push("-L".to_string());
            tunnel_args.push(bind_expr);
        }
        tunnel_args.push(dest.clone());

        let ssh_tunnel_command = format!("ssh -N -p {}{forward_flags} {dest}", self.ssh_port);

        let ssh_config_snippet = format!(
            "Host {}\n\
             \x20 HostName {}\n\
             \x20 User {}\n\
             \x20 Port {}\n\
             \x20 ServerAliveInterval 30\n\
             \x20 ServerAliveCountMax 3\n\
             {ssh_config_forwards}",
            self.alias, self.host, self.ssh_user, self.ssh_port
        );

        let tunnel_spec = CommandSpec::new(
            "Forward remote MacBook localhost ports to Windows 127.0.0.1",
            "ssh",
            tunnel_args,
        );

        Ok(PortTunnelPlan {
            ports: ports.to_vec(),
            ssh_tunnel_command,
            ssh_config_snippet,
            tunnel_spec: Some(tunnel_spec),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_peer_node_and_builds_git_ssh_sync_plan() -> Result<(), BridgeError> {
        let peer = PeerNode::new("otto-mac", "192.168.1.42", "admin", 22)?;
        assert_eq!(peer.transport, BridgeTransport::LocalLan);
        assert_eq!(peer.ssh_destination(), "admin@192.168.1.42");

        let sync = peer.build_sync_plan_for_branch("cheap-otto", Some("main"))?;
        assert_eq!(sync.remote_project_dir, "~/otto-workspaces/cheap-otto");
        assert_eq!(sync.source_branch.as_deref(), Some("main"));
        assert_eq!(
            sync.git_remote_url,
            "ssh://admin@192.168.1.42:22/~/otto-workspaces/cheap-otto"
        );
        assert!(sync.excluded_paths.contains(&"target/".to_string()));
        assert!(!sync.force_overwrite);
        assert!(sync.sync_specs[0].args[3].contains("status --porcelain"));
        assert_eq!(sync.sync_commands.len(), 4);
        assert_eq!(sync.sync_specs.len(), 5);
        assert!(sync.sync_specs[1].allow_failure);

        let forced = peer.build_sync_plan_with_options("cheap-otto", Some("main"), true)?;
        assert!(forced.force_overwrite);
        assert!(!forced.sync_specs[0].args[3].contains("status --porcelain"));
        assert!(forced.sync_specs[4].args[3].contains("checkout -f -B otto-sync"));
        Ok(())
    }

    #[test]
    fn builds_remote_exec_and_port_tunnel_commands() -> Result<(), BridgeError> {
        let peer = PeerNode::new("m1-air", "100.101.102.103", "admin", 22)?;
        assert_eq!(peer.transport, BridgeTransport::Tailscale);

        let exec_cmd = peer.build_remote_exec("cheap-otto", "cargo test --all-targets")?;
        assert!(exec_cmd.contains("cd ~/otto-workspaces/cheap-otto"));
        assert!(exec_cmd.contains("cargo test --all-targets"));

        let exec_spec = peer.build_remote_exec_spec("cheap-otto", "cargo test --all-targets")?;
        assert_eq!(exec_spec.program, "ssh");
        assert_eq!(exec_spec.args[0], "-t");

        let tunnel = peer.build_port_tunnel(&[3000, 5173])?;
        assert!(
            tunnel
                .ssh_tunnel_command
                .contains("-L 127.0.0.1:3000:127.0.0.1:3000")
        );
        assert!(
            tunnel
                .ssh_config_snippet
                .contains("LocalForward 127.0.0.1:5173 127.0.0.1:5173")
        );
        assert!(tunnel.tunnel_spec.is_some());
        Ok(())
    }

    #[test]
    fn merges_ssh_config_block_idempotently_without_duplicating_entries() -> Result<(), BridgeError>
    {
        let existing = "Host github.com\n  User git\n";
        let first = merge_ssh_config_block(
            existing,
            "otto-mac",
            "Host otto-mac\n  HostName 192.168.1.42\n",
        )?;
        assert!(first.contains("Host github.com"));
        assert!(first.contains("HostName 192.168.1.42"));

        let second = merge_ssh_config_block(
            &first,
            "otto-mac",
            "Host otto-mac\n  HostName 100.101.102.103\n",
        )?;
        assert!(!second.contains("192.168.1.42"));
        assert!(second.contains("100.101.102.103"));
        assert_eq!(
            second
                .matches("# --- BEGIN cheap-otto (otto-mac) ---")
                .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn rejects_unsafe_host_or_project_names() {
        let bad_host = PeerNode::new("otto-mac", "192.168.1.1; rm -rf /", "admin", 22).unwrap_err();
        assert!(matches!(bad_host, BridgeError::InvalidHost(_)));

        let peer = PeerNode::new("otto-mac", "192.168.1.42", "admin", 22)
            .unwrap_or_else(|_| unreachable!());
        let bad_proj = peer.build_sync_plan("../etc/passwd").unwrap_err();
        assert_eq!(
            bad_proj,
            BridgeError::InvalidProjectName("../etc/passwd".to_string())
        );
    }
}
