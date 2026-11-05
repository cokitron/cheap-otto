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

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when configuring or validating the Cross-OS Dev Bridge.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BridgeError {
    #[error("invalid host '{0}': host must be a valid IPv4, hostname, or .local/.ts.net address")]
    InvalidHost(String),
    #[error("invalid SSH user '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidSshUser(String),
    #[error("invalid project name '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidProjectName(String),
    #[error("invalid port {0}: port must be between 1 and 65535")]
    InvalidPort(u16),
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
    pub alias: String,
    pub host: String,
    pub ssh_user: String,
    pub ssh_port: u16,
    pub remote_workspace_root: String,
    pub transport: BridgeTransport,
}

/// Plan for syncing the active Windows project directory to the MacBook over SSH + Git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSyncPlan {
    pub project_name: String,
    pub remote_project_dir: String,
    pub git_remote_name: String,
    pub git_remote_url: String,
    pub excluded_paths: Vec<String>,
    pub sync_commands: Vec<String>,
}

/// Plan for forwarding ports from the headless MacBook back to `127.0.0.1` on Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortTunnelPlan {
    pub ports: Vec<u16>,
    pub ssh_tunnel_command: String,
    pub ssh_config_snippet: String,
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

    /// Builds a deterministic Git + SSH workspace sync plan from Windows to the MacBook.
    ///
    /// # Errors
    /// Returns [`BridgeError::InvalidProjectName`] if `project_name` is invalid.
    #[must_use = "workspace sync plan should be executed or displayed"]
    pub fn build_sync_plan(&self, project_name: &str) -> Result<WorkspaceSyncPlan, BridgeError> {
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

        let sync_commands = vec![
            format!(
                "ssh -p {} {} \"mkdir -p {remote_project_dir} && git -C {remote_project_dir} init && git -C {remote_project_dir} config receive.denyCurrentBranch updateInstead\"",
                self.ssh_port,
                self.ssh_destination()
            ),
            format!(
                "git remote add {git_remote_name} {git_remote_url} 2>$null; git remote set-url {git_remote_name} {git_remote_url}"
            ),
            format!("git push {git_remote_name} HEAD:refs/heads/otto-sync --force"),
            format!(
                "ssh -p {} {} \"git -C {remote_project_dir} checkout -B otto-sync\"",
                self.ssh_port,
                self.ssh_destination()
            ),
        ];

        Ok(WorkspaceSyncPlan {
            project_name: clean_project,
            remote_project_dir,
            git_remote_name,
            git_remote_url,
            excluded_paths,
            sync_commands,
        })
    }

    /// Builds a remote execution command to run a build or test command (`cargo test`, `npm test`, etc.)
    /// inside the synced project directory on the MacBook and stream output back to Windows.
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

        let mut forward_flags = String::new();
        let mut ssh_config_forwards = String::new();
        for &port in ports {
            forward_flags.push_str(&format!(" -L 127.0.0.1:{port}:127.0.0.1:{port}"));
            ssh_config_forwards.push_str(&format!(
                "  LocalForward 127.0.0.1:{port} 127.0.0.1:{port}\n"
            ));
        }

        let ssh_tunnel_command = format!(
            "ssh -N -p {}{} {}",
            self.ssh_port,
            forward_flags,
            self.ssh_destination()
        );

        let ssh_config_snippet = format!(
            "Host {}\n\
             \x20 HostName {}\n\
             \x20 User {}\n\
             \x20 Port {}\n\
             \x20 ServerAliveInterval 30\n\
             \x20 ServerAliveCountMax 3\n\
             {}",
            self.alias, self.host, self.ssh_user, self.ssh_port, ssh_config_forwards
        );

        Ok(PortTunnelPlan {
            ports: ports.to_vec(),
            ssh_tunnel_command,
            ssh_config_snippet,
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

        let sync = peer.build_sync_plan("cheap-otto")?;
        assert_eq!(sync.remote_project_dir, "~/otto-workspaces/cheap-otto");
        assert_eq!(
            sync.git_remote_url,
            "ssh://admin@192.168.1.42:22/~/otto-workspaces/cheap-otto"
        );
        assert!(sync.excluded_paths.contains(&"target/".to_string()));
        assert_eq!(sync.sync_commands.len(), 4);
        Ok(())
    }

    #[test]
    fn builds_remote_exec_and_port_tunnel_commands() -> Result<(), BridgeError> {
        let peer = PeerNode::new("m1-air", "100.101.102.103", "admin", 22)?;
        assert_eq!(peer.transport, BridgeTransport::Tailscale);

        let exec_cmd = peer.build_remote_exec("cheap-otto", "cargo test --all-targets")?;
        assert!(exec_cmd.contains("cd ~/otto-workspaces/cheap-otto"));
        assert!(exec_cmd.contains("cargo test --all-targets"));

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
