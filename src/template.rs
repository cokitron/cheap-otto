//! Repeatable Task Templates / Playbooks (`cheap-otto template` & `/api/templates`).
//!
//! Stores reusable multi-harness prompts, model/effort presets, and verification gates in
//! `.cheap-otto/templates.json` so both the Mission Control GUI and external AI agents can trigger
//! repeatable worktree tasks or bind them to recurring `launchd` schedules in one command.

use crate::bridge::validate_safe_identifier;
use crate::harness::{HarnessError, HarnessRunConfig, ReasoningEffort};
use crate::worktree::AgentRunner;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when creating or validating repeatable task templates.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    /// Template ID failed safe-identifier validation.
    #[error("invalid template ID '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidTemplateId(String),
    /// Optional project filter failed safe-identifier validation.
    #[error("invalid project name '{0}' in template")]
    InvalidProjectName(String),
    /// Prompt was blank.
    #[error("template task prompt cannot be empty")]
    EmptyPrompt,
    /// Invalid `--model` or `--effort` configuration.
    #[error(transparent)]
    Harness(#[from] HarnessError),
}

/// Reusable multi-harness task playbook stored in [`TemplateRegistry`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskTemplate {
    /// Unique template slug (`nightly-audit`, `pr-lint-fix`, etc.).
    pub template_id: String,
    /// Human-readable title shown in Mission Control UI cards.
    pub title: String,
    /// Optional bound project name (`None` applies to whichever project is active).
    pub project_name: Option<String>,
    /// Default coding harness (`claude`, `agy`, `kiro`, `custom`).
    pub harness: AgentRunner,
    /// Default model identifier (`sonnet`, `gemini-4-carbon-high`, `opus`, etc.).
    pub model: Option<String>,
    /// Default reasoning effort (`low`..`max`).
    pub effort: Option<ReasoningEffort>,
    /// Task prompt executed when the template runs.
    pub prompt: String,
    /// Verification gate command (`cargo test`, `npm test`, etc.).
    pub verify_command: String,
    /// Whether scheduled executions of this template should open/update a Draft PR.
    pub create_draft_pr: bool,
}

/// Input parameters for constructing a [`TaskTemplate`].
pub struct NewTemplateParams<'a> {
    /// Unique template slug (`nightly-audit`, `pr-lint-fix`, etc.).
    pub template_id: &'a str,
    /// Optional human-readable card title.
    pub title: Option<&'a str>,
    /// Optional bound project filter.
    pub project_name: Option<&'a str>,
    /// Default coding harness (`claude`, `agy`, `kiro`, `custom`).
    pub harness: AgentRunner,
    /// Optional default model identifier.
    pub model: Option<String>,
    /// Optional default reasoning effort.
    pub effort: Option<ReasoningEffort>,
    /// Task prompt executed when the template runs.
    pub prompt: &'a str,
    /// Verification gate command.
    pub verify_command: &'a str,
    /// Whether scheduled runs of this template open a Draft PR.
    pub create_draft_pr: bool,
}

impl TaskTemplate {
    /// Validates and constructs a [`TaskTemplate`].
    ///
    /// # Errors
    /// Returns [`TemplateError`] if `template_id`, `project_name`, `prompt`, or `model` is invalid.
    pub fn new(params: &NewTemplateParams<'_>) -> Result<Self, TemplateError> {
        let clean_id = params.template_id.trim();
        if !validate_safe_identifier(clean_id) {
            return Err(TemplateError::InvalidTemplateId(
                params.template_id.to_string(),
            ));
        }

        let clean_project = if let Some(p) = params.project_name
            && !p.trim().is_empty()
        {
            if !validate_safe_identifier(p.trim()) {
                return Err(TemplateError::InvalidProjectName(p.to_string()));
            }
            Some(p.trim().to_string())
        } else {
            None
        };

        let clean_prompt = params.prompt.trim();
        if clean_prompt.is_empty() {
            return Err(TemplateError::EmptyPrompt);
        }

        let cfg = HarnessRunConfig {
            runner: params.harness,
            model: params.model.clone(),
            effort: params.effort,
            session_id: None,
            resume_existing: false,
            stream_json: matches!(
                params.harness,
                AgentRunner::ClaudeCode | AgentRunner::Antigravity
            ),
        };
        cfg.validate()?;

        let resolved_title = match params.title {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => clean_id.replace(['-', '_'], " "),
        };
        let resolved_verify = if params.verify_command.trim().is_empty() {
            "cargo test".to_string()
        } else {
            params.verify_command.trim().to_string()
        };

        Ok(Self {
            template_id: clean_id.to_string(),
            title: resolved_title,
            project_name: clean_project,
            harness: params.harness,
            model: params.model.clone(),
            effort: params.effort,
            prompt: clean_prompt.to_string(),
            verify_command: resolved_verify,
            create_draft_pr: params.create_draft_pr,
        })
    }

    /// Converts this template (with optional runtime model/harness overrides) into a [`HarnessRunConfig`].
    #[must_use = "returns HarnessRunConfig ready for delegation"]
    pub fn to_harness_config(
        &self,
        override_harness: Option<AgentRunner>,
        override_model: Option<String>,
        override_effort: Option<ReasoningEffort>,
        session_id: Option<String>,
    ) -> HarnessRunConfig {
        let runner = override_harness.unwrap_or(self.harness);
        let same_runner = runner == self.harness;
        let model = override_model.or_else(|| same_runner.then(|| self.model.clone()).flatten());
        let effort = override_effort.or_else(|| same_runner.then_some(self.effort).flatten());
        HarnessRunConfig {
            runner,
            model,
            effort,
            session_id,
            resume_existing: false,
            stream_json: matches!(runner, AgentRunner::ClaudeCode | AgentRunner::Antigravity),
        }
    }
}

/// Persistent collection of repeatable task templates stored in `.cheap-otto/templates.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateRegistry {
    /// Registered templates sorted by insertion order.
    pub templates: Vec<TaskTemplate>,
}

impl TemplateRegistry {
    /// Inserts or updates a [`TaskTemplate`] by `template_id`.
    pub fn upsert(&mut self, template: TaskTemplate) {
        if let Some(existing) = self
            .templates
            .iter_mut()
            .find(|t| t.template_id == template.template_id)
        {
            *existing = template;
        } else {
            self.templates.push(template);
        }
    }

    /// Looks up a template by `template_id`.
    #[must_use = "returns matching TaskTemplate if present"]
    pub fn get(&self, template_id: &str) -> Option<&TaskTemplate> {
        let clean = template_id.trim();
        self.templates.iter().find(|t| t.template_id == clean)
    }

    /// Removes a template by `template_id`.
    pub fn remove(&mut self, template_id: &str) -> bool {
        let clean = template_id.trim();
        let before = self.templates.len();
        self.templates.retain(|t| t.template_id != clean);
        self.templates.len() < before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_upserts_and_overrides_repeatable_task_template() -> Result<(), TemplateError> {
        let tpl = TaskTemplate::new(&NewTemplateParams {
            template_id: "audit-edge-cases",
            title: Some("Edge-Case Test Audit"),
            project_name: None,
            harness: AgentRunner::Antigravity,
            model: Some("gemini-4-carbon-high".to_string()),
            effort: Some(ReasoningEffort::High),
            prompt: "Inspect public functions for missing error-path unit tests and add them",
            verify_command: "cargo test --all-targets",
            create_draft_pr: true,
        })?;

        let mut reg = TemplateRegistry::default();
        reg.upsert(tpl);
        assert_eq!(reg.templates.len(), 1);

        let found = reg.get("audit-edge-cases").expect("template stored");
        assert_eq!(found.title, "Edge-Case Test Audit");
        let cfg = found.to_harness_config(
            Some(AgentRunner::ClaudeCode),
            Some("opus".to_string()),
            None,
            None,
        );
        assert_eq!(cfg.runner, AgentRunner::ClaudeCode);
        assert_eq!(cfg.model.as_deref(), Some("opus"));
        assert_eq!(cfg.effort, None);

        assert!(reg.remove("audit-edge-cases"));
        assert!(reg.templates.is_empty());
        Ok(())
    }
}
