//! Unified Memory & Passive Thermal Arbiter for fanless 24/7 laptops.

use crate::hardware::ThermalLevel;
use serde::{Deserialize, Serialize};

/// Point-in-time system telemetry snapshot evaluated by [`ArbiterPolicy`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArbiterSnapshot {
    /// Observed passive or active thermal pressure tier.
    pub thermal_level: ThermalLevel,
    /// Reclaimable + free unified memory in MiB.
    pub available_memory_mb: u32,
    /// Whether a heavy CI build or delegated worktree verification is running.
    pub ci_build_running: bool,
    /// Whether local LLM weights are currently resident in unified memory.
    pub local_llm_loaded: bool,
    /// Seconds elapsed since the last local LLM inference call.
    pub local_llm_idle_secs: u32,
    /// Whether the operator has engaged the mobile/remote kill-switch.
    pub kill_switch_engaged: bool,
}

/// Deterministic remediation or scheduling action emitted by [`ArbiterPolicy::evaluate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArbiterAction {
    /// Evict cached local LLM weights from unified RAM immediately.
    EvictLocalLlm { reason: String },
    /// Pause background batch queues (scraping, bulk embeddings, CI) until thermals or kill-switch clear.
    PauseBackgroundQueues { reason: String },
    /// Resume normal scheduling across all enabled capability packs.
    AllowAllWorkloads,
}

/// Configurable thresholds governing unified memory eviction and thermal preemption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArbiterPolicy {
    /// Idle duration in seconds before resident LLM weights are evicted (`300s` default).
    pub llm_idle_timeout_secs: u32,
    /// Minimum free unified memory floor in MiB (`2048 MB` default).
    pub min_free_memory_mb: u32,
}

impl Default for ArbiterPolicy {
    fn default() -> Self {
        Self {
            llm_idle_timeout_secs: 300,
            min_free_memory_mb: 2048,
        }
    }
}

impl ArbiterPolicy {
    /// Evaluates current unified memory, thermal pressure, and active jobs to produce
    /// ordered arbitration actions that keep a fanless laptop responsive 24/7.
    #[must_use = "arbiter decisions must be applied to running services"]
    pub fn evaluate(&self, snapshot: &ArbiterSnapshot) -> Vec<ArbiterAction> {
        if snapshot.kill_switch_engaged {
            let mut actions = vec![ArbiterAction::PauseBackgroundQueues {
                reason: "mobile kill-switch engaged by operator".to_string(),
            }];
            if snapshot.local_llm_loaded {
                actions.push(ArbiterAction::EvictLocalLlm {
                    reason: "mobile kill-switch engaged".to_string(),
                });
            }
            return actions;
        }

        let mut actions = Vec::new();

        // Rule 1: Fanless Thermal Guard (pause heavy queues when thermal state >= Serious)
        if snapshot.thermal_level >= ThermalLevel::Serious {
            actions.push(ArbiterAction::PauseBackgroundQueues {
                reason: format!(
                    "fanless thermal guard triggered ({:?}); cooling down",
                    snapshot.thermal_level
                ),
            });
            if snapshot.local_llm_loaded {
                actions.push(ArbiterAction::EvictLocalLlm {
                    reason: "thermal pressure relief".to_string(),
                });
            }
            return actions;
        }

        // Rule 2: CI Build Priority (evict 5.5GB LLM weights when a GitHub Actions / worktree job starts)
        if snapshot.ci_build_running && snapshot.local_llm_loaded {
            actions.push(ArbiterAction::EvictLocalLlm {
                reason: "CI build started; freeing unified RAM headroom".to_string(),
            });
        }

        // Rule 3: Low-Memory Pressure Guard
        if snapshot.available_memory_mb < self.min_free_memory_mb
            && snapshot.local_llm_loaded
            && !actions
                .iter()
                .any(|a| matches!(a, ArbiterAction::EvictLocalLlm { .. }))
        {
            actions.push(ArbiterAction::EvictLocalLlm {
                reason: format!(
                    "available unified memory ({} MB) below safety floor ({} MB)",
                    snapshot.available_memory_mb, self.min_free_memory_mb
                ),
            });
        }

        // Rule 4: Fast Idle Eviction (free LLM RAM after 5 mins idle)
        if snapshot.local_llm_loaded
            && snapshot.local_llm_idle_secs >= self.llm_idle_timeout_secs
            && !actions
                .iter()
                .any(|a| matches!(a, ArbiterAction::EvictLocalLlm { .. }))
        {
            actions.push(ArbiterAction::EvictLocalLlm {
                reason: format!(
                    "local LLM idle for {}s (threshold: {}s)",
                    snapshot.local_llm_idle_secs, self.llm_idle_timeout_secs
                ),
            });
        }

        if actions.is_empty() {
            actions.push(ArbiterAction::AllowAllWorkloads);
        }

        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_local_llm_when_ci_build_starts() {
        let policy = ArbiterPolicy::default();
        let snapshot = ArbiterSnapshot {
            thermal_level: ThermalLevel::Nominal,
            available_memory_mb: 6000,
            ci_build_running: true,
            local_llm_loaded: true,
            local_llm_idle_secs: 10,
            kill_switch_engaged: false,
        };

        let actions = policy.evaluate(&snapshot);
        assert_eq!(
            actions,
            vec![ArbiterAction::EvictLocalLlm {
                reason: "CI build started; freeing unified RAM headroom".to_string()
            }]
        );
    }

    #[test]
    fn pauses_queues_and_evicts_llm_on_serious_thermal_pressure() {
        let policy = ArbiterPolicy::default();
        let snapshot = ArbiterSnapshot {
            thermal_level: ThermalLevel::Serious,
            available_memory_mb: 8000,
            ci_build_running: false,
            local_llm_loaded: true,
            local_llm_idle_secs: 30,
            kill_switch_engaged: false,
        };

        let actions = policy.evaluate(&snapshot);
        assert_eq!(actions.len(), 2);
        assert!(matches!(
            actions[0],
            ArbiterAction::PauseBackgroundQueues { .. }
        ));
        assert!(matches!(actions[1], ArbiterAction::EvictLocalLlm { .. }));
    }

    #[test]
    fn evicts_idle_llm_after_timeout() {
        let policy = ArbiterPolicy::default();
        let snapshot = ArbiterSnapshot {
            thermal_level: ThermalLevel::Nominal,
            available_memory_mb: 7000,
            ci_build_running: false,
            local_llm_loaded: true,
            local_llm_idle_secs: 305,
            kill_switch_engaged: false,
        };

        let actions = policy.evaluate(&snapshot);
        assert_eq!(
            actions,
            vec![ArbiterAction::EvictLocalLlm {
                reason: "local LLM idle for 305s (threshold: 300s)".to_string()
            }]
        );
    }

    #[test]
    fn allows_all_workloads_when_healthy() {
        let policy = ArbiterPolicy::default();
        let snapshot = ArbiterSnapshot {
            thermal_level: ThermalLevel::Nominal,
            available_memory_mb: 9000,
            ci_build_running: false,
            local_llm_loaded: false,
            local_llm_idle_secs: 0,
            kill_switch_engaged: false,
        };

        let actions = policy.evaluate(&snapshot);
        assert_eq!(actions, vec![ArbiterAction::AllowAllWorkloads]);
    }
}
