//! Cross-network control plane and workstation-to-appliance pairing.
//!
//! Enables a primary workstation (e.g., Windows PC, iPad, or phone) to discover,
//! pair with, and dispatch agent workloads to a headless `cheap-otto` appliance
//! over a local Wi-Fi network (LAN) or Tailscale private mesh (`100.x.y.z`).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when configuring or validating cross-network appliance nodes.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NetworkError {
    #[error("invalid host address '{0}': host cannot be empty or contain whitespace")]
    InvalidHost(String),
    #[error("invalid control port {0}: must be a non-privileged port between 1024 and 65535")]
    InvalidPort(u16),
    #[error("shared authentication token must be at least 16 characters long")]
    WeakSharedToken,
}

/// Network transport used between the workstation and the headless appliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkTransport {
    /// Authenticated Tailscale WireGuard mesh (`100.x.y.z` or MagicDNS).
    TailscaleMesh,
    /// Same-subnet residential Wi-Fi / Gigabit LAN (`192.168.x.x` / `10.x.x.x` / `.local`).
    LocalLan,
}

/// Connection profile for a remote `cheap-otto` appliance on the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplianceNode {
    pub node_name: String,
    pub host: String,
    pub control_port: u16,
    pub transport: NetworkTransport,
    pub ssh_user: String,
}

/// Resolved service URLs exposed by the appliance to the paired workstation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceEndpoints {
    pub control_plane_url: String,
    pub openai_compatible_base_url: String,
    pub ollama_host_url: String,
    pub embeddings_rag_url: String,
    pub whisper_transcribe_url: String,
    pub playwright_ws_endpoint: String,
    pub n8n_orchestrator_url: String,
    pub uptime_monitor_url: String,
}

/// Workloads that can be dispatched from the Windows workstation or phone to the appliance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteWorkload {
    /// Pillar 1: Pull a branch/issue labeled `agent-fix` or `triage`, run tests, and draft a PR.
    OvernightPrFix {
        repo: String,
        issue_number: u64,
        label: String,
    },
    /// Pillar 1: Run a residential-IP Playwright E2E canary check against a target URL.
    ScheduledCanary {
        suite_name: String,
        target_url: String,
    },
    /// Pillar 2: Triage recent inbox & calendar items and send a 3-bullet summary to Telegram/Slack.
    InboxTriage { lookback_minutes: u32 },
    /// Pillar 3: Transcribe and summarize a voice memo or meeting recording via local Whisper.
    VoiceTranscribe { audio_filename: String },
    /// Pillar 3: Score and filter a batch of scraped items locally with an 8B model before cloud escalation.
    BulkScraperFilter {
        batch_id: String,
        raw_item_count: u32,
    },
    /// Emergency mobile/workstation kill-switch to pause background queues and unload LLM weights.
    EngageKillSwitch { reason: String },
}

/// Signed RPC envelope sent over the LAN / Tailnet control plane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDispatchEnvelope {
    pub target_node: String,
    pub endpoint_url: String,
    pub requires_ci_ram_preemption: bool,
    pub workload: RemoteWorkload,
}

impl ApplianceNode {
    /// Creates and validates a new `ApplianceNode` connection profile.
    ///
    /// Automatically detects `NetworkTransport::TailscaleMesh` when the host is in
    /// the Tailscale CGNAT range (`100.`) or ends with `.ts.net`.
    ///
    /// # Errors
    /// Returns [`NetworkError::InvalidHost`] if `host` is empty or contains whitespace/control chars,
    /// or [`NetworkError::InvalidPort`] if `control_port < 1024`.
    pub fn new(
        node_name: &str,
        host: &str,
        control_port: u16,
        ssh_user: &str,
    ) -> Result<Self, NetworkError> {
        let trimmed_host = host.trim();
        if trimmed_host.is_empty()
            || trimmed_host.chars().any(char::is_whitespace)
            || trimmed_host.contains('/')
        {
            return Err(NetworkError::InvalidHost(host.to_string()));
        }
        if control_port < 1024 {
            return Err(NetworkError::InvalidPort(control_port));
        }

        let transport = if trimmed_host.starts_with("100.") || trimmed_host.ends_with(".ts.net") {
            NetworkTransport::TailscaleMesh
        } else {
            NetworkTransport::LocalLan
        };

        let resolved_name = if node_name.trim().is_empty() {
            "cheap-otto-m1"
        } else {
            node_name.trim()
        };

        let resolved_user = if ssh_user.trim().is_empty() {
            "otto"
        } else {
            ssh_user.trim()
        };

        Ok(Self {
            node_name: resolved_name.to_string(),
            host: trimmed_host.to_string(),
            control_port,
            transport,
            ssh_user: resolved_user.to_string(),
        })
    }

    /// Computes all service endpoints reachable from a paired workstation on the same LAN or Tailnet.
    #[must_use = "service endpoints should be saved to workstation config or displayed"]
    pub fn endpoints(&self) -> ServiceEndpoints {
        let h = &self.host;
        ServiceEndpoints {
            control_plane_url: format!("http://{h}:{}", self.control_port),
            openai_compatible_base_url: format!("http://{h}:11434/v1"),
            ollama_host_url: format!("http://{h}:11434"),
            embeddings_rag_url: format!("http://{h}:11434/api/embeddings"),
            whisper_transcribe_url: format!("http://{h}:8089/v1/audio/transcriptions"),
            playwright_ws_endpoint: format!("ws://{h}:3003"),
            n8n_orchestrator_url: format!("http://{h}:5678"),
            uptime_monitor_url: format!("http://{h}:3001"),
        }
    }

    /// Builds a deterministic dispatch envelope for a remote workload.
    #[must_use = "dispatch envelope must be transmitted or logged"]
    pub fn build_dispatch(&self, workload: RemoteWorkload) -> RemoteDispatchEnvelope {
        let requires_ci_ram_preemption = matches!(workload, RemoteWorkload::OvernightPrFix { .. });
        let endpoint_url = format!("http://{}:{}/v1/dispatch", self.host, self.control_port);

        RemoteDispatchEnvelope {
            target_node: self.node_name.clone(),
            endpoint_url,
            requires_ci_ram_preemption,
            workload,
        }
    }

    /// Renders a `.env` file for the main Windows/Linux/macOS workstation so local coding tools,
    /// scripts, and AI agents automatically route inference, RAG, Whisper, and Playwright traffic
    /// to the headless M1 appliance over the network.
    #[must_use = "workstation environment file should be written to disk or printed"]
    pub fn render_workstation_env(&self) -> String {
        let ep = self.endpoints();
        format!(
            "# Generated by cheap-otto — Workstation-to-Appliance Network Bridge\n\
             # Node: {} ({:?})\n\
             CHEAP_OTTO_NODE_NAME=\"{}\"\n\
             CHEAP_OTTO_CONTROL_URL=\"{}\"\n\
             OPENAI_BASE_URL=\"{}\"\n\
             OPENAI_API_KEY=\"cheap-otto-local\"\n\
             OLLAMA_HOST=\"{}\"\n\
             CHEAP_OTTO_RAG_EMBEDDINGS_URL=\"{}\"\n\
             CHEAP_OTTO_WHISPER_URL=\"{}\"\n\
             PLAYWRIGHT_WS_ENDPOINT=\"{}\"\n\
             CHEAP_OTTO_N8N_URL=\"{}\"\n",
            self.node_name,
            self.transport,
            self.node_name,
            ep.control_plane_url,
            ep.openai_compatible_base_url,
            ep.ollama_host_url,
            ep.embeddings_rag_url,
            ep.whisper_transcribe_url,
            ep.playwright_ws_endpoint,
            ep.n8n_orchestrator_url,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tailscale_mesh_transport_and_builds_endpoints() -> Result<(), NetworkError> {
        let node = ApplianceNode::new("m1-air-drawer", "100.101.102.103", 9400, "admin")?;
        assert_eq!(node.transport, NetworkTransport::TailscaleMesh);

        let ep = node.endpoints();
        assert_eq!(ep.control_plane_url, "http://100.101.102.103:9400");
        assert_eq!(
            ep.openai_compatible_base_url,
            "http://100.101.102.103:11434/v1"
        );
        assert_eq!(ep.playwright_ws_endpoint, "ws://100.101.102.103:3003");
        assert_eq!(
            ep.whisper_transcribe_url,
            "http://100.101.102.103:8089/v1/audio/transcriptions"
        );
        Ok(())
    }

    #[test]
    fn detects_local_lan_transport_for_private_subnet_ip() -> Result<(), NetworkError> {
        let node = ApplianceNode::new("m1-lan", "192.168.1.42", 9400, "admin")?;
        assert_eq!(node.transport, NetworkTransport::LocalLan);
        let env_file = node.render_workstation_env();
        assert!(env_file.contains("OPENAI_BASE_URL=\"http://192.168.1.42:11434/v1\""));
        assert!(env_file.contains("PLAYWRIGHT_WS_ENDPOINT=\"ws://192.168.1.42:3003\""));
        Ok(())
    }

    #[test]
    fn flags_ci_ram_preemption_when_dispatching_overnight_pr_fix() -> Result<(), NetworkError> {
        let node = ApplianceNode::new("m1-air", "100.64.0.10", 9400, "admin")?;
        let dispatch = node.build_dispatch(RemoteWorkload::OvernightPrFix {
            repo: "cokitron/cheap-otto".to_string(),
            issue_number: 12,
            label: "agent-fix".to_string(),
        });
        assert!(dispatch.requires_ci_ram_preemption);
        assert_eq!(dispatch.endpoint_url, "http://100.64.0.10:9400/v1/dispatch");
        Ok(())
    }

    #[test]
    fn rejects_invalid_host_and_privileged_port() {
        let bad_host = ApplianceNode::new("m1", "bad host/path", 9400, "admin").unwrap_err();
        assert_eq!(
            bad_host,
            NetworkError::InvalidHost("bad host/path".to_string())
        );

        let bad_port = ApplianceNode::new("m1", "192.168.1.50", 80, "admin").unwrap_err();
        assert_eq!(bad_port, NetworkError::InvalidPort(80));
    }
}
