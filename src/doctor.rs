//! 8-Point Headless Appliance Health Doctor & Raw OS Telemetry Parsers.

use crate::hardware::ThermalLevel;
use crate::macos::{MacOsStateBackup, parse_pmset_batt_ac_power};
use serde::{Deserialize, Serialize};

/// Outcome severity of a single health check invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    /// Invariant satisfied.
    Pass,
    /// Non-fatal degradation; appliance remains operational but recommends operator action.
    Warn,
    /// Critical invariant violated; headless reliability is compromised.
    Fail,
}

/// Origin of the telemetry snapshot evaluated by `cheap-otto doctor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeSource {
    /// Probed directly from local macOS system utilities (`pmset`, `sysctl`, `tailscale`, `df`).
    LocalMacOs,
    /// Probed live over SSH from a paired headless MacBook (`peer.json`).
    RemoteSsh { alias: String, destination: String },
    /// Deterministic offline baseline snapshot (`--offline` or when no MacBook is paired).
    SimulatedBaseline,
}

/// Single evaluated check row in a [`DoctorReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckItem {
    /// Human-readable invariant name.
    pub name: String,
    /// Pass / Warn / Fail status.
    pub status: CheckStatus,
    /// Observed telemetry summary.
    pub detail: String,
    /// Actionable remediation command or tip when status is `Warn` or `Fail`.
    pub remediation: Option<String>,
}

/// Raw command outputs gathered by the imperative shell (local macOS or remote SSH)
/// and fed into [`SystemProbeInput::from_raw_telemetry`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTelemetrySnapshot {
    /// Output of `pmset -g custom`.
    pub pmset_custom: Option<String>,
    /// Output of `pmset -g batt`.
    pub pmset_batt: Option<String>,
    /// Configured battery ceiling percentage from `.cheap-otto/state-backup.json` (if set).
    pub configured_battery_ceiling: Option<u8>,
    /// Output of thermal pressure probe (`sysctl` or thermal state string).
    pub thermal_raw: Option<String>,
    /// Output of `tailscale ip -4`.
    pub tailscale_ip_stdout: Option<String>,
    /// Whether `orb status` or `docker info` succeeded.
    pub container_runtime_ready: bool,
    /// Output of `df -P /`.
    pub df_root_stdout: Option<String>,
    /// Whether the local AI endpoint (`127.0.0.1:11434` / `ollama`) responded.
    pub local_ai_responding: bool,
    /// Optional output of `tmux list-sessions` for reporting active `otto-*` agent sessions.
    pub active_tmux_sessions_stdout: Option<String>,
}

/// Structured system probe inputs evaluated against the 8 core appliance invariants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProbeInput {
    /// Whether continuous AC wall power is connected.
    pub on_ac_power: bool,
    /// Whether `pmset disablesleep` is `1`.
    pub lid_sleep_disabled: bool,
    /// Active battery charge ceiling percentage (`<= 80%` recommended for 24/7 AC).
    pub battery_ceiling_percent: u8,
    /// Current passive/active thermal pressure tier.
    pub thermal_level: ThermalLevel,
    /// Whether Tailscale mesh networking is connected.
    pub tailscale_connected: bool,
    /// Assigned Tailscale IPv4 address (`100.x.y.z`), if connected.
    pub tailscale_ip: Option<String>,
    /// Whether OrbStack / Docker container daemon is ready.
    pub container_runtime_ready: bool,
    /// Free root NVMe storage percentage (`0..=100`).
    pub disk_free_percent: u8,
    /// Whether the local AI gateway is reachable.
    pub local_ai_responding: bool,
}

/// Structured 8-point health report produced by [`SystemProbeInput::evaluate_health`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    /// Ordered list of all 8 evaluated checks.
    pub checks: Vec<HealthCheckItem>,
    /// `true` when zero checks are in [`CheckStatus::Fail`].
    pub overall_healthy: bool,
}

/// Parses POSIX `df -P /` output to compute free disk percentage (`100 - Use%`).
#[must_use = "parsed free disk percentage populates SystemProbeInput::disk_free_percent"]
pub fn parse_df_free_percent(df_stdout: &str) -> Option<u8> {
    for line in df_stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Filesystem") {
            continue;
        }
        for token in trimmed.split_whitespace() {
            if let Some(pct_str) = token.strip_suffix('%')
                && let Ok(used) = pct_str.parse::<u16>()
            {
                let clamped_used = u8::try_from(used.min(100)).unwrap_or(100);
                return Some(100_u8.saturating_sub(clamped_used));
            }
        }
    }
    None
}

/// Parses `tailscale ip -4` output and returns the first valid `100.x.y.z` Tailscale IPv4 address.
#[must_use = "parsed Tailscale IPv4 populates SystemProbeInput::tailscale_ip"]
pub fn parse_tailscale_ipv4(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let candidate = line.trim();
        let mut octets = candidate.split('.');
        if let (Some(a), Some(b), Some(c), Some(d), None) = (
            octets.next(),
            octets.next(),
            octets.next(),
            octets.next(),
            octets.next(),
        ) && let (Ok(o1), Ok(_o2), Ok(_o3), Ok(_o4)) = (
            a.parse::<u8>(),
            b.parse::<u8>(),
            c.parse::<u8>(),
            d.parse::<u8>(),
        ) && o1 == 100
        {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Extracts active `otto-*` agent `tmux` session names from `tmux list-sessions` output.
#[must_use = "parsed otto tmux session names show active delegated jobs"]
pub fn parse_active_otto_sessions(tmux_stdout: &str) -> Vec<String> {
    tmux_stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let session = trimmed.split(':').next().unwrap_or(trimmed).trim();
            if session.starts_with("otto-") {
                Some(session.to_string())
            } else {
                None
            }
        })
        .collect()
}

impl SystemProbeInput {
    /// Converts raw OS / SSH command outputs into a validated [`SystemProbeInput`].
    #[must_use = "constructed probe input should be passed to evaluate_health"]
    pub fn from_raw_telemetry(raw: &RawTelemetrySnapshot) -> Self {
        let on_ac_power = raw
            .pmset_batt
            .as_deref()
            .and_then(parse_pmset_batt_ac_power)
            .unwrap_or(false);

        let lid_sleep_disabled = raw
            .pmset_custom
            .as_deref()
            .and_then(|txt| MacOsStateBackup::from_pmset_custom(txt, 0).ok())
            .is_some_and(|backup| backup.ac_disablesleep);

        let battery_ceiling_percent = raw.configured_battery_ceiling.unwrap_or(100);

        let thermal_level = raw
            .thermal_raw
            .as_deref()
            .and_then(ThermalLevel::parse)
            .unwrap_or(ThermalLevel::Nominal);

        let tailscale_ip = raw
            .tailscale_ip_stdout
            .as_deref()
            .and_then(parse_tailscale_ipv4);
        let tailscale_connected = tailscale_ip.is_some();

        let disk_free_percent = raw
            .df_root_stdout
            .as_deref()
            .and_then(parse_df_free_percent)
            .unwrap_or(0);

        Self {
            on_ac_power,
            lid_sleep_disabled,
            battery_ceiling_percent,
            thermal_level,
            tailscale_connected,
            tailscale_ip,
            container_runtime_ready: raw.container_runtime_ready,
            disk_free_percent,
            local_ai_responding: raw.local_ai_responding,
        }
    }

    /// Evaluates the 8 core appliance invariants and returns a structured [`DoctorReport`].
    #[must_use = "doctor report should be displayed or logged"]
    pub fn evaluate_health(&self) -> DoctorReport {
        let mut checks = Vec::with_capacity(8);

        checks.push(if self.on_ac_power {
            HealthCheckItem {
                name: "AC Power Connected".to_string(),
                status: CheckStatus::Pass,
                detail: "Running on continuous AC wall power".to_string(),
                remediation: None,
            }
        } else {
            HealthCheckItem {
                name: "AC Power Connected".to_string(),
                status: CheckStatus::Fail,
                detail: "Running on battery power; appliance will sleep when battery drains"
                    .to_string(),
                remediation: Some("Plug the MagSafe / USB-C charger into wall power.".to_string()),
            }
        });

        checks.push(if self.lid_sleep_disabled {
            HealthCheckItem {
                name: "Clamshell Lid-Sleep Disabled".to_string(),
                status: CheckStatus::Pass,
                detail: "pmset disablesleep=1 (stays awake with broken/closed screen)".to_string(),
                remediation: None,
            }
        } else {
            HealthCheckItem {
                name: "Clamshell Lid-Sleep Disabled".to_string(),
                status: CheckStatus::Fail,
                detail: "Closing the laptop lid will suspend the OS".to_string(),
                remediation: Some(
                    "Run `cheap-otto init --apply` or `sudo pmset -a disablesleep 1`.".to_string(),
                ),
            }
        });

        checks.push(if self.battery_ceiling_percent <= 80 {
            HealthCheckItem {
                name: "Battery Swell Protection".to_string(),
                status: CheckStatus::Pass,
                detail: format!(
                    "Charge ceiling capped at {}% for 24/7 AC safety",
                    self.battery_ceiling_percent
                ),
                remediation: None,
            }
        } else {
            HealthCheckItem {
                name: "Battery Swell Protection".to_string(),
                status: CheckStatus::Warn,
                detail: format!(
                    "Battery charges to {}% (risk of swelling over months on AC)",
                    self.battery_ceiling_percent
                ),
                remediation: Some(
                    "Cap battery at 75-80% via `cheap-otto init --battery-cap 78` or AlDente."
                        .to_string(),
                ),
            }
        });

        checks.push(match self.thermal_level {
            ThermalLevel::Nominal | ThermalLevel::Fair => HealthCheckItem {
                name: "Passive Thermal Headroom".to_string(),
                status: CheckStatus::Pass,
                detail: format!("Thermal pressure: {:?}", self.thermal_level),
                remediation: None,
            },
            ThermalLevel::Serious | ThermalLevel::Critical => HealthCheckItem {
                name: "Passive Thermal Headroom".to_string(),
                status: CheckStatus::Warn,
                detail: format!(
                    "Thermal throttling active ({:?}); Arbiter is pausing heavy batch queues",
                    self.thermal_level
                ),
                remediation: Some(
                    "Place the closed MacBook vertically in a stand for better passive airflow."
                        .to_string(),
                ),
            },
        });

        checks.push(if self.tailscale_connected {
            HealthCheckItem {
                name: "Tailscale Private Mesh".to_string(),
                status: CheckStatus::Pass,
                detail: format!(
                    "Reachable at {}",
                    self.tailscale_ip.as_deref().unwrap_or("100.x.y.z")
                ),
                remediation: None,
            }
        } else {
            HealthCheckItem {
                name: "Tailscale Private Mesh".to_string(),
                status: CheckStatus::Warn,
                detail: "Tailscale mesh offline (reachable on local LAN only)".to_string(),
                remediation: Some(
                    "Optional for off-LAN access: run `sudo tailscale up --ssh`.".to_string(),
                ),
            }
        });

        checks.push(if self.container_runtime_ready {
            HealthCheckItem {
                name: "Container Engine (OrbStack)".to_string(),
                status: CheckStatus::Pass,
                detail: "Lightweight container engine running".to_string(),
                remediation: None,
            }
        } else {
            HealthCheckItem {
                name: "Container Engine (OrbStack)".to_string(),
                status: CheckStatus::Fail,
                detail: "Container daemon not responding".to_string(),
                remediation: Some(
                    "Install/start OrbStack (`brew install --cask orbstack`).".to_string(),
                ),
            }
        });

        checks.push(if self.disk_free_percent >= 15 {
            HealthCheckItem {
                name: "NVMe Storage Headroom".to_string(),
                status: CheckStatus::Pass,
                detail: format!("{}% free disk space", self.disk_free_percent),
                remediation: None,
            }
        } else {
            HealthCheckItem {
                name: "NVMe Storage Headroom".to_string(),
                status: CheckStatus::Warn,
                detail: format!("Only {}% free disk space remaining", self.disk_free_percent),
                remediation: Some("Run `docker system prune` to clean unused layers.".to_string()),
            }
        });

        checks.push(if self.local_ai_responding {
            HealthCheckItem {
                name: "Local AI Gateway".to_string(),
                status: CheckStatus::Pass,
                detail: "OpenAI-compatible endpoint ready (5m auto-unload active)".to_string(),
                remediation: None,
            }
        } else {
            HealthCheckItem {
                name: "Local AI Gateway".to_string(),
                status: CheckStatus::Warn,
                detail: "Local AI gateway not started yet".to_string(),
                remediation: Some("Start stack with `docker compose up -d`.".to_string()),
            }
        });

        let overall_healthy = !checks.iter().any(|c| c.status == CheckStatus::Fail);
        DoctorReport {
            checks,
            overall_healthy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_healthy_when_all_critical_invariants_pass() {
        let probe = SystemProbeInput {
            on_ac_power: true,
            lid_sleep_disabled: true,
            battery_ceiling_percent: 78,
            thermal_level: ThermalLevel::Nominal,
            tailscale_connected: true,
            tailscale_ip: Some("100.84.12.9".to_string()),
            container_runtime_ready: true,
            disk_free_percent: 48,
            local_ai_responding: true,
        };

        let report = probe.evaluate_health();
        assert!(report.overall_healthy);
        assert_eq!(report.checks.len(), 8);
        assert!(report.checks.iter().all(|c| c.status == CheckStatus::Pass));
    }

    #[test]
    fn fails_overall_health_when_lid_sleep_is_still_enabled() {
        let probe = SystemProbeInput {
            on_ac_power: true,
            lid_sleep_disabled: false,
            battery_ceiling_percent: 78,
            thermal_level: ThermalLevel::Nominal,
            tailscale_connected: true,
            tailscale_ip: Some("100.84.12.9".to_string()),
            container_runtime_ready: true,
            disk_free_percent: 48,
            local_ai_responding: true,
        };

        let report = probe.evaluate_health();
        assert!(!report.overall_healthy);
    }

    #[test]
    fn builds_probe_input_from_raw_macos_telemetry_outputs() {
        let raw = RawTelemetrySnapshot {
            pmset_custom: Some(
                "AC Power:\n sleep 0\n disablesleep 1\n displaysleep 1\n".to_string(),
            ),
            pmset_batt: Some("Now drawing from 'AC Power'\n".to_string()),
            configured_battery_ceiling: Some(78),
            thermal_raw: Some("kern.thermalstate: 0\n".to_string()),
            tailscale_ip_stdout: Some("100.101.102.103\n".to_string()),
            container_runtime_ready: true,
            df_root_stdout: Some(
                "Filesystem 512-blocks Used Available Capacity Mounted on\n/dev/disk3s1s1 488245288 268534908 219710380 55% /\n"
                    .to_string(),
            ),
            local_ai_responding: true,
            active_tmux_sessions_stdout: Some(
                "otto-fix-auth: 1 windows (created Tue Nov 10)\nother-session: 1 windows\n"
                    .to_string(),
            ),
        };

        let probe = SystemProbeInput::from_raw_telemetry(&raw);
        assert_eq!(
            probe,
            SystemProbeInput {
                on_ac_power: true,
                lid_sleep_disabled: true,
                battery_ceiling_percent: 78,
                thermal_level: ThermalLevel::Nominal,
                tailscale_connected: true,
                tailscale_ip: Some("100.101.102.103".to_string()),
                container_runtime_ready: true,
                disk_free_percent: 45,
                local_ai_responding: true,
            }
        );

        let sessions =
            parse_active_otto_sessions(raw.active_tmux_sessions_stdout.as_deref().unwrap_or(""));
        assert_eq!(sessions, vec!["otto-fix-auth".to_string()]);
    }

    #[test]
    fn handles_malformed_or_missing_raw_telemetry_safely() {
        let empty = RawTelemetrySnapshot::default();
        let probe = SystemProbeInput::from_raw_telemetry(&empty);
        assert!(!probe.on_ac_power);
        assert!(!probe.lid_sleep_disabled);
        assert!(!probe.tailscale_connected);
        assert_eq!(probe.disk_free_percent, 0);
        assert_eq!(parse_tailscale_ipv4("192.168.1.42"), None);
        assert_eq!(parse_df_free_percent("garbage output"), None);
    }
}
