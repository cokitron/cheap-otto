//! Addressable Stateful Agent Sessions, Inspection & Direct Per-Agent Querying (`inspect` & `ask`).
//!
//! Ensures every delegated agent (`otto-<id>`) is an individually addressable worker with persistent
//! conversation continuity (`--resume` / `--conversation` / `--resume-id`), independent of any visual
//! office UI. Supports inspecting live status/diffs/responses and sending direct follow-up queries
//! (even switching models or harnesses mid-worktree).

use crate::bridge::{BridgeError, PeerNode, validate_safe_identifier};
use crate::command::CommandSpec;
use crate::harness::{
    AgentActivityKind, HarnessError, HarnessRunConfig, ReasoningEffort, generate_session_uuid,
    parse_stream_json_line,
};
use crate::worktree::{AgentRunner, WorktreeError, posix_single_quote};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when inspecting or querying an addressable agent session.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentSessionError {
    /// Underlying SSH peer or project validation error.
    #[error(transparent)]
    Bridge(#[from] BridgeError),
    /// Underlying harness/model/session validation error.
    #[error(transparent)]
    Harness(#[from] HarnessError),
    /// Underlying worktree validation error.
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    /// Invalid agent identifier.
    #[error("invalid agent ID '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidAgentId(String),
    /// Follow-up prompt was empty.
    #[error("query prompt cannot be empty")]
    EmptyQuery,
}

/// High-level execution state of an addressable agent worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionStatus {
    /// Currently executing a turn or verification command inside `tmux`.
    Active,
    /// Turn completed cleanly (`exit=0`); ready for follow-up `ask` queries or `pull`.
    IdleDone,
    /// Blocked waiting for user input.
    WaitingInput,
    /// Turn or verification command exited non-zero.
    Failed,
}

/// Summary of a single prompt/response turn on an addressable agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTurnEntry {
    /// 1-based turn index.
    pub turn: u32,
    /// User/operator prompt sent on this turn.
    pub prompt: String,
    /// Harness used on this turn (`claude`, `agy`, `kiro`, `custom`).
    pub harness: AgentRunner,
    /// Model used on this turn, if specified.
    pub model: Option<String>,
    /// Effort level used on this turn, if specified.
    pub effort: Option<ReasoningEffort>,
    /// Final assistant response summary captured after completion, if available.
    pub response_summary: Option<String>,
    /// Unix epoch seconds when the turn started.
    pub started_epoch_secs: u64,
}

/// Persistent metadata record for an addressable agent (`otto-<id>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionRecord {
    /// Sanitized agent ID (`<id>`, without `otto-` prefix).
    pub agent_id: String,
    /// Repository project name (`<project>`).
    pub project_name: String,
    /// Worktree Git branch (`otto/<id>`).
    pub branch_name: String,
    /// Remote worktree path on the MacBook (`~/otto-workspaces/<project>/.otto-worktrees/<id>`).
    pub remote_worktree_dir: String,
    /// Detached `tmux` session name (`otto-<id>`).
    pub tmux_session_name: String,
    /// Remote `stream-json` NDJSON event file (`~/.cheap-otto/events/<project>/<id>.ndjson`).
    pub remote_events_file: String,
    /// Remote human-readable log file (`~/.cheap-otto/logs/<project>/<id>.log`).
    pub remote_log_file: String,
    /// Active coding harness (`claude`, `agy`, `kiro`, `custom`).
    pub harness: AgentRunner,
    /// Active model identifier (`sonnet`, `gemini-4-carbon-high`, etc.).
    pub model: Option<String>,
    /// Active reasoning effort (`low`..`max`).
    pub effort: Option<ReasoningEffort>,
    /// Bound provider session/conversation ID for stateful `--resume` / `--conversation` turns.
    pub provider_session_id: Option<String>,
    /// Verification command configured for this agent's worktree.
    pub verify_command: String,
    /// Latest known status.
    pub status: AgentSessionStatus,
    /// Latest normalized activity label (e.g. `"Editing bridge.rs"`).
    pub current_activity: Option<String>,
    /// Latest assistant response text.
    pub last_response: Option<String>,
    /// Recorded turns on this agent.
    #[serde(default)]
    pub turns: Vec<AgentTurnEntry>,
    /// Last updated timestamp (epoch seconds).
    pub updated_epoch_secs: u64,
}

impl AgentSessionRecord {
    /// Generates a deterministic initial [`AgentSessionRecord`] when delegating a new worktree task.
    ///
    /// Automatically assigns a fresh UUIDv4 `provider_session_id` for `ClaudeCode` so subsequent
    /// `cheap-otto ask` calls can deterministically pass `--resume <uuid>`.
    #[must_use = "session record should be saved to AgentSessionRegistry"]
    pub fn new_initial(
        project_name: &str,
        agent_id: &str,
        remote_worktree_dir: &str,
        harness_cfg: &HarnessRunConfig,
        verify_command: &str,
        initial_prompt: &str,
        now_epoch_secs: u64,
    ) -> Self {
        let clean_id = normalize_agent_id(agent_id);
        let clean_project = project_name.trim().to_string();
        let assigned_sid = harness_cfg.session_id.clone().or_else(|| {
            if harness_cfg.runner == AgentRunner::ClaudeCode {
                Some(generate_session_uuid())
            } else {
                None
            }
        });

        Self {
            agent_id: clean_id.clone(),
            project_name: clean_project.clone(),
            branch_name: format!("otto/{clean_id}"),
            remote_worktree_dir: remote_worktree_dir.to_string(),
            tmux_session_name: format!("otto-{clean_id}"),
            remote_events_file: format!("~/.cheap-otto/events/{clean_project}/{clean_id}.ndjson"),
            remote_log_file: format!("~/.cheap-otto/logs/{clean_project}/{clean_id}.log"),
            harness: harness_cfg.runner,
            model: harness_cfg.model.clone(),
            effort: harness_cfg.effort,
            provider_session_id: assigned_sid,
            verify_command: verify_command.to_string(),
            status: AgentSessionStatus::Active,
            current_activity: Some("Starting worktree session".to_string()),
            last_response: None,
            turns: vec![AgentTurnEntry {
                turn: 1,
                prompt: initial_prompt.trim().to_string(),
                harness: harness_cfg.runner,
                model: harness_cfg.model.clone(),
                effort: harness_cfg.effort,
                response_summary: None,
                started_epoch_secs: now_epoch_secs,
            }],
            updated_epoch_secs: now_epoch_secs,
        }
    }
}

/// Persistent registry of addressable agents stored in `.cheap-otto/agents.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionRegistry {
    /// Ordered list of tracked agent sessions.
    pub agents: Vec<AgentSessionRecord>,
}

impl AgentSessionRegistry {
    /// Inserts or updates an [`AgentSessionRecord`] keyed by `(project_name, agent_id)`.
    pub fn upsert(&mut self, record: AgentSessionRecord) {
        if let Some(existing) = self
            .agents
            .iter_mut()
            .find(|a| a.project_name == record.project_name && a.agent_id == record.agent_id)
        {
            *existing = record;
        } else {
            self.agents.push(record);
        }
    }

    /// Finds an agent by `(project_name, agent_id)` (accepting optional `otto-` prefix).
    #[must_use = "returns matching AgentSessionRecord if tracked"]
    pub fn find(&self, project_name: &str, agent_id: &str) -> Option<&AgentSessionRecord> {
        let clean_id = normalize_agent_id(agent_id);
        self.agents
            .iter()
            .find(|a| a.project_name == project_name.trim() && a.agent_id == clean_id)
    }

    /// Finds an agent by `agent_id` across all projects when unambiguous.
    #[must_use = "returns matching AgentSessionRecord"]
    pub fn find_any_project(&self, agent_id: &str) -> Option<&AgentSessionRecord> {
        let clean_id = normalize_agent_id(agent_id);
        self.agents.iter().find(|a| a.agent_id == clean_id)
    }

    /// Removes an agent record by `(project_name, agent_id)`.
    pub fn remove(&mut self, project_name: &str, agent_id: &str) -> bool {
        let clean_id = normalize_agent_id(agent_id);
        let before = self.agents.len();
        self.agents
            .retain(|a| !(a.project_name == project_name.trim() && a.agent_id == clean_id));
        self.agents.len() < before
    }
}

/// Strips optional `otto-` session prefix from a user-supplied agent identifier.
#[must_use = "returns normalized bare agent ID"]
pub fn normalize_agent_id(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed.strip_prefix("otto-").unwrap_or(trimmed).to_string()
}

/// Summarized telemetry extracted from an agent's `.ndjson` and `.log` outputs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedAgentTelemetry {
    /// Discovered provider session/conversation ID.
    pub provider_session_id: Option<String>,
    /// Latest normalized activity kind.
    pub latest_activity: Option<AgentActivityKind>,
    /// Latest human-readable activity label.
    pub latest_activity_label: Option<String>,
    /// Latest assistant response text.
    pub latest_response: Option<String>,
    /// Exit code parsed from `[OTTO_STATUS exit=N]`, if finished.
    pub exit_code: Option<i32>,
}

/// Parses an NDJSON event stream + optional log tail into [`ParsedAgentTelemetry`].
#[must_use = "parsed telemetry updates AgentSessionRecord and powers inspect/ask responses"]
pub fn summarize_ndjson_and_log(
    ndjson_content: &str,
    log_tail: &str,
    fallback_sid: &str,
) -> ParsedAgentTelemetry {
    let mut out = ParsedAgentTelemetry::default();

    for line in ndjson_content.lines() {
        if let Some(upd) = parse_stream_json_line(line, fallback_sid) {
            if let Some(sid) = upd.session_id {
                out.provider_session_id = Some(sid);
            }
            if let Some(act) = upd.activity {
                out.latest_activity = Some(act);
            }
            if let Some(lbl) = upd.activity_label {
                out.latest_activity_label = Some(lbl);
            }
            if let Some(txt) = upd.assistant_text {
                out.latest_response = Some(txt);
            }
        }
    }

    for line in log_tail.lines().rev() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("[OTTO_STATUS exit=")
            && let Some(code_str) = rest.strip_suffix(']')
            && let Ok(code) = code_str.parse::<i32>()
        {
            out.exit_code = Some(code);
            break;
        }
    }

    if out.latest_response.is_none() {
        // Fallback for non-stream-json harnesses (`kiro`, `custom`): capture non-marker log tail
        let plain: Vec<&str> = log_tail
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("[OTTO_STATUS"))
            .collect();
        if !plain.is_empty() {
            let start = plain.len().saturating_sub(12);
            out.latest_response = Some(plain[start..].join("\n"));
        }
    }

    out
}

/// Plan to inspect an individual agent's live worktree, git diff, and NDJSON telemetry over SSH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInspectPlan {
    /// Normalized agent ID.
    pub agent_id: String,
    /// Target project name.
    pub project_name: String,
    /// SSH probe command gathering `tmux`, `git`, `.ndjson`, and `.log` state in a single round-trip.
    pub probe_spec: CommandSpec,
}

/// Builds a single-round-trip [`AgentInspectPlan`] to query an agent's live state on the MacBook.
///
/// # Errors
/// Returns [`AgentSessionError`] if `project_name` or `agent_id` is invalid.
pub fn build_agent_inspect_plan(
    peer: &PeerNode,
    project_name: &str,
    agent_id: &str,
) -> Result<AgentInspectPlan, AgentSessionError> {
    let remote_repo = peer.remote_project_dir(project_name)?;
    let clean_project = project_name.trim().to_string();
    let clean_id = normalize_agent_id(agent_id);
    if !validate_safe_identifier(&clean_id) {
        return Err(AgentSessionError::InvalidAgentId(agent_id.to_string()));
    }

    let wt_dir = format!("{remote_repo}/.otto-worktrees/{clean_id}");
    let tmux_name = format!("otto-{clean_id}");
    let events_file = format!("~/.cheap-otto/events/{clean_project}/{clean_id}.ndjson");
    let log_file = format!("~/.cheap-otto/logs/{clean_project}/{clean_id}.log");

    let remote_probe = format!(
        "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$PATH\"; \
         echo '===TMUX==='; (tmux has-session -t {tmux_name} 2>/dev/null && tmux list-panes -t {tmux_name} -F '#{{pane_dead}}:#{{pane_dead_status}}' || echo 'no-session'); \
         echo '===GIT==='; (if [ -d {wt_dir} ]; then git -C {wt_dir} status -sb 2>/dev/null; git -C {wt_dir} diff --stat 2>/dev/null; else echo 'no-worktree'; fi); \
         echo '===NDJSON==='; (if [ -f {events_file} ]; then tail -n 80 {events_file}; fi); \
         echo '===LOG==='; (if [ -f {log_file} ]; then tail -n 30 {log_file}; fi)"
    );

    let probe_spec = peer.build_ssh_probe_spec(
        "Inspect live agent session state, git diff, and event stream",
        &remote_probe,
    );

    Ok(AgentInspectPlan {
        agent_id: clean_id,
        project_name: clean_project,
        probe_spec,
    })
}

/// Parameters for sending a direct follow-up query or instruction to an existing agent (`ask`).
pub struct AgentAskParams<'a> {
    /// Existing session metadata (or synthesized fallback if querying an unmanaged worktree).
    pub record: &'a AgentSessionRecord,
    /// Question or follow-up coding instruction for this specific agent.
    pub prompt: &'a str,
    /// Optional harness switch (`None` keeps the agent's current harness).
    pub override_harness: Option<AgentRunner>,
    /// Optional model switch (`None` keeps the agent's current model).
    pub override_model: Option<String>,
    /// Optional reasoning effort switch (`None` keeps the agent's current effort).
    pub override_effort: Option<ReasoningEffort>,
    /// Optional verification command to run after the follow-up turn (`None` skips commit gate for fast Q&A).
    pub verify_and_commit: Option<&'a str>,
    /// When `true`, dispatches asynchronously inside detached `tmux` (`otto-<id>`) instead of waiting.
    pub run_async: bool,
}

/// Executable plan for querying an individual agent directly (`cheap-otto ask` / `POST /api/agents/:id/ask`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAskPlan {
    /// Resolved [`HarnessRunConfig`] used for this follow-up turn.
    pub harness_config: HarnessRunConfig,
    /// Human-readable SSH command.
    pub display_command: String,
    /// Shell-free process spec to run synchronously or dispatch asynchronously.
    pub exec_spec: CommandSpec,
}

/// Builds an [`AgentAskPlan`] to query or instruct a specific agent inside its isolated worktree.
///
/// Automatically preserves conversation continuity (`--resume` / `--conversation`) when the harness
/// is unchanged, or cleanly starts a fresh session inside the same worktree when switching harnesses
/// (e.g., asking Antigravity `gemini-4-carbon-high` to review a branch created by Claude `sonnet`).
///
/// # Errors
/// Returns [`AgentSessionError`] if `prompt`, `agent_id`, or `model` is invalid.
pub fn build_agent_ask_plan(
    peer: &PeerNode,
    params: &AgentAskParams<'_>,
) -> Result<AgentAskPlan, AgentSessionError> {
    let trimmed_prompt = params.prompt.trim();
    if trimmed_prompt.is_empty() {
        return Err(AgentSessionError::EmptyQuery);
    }
    if !validate_safe_identifier(&params.record.agent_id) {
        return Err(AgentSessionError::InvalidAgentId(
            params.record.agent_id.clone(),
        ));
    }
    let _ = peer.remote_project_dir(&params.record.project_name)?;

    let target_runner = params.override_harness.unwrap_or(params.record.harness);
    let same_harness = target_runner == params.record.harness;
    let target_model = params
        .override_model
        .clone()
        .or_else(|| same_harness.then(|| params.record.model.clone()).flatten());
    let target_effort = params
        .override_effort
        .or_else(|| same_harness.then_some(params.record.effort).flatten());

    let session_id = if same_harness {
        params.record.provider_session_id.clone()
    } else if target_runner == AgentRunner::ClaudeCode {
        Some(generate_session_uuid())
    } else {
        None
    };

    let stream_json = matches!(
        target_runner,
        AgentRunner::ClaudeCode | AgentRunner::Antigravity
    );
    let harness_config = HarnessRunConfig {
        runner: target_runner,
        model: target_model,
        effort: target_effort,
        session_id,
        resume_existing: same_harness,
        stream_json,
    };

    let safe_agent_cmd = harness_config.build_remote_command(trimmed_prompt)?;
    let wt_dir = &params.record.remote_worktree_dir;
    let events_file = &params.record.remote_events_file;
    let log_file = &params.record.remote_log_file;
    let tmux_name = &params.record.tmux_session_name;
    let agent_id = &params.record.agent_id;
    let project = &params.record.project_name;

    let post_verify = if let Some(v_cmd) = params.verify_and_commit
        && !v_cmd.trim().is_empty()
    {
        format!(
            " && {} && (git diff --quiet && git diff --cached --quiet || (git add -A && git commit -m \"otto({agent_id}): follow-up update\"))",
            v_cmd.trim()
        )
    } else {
        String::new()
    };

    let agent_exec = if stream_json {
        format!(
            "mkdir -p ~/.cheap-otto/events/{project} ~/.cheap-otto/logs/{project}; \
             rm -f ~/.cheap-otto/logs/{project}/.{agent_id}.ask_rc; \
             {{ {safe_agent_cmd}; printf '%s' \"$?\" > ~/.cheap-otto/logs/{project}/.{agent_id}.ask_rc; }} | tee -a {events_file} {log_file}; \
             test \"$(cat ~/.cheap-otto/logs/{project}/.{agent_id}.ask_rc 2>/dev/null || echo 1)\" = \"0\"{post_verify}"
        )
    } else {
        format!(
            "mkdir -p ~/.cheap-otto/logs/{project}; {{ {safe_agent_cmd}{post_verify}; }} 2>&1 | tee -a {log_file}"
        )
    };

    let full_remote = if params.run_async {
        let inner = format!(
            "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; \
             cd {wt_dir} && source ~/.zshrc 2>/dev/null || true; source ~/.cargo/env 2>/dev/null || true; \
             {agent_exec}; rc=$?; printf '\\n[OTTO_STATUS exit=%s]\\n' \"$rc\" | tee -a {log_file}; exit \"$rc\""
        );
        format!(
            "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$PATH\"; \
             tmux kill-session -t {tmux_name} 2>/dev/null || true; \
             tmux new-session -d -s {tmux_name} {} && tmux set-option -t {tmux_name} remain-on-exit on",
            posix_single_quote(&inner)
        )
    } else {
        // Execute synchronous `ask` inside the `tmux` server so macOS Login Keychain OAuth
        // credentials (`claude` / `agy`) are inherited identically to `delegate`, while blocking
        // cleanly via `tmux wait-for` (zero polling).
        let out_tmp = format!("~/.cheap-otto/logs/{project}/.{agent_id}.ask_out");
        let sync_rc_tmp = format!("~/.cheap-otto/logs/{project}/.{agent_id}.ask_sync_rc");
        let wait_chan = format!("otto-ask-{project}-{agent_id}");
        let inner = format!(
            "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; \
             cd {wt_dir} && source ~/.zshrc 2>/dev/null || true; source ~/.cargo/env 2>/dev/null || true; \
             {{ {agent_exec}; }} > {out_tmp} 2>&1; rc=$?; printf '%s' \"$rc\" > {sync_rc_tmp}; \
             printf '\\n[OTTO_STATUS exit=%s]\\n' \"$rc\" >> {log_file}; \
             tmux wait-for -S {wait_chan}; exit \"$rc\""
        );
        format!(
            "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$PATH\"; \
             mkdir -p ~/.cheap-otto/logs/{project}; rm -f {out_tmp} {sync_rc_tmp}; \
             tmux kill-session -t {tmux_name} 2>/dev/null || true; \
             tmux new-session -d -s {tmux_name} {} && tmux set-option -t {tmux_name} remain-on-exit on && \
             tmux wait-for {wait_chan} && cat {out_tmp} && test \"$(cat {sync_rc_tmp} 2>/dev/null || echo 1)\" = \"0\"",
            posix_single_quote(&inner)
        )
    };

    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let display_command = format!(
        "ssh -p {port_str} {dest} \"tmux [{tmux_name}] -> cd {wt_dir} && {safe_agent_cmd}\""
    );
    let exec_spec = CommandSpec::new(
        "Send direct query/instruction to addressable agent worktree on MacBook",
        "ssh",
        ["-p", &port_str, &dest, &full_remote],
    );

    Ok(AgentAskPlan {
        harness_config,
        display_command,
        exec_spec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_session_id_on_same_harness_and_resets_when_switching_harness()
    -> Result<(), AgentSessionError> {
        let peer = PeerNode::new("otto-mac", "192.168.1.42", "dev", 22)?;
        let init_cfg = HarnessRunConfig {
            runner: AgentRunner::ClaudeCode,
            model: Some("sonnet".to_string()),
            effort: Some(ReasoningEffort::Medium),
            session_id: Some("11111111-2222-3333-4444-555555555555".to_string()),
            resume_existing: false,
            stream_json: true,
        };
        let rec = AgentSessionRecord::new_initial(
            "cheapotto",
            "auth-worker",
            "~/otto-workspaces/cheapotto/.otto-worktrees/auth-worker",
            &init_cfg,
            "cargo test",
            "Initial task",
            1_700_000_000,
        );

        // 1. Follow-up query on same harness (`claude`) uses `--resume <uuid>` and allows model upgrade to `opus`
        let same_plan = build_agent_ask_plan(
            &peer,
            &AgentAskParams {
                record: &rec,
                prompt: "Why did you choose a mutex here?",
                override_harness: None,
                override_model: Some("opus".to_string()),
                override_effort: None,
                verify_and_commit: None,
                run_async: false,
            },
        )?;
        assert!(same_plan.harness_config.resume_existing);
        assert_eq!(same_plan.harness_config.model.as_deref(), Some("opus"));
        assert!(
            same_plan
                .display_command
                .contains("--resume '11111111-2222-3333-4444-555555555555'")
        );

        // 2. Switching harness mid-worktree (`claude` -> `agy`) starts fresh AGY session in same worktree
        let switch_plan = build_agent_ask_plan(
            &peer,
            &AgentAskParams {
                record: &rec,
                prompt: "Review Claude's changes in this worktree",
                override_harness: Some(AgentRunner::Antigravity),
                override_model: Some("gemini-4-carbon-high".to_string()),
                override_effort: Some(ReasoningEffort::High),
                verify_and_commit: None,
                run_async: false,
            },
        )?;
        assert!(!switch_plan.harness_config.resume_existing);
        assert_eq!(switch_plan.harness_config.runner, AgentRunner::Antigravity);
        assert!(switch_plan.display_command.contains("agy -p"));
        assert!(
            switch_plan
                .display_command
                .contains("--model 'gemini-4-carbon-high'")
        );
        Ok(())
    }

    #[test]
    fn summarizes_ndjson_events_and_exit_status() {
        let ndjson = concat!(
            r#"{"type":"system","session_id":"conv-abc"}"#,
            "\n",
            r#"{"type":"tool_use","name":"Read","input":{"file_path":"src/lib.rs"}}"#,
            "\n",
            r#"{"type":"result","is_error":false,"result":"All 10 modules inspected cleanly."}"#,
            "\n"
        );
        let log_tail = "Finished checking\n[OTTO_STATUS exit=0]\n";
        let summary = summarize_ndjson_and_log(ndjson, log_tail, "fallback");
        assert_eq!(summary.provider_session_id.as_deref(), Some("conv-abc"));
        assert_eq!(summary.latest_activity, Some(AgentActivityKind::Completed));
        assert_eq!(
            summary.latest_response.as_deref(),
            Some("All 10 modules inspected cleanly.")
        );
        assert_eq!(summary.exit_code, Some(0));
    }
}
