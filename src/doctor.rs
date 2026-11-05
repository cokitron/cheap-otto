use crate::hardware::ThermalLevel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckItem {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProbeInput {
    pub on_ac_power: bool,
    pub lid_sleep_disabled: bool,
    pub battery_ceiling_percent: u8,
    pub thermal_level: ThermalLevel,
    pub tailscale_connected: bool,
    pub tailscale_ip: Option<String>,
    pub container_runtime_ready: bool,
    pub disk_free_percent: u8,
    pub local_ai_responding: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub checks: Vec<HealthCheckItem>,
    pub overall_healthy: bool,
}

impl SystemProbeInput {
    /// Evaluates the 8 core appliance invariants and returns a structured `DoctorReport`.
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
                    "Run `cheap-otto init` or `sudo pmset -a disablesleep 1`.".to_string(),
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
                status: CheckStatus::Fail,
                detail: "Tailscale tunnel is offline".to_string(),
                remediation: Some(
                    "Run `tailscale up --ssh` to connect your appliance.".to_string(),
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
}
