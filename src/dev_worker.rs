//! Pillar 1: Autonomous Dev & PR Worker ("Overnight Junior Engineer").
//!
//! Provides deterministic planning and evaluation for:
//! 1. Self-Hosted GitHub Actions Runner (native Apple Silicon ARM64 / x86_64 builds).
//! 2. Isolated Background Agent Sandbox (watching `agent-fix` / `triage` labels in OrbStack).
//! 3. Scheduled Canary & E2E Monitor (Playwright over home residential IP with Slack/Telegram alerts).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when validating autonomous dev worker or canary specifications.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DevWorkerError {
    #[error(
        "invalid GitHub repository slug '{0}': expected 'owner/repo' with alphanumeric/hyphen characters"
    )]
    InvalidRepoSlug(String),
    #[error(
        "unsupported trigger label '{0}': allowed labels are 'agent-fix', 'triage', or 'autofix'"
    )]
    UnsupportedLabel(String),
    #[error("invalid canary target URL '{0}': must start with http:// or https://")]
    InvalidCanaryUrl(String),
}

/// Trigger label watched by the overnight background agent sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchedLabel {
    AgentFix,
    Triage,
    Autofix,
}

impl WatchedLabel {
    /// Parses a GitHub label string into a validated [`WatchedLabel`].
    ///
    /// # Errors
    /// Returns [`DevWorkerError::UnsupportedLabel`] if the label is not one of the
    /// approved autonomous agent triggers.
    pub fn parse(raw: &str) -> Result<Self, DevWorkerError> {
        match raw.trim() {
            "agent-fix" => Ok(Self::AgentFix),
            "triage" => Ok(Self::Triage),
            "autofix" => Ok(Self::Autofix),
            other => Err(DevWorkerError::UnsupportedLabel(other.to_string())),
        }
    }

    #[must_use = "returns the canonical GitHub label string"]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentFix => "agent-fix",
            Self::Triage => "triage",
            Self::Autofix => "autofix",
        }
    }
}

/// Deterministic execution plan for an isolated OrbStack Background Agent Sandbox job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrSandboxPlan {
    pub repo_slug: String,
    pub issue_number: u64,
    pub trigger_label: WatchedLabel,
    pub branch_name: String,
    pub container_name: String,
    pub evict_local_llm_before_build: bool,
    pub sandbox_steps: Vec<String>,
}

/// Specification for a scheduled residential-IP Playwright E2E canary check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanarySpec {
    pub suite_name: String,
    pub target_url: String,
    pub cron_schedule: String,
    pub max_latency_ms: u32,
}

/// Result of a residential-IP Playwright canary check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanaryProbeResult {
    pub http_status: u16,
    pub latency_ms: u32,
    pub assertion_passed: bool,
    pub failure_reason: Option<String>,
}

/// Evaluation outcome of a canary run, deciding whether to ping the operator on Slack/Telegram.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanaryAlertDecision {
    pub healthy: bool,
    pub notify_mobile_channel: bool,
    pub alert_message: Option<String>,
}

/// Validates a `owner/repo` slug so it can be safely interpolated into container/git arguments.
fn validate_repo_slug(repo_slug: &str) -> Result<&str, DevWorkerError> {
    let trimmed = repo_slug.trim();
    let Some((owner, repo)) = trimmed.split_once('/') else {
        return Err(DevWorkerError::InvalidRepoSlug(repo_slug.to_string()));
    };
    let is_safe_segment = |s: &str| {
        !s.is_empty()
            && !s.contains('/')
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    };
    if !is_safe_segment(owner) || !is_safe_segment(repo) {
        return Err(DevWorkerError::InvalidRepoSlug(repo_slug.to_string()));
    }
    Ok(trimmed)
}

/// Builds an isolated OrbStack sandbox plan for fixing an issue or drafting a PR overnight.
///
/// # Errors
/// Returns [`DevWorkerError`] if `repo_slug` or `label` fails validation.
#[must_use = "sandbox plan must be executed or displayed"]
pub fn build_pr_sandbox_plan(
    repo_slug: &str,
    issue_number: u64,
    label: &str,
) -> Result<PrSandboxPlan, DevWorkerError> {
    let valid_repo = validate_repo_slug(repo_slug)?;
    let trigger_label = WatchedLabel::parse(label)?;
    let branch_name = format!("otto/{}/issue-{}", trigger_label.as_str(), issue_number);
    let container_name = format!("otto-sandbox-issue-{issue_number}");

    let sandbox_steps = vec![
        "cheap-otto arbiter --ci-running --llm-loaded (preempt 5.5GB LLM weights for build headroom)"
            .to_string(),
        format!(
            "docker run --rm --name {container_name} --network none -v otto-worktree:/work rust:1.85-slim"
        ),
        format!("gh repo clone {valid_repo} /work/repo -- --depth=50"),
        format!("git -C /work/repo checkout -b {branch_name}"),
        "cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test"
            .to_string(),
        format!(
            "gh pr create --repo {valid_repo} --draft --title \"fix(#{issue_number}): automated {label} resolution\" --head {branch_name}"
        ),
    ];

    Ok(PrSandboxPlan {
        repo_slug: valid_repo.to_string(),
        issue_number,
        trigger_label,
        branch_name,
        container_name,
        evict_local_llm_before_build: true,
        sandbox_steps,
    })
}

impl CanarySpec {
    /// Creates a validated residential-IP Playwright canary check specification.
    ///
    /// # Errors
    /// Returns [`DevWorkerError::InvalidCanaryUrl`] if `target_url` does not start with `http://` or `https://`.
    pub fn new(
        suite_name: &str,
        target_url: &str,
        cron_schedule: &str,
        max_latency_ms: u32,
    ) -> Result<Self, DevWorkerError> {
        let trimmed_url = target_url.trim();
        if !trimmed_url.starts_with("https://") && !trimmed_url.starts_with("http://") {
            return Err(DevWorkerError::InvalidCanaryUrl(target_url.to_string()));
        }

        Ok(Self {
            suite_name: suite_name.trim().to_string(),
            target_url: trimmed_url.to_string(),
            cron_schedule: cron_schedule.trim().to_string(),
            max_latency_ms,
        })
    }

    /// Evaluates a residential Playwright canary run and only notifies Slack/Telegram if a check fails
    /// or latency breaches the configured threshold.
    #[must_use = "alert decision determines whether to page the operator"]
    pub fn evaluate_probe(&self, probe: &CanaryProbeResult) -> CanaryAlertDecision {
        let status_ok = (200..300).contains(&probe.http_status);
        let latency_ok = probe.latency_ms <= self.max_latency_ms;

        if status_ok && latency_ok && probe.assertion_passed {
            return CanaryAlertDecision {
                healthy: true,
                notify_mobile_channel: false,
                alert_message: None,
            };
        }

        let detail = if let Some(reason) = &probe.failure_reason {
            reason.clone()
        } else if !status_ok {
            format!("unexpected HTTP status {}", probe.http_status)
        } else {
            format!(
                "latency {}ms exceeded threshold {}ms",
                probe.latency_ms, self.max_latency_ms
            )
        };

        CanaryAlertDecision {
            healthy: false,
            notify_mobile_channel: true,
            alert_message: Some(format!(
                "[CANARY FAIL] '{}' on {} ({})",
                self.suite_name, self.target_url, detail
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_isolated_pr_sandbox_plan_with_llm_preemption() -> Result<(), DevWorkerError> {
        let plan = build_pr_sandbox_plan("cokitron/cheap-otto", 42, "agent-fix")?;
        assert_eq!(plan.branch_name, "otto/agent-fix/issue-42");
        assert!(plan.evict_local_llm_before_build);
        assert_eq!(plan.sandbox_steps.len(), 6);
        assert!(plan.sandbox_steps[5].contains("--draft"));
        Ok(())
    }

    #[test]
    fn rejects_malformed_repo_slug_and_unknown_label() {
        let bad_repo = build_pr_sandbox_plan("invalid_slug_no_slash", 1, "agent-fix").unwrap_err();
        assert_eq!(
            bad_repo,
            DevWorkerError::InvalidRepoSlug("invalid_slug_no_slash".to_string())
        );

        let bad_label = build_pr_sandbox_plan("cokitron/cheap-otto", 1, "deploy-prod").unwrap_err();
        assert_eq!(
            bad_label,
            DevWorkerError::UnsupportedLabel("deploy-prod".to_string())
        );
    }

    #[test]
    fn canary_stays_silent_when_healthy_and_pages_on_failure() -> Result<(), DevWorkerError> {
        let canary = CanarySpec::new(
            "checkout-e2e",
            "https://staging.example.com/checkout",
            "*/15 * * * *",
            1500,
        )?;

        let ok_probe = CanaryProbeResult {
            http_status: 200,
            latency_ms: 420,
            assertion_passed: true,
            failure_reason: None,
        };
        let ok_decision = canary.evaluate_probe(&ok_probe);
        assert!(ok_decision.healthy);
        assert!(!ok_decision.notify_mobile_channel);

        let fail_probe = CanaryProbeResult {
            http_status: 502,
            latency_ms: 1800,
            assertion_passed: false,
            failure_reason: Some("Stripe webhook timeout".to_string()),
        };
        let fail_decision = canary.evaluate_probe(&fail_probe);
        assert!(!fail_decision.healthy);
        assert!(fail_decision.notify_mobile_channel);
        assert!(
            fail_decision
                .alert_message
                .as_deref()
                .unwrap_or_default()
                .contains("Stripe webhook timeout")
        );
        Ok(())
    }
}
