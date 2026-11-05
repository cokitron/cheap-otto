//! Pillar 4: Remote Dev Environment From Anywhere (Windows PC / iPad / Phone / Laptop).
//!
//! Generates SSH config snippets, persistent `tmux` session commands, and VS Code Remote Tunnel
//! configurations for connecting to the always-awake M1 appliance over Tailscale or LAN.

use crate::network::ApplianceNode;
use serde::{Deserialize, Serialize};

/// Complete remote development access kit for a paired [`ApplianceNode`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDevAccessKit {
    pub ssh_config_snippet: String,
    pub ssh_tmux_attach_command: String,
    pub vscode_tunnel_service_command: String,
    pub tailscale_up_command: String,
}

impl RemoteDevAccessKit {
    /// Builds a deterministic [`RemoteDevAccessKit`] for connecting to the appliance from any device.
    #[must_use = "remote dev access kit should be displayed or saved for the operator"]
    pub fn for_node(node: &ApplianceNode) -> Self {
        let ssh_config_snippet = format!(
            "Host {}\n\
             \x20 HostName {}\n\
             \x20 User {}\n\
             \x20 ServerAliveInterval 30\n\
             \x20 ServerAliveCountMax 3\n\
             \x20 LocalForward 11434 127.0.0.1:11434\n\
             \x20 LocalForward 8089 127.0.0.1:8089\n\
             \x20 LocalForward 3003 127.0.0.1:3003\n\
             \x20 LocalForward 5678 127.0.0.1:5678\n",
            node.node_name, node.host, node.ssh_user
        );

        let ssh_tmux_attach_command = format!(
            "ssh -t {}@{} \"tmux new-session -A -s cheap-otto\"",
            node.ssh_user, node.host
        );

        let vscode_tunnel_service_command = format!(
            "code tunnel --accept-server-license-terms --name {}",
            node.node_name
        );

        let tailscale_up_command = "sudo tailscale up --ssh".to_string();

        Self {
            ssh_config_snippet,
            ssh_tmux_attach_command,
            vscode_tunnel_service_command,
            tailscale_up_command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_ssh_port_forwards_and_tmux_attach_command() {
        let node = ApplianceNode::new("m1-air", "100.88.10.5", 9400, "admin")
            .unwrap_or_else(|_| unreachable!());
        let kit = RemoteDevAccessKit::for_node(&node);

        assert!(kit.ssh_config_snippet.contains("Host m1-air"));
        assert!(kit.ssh_config_snippet.contains("HostName 100.88.10.5"));
        assert!(
            kit.ssh_config_snippet
                .contains("LocalForward 11434 127.0.0.1:11434")
        );
        assert!(
            kit.ssh_config_snippet
                .contains("LocalForward 3003 127.0.0.1:3003")
        );
        assert_eq!(
            kit.ssh_tmux_attach_command,
            "ssh -t admin@100.88.10.5 \"tmux new-session -A -s cheap-otto\""
        );
        assert_eq!(
            kit.vscode_tunnel_service_command,
            "code tunnel --accept-server-license-terms --name m1-air"
        );
    }
}
