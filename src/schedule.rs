//! Recurring Task & Autonomous CI / Agent Scheduler (`launchd` + Run History Ledger).
//!
//! Compiles human-friendly schedules (`--every 15m`, `--daily 02:00`, `--weekly MON@03:00`) into
//! 100% reversible macOS user `LaunchAgents` (`~/Library/LaunchAgents/com.cheapotto.<project>.<id>.plist`)
//! with built-in M1 AC-power and passive thermal safety gates, isolated worktrees, `tmux` visibility,
//! and structured JSONL run history (`~/.cheap-otto/runs/<project>/history.jsonl`).

use crate::bridge::{BridgeError, PeerNode, validate_safe_identifier};
use crate::command::CommandSpec;
use crate::harness::{HarnessError, HarnessRunConfig, ReasoningEffort};
use crate::workspace::parse_git_branch;
use crate::worktree::{AgentRunner, WorktreeError, posix_single_quote};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when parsing schedules, validating CI/agent jobs, or building `launchd` plans.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScheduleError {
    /// Underlying SSH peer or project name validation failure.
    #[error(transparent)]
    Bridge(#[from] BridgeError),
    /// Underlying agent runner validation failure.
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    /// Job identifier contains unsafe path/shell characters.
    #[error("invalid schedule ID '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidJobId(String),
    /// Target Git branch failed ref safety validation.
    #[error("invalid Git branch '{0}' for scheduled job")]
    InvalidBranchName(String),
    /// Cadence expression (`--every`, `--daily`, or `--weekly`) was malformed or out of bounds.
    #[error("invalid schedule cadence '{0}': {1}")]
    InvalidCadence(String, String),
    /// Scheduled CI command was empty.
    #[error("scheduled CI command cannot be empty")]
    EmptyCommand,
    /// Scheduled agent task prompt was empty.
    #[error("scheduled agent task prompt cannot be empty")]
    EmptyPrompt,
    /// Invalid `--model` or `--effort` flag on a scheduled agent job.
    #[error(transparent)]
    Harness(#[from] HarnessError),
}

/// Day of week for weekly `launchd` `StartCalendarInterval` rules (`0 = Sunday .. 6 = Saturday`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weekday {
    /// Sunday (`Weekday = 0`).
    Sun,
    /// Monday (`Weekday = 1`).
    Mon,
    /// Tuesday (`Weekday = 2`).
    Tue,
    /// Wednesday (`Weekday = 3`).
    Wed,
    /// Thursday (`Weekday = 4`).
    Thu,
    /// Friday (`Weekday = 5`).
    Fri,
    /// Saturday (`Weekday = 6`).
    Sat,
}

impl Weekday {
    /// Parses a weekday name or 3-letter abbreviation (case-insensitive).
    ///
    /// # Errors
    /// Returns [`ScheduleError::InvalidCadence`] if the weekday token is unrecognized.
    pub fn parse(raw: &str) -> Result<Self, ScheduleError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "sun" | "sunday" => Ok(Self::Sun),
            "mon" | "monday" => Ok(Self::Mon),
            "tue" | "tues" | "tuesday" => Ok(Self::Tue),
            "wed" | "wednesday" => Ok(Self::Wed),
            "thu" | "thur" | "thurs" | "thursday" => Ok(Self::Thu),
            "fri" | "friday" => Ok(Self::Fri),
            "sat" | "saturday" => Ok(Self::Sat),
            _ => Err(ScheduleError::InvalidCadence(
                raw.to_string(),
                "expected weekday SUN, MON, TUE, WED, THU, FRI, or SAT".to_string(),
            )),
        }
    }

    /// Returns the numeric `launchd` `Weekday` integer (`0..=6`).
    #[must_use = "returns the launchd Weekday integer"]
    pub const fn launchd_int(self) -> u8 {
        match self {
            Self::Sun => 0,
            Self::Mon => 1,
            Self::Tue => 2,
            Self::Wed => 3,
            Self::Thu => 4,
            Self::Fri => 5,
            Self::Sat => 6,
        }
    }

    /// Returns the canonical 3-letter uppercase abbreviation.
    #[must_use = "returns the canonical 3-letter weekday code"]
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Sun => "SUN",
            Self::Mon => "MON",
            Self::Tue => "TUE",
            Self::Wed => "WED",
            Self::Thu => "THU",
            Self::Fri => "FRI",
            Self::Sat => "SAT",
        }
    }
}

/// Trigger cadence compiled into macOS `launchd` `StartInterval` or `StartCalendarInterval`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleCadence {
    /// Fixed interval in seconds (`>= 60`, e.g., `900` for `15m`).
    IntervalSecs(u32),
    /// Runs once every day at `hour:minute` (`00:00..=23:59`) in the M1's local timezone.
    Daily {
        /// Hour in 24-hour clock (`0..=23`).
        hour: u8,
        /// Minute (`0..=59`).
        minute: u8,
    },
    /// Runs once per week on `weekday` at `hour:minute`.
    Weekly {
        /// Day of the week.
        weekday: Weekday,
        /// Hour in 24-hour clock (`0..=23`).
        hour: u8,
        /// Minute (`0..=59`).
        minute: u8,
    },
}

impl ScheduleCadence {
    /// Parses a human-friendly interval string (`"60s"`, `"15m"`, `"2h"`, `"1d"`).
    ///
    /// Enforces a minimum interval of `60` seconds to protect the fanless M1 appliance.
    ///
    /// # Errors
    /// Returns [`ScheduleError::InvalidCadence`] if the unit is missing/unknown or `< 60s`.
    pub fn parse_interval(raw: &str) -> Result<Self, ScheduleError> {
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.len() < 2 {
            return Err(ScheduleError::InvalidCadence(
                raw.to_string(),
                "expected duration with unit such as '15m', '2h', or '120s'".to_string(),
            ));
        }

        let (num_part, unit) = trimmed.split_at(trimmed.len() - 1);
        let value = num_part.parse::<u32>().map_err(|_| {
            ScheduleError::InvalidCadence(
                raw.to_string(),
                "numeric duration prefix must be a positive integer".to_string(),
            )
        })?;

        let multiplier: u32 = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3_600,
            "d" => 86_400,
            _ => {
                return Err(ScheduleError::InvalidCadence(
                    raw.to_string(),
                    "unit must be 's' (seconds), 'm' (minutes), 'h' (hours), or 'd' (days)"
                        .to_string(),
                ));
            }
        };

        let secs = value.checked_mul(multiplier).ok_or_else(|| {
            ScheduleError::InvalidCadence(raw.to_string(), "interval overflows u32 seconds".into())
        })?;

        if secs < 60 {
            return Err(ScheduleError::InvalidCadence(
                raw.to_string(),
                "minimum recurring interval is 60s (1m)".to_string(),
            ));
        }

        Ok(Self::IntervalSecs(secs))
    }

    /// Parses a daily `HH:MM` 24-hour clock string (e.g., `"02:30"` or `"18:05"`).
    ///
    /// # Errors
    /// Returns [`ScheduleError::InvalidCadence`] if `HH:MM` is malformed or out of `00:00..=23:59`.
    pub fn parse_daily(raw: &str) -> Result<Self, ScheduleError> {
        let (hour, minute) = parse_hh_mm(raw)?;
        Ok(Self::Daily { hour, minute })
    }

    /// Parses a weekly `DAY@HH:MM` expression (e.g., `"MON@02:00"` or `"FRI@17:30"`).
    ///
    /// # Errors
    /// Returns [`ScheduleError::InvalidCadence`] if the weekday or `HH:MM` part is invalid.
    pub fn parse_weekly(raw: &str) -> Result<Self, ScheduleError> {
        let Some((day_part, time_part)) = raw.trim().split_once('@') else {
            return Err(ScheduleError::InvalidCadence(
                raw.to_string(),
                "expected 'DAY@HH:MM' format such as 'MON@02:00'".to_string(),
            ));
        };
        let weekday = Weekday::parse(day_part)?;
        let (hour, minute) = parse_hh_mm(time_part)?;
        Ok(Self::Weekly {
            weekday,
            hour,
            minute,
        })
    }

    /// Formats a concise human-readable cadence summary.
    #[must_use = "returns formatted schedule summary"]
    pub fn human_summary(self) -> String {
        match self {
            Self::IntervalSecs(secs) if secs.is_multiple_of(3_600) => {
                format!("every {}h", secs / 3_600)
            }
            Self::IntervalSecs(secs) if secs.is_multiple_of(60) => {
                format!("every {}m", secs / 60)
            }
            Self::IntervalSecs(secs) => format!("every {secs}s"),
            Self::Daily { hour, minute } => format!("daily at {hour:02}:{minute:02}"),
            Self::Weekly {
                weekday,
                hour,
                minute,
            } => format!("weekly {} at {hour:02}:{minute:02}", weekday.short_name()),
        }
    }
}

fn parse_hh_mm(raw: &str) -> Result<(u8, u8), ScheduleError> {
    let Some((hh, mm)) = raw.trim().split_once(':') else {
        return Err(ScheduleError::InvalidCadence(
            raw.to_string(),
            "expected 24-hour 'HH:MM' time such as '02:30'".to_string(),
        ));
    };
    let hour = hh.parse::<u8>().map_err(|_| {
        ScheduleError::InvalidCadence(raw.to_string(), "hour must be 0..=23".to_string())
    })?;
    let minute = mm.parse::<u8>().map_err(|_| {
        ScheduleError::InvalidCadence(raw.to_string(), "minute must be 0..=59".to_string())
    })?;
    if hour > 23 || minute > 59 {
        return Err(ScheduleError::InvalidCadence(
            raw.to_string(),
            "time out of range (hour must be 0..=23, minute 0..=59)".to_string(),
        ));
    }
    Ok((hour, minute))
}

/// Workload executed on the M1 MacBook whenever the schedule fires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduledWorkload {
    /// Runs an automated CI pipeline (e.g., `cargo fmt --check && cargo clippy && cargo test`).
    CiPipeline {
        /// Branch to fetch/checkout in an isolated CI worktree.
        branch: String,
        /// Shell command executed inside the isolated CI worktree.
        command: String,
        /// When `true`, skips execution if the target branch SHA has not changed since the last run.
        only_on_change: bool,
    },
    /// Runs an autonomous coding agent inside an isolated worktree followed by a verification gate.
    AgentTask {
        /// Base branch to branch from on each scheduled run.
        branch: String,
        /// Coding agent CLI (`claude`, `agy`, `kiro`, or `custom`).
        agent: AgentRunner,
        /// Optional model override (`sonnet`, `gemini-4-carbon-high`, etc.).
        #[serde(default)]
        model: Option<String>,
        /// Optional reasoning effort override (`low`..`max`).
        #[serde(default)]
        effort: Option<ReasoningEffort>,
        /// Task prompt passed to the agent.
        prompt: String,
        /// Verification gate command that must pass before committing.
        verify_command: String,
        /// Whether to push `otto/sched-<id>` to `origin` and open/update a Draft PR via `gh`.
        create_draft_pr: bool,
    },
}

impl ScheduledWorkload {
    /// Returns `"ci"` or `"agent"` for compact ledger & CLI display.
    #[must_use = "returns the workload kind label"]
    pub const fn kind_label(&self) -> &'static str {
        match self {
            Self::CiPipeline { .. } => "ci",
            Self::AgentTask { .. } => "agent",
        }
    }
}

/// Validated specification for a recurring M1 CI or autonomous agent job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledJobSpec {
    /// Sanitized job identifier (`<id>`).
    pub job_id: String,
    /// Sanitized project workspace name (`<project>`).
    pub project_name: String,
    /// Reverse-DNS macOS `launchd` label (`com.cheapotto.<project>.<id>`).
    pub launchd_label: String,
    /// Trigger cadence.
    pub cadence: ScheduleCadence,
    /// CI or agent workload configuration.
    pub workload: ScheduledWorkload,
    /// Allow scheduled execution even when unplugged on battery power (default `false`).
    #[serde(default)]
    pub allow_battery: bool,
    /// Trigger immediately when loaded via `launchctl bootstrap` (`RunAtLoad`).
    #[serde(default)]
    pub run_at_load: bool,
    /// Absolute path to the generated runner script on macOS.
    pub remote_script_path: String,
    /// Absolute path to the `LaunchAgents` `.plist` file on macOS.
    pub remote_plist_path: String,
    /// Persistent log file path (`~/.cheap-otto/logs/<project>/sched-<id>.log`).
    pub remote_log_file: String,
}

/// Persistent local registry of configured M1 schedules stored in `.cheap-otto/schedules.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleRegistry {
    /// Registered recurring jobs across projects.
    #[serde(default)]
    pub jobs: Vec<ScheduledJobSpec>,
}

impl ScheduleRegistry {
    /// Inserts or replaces a scheduled job matching `(project_name, job_id)`.
    pub fn upsert(&mut self, spec: ScheduledJobSpec) {
        if let Some(existing) = self
            .jobs
            .iter_mut()
            .find(|j| j.project_name == spec.project_name && j.job_id == spec.job_id)
        {
            *existing = spec;
        } else {
            self.jobs.push(spec);
        }
    }

    /// Removes a scheduled job matching `(project_name, job_id)`, returning `true` if found.
    pub fn remove(&mut self, project_name: &str, job_id: &str) -> bool {
        let before = self.jobs.len();
        self.jobs
            .retain(|j| !(j.project_name == project_name && j.job_id == job_id));
        self.jobs.len() != before
    }
}

/// Execution plan to install and load a recurring `launchd` job on the M1 MacBook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleInstallPlan {
    /// Validated job specification.
    pub spec: ScheduledJobSpec,
    /// Generated macOS `launchd` XML `.plist` document.
    pub plist_xml: String,
    /// Generated POSIX runner script with AC/thermal gates, `tmux` visibility, and JSONL logging.
    pub runner_script: String,
    /// Human-readable command summary.
    pub display_steps: Vec<String>,
    /// Shell-free SSH installation & `launchctl` specifications.
    pub install_specs: Vec<CommandSpec>,
}

/// Execution plan to unload and delete a recurring `launchd` job from the M1 MacBook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleRemovePlan {
    /// Project slug.
    pub project_name: String,
    /// Job slug.
    pub job_id: String,
    /// `launchd` reverse-DNS label.
    pub launchd_label: String,
    /// Human-readable command preview.
    pub display_command: String,
    /// Shell-free SSH removal specification.
    pub ssh_spec: CommandSpec,
}

/// Outcome recorded at the completion of a scheduled CI or agent execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Command / agent + verification exited `0`.
    Pass,
    /// Command or verification exited non-zero.
    Fail,
    /// `--on-change` detected no new commits on the target branch.
    SkippedUnchanged,
    /// Skipped because the MacBook was on battery power or `ThermalLevel >= Serious`.
    SkippedHardwareGuard,
}

/// Structured JSONL entry written to `~/.cheap-otto/runs/<project>/history.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRunRecord {
    /// Scheduled job identifier.
    pub job_id: String,
    /// Project workspace name.
    pub project: String,
    /// `"ci"` or `"agent"`.
    pub kind: String,
    /// Execution status.
    pub status: RunStatus,
    /// Git commit SHA evaluated (or `"none"` if skipped prior to Git checkout).
    pub commit_sha: String,
    /// Unix epoch timestamp when the tick started.
    pub started_epoch_secs: u64,
    /// Wall-clock duration in seconds.
    pub duration_secs: u64,
    /// Path to the persistent log file on the MacBook.
    pub log_file: String,
}

/// Input options for constructing a [`ScheduledJobSpec`].
#[derive(Debug, Clone)]
pub struct NewScheduleParams<'a> {
    /// Unique job slug within the project (e.g., `"main-ci"`, `"nightly-audit"`).
    pub job_id: &'a str,
    /// Project workspace slug.
    pub project_name: &'a str,
    /// Trigger cadence.
    pub cadence: ScheduleCadence,
    /// CI or autonomous agent workload.
    pub workload: ScheduledWorkload,
    /// Whether battery execution is permitted.
    pub allow_battery: bool,
    /// Whether `RunAtLoad` is enabled in the `.plist`.
    pub run_at_load: bool,
}

/// Escapes special XML characters (`&`, `<`, `>`, `"`, `'`) for safe `.plist` generation.
fn xml_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

/// Validates inputs and builds a deterministic [`ScheduleInstallPlan`] (`.plist` + runner script + SSH specs).
///
/// # Errors
/// Returns [`ScheduleError`] if `job_id`, `project_name`, `branch`, or workload fields are invalid.
#[must_use = "schedule install plan must be persisted, displayed, or executed"]
pub fn build_schedule_install_plan(
    peer: &PeerNode,
    params: &NewScheduleParams<'_>,
) -> Result<ScheduleInstallPlan, ScheduleError> {
    let _ = peer.remote_project_dir(params.project_name)?;
    let clean_project = params.project_name.trim().to_string();

    let clean_id = params.job_id.trim();
    if !validate_safe_identifier(clean_id) {
        return Err(ScheduleError::InvalidJobId(params.job_id.to_string()));
    }

    let validated_workload = match &params.workload {
        ScheduledWorkload::CiPipeline {
            branch,
            command,
            only_on_change,
        } => {
            let clean_branch = parse_git_branch(branch)
                .map_err(|_| ScheduleError::InvalidBranchName(branch.clone()))?;
            let clean_cmd = command.trim();
            if clean_cmd.is_empty() {
                return Err(ScheduleError::EmptyCommand);
            }
            ScheduledWorkload::CiPipeline {
                branch: clean_branch,
                command: clean_cmd.to_string(),
                only_on_change: *only_on_change,
            }
        }
        ScheduledWorkload::AgentTask {
            branch,
            agent,
            model,
            effort,
            prompt,
            verify_command,
            create_draft_pr,
        } => {
            let clean_branch = parse_git_branch(branch)
                .map_err(|_| ScheduleError::InvalidBranchName(branch.clone()))?;
            let clean_prompt = prompt.trim();
            if clean_prompt.is_empty() {
                return Err(ScheduleError::EmptyPrompt);
            }
            let h_cfg = HarnessRunConfig {
                runner: *agent,
                model: model.clone(),
                effort: *effort,
                session_id: None,
                resume_existing: false,
                stream_json: false,
            };
            h_cfg.validate()?;
            let clean_verify = if verify_command.trim().is_empty() {
                "cargo test".to_string()
            } else {
                verify_command.trim().to_string()
            };
            ScheduledWorkload::AgentTask {
                branch: clean_branch,
                agent: *agent,
                model: model.clone(),
                effort: *effort,
                prompt: clean_prompt.to_string(),
                verify_command: clean_verify,
                create_draft_pr: *create_draft_pr,
            }
        }
    };

    let macos_home = format!("/Users/{}", peer.ssh_user);
    let launchd_label = format!("com.cheapotto.{clean_project}.{clean_id}");
    let remote_script_path =
        format!("{macos_home}/.cheap-otto/bin/otto-job-{clean_project}-{clean_id}.sh");
    let remote_plist_path = format!("{macos_home}/Library/LaunchAgents/{launchd_label}.plist");
    let remote_log_file = format!("~/.cheap-otto/logs/{clean_project}/sched-{clean_id}.log");

    let spec = ScheduledJobSpec {
        job_id: clean_id.to_string(),
        project_name: clean_project,
        launchd_label,
        cadence: params.cadence,
        workload: validated_workload,
        allow_battery: params.allow_battery,
        run_at_load: params.run_at_load,
        remote_script_path,
        remote_plist_path,
        remote_log_file,
    };

    let plist_xml = render_launchd_plist(peer, &spec);
    let runner_script = render_runner_script(peer, &spec);

    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let install_remote_cmd = format!(
        "mkdir -p \"$HOME/.cheap-otto/bin\" \"$HOME/.cheap-otto/logs/{}\" \"$HOME/.cheap-otto/runs/{}\" \"$HOME/Library/LaunchAgents\" && printf '%s\\n' {} > {} && chmod 755 {} && printf '%s\\n' {} > {} && (launchctl bootout \"gui/$(id -u)\" {} 2>/dev/null || true) && launchctl bootstrap \"gui/$(id -u)\" {}",
        spec.project_name,
        spec.project_name,
        posix_single_quote(&runner_script),
        posix_single_quote(&spec.remote_script_path),
        posix_single_quote(&spec.remote_script_path),
        posix_single_quote(&plist_xml),
        posix_single_quote(&spec.remote_plist_path),
        posix_single_quote(&spec.remote_plist_path),
        posix_single_quote(&spec.remote_plist_path),
    );

    let install_specs = vec![CommandSpec::new(
        "Install runner script, write launchd plist, and bootstrap LaunchAgent on MacBook",
        "ssh",
        ["-p", &port_str, &dest, &install_remote_cmd],
    )];

    let display_steps = vec![
        format!("Write runner script -> {}", spec.remote_script_path),
        format!("Write LaunchAgent   -> {}", spec.remote_plist_path),
        format!(
            "ssh -p {port_str} {dest} \"launchctl bootstrap gui/\\$(id -u) {}\"",
            spec.remote_plist_path
        ),
    ];

    Ok(ScheduleInstallPlan {
        spec,
        plist_xml,
        runner_script,
        display_steps,
        install_specs,
    })
}

/// Renders a deterministic macOS `launchd` XML `.plist` for the given [`ScheduledJobSpec`].
#[must_use = "plist XML must be written to ~/Library/LaunchAgents or displayed"]
pub fn render_launchd_plist(peer: &PeerNode, spec: &ScheduledJobSpec) -> String {
    let macos_home = format!("/Users/{}", peer.ssh_user);
    let launchd_out_log = format!(
        "{macos_home}/.cheap-otto/logs/{}/launchd-{}.out.log",
        spec.project_name, spec.job_id
    );
    let launchd_err_log = format!(
        "{macos_home}/.cheap-otto/logs/{}/launchd-{}.err.log",
        spec.project_name, spec.job_id
    );

    let cadence_xml = match spec.cadence {
        ScheduleCadence::IntervalSecs(secs) => {
            format!("  <key>StartInterval</key>\n  <integer>{secs}</integer>\n")
        }
        ScheduleCadence::Daily { hour, minute } => format!(
            "  <key>StartCalendarInterval</key>\n  <dict>\n    <key>Hour</key>\n    <integer>{hour}</integer>\n    <key>Minute</key>\n    <integer>{minute}</integer>\n  </dict>\n"
        ),
        ScheduleCadence::Weekly {
            weekday,
            hour,
            minute,
        } => format!(
            "  <key>StartCalendarInterval</key>\n  <dict>\n    <key>Weekday</key>\n    <integer>{}</integer>\n    <key>Hour</key>\n    <integer>{hour}</integer>\n    <key>Minute</key>\n    <integer>{minute}</integer>\n  </dict>\n",
            weekday.launchd_int()
        ),
    };

    let run_at_load_tag = if spec.run_at_load {
        "<true/>"
    } else {
        "<false/>"
    };

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \x20 <key>Label</key>\n\
         \x20 <string>{label}</string>\n\
         \x20 <key>ProgramArguments</key>\n\
         \x20 <array>\n\
         \x20   <string>/bin/sh</string>\n\
         \x20   <string>{script}</string>\n\
         \x20 </array>\n\
         \x20 <key>RunAtLoad</key>\n\
         \x20 {run_at_load_tag}\n\
         {cadence_xml}\
         \x20 <key>ProcessType</key>\n\
         \x20 <string>Background</string>\n\
         \x20 <key>Nice</key>\n\
         \x20 <integer>5</integer>\n\
         \x20 <key>StandardOutPath</key>\n\
         \x20 <string>{out_log}</string>\n\
         \x20 <key>StandardErrorPath</key>\n\
         \x20 <string>{err_log}</string>\n\
         </dict>\n\
         </plist>",
        label = xml_escape(&spec.launchd_label),
        script = xml_escape(&spec.remote_script_path),
        out_log = xml_escape(&launchd_out_log),
        err_log = xml_escape(&launchd_err_log),
    )
}

/// Renders the self-contained POSIX runner script executed by `launchd` on the M1 MacBook.
#[must_use = "runner script must be installed on the MacBook or inspected"]
pub fn render_runner_script(peer: &PeerNode, spec: &ScheduledJobSpec) -> String {
    let project = &spec.project_name;
    let job_id = &spec.job_id;
    let kind = spec.workload.kind_label();
    let base_repo = format!("$HOME/otto-workspaces/{project}");
    let sched_wt = format!("$HOME/.cheap-otto/scheduled-worktrees/{project}/{job_id}");
    let log_file = format!("$HOME/.cheap-otto/logs/{project}/sched-{job_id}.log");
    let ledger_file = format!("$HOME/.cheap-otto/runs/{project}/history.jsonl");
    let sha_cache_file = format!("$HOME/.cheap-otto/runs/{project}/{job_id}.last_sha");
    let rc_file = format!("$HOME/.cheap-otto/runs/{project}/.{job_id}.rc");
    let tmux_session = format!("otto-sched-{job_id}");
    let wait_channel = format!("otto-done-{project}-{job_id}");

    let battery_guard = if spec.allow_battery {
        "# Battery execution explicitly allowed (--allow-battery)".to_string()
    } else {
        format!(
            "if ! pmset -g batt 2>/dev/null | grep -q \"AC Power\"; then\n\
             \x20 printf '{{\"job_id\":\"{job_id}\",\"project\":\"{project}\",\"kind\":\"{kind}\",\"status\":\"skipped_hardware_guard\",\"commit_sha\":\"none\",\"started_epoch_secs\":%s,\"duration_secs\":0,\"log_file\":\"~/.cheap-otto/logs/{project}/sched-{job_id}.log\"}}\\n' \"$START_TS\" >> \"{ledger_file}\"\n\
             \x20 exit 0\n\
             fi"
        )
    };

    let (branch, only_on_change, inner_action) = match &spec.workload {
        ScheduledWorkload::CiPipeline {
            branch,
            command,
            only_on_change,
        } => (branch.as_str(), *only_on_change, command.clone()),
        ScheduledWorkload::AgentTask {
            branch,
            agent,
            model,
            effort,
            prompt,
            verify_command,
            create_draft_pr,
        } => {
            let h_cfg = HarnessRunConfig {
                runner: *agent,
                model: model.clone(),
                effort: *effort,
                session_id: None,
                resume_existing: false,
                stream_json: false,
            };
            let safe_agent = h_cfg
                .build_remote_command(prompt)
                .unwrap_or_else(|_| agent.build_remote_safe_command(prompt));
            let pr_tail = if *create_draft_pr {
                format!(
                    " && git push origin HEAD:refs/heads/otto/sched-{job_id} --force && (gh pr create --draft --title \"otto({job_id}): scheduled agent update\" --body \"Automated scheduled run from cheap-otto.\" --head \"otto/sched-{job_id}\" --base \"{branch}\" 2>/dev/null || true)"
                )
            } else {
                String::new()
            };
            let pipeline = format!(
                "{safe_agent} && {verify_command} && (git diff --quiet && git diff --cached --quiet || (git add -A && git commit -m \"otto({job_id}): scheduled agent update\"{pr_tail}))"
            );
            (branch.as_str(), false, pipeline)
        }
    };

    let on_change_block = if only_on_change {
        format!(
            "LAST_SHA=$(cat \"{sha_cache_file}\" 2>/dev/null || echo \"\")\n\
             if [ \"$TARGET_SHA\" = \"$LAST_SHA\" ] && [ -n \"$TARGET_SHA\" ]; then\n\
             \x20 printf '{{\"job_id\":\"{job_id}\",\"project\":\"{project}\",\"kind\":\"{kind}\",\"status\":\"skipped_unchanged\",\"commit_sha\":\"%s\",\"started_epoch_secs\":%s,\"duration_secs\":0,\"log_file\":\"~/.cheap-otto/logs/{project}/sched-{job_id}.log\"}}\\n' \"$TARGET_SHA\" \"$START_TS\" >> \"{ledger_file}\"\n\
             \x20 exit 0\n\
             fi\n\
             printf '%s' \"$TARGET_SHA\" > \"{sha_cache_file}\""
        )
    } else {
        "# Always run on schedule tick".to_string()
    };

    let inner_tmux_cmd = format!(
        "cd \"{sched_wt}\" && source ~/.zshrc 2>/dev/null || true; source ~/.cargo/env 2>/dev/null || true; {{ {{ {inner_action}; printf '%s' \"$?\" > \"{rc_file}\"; }} 2>&1 | tee -a \"{log_file}\"; }}; rc=$(cat \"{rc_file}\" 2>/dev/null || echo 1); printf '\\n[OTTO_STATUS exit=%s]\\n' \"$rc\" | tee -a \"{log_file}\"; tmux wait-for -S {wait_channel} 2>/dev/null || true; exit \"$rc\""
    );

    let _ = peer; // peer.ssh_user already reflected in spec paths; script uses $HOME cleanly at runtime.

    format!(
        "#!/bin/sh\n\
         set -u\n\
         export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin\"\n\
         START_TS=$(date +%s)\n\
         mkdir -p \"$HOME/.cheap-otto/logs/{project}\" \"$HOME/.cheap-otto/runs/{project}\" \"$HOME/.cheap-otto/scheduled-worktrees/{project}\"\n\
         \n\
         # 1. Hardware & Fanless Thermal Guard\n\
         {battery_guard}\n\
         THERM=$(sysctl -n kern.thermalstate 2>/dev/null || echo 0)\n\
         if [ \"$THERM\" -ge 2 ] 2>/dev/null; then\n\
         \x20 printf '{{\"job_id\":\"{job_id}\",\"project\":\"{project}\",\"kind\":\"{kind}\",\"status\":\"skipped_hardware_guard\",\"commit_sha\":\"none\",\"started_epoch_secs\":%s,\"duration_secs\":0,\"log_file\":\"~/.cheap-otto/logs/{project}/sched-{job_id}.log\"}}\\n' \"$START_TS\" >> \"{ledger_file}\"\n\
         \x20 exit 0\n\
         fi\n\
         \n\
         # 2. Resolve Base Git Commit (origin/{branch} -> otto-sync -> HEAD)\n\
         if [ ! -d \"{base_repo}/.git\" ]; then\n\
         \x20 echo \"Base repository {base_repo} not initialized yet; run cheap-otto sync first.\" >> \"{log_file}\"\n\
         \x20 exit 1\n\
         fi\n\
         if git -C \"{base_repo}\" remote get-url origin >/dev/null 2>&1; then\n\
         \x20 git -C \"{base_repo}\" fetch origin \"{branch}\" --quiet 2>/dev/null || true\n\
         fi\n\
         TARGET_REF=\"HEAD\"\n\
         if git -C \"{base_repo}\" rev-parse --verify \"refs/remotes/origin/{branch}\" >/dev/null 2>&1; then\n\
         \x20 TARGET_REF=\"refs/remotes/origin/{branch}\"\n\
         elif git -C \"{base_repo}\" rev-parse --verify \"refs/heads/otto-sync\" >/dev/null 2>&1; then\n\
         \x20 TARGET_REF=\"refs/heads/otto-sync\"\n\
         fi\n\
         TARGET_SHA=$(git -C \"{base_repo}\" rev-parse --short=12 \"$TARGET_REF\" 2>/dev/null || echo \"unknown\")\n\
         {on_change_block}\n\
         \n\
         # 3. Prepare Isolated Scheduled Worktree (never touches manual workspace)\n\
         git -C \"{base_repo}\" worktree add --force -B \"otto/sched-{job_id}\" \"{sched_wt}\" \"$TARGET_REF\" >> \"{log_file}\" 2>&1\n\
         \n\
         # 4. Run Inside Observable tmux Session (remain-on-exit + synchronous wait)\n\
         rm -f \"{rc_file}\"\n\
         if command -v tmux >/dev/null 2>&1; then\n\
         \x20 tmux kill-session -t \"{tmux_session}\" 2>/dev/null || true\n\
         \x20 tmux new-session -d -s \"{tmux_session}\" {quoted_tmux_cmd}\n\
         \x20 tmux set-option -t \"{tmux_session}\" remain-on-exit on 2>/dev/null || true\n\
         \x20 tmux wait-for \"{wait_channel}\"\n\
         else\n\
         \x20 /bin/sh -c {quoted_tmux_cmd}\n\
         fi\n\
         RC=$(cat \"{rc_file}\" 2>/dev/null || echo 1)\n\
         rm -f \"{rc_file}\"\n\
         END_TS=$(date +%s)\n\
         DUR=$((END_TS - START_TS))\n\
         if [ \"$RC\" -eq 0 ] 2>/dev/null; then STATUS=\"pass\"; else STATUS=\"fail\"; fi\n\
         printf '{{\"job_id\":\"{job_id}\",\"project\":\"{project}\",\"kind\":\"{kind}\",\"status\":\"%s\",\"commit_sha\":\"%s\",\"started_epoch_secs\":%s,\"duration_secs\":%s,\"log_file\":\"~/.cheap-otto/logs/{project}/sched-{job_id}.log\"}}\\n' \"$STATUS\" \"$TARGET_SHA\" \"$START_TS\" \"$DUR\" >> \"{ledger_file}\"\n\
         exit \"$RC\"\n",
        quoted_tmux_cmd = posix_single_quote(&inner_tmux_cmd),
    )
}

/// Builds a [`ScheduleRemovePlan`] to unload and delete a `launchd` schedule from the M1 MacBook.
///
/// # Errors
/// Returns [`ScheduleError`] if `project_name` or `job_id` contains unsafe characters.
#[must_use = "schedule remove plan must be executed or displayed"]
pub fn build_schedule_remove_plan(
    peer: &PeerNode,
    project_name: &str,
    job_id: &str,
) -> Result<ScheduleRemovePlan, ScheduleError> {
    let _ = peer.remote_project_dir(project_name)?;
    let clean_project = project_name.trim().to_string();
    let clean_id = job_id.trim();
    if !validate_safe_identifier(clean_id) {
        return Err(ScheduleError::InvalidJobId(job_id.to_string()));
    }

    let launchd_label = format!("com.cheapotto.{clean_project}.{clean_id}");
    let macos_home = format!("/Users/{}", peer.ssh_user);
    let plist_path = format!("{macos_home}/Library/LaunchAgents/{launchd_label}.plist");
    let script_path =
        format!("{macos_home}/.cheap-otto/bin/otto-job-{clean_project}-{clean_id}.sh");
    let tmux_session = format!("otto-sched-{clean_id}");

    let remote_cmd = format!(
        "(launchctl bootout \"gui/$(id -u)\" {} 2>/dev/null || true) && (tmux kill-session -t {} 2>/dev/null || true) && rm -f {} {}",
        posix_single_quote(&plist_path),
        posix_single_quote(&tmux_session),
        posix_single_quote(&plist_path),
        posix_single_quote(&script_path),
    );
    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let ssh_spec = CommandSpec::new(
        "Unload LaunchAgent and remove schedule artifacts on MacBook",
        "ssh",
        ["-p", &port_str, &dest, &remote_cmd],
    );
    let display_command = ssh_spec.display_command();

    Ok(ScheduleRemovePlan {
        project_name: clean_project,
        job_id: clean_id.to_string(),
        launchd_label,
        display_command,
        ssh_spec,
    })
}

/// Builds a shell-free [`CommandSpec`] to trigger an installed `launchd` job immediately (`run-now`).
///
/// # Errors
/// Returns [`ScheduleError`] if `project_name` or `job_id` contains unsafe characters.
#[must_use = "kickstart spec must be executed or displayed"]
pub fn build_schedule_kickstart_spec(
    peer: &PeerNode,
    project_name: &str,
    job_id: &str,
) -> Result<CommandSpec, ScheduleError> {
    let _ = peer.remote_project_dir(project_name)?;
    let clean_project = project_name.trim();
    let clean_id = job_id.trim();
    if !validate_safe_identifier(clean_id) {
        return Err(ScheduleError::InvalidJobId(job_id.to_string()));
    }
    let label = format!("com.cheapotto.{clean_project}.{clean_id}");
    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let remote_cmd = format!("launchctl kickstart -k \"gui/$(id -u)/{label}\"");

    Ok(CommandSpec::new(
        "Trigger scheduled LaunchAgent run immediately on MacBook",
        "ssh",
        ["-p", &port_str, &dest, &remote_cmd],
    ))
}

/// Builds a shell-free [`CommandSpec`] to query live loaded `com.cheapotto.*` `launchd` jobs on macOS.
#[must_use = "live schedule status spec should be executed via std::process::Command"]
pub fn build_live_launchctl_list_spec(peer: &PeerNode) -> CommandSpec {
    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let remote_cmd = "launchctl list 2>/dev/null | grep 'com\\.cheapotto\\.' || echo 'No loaded com.cheapotto LaunchAgents'";
    CommandSpec::new(
        "Query loaded com.cheapotto LaunchAgents on MacBook",
        "ssh",
        ["-p", &port_str, &dest, remote_cmd],
    )
}

/// Builds a shell-free [`CommandSpec`] to unload all `com.cheapotto.*` `LaunchAgents` during `restore`.
#[must_use = "unload-all spec should be executed during restore --unload-schedules"]
pub fn build_unload_all_schedules_spec(peer: &PeerNode) -> CommandSpec {
    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let remote_cmd = "for plist in \"$HOME\"/Library/LaunchAgents/com.cheapotto.*.plist; do [ -f \"$plist\" ] || continue; launchctl bootout \"gui/$(id -u)\" \"$plist\" 2>/dev/null || true; rm -f \"$plist\"; done";
    CommandSpec::new(
        "Unload and remove all com.cheapotto LaunchAgents on MacBook",
        "ssh",
        ["-p", &port_str, &dest, remote_cmd],
    )
}

/// Builds a shell-free [`CommandSpec`] to fetch the trailing lines of `history.jsonl` for `cheap-otto runs`.
///
/// # Errors
/// Returns [`ScheduleError::Bridge`] if `project_name` is invalid.
#[must_use = "runs fetch spec should be executed or displayed"]
pub fn build_fetch_runs_spec(
    peer: &PeerNode,
    project_name: &str,
    scan_lines: usize,
) -> Result<CommandSpec, ScheduleError> {
    let _ = peer.remote_project_dir(project_name)?;
    let clean_project = project_name.trim();
    let capped = scan_lines.max(1);
    let port_str = peer.ssh_port.to_string();
    let dest = peer.ssh_destination();
    let remote_cmd = format!(
        "if [ -f \"$HOME/.cheap-otto/runs/{clean_project}/history.jsonl\" ]; then tail -n {capped} \"$HOME/.cheap-otto/runs/{clean_project}/history.jsonl\"; fi"
    );

    Ok(CommandSpec::new(
        "Fetch scheduled CI and agent run history ledger from MacBook",
        "ssh",
        ["-p", &port_str, &dest, &remote_cmd],
    ))
}

/// Defensively parses JSONL run history output into [`JobRunRecord`] items, skipping blank or malformed lines.
#[must_use = "parsed run history records should be filtered or displayed"]
pub fn parse_run_history_jsonl(raw_jsonl: &str) -> Vec<JobRunRecord> {
    raw_jsonl
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                serde_json::from_str::<JobRunRecord>(trimmed).ok()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_interval_daily_and_weekly_cadences() -> Result<(), ScheduleError> {
        let every_15m = ScheduleCadence::parse_interval("15m")?;
        assert_eq!(every_15m, ScheduleCadence::IntervalSecs(900));
        assert_eq!(every_15m.human_summary(), "every 15m");

        let daily = ScheduleCadence::parse_daily("02:30")?;
        assert_eq!(
            daily,
            ScheduleCadence::Daily {
                hour: 2,
                minute: 30
            }
        );
        assert_eq!(daily.human_summary(), "daily at 02:30");

        let weekly = ScheduleCadence::parse_weekly("mon@18:05")?;
        assert_eq!(
            weekly,
            ScheduleCadence::Weekly {
                weekday: Weekday::Mon,
                hour: 18,
                minute: 5
            }
        );
        assert_eq!(weekly.human_summary(), "weekly MON at 18:05");
        Ok(())
    }

    #[test]
    fn rejects_sub_minute_intervals_and_out_of_range_times() {
        assert!(ScheduleCadence::parse_interval("10s").is_err());
        assert!(ScheduleCadence::parse_interval("45").is_err());
        assert!(ScheduleCadence::parse_daily("25:00").is_err());
        assert!(ScheduleCadence::parse_weekly("FUNDAY@02:00").is_err());
    }

    #[test]
    fn builds_on_change_ci_launchd_plist_and_thermal_gated_runner_script()
    -> Result<(), ScheduleError> {
        let peer = PeerNode::new("otto-mac", "100.101.102.103", "admin", 22)?;
        let plan = build_schedule_install_plan(
            &peer,
            &NewScheduleParams {
                job_id: "main-ci",
                project_name: "cheap-otto",
                cadence: ScheduleCadence::IntervalSecs(300),
                workload: ScheduledWorkload::CiPipeline {
                    branch: "main".to_string(),
                    command: "cargo clippy --all-targets -- -D warnings && cargo test".to_string(),
                    only_on_change: true,
                },
                allow_battery: false,
                run_at_load: true,
            },
        )?;

        assert_eq!(plan.spec.launchd_label, "com.cheapotto.cheap-otto.main-ci");
        assert_eq!(
            plan.spec.remote_plist_path,
            "/Users/admin/Library/LaunchAgents/com.cheapotto.cheap-otto.main-ci.plist"
        );
        // Verify XML escaping of `&&` inside runner script doesn't corrupt plist (plist invokes script path)
        assert!(
            plan.plist_xml
                .contains("<key>StartInterval</key>\n  <integer>300</integer>")
        );
        assert!(plan.plist_xml.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(plan.runner_script.contains("kern.thermalstate"));
        assert!(plan.runner_script.contains("pmset -g batt"));
        assert!(plan.runner_script.contains("skipped_unchanged"));
        assert!(plan.runner_script.contains("otto-sched-main-ci"));
        assert!(plan.runner_script.contains("remain-on-exit on"));
        Ok(())
    }

    #[test]
    fn builds_scheduled_agent_job_with_draft_pr_and_calendar_plist() -> Result<(), ScheduleError> {
        let peer = PeerNode::new("otto-mac", "192.168.1.42", "admin", 22)?;
        let plan = build_schedule_install_plan(
            &peer,
            &NewScheduleParams {
                job_id: "nightly-audit",
                project_name: "cheap-otto",
                cadence: ScheduleCadence::Daily { hour: 2, minute: 0 },
                workload: ScheduledWorkload::AgentTask {
                    branch: "main".to_string(),
                    agent: AgentRunner::ClaudeCode,
                    model: Some("opus".to_string()),
                    effort: Some(ReasoningEffort::High),
                    prompt: "Audit dependencies & fix compiler warnings".to_string(),
                    verify_command: "cargo test --all-targets".to_string(),
                    create_draft_pr: true,
                },
                allow_battery: false,
                run_at_load: false,
            },
        )?;

        assert!(plan.plist_xml.contains("<key>StartCalendarInterval</key>"));
        assert!(
            plan.plist_xml
                .contains("<key>Hour</key>\n    <integer>2</integer>")
        );
        assert!(plan.runner_script.contains("claude -p"));
        assert!(plan.runner_script.contains("gh pr create --draft"));
        Ok(())
    }

    #[test]
    fn parses_jsonl_run_history_and_skips_corrupt_lines() {
        let sample = "\
{\"job_id\":\"main-ci\",\"project\":\"cheap-otto\",\"kind\":\"ci\",\"status\":\"pass\",\"commit_sha\":\"5810fa6\",\"started_epoch_secs\":1760000100,\"duration_secs\":42,\"log_file\":\"~/.cheap-otto/logs/cheap-otto/sched-main-ci.log\"}\n\
not-valid-json-line\n\
{\"job_id\":\"nightly-audit\",\"project\":\"cheap-otto\",\"kind\":\"agent\",\"status\":\"skipped_hardware_guard\",\"commit_sha\":\"none\",\"started_epoch_secs\":1760003700,\"duration_secs\":0,\"log_file\":\"~/.cheap-otto/logs/cheap-otto/sched-nightly-audit.log\"}\n";

        let records = parse_run_history_jsonl(sample);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].job_id, "main-ci");
        assert_eq!(records[0].status, RunStatus::Pass);
        assert_eq!(records[0].duration_secs, 42);
        assert_eq!(records[1].status, RunStatus::SkippedHardwareGuard);
    }
}
