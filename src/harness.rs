//! Multi-Harness Execution Profiles, Dynamic Model Catalogs & `stream-json` Telemetry Normalizer.
//!
//! Unifies headless CLI execution, model/effort switching, session resumption (`--resume` /
//! `--conversation` / `--resume-id`), and real-time NDJSON (`stream-json`) telemetry across
//! Claude Code (`claude`), Antigravity (`agy`), Kiro (`kiro-cli`), and custom shell commands.

use crate::worktree::{AgentRunner, WorktreeError, posix_single_quote};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

static UUID_SEQ: AtomicU64 = AtomicU64::new(1);

/// Generates a standard RFC 4122 v4-formatted UUID (`xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`)
/// using `std` entropy (`RandomState` + `SystemTime` + monotonic counter) without pulling
/// external `uuid`/`getrandom` build scripts.
#[must_use = "session UUID binds `claude --session-id` and `--resume` across turns"]
pub fn generate_session_uuid() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0_u128, |d| d.as_nanos());
    let seq = u128::from(UUID_SEQ.fetch_add(1, Ordering::Relaxed));
    let mut h1 = RandomState::new().build_hasher();
    h1.write_u128(nanos ^ seq);
    let mut h2 = RandomState::new().build_hasher();
    h2.write_u128(nanos.wrapping_mul(6_364_136_223_846_793_005) ^ (seq << 32));

    let hi = h1.finish();
    let lo = h2.finish();
    let p1 = (hi >> 32) as u32;
    let p2 = ((hi >> 16) & 0xFFFF) as u16;
    let p3 = ((hi & 0x0FFF) as u16) | 0x4000;
    let p4 = (((lo >> 48) & 0x3FFF) as u16) | 0x8000;
    let p5 = lo & 0x0000_FFFF_FFFF_FFFF;
    format!("{p1:08x}-{p2:04x}-{p3:04x}-{p4:04x}-{p5:012x}")
}

/// Errors returned when validating harness models, session IDs, or reasoning effort levels.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HarnessError {
    /// Underlying agent runner parse or prompt validation error.
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    /// Model identifier contains whitespace or unsafe shell characters.
    #[error(
        "invalid model identifier '{0}': must contain only ASCII alphanumeric, '-', '_', '.', ':', or '/'"
    )]
    InvalidModelId(String),
    /// Session / conversation identifier contains unsafe characters.
    #[error(
        "invalid session identifier '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'"
    )]
    InvalidSessionId(String),
    /// Unrecognized `--effort` value.
    #[error(
        "unsupported reasoning effort '{0}': expected 'low', 'medium', 'high', 'xhigh', or 'max'"
    )]
    InvalidEffort(String),
}

/// Standardized reasoning effort level supported across `claude`, `agy`, and `kiro-cli`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    /// Fast, minimal-thinking execution (`low`).
    Low,
    /// Balanced everyday execution (`medium`).
    Medium,
    /// Deeper reasoning for multi-file changes (`high`).
    High,
    /// Extended multi-step reasoning (`xhigh`).
    XHigh,
    /// Maximum reasoning budget (`max`).
    Max,
}

impl ReasoningEffort {
    /// Parses a CLI / JSON effort token (`low`, `medium`, `high`, `xhigh`, `max`).
    ///
    /// # Errors
    /// Returns [`HarnessError::InvalidEffort`] if unrecognized.
    pub fn parse(raw: &str) -> Result<Self, HarnessError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" | "med" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" | "extra-high" => Ok(Self::XHigh),
            "max" => Ok(Self::Max),
            other => Err(HarnessError::InvalidEffort(other.to_string())),
        }
    }

    /// Returns the canonical CLI flag value passed to `--effort`.
    #[must_use = "returns the CLI flag token"]
    pub const fn as_flag_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

/// Validates a model identifier (`sonnet`, `gemini-4-carbon-high`, `anthropic/claude-sonnet-4`, etc.).
#[must_use = "validation result must be checked before building CLI flags"]
pub fn validate_model_identifier(raw: &str) -> bool {
    let trimmed = raw.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('-')
        && trimmed.len() <= 128
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/'))
}

/// Validates a provider session/conversation ID (`UUID` or alphanumeric slug).
#[must_use = "validation result must be checked before building CLI flags"]
pub fn validate_session_identifier(raw: &str) -> bool {
    let trimmed = raw.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('-')
        && trimmed.len() <= 128
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Complete per-turn execution configuration for a coding harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessRunConfig {
    /// Selected coding agent harness (`claude`, `agy`, `kiro`, `custom`).
    pub runner: AgentRunner,
    /// Optional model override (`sonnet`, `opus`, `gemini-4-carbon-high`, etc.).
    pub model: Option<String>,
    /// Optional reasoning effort override (`low`..`max`).
    pub effort: Option<ReasoningEffort>,
    /// Optional session/conversation ID to bind (`--session-id`) or resume (`--resume` / `--conversation`).
    pub session_id: Option<String>,
    /// When `true`, resumes an existing conversation rather than starting a brand-new session ID.
    pub resume_existing: bool,
    /// When `true`, enables `--output-format stream-json` on supported harnesses (`claude`, `agy`).
    pub stream_json: bool,
}

impl HarnessRunConfig {
    /// Creates a baseline [`HarnessRunConfig`] for the given [`AgentRunner`].
    #[must_use = "constructed config should be customized or validated"]
    pub const fn new(runner: AgentRunner) -> Self {
        Self {
            runner,
            model: None,
            effort: None,
            session_id: None,
            resume_existing: false,
            stream_json: false,
        }
    }

    /// Validates `model` and `session_id` fields against shell/argument injection.
    ///
    /// # Errors
    /// Returns [`HarnessError`] if `model` or `session_id` contains unsafe characters.
    pub fn validate(&self) -> Result<(), HarnessError> {
        if let Some(m) = &self.model
            && !validate_model_identifier(m)
        {
            return Err(HarnessError::InvalidModelId(m.clone()));
        }
        if let Some(sid) = &self.session_id
            && !validate_session_identifier(sid)
        {
            return Err(HarnessError::InvalidSessionId(sid.clone()));
        }
        Ok(())
    }

    /// Builds a POSIX single-quoted headless command string safe for remote SSH / `tmux` execution.
    ///
    /// # Errors
    /// Returns [`HarnessError`] if `raw_prompt` is empty or any identifier fails validation.
    pub fn build_remote_command(&self, raw_prompt: &str) -> Result<String, HarnessError> {
        if raw_prompt.trim().is_empty() {
            return Err(HarnessError::Worktree(WorktreeError::EmptyPrompt));
        }
        self.validate()?;

        let quoted_prompt = posix_single_quote(raw_prompt.trim());
        let model_flag = self
            .model
            .as_deref()
            .map(|m| format!(" --model {}", posix_single_quote(m.trim())))
            .unwrap_or_default();
        let effort_flag = self
            .effort
            .map(|e| format!(" --effort {}", e.as_flag_str()))
            .unwrap_or_default();

        let cmd = match self.runner {
            AgentRunner::ClaudeCode => {
                let session_flag = match (&self.session_id, self.resume_existing) {
                    (Some(sid), true) => format!(" --resume {}", posix_single_quote(sid.trim())),
                    (Some(sid), false) => {
                        format!(" --session-id {}", posix_single_quote(sid.trim()))
                    }
                    (None, true) => " --continue".to_string(),
                    (None, false) => String::new(),
                };
                let format_flag = if self.stream_json {
                    " --output-format stream-json --verbose"
                } else {
                    ""
                };
                format!(
                    "claude -p {quoted_prompt} --dangerously-skip-permissions{model_flag}{effort_flag}{session_flag}{format_flag}"
                )
            }
            AgentRunner::Antigravity => {
                let session_flag = match (&self.session_id, self.resume_existing) {
                    (Some(sid), _) => format!(" --conversation {}", posix_single_quote(sid.trim())),
                    (None, true) => " --continue".to_string(),
                    (None, false) => String::new(),
                };
                let format_flag = if self.stream_json {
                    " --output-format stream-json"
                } else {
                    ""
                };
                format!(
                    "agy -p {quoted_prompt} --dangerously-skip-permissions{model_flag}{effort_flag}{session_flag}{format_flag}"
                )
            }
            AgentRunner::KiroCli => {
                let session_flag = match (&self.session_id, self.resume_existing) {
                    (Some(sid), true) => {
                        format!(" --resume-id {}", posix_single_quote(sid.trim()))
                    }
                    (None, true) => " --resume".to_string(),
                    _ => String::new(),
                };
                format!(
                    "kiro-cli chat --no-interactive --trust-all-tools --trust-workspace{model_flag}{effort_flag}{session_flag} {quoted_prompt}"
                )
            }
            AgentRunner::CustomShell => raw_prompt.trim().to_string(),
        };

        Ok(cmd)
    }
}

/// Selectable model entry exposed to the GUI dropdowns and `cheap-otto harnesses --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    /// Exact `--model` identifier passed to the CLI (`sonnet`, `gemini-4-carbon-high`, etc.).
    pub id: String,
    /// Human-readable display label.
    pub label: String,
}

/// Installed harness capability & live model catalog entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessCatalogEntry {
    /// Canonical harness slug (`claude`, `agy`, `kiro`, `custom`).
    pub id: String,
    /// Human-readable application name.
    pub display_name: String,
    /// Whether the binary was found on the target M1 `PATH`.
    pub installed: bool,
    /// Detected CLI version string, if installed.
    pub version: Option<String>,
    /// Whether the CLI appears signed in and ready for headless execution.
    pub authenticated: bool,
    /// Actionable hint if `installed == false` or `authenticated == false`.
    pub status_note: Option<String>,
    /// Whether `--output-format stream-json` real-time telemetry is supported.
    pub supports_stream_json: bool,
    /// Supported `--effort` levels.
    pub supported_efforts: Vec<ReasoningEffort>,
    /// Available models (dynamically probed or curated aliases).
    pub models: Vec<ModelDescriptor>,
}

/// Parses `agy models` stdout/stderr into `(authenticated, Vec<ModelDescriptor>)`.
#[must_use = "parsed Antigravity model catalog populates HarnessCatalogEntry"]
pub fn parse_agy_models_output(output: &str) -> (bool, Vec<ModelDescriptor>) {
    let lower = output.to_ascii_lowercase();
    if lower.contains("please sign in") || lower.contains("authentication required") {
        return (false, default_agy_models());
    }

    let mut models = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Fetching")
            || trimmed.starts_with("Error:")
            || trimmed.starts_with("Warning:")
        {
            continue;
        }
        if let Some((id_part, label_part)) = trimmed.split_once('\t') {
            let id = id_part.trim();
            let label = label_part.trim();
            if validate_model_identifier(id) && !label.is_empty() {
                models.push(ModelDescriptor {
                    id: id.to_string(),
                    label: label.to_string(),
                });
            }
        } else if validate_model_identifier(trimmed) {
            models.push(ModelDescriptor {
                id: trimmed.to_string(),
                label: trimmed.to_string(),
            });
        }
    }

    if models.is_empty() {
        (true, default_agy_models())
    } else {
        (true, models)
    }
}

/// Default curated model aliases for Claude Code (`claude`).
#[must_use = "returns default Claude Code model descriptors"]
pub fn default_claude_models() -> Vec<ModelDescriptor> {
    vec![
        ModelDescriptor {
            id: "sonnet".to_string(),
            label: "Claude Sonnet (Latest Alias)".to_string(),
        },
        ModelDescriptor {
            id: "opus".to_string(),
            label: "Claude Opus (Latest Alias)".to_string(),
        },
        ModelDescriptor {
            id: "haiku".to_string(),
            label: "Claude Haiku (Fast / Low Cost)".to_string(),
        },
        ModelDescriptor {
            id: "fable".to_string(),
            label: "Claude Fable (Alias)".to_string(),
        },
    ]
}

/// Default model descriptors for Antigravity (`agy`) used before first M1 sign-in or offline.
#[must_use = "returns default Antigravity model descriptors"]
pub fn default_agy_models() -> Vec<ModelDescriptor> {
    vec![
        ModelDescriptor {
            id: "gemini-4-carbon-high".to_string(),
            label: "Gemini 4 Carbon (High)".to_string(),
        },
        ModelDescriptor {
            id: "gemini-4-carbon-medium".to_string(),
            label: "Gemini 4 Carbon (Medium)".to_string(),
        },
        ModelDescriptor {
            id: "gemini-3.8-flash-high".to_string(),
            label: "Gemini 3.8 Flash (High)".to_string(),
        },
        ModelDescriptor {
            id: "claude-sonnet-5-5-high".to_string(),
            label: "Claude Sonnet 5.5 via AGY (High)".to_string(),
        },
        ModelDescriptor {
            id: "claude-opus-5-5-high".to_string(),
            label: "Claude Opus 5.5 via AGY (High)".to_string(),
        },
    ]
}

/// Default model descriptors for Kiro CLI (`kiro-cli`).
#[must_use = "returns default Kiro model descriptors"]
pub fn default_kiro_models() -> Vec<ModelDescriptor> {
    vec![
        ModelDescriptor {
            id: "claude-sonnet-4".to_string(),
            label: "Claude Sonnet 4 (Kiro)".to_string(),
        },
        ModelDescriptor {
            id: "claude-opus-4".to_string(),
            label: "Claude Opus 4 (Kiro)".to_string(),
        },
    ]
}

/// Standard reasoning effort list shared by `claude`, `agy`, and `kiro-cli`.
#[must_use = "returns all supported ReasoningEffort tiers"]
pub fn standard_efforts() -> Vec<ReasoningEffort> {
    vec![
        ReasoningEffort::Low,
        ReasoningEffort::Medium,
        ReasoningEffort::High,
        ReasoningEffort::XHigh,
        ReasoningEffort::Max,
    ]
}

/// Normalized live activity state extracted from `stream-json` NDJSON telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActivityKind {
    /// Session initialized / spawning into workspace.
    Starting,
    /// Model is reasoning or generating text.
    Thinking,
    /// Reading files (`Read`, `read_file`, `cat`).
    Reading,
    /// Writing or patching files (`Edit`, `Write`, `edit_file`, `write_file`).
    Editing,
    /// Executing shell / build / test commands (`Bash`, `run_command`).
    RunningCommand,
    /// Searching files, symbols, or web (`Grep`, `Glob`, `search_web`).
    Searching,
    /// Spawning or coordinating a subagent (`Task`, `Agent`, `invoke_subagent`).
    Subagent,
    /// Blocked waiting for user input or permission.
    WaitingInput,
    /// Turn completed cleanly.
    Completed,
    /// Turn terminated with an error.
    Failed,
}

/// Normalized update parsed from a single `stream-json` NDJSON line.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamEventUpdate {
    /// Provider session or conversation ID discovered in the stream.
    pub session_id: Option<String>,
    /// Normalized activity kind, if this event transitions state.
    pub activity: Option<AgentActivityKind>,
    /// Human-readable activity label (e.g. `"Editing src/bridge.rs"`).
    pub activity_label: Option<String>,
    /// Assistant text snippet or final result text emitted on this line.
    pub assistant_text: Option<String>,
    /// Optional `pixel-agents` compatible HTTP hook payload (`PreToolUse`, `Stop`, etc.).
    pub pixel_hook: Option<PixelHookPayload>,
}

/// Payload compatible with `pixel-agents` v1.6.0 `POST /api/hooks/claude`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelHookPayload {
    /// Hook event name (`SessionStart`, `PreToolUse`, `PostToolUse`, `Stop`, `SessionEnd`).
    pub hook_event_name: String,
    /// Deterministic session identifier routing events to a single office character.
    pub session_id: String,
    /// Optional workspace path (`cwd`) used on `SessionStart`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Canonical tool name (`Read`, `Edit`, `Write`, `Bash`, `Grep`, `Task`) for character animations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Optional tool input metadata (`file_path`, `command`, `description`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<Value>,
}

/// Classifies an arbitrary tool name from `claude` or `agy` into `(AgentActivityKind, canonical_pixel_tool, label)`.
#[must_use = "classified tool state drives both AgentSessionRecord and pixel-agents animations"]
pub fn classify_tool_invocation(
    raw_tool_name: &str,
    input: Option<&Value>,
) -> (AgentActivityKind, &'static str, String) {
    let lower = raw_tool_name.trim().to_ascii_lowercase();
    let file_hint = input
        .and_then(|v| {
            v.get("file_path")
                .or_else(|| v.get("path"))
                .or_else(|| v.get("Path"))
                .and_then(Value::as_str)
        })
        .and_then(|p| p.rsplit(['/', '\\']).next())
        .unwrap_or("");
    let cmd_hint = input
        .and_then(|v| {
            v.get("command")
                .or_else(|| v.get("CommandLine"))
                .and_then(Value::as_str)
        })
        .unwrap_or("");

    if matches!(
        lower.as_str(),
        "read" | "read_file" | "view_file" | "cat" | "notebookread"
    ) {
        let label = if file_hint.is_empty() {
            "Reading files".to_string()
        } else {
            format!("Reading {file_hint}")
        };
        (AgentActivityKind::Reading, "Read", label)
    } else if matches!(
        lower.as_str(),
        "edit" | "write" | "multiedit" | "edit_file" | "write_file" | "notebookedit"
    ) {
        let label = if file_hint.is_empty() {
            "Editing code".to_string()
        } else {
            format!("Editing {file_hint}")
        };
        (AgentActivityKind::Editing, "Edit", label)
    } else if matches!(
        lower.as_str(),
        "bash" | "sh" | "run_command" | "execute_command" | "terminal"
    ) {
        let short_cmd: String = cmd_hint.chars().take(28).collect();
        let label = if short_cmd.is_empty() {
            "Running command".to_string()
        } else {
            format!("Running: {short_cmd}")
        };
        (AgentActivityKind::RunningCommand, "Bash", label)
    } else if matches!(
        lower.as_str(),
        "grep" | "glob" | "search" | "search_web" | "websearch" | "webfetch" | "find_by_name"
    ) {
        (
            AgentActivityKind::Searching,
            "Grep",
            format!("Searching ({raw_tool_name})"),
        )
    } else if matches!(lower.as_str(), "task" | "agent" | "invoke_subagent") {
        (
            AgentActivityKind::Subagent,
            "Task",
            "Delegating subtask".to_string(),
        )
    } else if lower.contains("question") || lower.contains("ask_user") {
        (
            AgentActivityKind::WaitingInput,
            "AskUserQuestion",
            "Waiting for input".to_string(),
        )
    } else {
        (
            AgentActivityKind::Thinking,
            "Read",
            format!("Using {raw_tool_name}"),
        )
    }
}

/// Parses one line of `stream-json` (NDJSON) emitted by `claude` or `agy` into a [`StreamEventUpdate`].
#[must_use = "parsed update updates session metadata and forwards hooks to pixel-agents"]
pub fn parse_stream_json_line(line: &str, fallback_session_id: &str) -> Option<StreamEventUpdate> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let val: Value = serde_json::from_str(trimmed).ok()?;
    let mut update = StreamEventUpdate::default();

    if let Some(sid) = val
        .get("session_id")
        .or_else(|| val.get("conversation_id"))
        .or_else(|| val.get("conversationId"))
        .and_then(Value::as_str)
        && validate_session_identifier(sid)
    {
        update.session_id = Some(sid.to_string());
    }

    let effective_sid = update
        .session_id
        .as_deref()
        .unwrap_or(fallback_session_id)
        .to_string();

    let event_type = val.get("type").and_then(Value::as_str).unwrap_or("");

    match event_type {
        "system" | "init" | "session_start" => {
            update.activity = Some(AgentActivityKind::Starting);
            update.activity_label = Some("Session initialized".to_string());
            update.pixel_hook = Some(PixelHookPayload {
                hook_event_name: "SessionStart".to_string(),
                session_id: effective_sid,
                cwd: val
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                tool_name: None,
                tool_input: None,
            });
        }
        "tool_use" | "tool_call" => {
            let name = val
                .get("name")
                .or_else(|| val.get("tool_name"))
                .and_then(Value::as_str)
                .unwrap_or("Tool");
            let input = val.get("input").or_else(|| val.get("arguments"));
            let (kind, canonical_tool, label) = classify_tool_invocation(name, input);
            update.activity = Some(kind);
            update.activity_label = Some(label);
            update.pixel_hook = Some(PixelHookPayload {
                hook_event_name: "PreToolUse".to_string(),
                session_id: effective_sid,
                cwd: None,
                tool_name: Some(canonical_tool.to_string()),
                tool_input: input.cloned(),
            });
        }
        "tool_result" => {
            update.pixel_hook = Some(PixelHookPayload {
                hook_event_name: "PostToolUse".to_string(),
                session_id: effective_sid,
                cwd: None,
                tool_name: None,
                tool_input: None,
            });
        }
        "assistant" | "message" => {
            if let Some(contents) = val
                .get("message")
                .and_then(|m| m.get("content"))
                .or_else(|| val.get("content"))
                .and_then(Value::as_array)
            {
                let mut texts = Vec::new();
                for item in contents {
                    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
                    if item_type == "tool_use" {
                        let name = item.get("name").and_then(Value::as_str).unwrap_or("Tool");
                        let input = item.get("input");
                        let (kind, canonical_tool, label) = classify_tool_invocation(name, input);
                        update.activity = Some(kind);
                        update.activity_label = Some(label);
                        update.pixel_hook = Some(PixelHookPayload {
                            hook_event_name: "PreToolUse".to_string(),
                            session_id: effective_sid.clone(),
                            cwd: None,
                            tool_name: Some(canonical_tool.to_string()),
                            tool_input: input.cloned(),
                        });
                    } else if item_type == "text"
                        && let Some(t) = item.get("text").and_then(Value::as_str)
                        && !t.trim().is_empty()
                    {
                        texts.push(t.trim().to_string());
                    }
                }
                if !texts.is_empty() {
                    update.assistant_text = Some(texts.join("\n"));
                    if update.activity.is_none() {
                        update.activity = Some(AgentActivityKind::Thinking);
                        update.activity_label = Some("Responding".to_string());
                    }
                }
            } else if let Some(t) = val.get("text").and_then(Value::as_str)
                && !t.trim().is_empty()
            {
                update.assistant_text = Some(t.trim().to_string());
            }
        }
        "result" => {
            let is_err = val
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || val.get("subtype").and_then(Value::as_str) == Some("error");
            update.activity = Some(if is_err {
                AgentActivityKind::Failed
            } else {
                AgentActivityKind::Completed
            });
            update.activity_label = Some(if is_err {
                "Turn failed".to_string()
            } else {
                "Turn completed".to_string()
            });
            if let Some(res_text) = val.get("result").and_then(Value::as_str)
                && !res_text.trim().is_empty()
            {
                update.assistant_text = Some(res_text.trim().to_string());
            }
            update.pixel_hook = Some(PixelHookPayload {
                hook_event_name: "Stop".to_string(),
                session_id: effective_sid,
                cwd: None,
                tool_name: None,
                tool_input: None,
            });
        }
        _ => {}
    }

    Some(update)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_claude_agy_and_kiro_commands_with_model_effort_and_session_resume()
    -> Result<(), HarnessError> {
        let claude_cfg = HarnessRunConfig {
            runner: AgentRunner::ClaudeCode,
            model: Some("opus".to_string()),
            effort: Some(ReasoningEffort::High),
            session_id: Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
            resume_existing: true,
            stream_json: true,
        };
        let claude_cmd = claude_cfg.build_remote_command("Explain why auth failed")?;
        assert!(claude_cmd.contains("claude -p 'Explain why auth failed'"));
        assert!(claude_cmd.contains("--model 'opus'"));
        assert!(claude_cmd.contains("--effort high"));
        assert!(claude_cmd.contains("--resume '550e8400-e29b-41d4-a716-446655440000'"));
        assert!(claude_cmd.contains("--output-format stream-json --verbose"));

        let agy_cfg = HarnessRunConfig {
            runner: AgentRunner::Antigravity,
            model: Some("gemini-4-carbon-high".to_string()),
            effort: Some(ReasoningEffort::Max),
            session_id: Some("conv-123".to_string()),
            resume_existing: true,
            stream_json: true,
        };
        let agy_cmd = agy_cfg.build_remote_command("Refactor scheduler")?;
        assert!(agy_cmd.contains("agy -p 'Refactor scheduler' --dangerously-skip-permissions"));
        assert!(agy_cmd.contains("--model 'gemini-4-carbon-high'"));
        assert!(agy_cmd.contains("--effort max"));
        assert!(agy_cmd.contains("--conversation 'conv-123'"));
        assert!(agy_cmd.contains("--output-format stream-json"));

        let kiro_cfg = HarnessRunConfig {
            runner: AgentRunner::KiroCli,
            model: Some("claude-sonnet-4".to_string()),
            effort: Some(ReasoningEffort::Medium),
            session_id: Some("kiro-sess-9".to_string()),
            resume_existing: true,
            stream_json: false,
        };
        let kiro_cmd = kiro_cfg.build_remote_command("Run checks")?;
        assert!(
            kiro_cmd.contains("kiro-cli chat --no-interactive --trust-all-tools --trust-workspace")
        );
        assert!(kiro_cmd.contains("--resume-id 'kiro-sess-9'"));
        Ok(())
    }

    #[test]
    fn parses_agy_models_tab_output_and_detects_sign_in_prompt() {
        let unauth =
            "Fetching available models...\nError: Please sign in to view available models.";
        let (auth1, fallback) = parse_agy_models_output(unauth);
        assert!(!auth1);
        assert!(!fallback.is_empty());

        let live = "Fetching available models...\ngemini-4-carbon-high\tGemini 4 Carbon (High)\nclaude-sonnet-5-5-high\tClaude Sonnet 5.5 (High)\n";
        let (auth2, parsed) = parse_agy_models_output(live);
        assert!(auth2);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "gemini-4-carbon-high");
        assert_eq!(parsed[1].label, "Claude Sonnet 5.5 (High)");
    }

    #[test]
    fn parses_stream_json_tool_use_and_final_result_for_pixel_agents_bridge() {
        let assistant_line = r#"{"type":"assistant","session_id":"sess-42","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/Users/admin/otto-workspaces/app/src/main.rs"}}]}}"#;
        let upd = parse_stream_json_line(assistant_line, "fallback").expect("valid json");
        assert_eq!(upd.session_id.as_deref(), Some("sess-42"));
        assert_eq!(upd.activity, Some(AgentActivityKind::Editing));
        assert_eq!(upd.activity_label.as_deref(), Some("Editing main.rs"));
        let hook = upd.pixel_hook.expect("pixel hook");
        assert_eq!(hook.hook_event_name, "PreToolUse");
        assert_eq!(hook.tool_name.as_deref(), Some("Edit"));

        let res_line = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"sess-42","result":"Updated main.rs and verified all 48 tests pass."}"#;
        let res_upd = parse_stream_json_line(res_line, "fallback").expect("valid result");
        assert_eq!(res_upd.activity, Some(AgentActivityKind::Completed));
        assert_eq!(
            res_upd.assistant_text.as_deref(),
            Some("Updated main.rs and verified all 48 tests pass.")
        );
        assert_eq!(
            res_upd.pixel_hook.expect("stop hook").hook_event_name,
            "Stop"
        );
    }

    #[test]
    fn rejects_unsafe_model_or_session_identifiers() {
        let bad = HarnessRunConfig {
            runner: AgentRunner::ClaudeCode,
            model: Some("opus; rm -rf /".to_string()),
            effort: None,
            session_id: None,
            resume_existing: false,
            stream_json: false,
        };
        assert!(matches!(
            bad.build_remote_command("hi"),
            Err(HarnessError::InvalidModelId(_))
        ));
    }
}
