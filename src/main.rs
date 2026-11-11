#![forbid(unsafe_code)]

use cheap_otto::agent_session::{
    AgentAskParams, AgentSessionRecord, AgentSessionRegistry, AgentSessionStatus, AgentTurnEntry,
    build_agent_ask_plan, build_agent_inspect_plan, normalize_agent_id, summarize_ndjson_and_log,
};
use cheap_otto::arbiter::{ArbiterPolicy, ArbiterSnapshot};
use cheap_otto::bridge::{PeerNode, merge_ssh_config_block};
use cheap_otto::command::CommandSpec;
use cheap_otto::doctor::{
    CheckStatus, ProbeSource, RawTelemetrySnapshot, SystemProbeInput, parse_active_otto_sessions,
};
use cheap_otto::hardware::{
    CpuArch, HardwareProfile, ThermalLevel, parse_battery_health_percent,
    parse_macos_vm_stat_available_mb,
};
use cheap_otto::harness::{
    HarnessCatalogEntry, HarnessRunConfig, ReasoningEffort, default_agy_models,
    default_claude_models, default_kiro_models, generate_session_uuid, parse_agy_models_output,
    standard_efforts,
};
use cheap_otto::macos::MacOsStateBackup;
use cheap_otto::schedule::{
    NewScheduleParams, RunStatus, ScheduleCadence, ScheduleRegistry, ScheduledWorkload,
    build_fetch_runs_spec, build_live_launchctl_list_spec, build_schedule_install_plan,
    build_schedule_kickstart_spec, build_schedule_remove_plan, build_unload_all_schedules_spec,
    parse_run_history_jsonl,
};
use cheap_otto::server::{ServeConfig, probe_local_harnesses, run_serve};
use cheap_otto::template::{NewTemplateParams, TaskTemplate, TemplateRegistry};
use cheap_otto::terminal::{
    ShellMode, ShellTarget, build_attach_session_plan, build_interactive_shell_plan,
    build_job_logs_plan,
};
use cheap_otto::workspace::{
    DetectedWorkspace, WorkspaceRegistry, detect_workspace_from_probes, select_state_dir,
};
use cheap_otto::worktree::{
    AgentRunner, build_delegate_plan_with_harness, build_jobs_status_command,
    build_jobs_status_spec, build_pull_plan, posix_single_quote,
};
use clap::{Args, Parser, Subcommand};
use serde_json::json;
use std::env;
use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(
    name = "cheap-otto",
    version,
    about = "Cross-OS Dev Bridge & Parallel Agent Worktree Runner: Pair your Windows PC with a spare headless MacBook over LAN/Tailscale SSH."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Configure the spare MacBook for 100% reversible headless clamshell mode (AC no-sleep + 78% battery cap)
    Init {
        /// Override detected RAM in GB (auto-detected from live macOS sysctl when omitted; fallback: 16 GB)
        #[arg(long)]
        ram_gb: Option<u32>,
        /// Target battery charge ceiling percentage (50..=100)
        #[arg(long, default_value_t = 78)]
        battery_cap: u8,
        /// Output directory for state backup (`state-backup.json`)
        #[arg(long, default_value = ".cheap-otto")]
        out_dir: PathBuf,
        /// Execute the generated `sudo pmset` power rules after saving `state-backup.json`
        #[arg(long, visible_alias = "execute")]
        apply: bool,
        /// Probe (and with `--apply`, configure) the paired headless MacBook over SSH
        #[arg(long)]
        remote: bool,
        /// Skip live OS/SSH hardware & pmset probing and use the baseline M1 Air snapshot
        #[arg(long)]
        offline: bool,
    },
    /// Option 5 (Cross-OS Dev Bridge): Pair your Windows workstation with the MacBook over LAN or Tailscale SSH
    Pair {
        /// MacBook LAN IP (e.g. 192.168.1.42), mDNS (.local), or Tailscale IP (e.g. 100.101.102.103)
        #[arg(long, default_value = "100.101.102.103")]
        host: String,
        /// SSH username on the MacBook
        #[arg(long, default_value = "admin")]
        user: String,
        /// SSH alias for the MacBook
        #[arg(long, default_value = "otto-mac")]
        alias: String,
        /// SSH port on the MacBook
        #[arg(long, default_value_t = 22)]
        port: u16,
        /// Localhost ports to forward from the MacBook to Windows (comma-separated, e.g. 3000,5173,8080)
        #[arg(long, value_delimiter = ',', default_values_t = [3000, 5173, 8080])]
        ports: Vec<u16>,
        /// State directory where `peer.json` is stored
        #[arg(long, default_value = ".cheap-otto")]
        out_dir: PathBuf,
        /// Merge the generated Host block into `~/.ssh/config` and verify SSH connectivity
        #[arg(long, visible_alias = "execute")]
        apply: bool,
        /// Launch the live localhost port-forwarding SSH tunnel immediately
        #[arg(long)]
        tunnel: bool,
    },
    /// Option 5 (Cross-OS Dev Bridge): Sync your current Windows project to the MacBook over Git + SSH
    Sync {
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory containing `peer.json` (auto-discovered when omitted)
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Overwrite uncommitted manual edits on the MacBook (`git checkout -f`) instead of aborting safely
        #[arg(long)]
        force: bool,
        /// Execute the Git + SSH workspace sync commands immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Open an interactive PTY terminal on the paired MacBook (isolated `manual` tmux session or login shell)
    Shell {
        /// Project workspace to start inside (auto-detected from Git repo unless `--home` is set)
        #[arg(long)]
        project: Option<String>,
        /// Persistent manual `tmux` session name on the MacBook (never touched by automated `otto-*` jobs)
        #[arg(long, default_value = "manual")]
        session: String,
        /// Open a bare `zsh -l` login shell instead of attaching/creating a `tmux` session
        #[arg(long)]
        plain: bool,
        /// Start in the MacBook home directory (`~`) instead of `~/otto-workspaces/<project>`
        #[arg(long)]
        home: bool,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Spawn the interactive SSH PTY session immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Attach live to a running or retained `otto-<id>` / `otto-sched-<id>` `tmux` pane on the MacBook
    Attach {
        /// Task or schedule ID to attach to (e.g. `fix-auth-race` or `main-ci`)
        #[arg(long)]
        id: String,
        /// Attach in read-only mode (`tmux attach -r`) for safe live observation
        #[arg(long)]
        read_only: bool,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Spawn the interactive SSH `tmux attach` session immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// View or stream persistent terminal logs (`~/.cheap-otto/logs/<project>/`) for any delegated or scheduled run
    Logs {
        /// Task or schedule ID whose logs should be displayed
        #[arg(long)]
        id: String,
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Number of trailing log lines to show
        #[arg(long, default_value_t = 80)]
        lines: u32,
        /// Stream live output (`tail -f`) until interrupted with Ctrl+C
        #[arg(long, short = 'f')]
        follow: bool,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Fetch or stream logs over SSH immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Option 5 (Cross-OS Dev Bridge): Run a native macOS / ARM64 command inside the synced MacBook workspace
    Exec {
        /// Command to run on the MacBook (e.g. "cargo test --all-targets")
        #[arg(long, default_value = "cargo test --all-targets")]
        cmd: String,
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Execute the remote SSH command immediately and stream output
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Option 1 (Parallel Agent Farm): Delegate a coding task to an isolated Git worktree on the MacBook
    Delegate {
        /// Coding task prompt for the remote agent
        #[arg(long)]
        task: String,
        /// Optional explicit task slug (auto-generated from prompt if omitted)
        #[arg(long)]
        id: Option<String>,
        /// Coding agent CLI to invoke on the MacBook (`claude`, `agy`, `kiro`, or `custom`)
        #[arg(long, default_value = "claude")]
        agent: String,
        /// Optional model override within the selected harness (e.g. `sonnet`, `opus`, `gemini-4-carbon-high`)
        #[arg(long)]
        model: Option<String>,
        /// Optional reasoning effort (`low`, `medium`, `high`, `xhigh`, `max`)
        #[arg(long)]
        effort: Option<String>,
        /// Verification gate command executed after the agent finishes
        #[arg(long, default_value = "cargo test --all-targets")]
        verify: String,
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
        /// Execute the worktree creation and background `tmux` launch over SSH immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Discover installed coding harnesses (`claude`, `agy`, `kiro`, `custom`), auth status, and selectable models
    Harnesses {
        /// Require live SSH probing against the paired MacBook
        #[arg(long)]
        remote: bool,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Inspect an individual addressable agent's live worktree status, Git diff, and latest response
    Inspect {
        /// Agent task ID (`<id>` or `otto-<id>`)
        #[arg(long)]
        id: String,
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(short, long)]
        project: Option<String>,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
        /// Query live `tmux`, Git diff, and NDJSON telemetry over SSH
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Send a direct follow-up question or instruction to a specific addressable agent (`--resume` / model switch)
    Ask {
        /// Agent task ID (`<id>` or `otto-<id>`)
        #[arg(long)]
        id: String,
        /// Question or follow-up coding instruction for this agent
        #[arg(short, long)]
        prompt: String,
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional harness switch (`claude`, `agy`, `kiro`, `custom`); defaults to the agent's bound harness
        #[arg(long)]
        agent: Option<String>,
        /// Optional model switch within the harness (e.g. upgrade `sonnet` -> `opus`)
        #[arg(long)]
        model: Option<String>,
        /// Optional reasoning effort switch (`low`, `medium`, `high`, `xhigh`, `max`)
        #[arg(long)]
        effort: Option<String>,
        /// Optional verification command to run & commit if the follow-up turn edits code
        #[arg(long)]
        verify: Option<String>,
        /// Dispatch asynchronously inside detached `tmux` instead of waiting for the response
        #[arg(long = "async")]
        run_async: bool,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
        /// Execute the follow-up query on the paired MacBook over SSH
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Manage repeatable multi-harness task playbooks (`templates.json`) for one-click UI & CLI reuse
    Template {
        #[command(subcommand)]
        action: TemplateCommands,
    },
    /// Run the Mission Control HTTP/JSON server & `pixel-agents` event bridge (typically on the M1 at `:7420`)
    Serve {
        /// Bind address
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// HTTP port for Mission Control UI & REST API
        #[arg(long, default_value_t = 7420)]
        port: u16,
        /// Local port where `pixel-agents` standalone runs (`:7421`)
        #[arg(long, default_value_t = 7421)]
        pixel_port: u16,
        /// Optional state directory (`~/.cheap-otto` default)
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Optional workspaces root (`~/otto-workspaces` default)
        #[arg(long)]
        workspaces_root: Option<PathBuf>,
    },
    /// Start `pixel-agents` + `cheap-otto serve` on the paired M1, tunnel `:7420`/`:7421`, and open Mission Control
    Ui {
        /// Local & remote port for Mission Control (`http://localhost:7420`)
        #[arg(long, default_value_t = 7420)]
        port: u16,
        /// Local & remote port for `pixel-agents` visual office (`http://localhost:7421`)
        #[arg(long, default_value_t = 7421)]
        pixel_port: u16,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Do not automatically launch the system web browser
        #[arg(long)]
        no_open: bool,
        /// Start remote services, establish SSH localhost port forwarding, and open the UI
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Option 1 (Parallel Agent Farm): List active and completed agent worktrees on the MacBook
    Jobs {
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Query the MacBook live over SSH to list active `otto-*` `tmux` sessions and worktrees
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Option 1 (Parallel Agent Farm): Pull a completed MacBook worktree branch (`otto/<task-id>`) back into Windows
    Pull {
        /// Task ID of the MacBook worktree to fetch into the local Windows repo
        #[arg(long)]
        id: String,
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Execute `git fetch` and `git diff --stat` immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
        /// Also run `git merge --no-ff otto/<id>` when `--execute` is enabled
        #[arg(long)]
        merge: bool,
        /// Also remove the remote MacBook worktree when `--execute` is enabled
        #[arg(long)]
        cleanup: bool,
    },
    /// Inspect auto-detected Git workspace metadata, active config path, and tracked multi-project workspaces
    Projects {
        /// Optional path to state directory containing `peer.json` and `projects.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Run the 8-point headless appliance health check (probes local macOS or paired SSH MacBook automatically)
    Doctor {
        /// Optional path to state directory containing `peer.json` and `state-backup.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Require live SSH telemetry probing against the paired MacBook
        #[arg(long)]
        remote: bool,
        /// Force deterministic offline baseline evaluation without running OS/SSH probes
        #[arg(long)]
        offline: bool,
    },
    /// Evaluate the Unified Memory & Thermal Arbiter against system telemetry
    Arbiter {
        /// Available unified memory in MB (overridden when `--live` probes `vm_stat`)
        #[arg(long, default_value_t = 6144)]
        available_mb: u32,
        /// Whether a heavy build or agent worktree verification is active
        #[arg(long, default_value_t = false)]
        ci_running: bool,
        /// Whether a local LLM is currently loaded in RAM
        #[arg(long, default_value_t = true)]
        llm_loaded: bool,
        /// Seconds since the last local LLM inference request
        #[arg(long, default_value_t = 30)]
        llm_idle_secs: u32,
        /// Probe live `vm_stat`, thermal level, and `otto-*` `tmux` sessions locally or over SSH
        #[arg(long)]
        live: bool,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Manage recurring CI pipelines and scheduled autonomous agent runs (`launchd`) on the M1 MacBook
    Schedule {
        #[command(subcommand)]
        action: ScheduleCommands,
    },
    /// Inspect pass/fail execution history (`history.jsonl`) for scheduled CI and agent runs on the MacBook
    Runs {
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional schedule ID filter (e.g. `main-ci`)
        #[arg(long)]
        id: Option<String>,
        /// Maximum number of recent runs to display
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Optional path to state directory containing `peer.json`
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Query live `history.jsonl` from the paired MacBook over SSH
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Restore original macOS sleep and power behavior from `state-backup.json`
    Restore {
        /// Optional path to state backup directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Also unload and remove all `com.cheapotto.*` `LaunchAgents` on the MacBook
        #[arg(long)]
        unload_schedules: bool,
        /// Execute the `sudo pmset` restore commands immediately
        #[arg(long, visible_alias = "execute")]
        apply: bool,
        /// Execute the restore commands on the paired MacBook over SSH
        #[arg(long)]
        remote: bool,
    },
}

#[derive(Args, Debug)]
struct ScheduleAddArgs {
    /// Unique schedule identifier within the project (e.g. `main-ci` or `nightly-audit`)
    #[arg(long)]
    id: String,
    /// Project folder name (auto-detected from current Git repository when omitted)
    #[arg(long)]
    project: Option<String>,
    /// Fixed interval cadence (`>= 60s`, e.g. `15m`, `2h`, `300s`)
    #[arg(long, group = "cadence")]
    every: Option<String>,
    /// Daily 24-hour clock cadence (`HH:MM`, e.g. `02:30`)
    #[arg(long, group = "cadence")]
    daily: Option<String>,
    /// Weekly cadence (`DAY@HH:MM`, e.g. `MON@02:00` or `FRI@18:00`)
    #[arg(long, group = "cadence")]
    weekly: Option<String>,
    /// CI command to run in an isolated worktree (mutually exclusive with `--task`)
    #[arg(long, group = "workload")]
    ci: Option<String>,
    /// Autonomous coding agent prompt to run in an isolated worktree (mutually exclusive with `--ci`)
    #[arg(long, group = "workload")]
    task: Option<String>,
    /// Coding agent CLI for `--task` (`claude`, `agy`, `kiro`, or `custom`)
    #[arg(long, default_value = "claude")]
    agent: String,
    /// Optional model override for `--task` (e.g. `sonnet`, `gemini-4-carbon-high`)
    #[arg(long)]
    model: Option<String>,
    /// Optional reasoning effort for `--task` (`low`, `medium`, `high`, `xhigh`, `max`)
    #[arg(long)]
    effort: Option<String>,
    /// Verification gate command executed after `--task` finishes
    #[arg(long, default_value = "cargo test --all-targets")]
    verify: String,
    /// Target Git branch to fetch/checkout on each scheduled tick
    #[arg(long, default_value = "main")]
    branch: String,
    /// For `--ci`: skip execution when the target branch SHA has not changed since the last run
    #[arg(long)]
    on_change: bool,
    /// For `--task`: push `otto/sched-<id>` to `origin` and open a Draft PR via `gh` when changes pass verification
    #[arg(long)]
    draft_pr: bool,
    /// Allow scheduled runs even when the MacBook is unplugged on battery power
    #[arg(long)]
    allow_battery: bool,
    /// Trigger an immediate initial run as soon as the LaunchAgent is loaded (`RunAtLoad`)
    #[arg(long)]
    run_at_load: bool,
    /// Optional path to state directory
    #[arg(long)]
    state_dir: Option<PathBuf>,
    /// Install the runner script & `.plist` and bootstrap `launchd` over SSH immediately
    #[arg(long, visible_alias = "apply")]
    execute: bool,
}

#[derive(Subcommand, Debug)]
enum ScheduleCommands {
    /// Register a recurring CI check or autonomous agent task via macOS `launchd`
    Add(Box<ScheduleAddArgs>),
    /// List configured recurring jobs in `.cheap-otto/schedules.json` (and live `launchctl` status with `--execute`)
    List {
        /// Optional project filter
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Query live `launchctl list` status on the paired MacBook over SSH
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Trigger an installed `launchd` job immediately (`launchctl kickstart -k`)
    RunNow {
        /// Schedule identifier to trigger
        #[arg(long)]
        id: String,
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Execute `launchctl kickstart` over SSH immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Unload and delete a recurring `launchd` job from the MacBook and `.cheap-otto/schedules.json`
    Remove {
        /// Schedule identifier to remove
        #[arg(long)]
        id: String,
        /// Project folder name (auto-detected from current Git repository when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Unload and delete remote `.plist` and script over SSH immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
}

#[derive(Subcommand, Debug)]
enum TemplateCommands {
    /// Save or update a repeatable multi-harness task playbook in `.cheap-otto/templates.json`
    Add {
        /// Unique template slug (e.g. `audit-edge-cases` or `pr-lint-fix`)
        #[arg(long)]
        id: String,
        /// Optional human-readable card title
        #[arg(long)]
        title: Option<String>,
        /// Optional bound project name (omit to keep the template project-agnostic)
        #[arg(long)]
        project: Option<String>,
        /// Default coding harness (`claude`, `agy`, `kiro`, or `custom`)
        #[arg(long, default_value = "claude")]
        agent: String,
        /// Optional default model (`sonnet`, `opus`, `gemini-4-carbon-high`, etc.)
        #[arg(long)]
        model: Option<String>,
        /// Optional default reasoning effort (`low`, `medium`, `high`, `xhigh`, `max`)
        #[arg(long)]
        effort: Option<String>,
        /// Task prompt executed when this template runs
        #[arg(long)]
        task: String,
        /// Verification gate command executed after the agent finishes
        #[arg(long, default_value = "cargo test --all-targets")]
        verify: String,
        /// Whether scheduled runs of this template should open a Draft PR
        #[arg(long)]
        draft_pr: bool,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// List repeatable task playbooks stored in `.cheap-otto/templates.json`
    List {
        /// Optional project filter
        #[arg(long)]
        project: Option<String>,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// Launch an isolated worktree agent from a saved template (with optional harness/model overrides)
    Run {
        /// Template ID to run
        #[arg(long)]
        id: String,
        /// Project folder name (auto-detected when omitted if not pinned on the template)
        #[arg(long)]
        project: Option<String>,
        /// Optional explicit agent task ID override
        #[arg(long)]
        task_id: Option<String>,
        /// Optional harness override (`claude`, `agy`, `kiro`, `custom`)
        #[arg(long)]
        agent: Option<String>,
        /// Optional model override
        #[arg(long)]
        model: Option<String>,
        /// Optional reasoning effort override
        #[arg(long)]
        effort: Option<String>,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Emit machine-readable JSON output
        #[arg(long)]
        json: bool,
        /// Execute the worktree creation and background `tmux` launch over SSH immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Bind a saved template directly to a recurring macOS `launchd` schedule
    Schedule {
        /// Template ID to bind
        #[arg(long)]
        id: String,
        /// Optional schedule ID override (defaults to the template ID)
        #[arg(long)]
        schedule_id: Option<String>,
        /// Project folder name (auto-detected when omitted)
        #[arg(long)]
        project: Option<String>,
        /// Fixed interval cadence (`>= 60s`, e.g. `15m`, `2h`)
        #[arg(long, group = "tpl_cadence")]
        every: Option<String>,
        /// Daily cadence (`HH:MM`, e.g. `02:30`)
        #[arg(long, group = "tpl_cadence")]
        daily: Option<String>,
        /// Weekly cadence (`DAY@HH:MM`, e.g. `MON@02:00`)
        #[arg(long, group = "tpl_cadence")]
        weekly: Option<String>,
        /// Target Git branch (`main` default)
        #[arg(long, default_value = "main")]
        branch: String,
        /// Allow scheduled runs on battery power
        #[arg(long)]
        allow_battery: bool,
        /// Trigger an immediate initial run (`RunAtLoad`)
        #[arg(long)]
        run_at_load: bool,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Install & bootstrap on the paired MacBook over SSH immediately
        #[arg(long, visible_alias = "apply")]
        execute: bool,
    },
    /// Remove a saved template from `.cheap-otto/templates.json`
    Remove {
        /// Template ID to delete
        #[arg(long)]
        id: String,
        /// Optional path to state directory
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
}

struct InitOptions<'a> {
    ram_gb: Option<u32>,
    battery_cap: u8,
    out_dir: &'a Path,
    apply: bool,
    remote: bool,
    offline: bool,
}

struct PairOptions<'a> {
    alias: &'a str,
    host: &'a str,
    user: &'a str,
    port: u16,
    ports: &'a [u16],
    out_dir: &'a Path,
    apply: bool,
    tunnel: bool,
}

struct DelegateOptions<'a> {
    project: Option<&'a str>,
    task: &'a str,
    id: Option<&'a str>,
    agent: &'a str,
    model: Option<&'a str>,
    effort: Option<&'a str>,
    verify: &'a str,
    state_dir: Option<&'a Path>,
    json: bool,
    execute: bool,
}

struct InspectOptions<'a> {
    id: &'a str,
    project: Option<&'a str>,
    state_dir: Option<&'a Path>,
    json: bool,
    execute: bool,
}

struct AskOptions<'a> {
    id: &'a str,
    prompt: &'a str,
    project: Option<&'a str>,
    agent: Option<&'a str>,
    model: Option<&'a str>,
    effort: Option<&'a str>,
    verify: Option<&'a str>,
    run_async: bool,
    state_dir: Option<&'a Path>,
    json: bool,
    execute: bool,
}

struct TemplateAddOptions<'a> {
    id: &'a str,
    title: Option<&'a str>,
    project: Option<&'a str>,
    agent: &'a str,
    model: Option<&'a str>,
    effort: Option<&'a str>,
    task: &'a str,
    verify: &'a str,
    draft_pr: bool,
    state_dir: Option<&'a Path>,
    json: bool,
}

struct TemplateRunOptions<'a> {
    id: &'a str,
    project: Option<&'a str>,
    task_id: Option<&'a str>,
    agent: Option<&'a str>,
    model: Option<&'a str>,
    effort: Option<&'a str>,
    state_dir: Option<&'a Path>,
    json: bool,
    execute: bool,
}

struct PullOptions<'a> {
    project: Option<&'a str>,
    id: &'a str,
    state_dir: Option<&'a Path>,
    execute: bool,
    merge: bool,
    cleanup: bool,
}

struct ShellOptions<'a> {
    project: Option<&'a str>,
    session: &'a str,
    plain: bool,
    home: bool,
    state_dir: Option<&'a Path>,
    execute: bool,
}

struct LogsOptions<'a> {
    id: &'a str,
    project: Option<&'a str>,
    lines: u32,
    follow: bool,
    state_dir: Option<&'a Path>,
    execute: bool,
}

struct ScheduleAddOptions<'a> {
    id: &'a str,
    project: Option<&'a str>,
    every: Option<&'a str>,
    daily: Option<&'a str>,
    weekly: Option<&'a str>,
    ci: Option<&'a str>,
    task: Option<&'a str>,
    agent: &'a str,
    model: Option<&'a str>,
    effort: Option<&'a str>,
    verify: &'a str,
    branch: &'a str,
    on_change: bool,
    draft_pr: bool,
    allow_battery: bool,
    run_at_load: bool,
    state_dir: Option<&'a Path>,
    execute: bool,
}

struct RunsOptions<'a> {
    project: Option<&'a str>,
    id: Option<&'a str>,
    limit: usize,
    state_dir: Option<&'a Path>,
    execute: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            ram_gb,
            battery_cap,
            out_dir,
            apply,
            remote,
            offline,
        } => run_init(&InitOptions {
            ram_gb,
            battery_cap,
            out_dir: &out_dir,
            apply,
            remote,
            offline,
        })?,
        Commands::Pair {
            host,
            user,
            alias,
            port,
            ports,
            out_dir,
            apply,
            tunnel,
        } => run_pair(&PairOptions {
            alias: &alias,
            host: &host,
            user: &user,
            port,
            ports: &ports,
            out_dir: &out_dir,
            apply,
            tunnel,
        })?,
        Commands::Sync {
            project,
            state_dir,
            force,
            execute,
        } => run_sync(project.as_deref(), state_dir.as_deref(), force, execute)?,
        Commands::Shell {
            project,
            session,
            plain,
            home,
            state_dir,
            execute,
        } => run_shell(&ShellOptions {
            project: project.as_deref(),
            session: &session,
            plain,
            home,
            state_dir: state_dir.as_deref(),
            execute,
        })?,
        Commands::Attach {
            id,
            read_only,
            state_dir,
            execute,
        } => run_attach(&id, read_only, state_dir.as_deref(), execute)?,
        Commands::Logs {
            id,
            project,
            lines,
            follow,
            state_dir,
            execute,
        } => run_logs(&LogsOptions {
            id: &id,
            project: project.as_deref(),
            lines,
            follow,
            state_dir: state_dir.as_deref(),
            execute,
        })?,
        Commands::Exec {
            cmd,
            project,
            state_dir,
            execute,
        } => run_exec(project.as_deref(), &cmd, state_dir.as_deref(), execute)?,
        Commands::Delegate {
            task,
            id,
            agent,
            model,
            effort,
            verify,
            project,
            state_dir,
            json,
            execute,
        } => run_delegate(&DelegateOptions {
            project: project.as_deref(),
            task: &task,
            id: id.as_deref(),
            agent: &agent,
            model: model.as_deref(),
            effort: effort.as_deref(),
            verify: &verify,
            state_dir: state_dir.as_deref(),
            json,
            execute,
        })?,
        Commands::Harnesses {
            remote,
            json,
            state_dir,
        } => run_harnesses(remote, json, state_dir.as_deref())?,
        Commands::Inspect {
            id,
            project,
            state_dir,
            json,
            execute,
        } => run_inspect(&InspectOptions {
            id: &id,
            project: project.as_deref(),
            state_dir: state_dir.as_deref(),
            json,
            execute,
        })?,
        Commands::Ask {
            id,
            prompt,
            project,
            agent,
            model,
            effort,
            verify,
            run_async,
            state_dir,
            json,
            execute,
        } => run_ask(&AskOptions {
            id: &id,
            prompt: &prompt,
            project: project.as_deref(),
            agent: agent.as_deref(),
            model: model.as_deref(),
            effort: effort.as_deref(),
            verify: verify.as_deref(),
            run_async,
            state_dir: state_dir.as_deref(),
            json,
            execute,
        })?,
        Commands::Template { action } => match action {
            TemplateCommands::Add {
                id,
                title,
                project,
                agent,
                model,
                effort,
                task,
                verify,
                draft_pr,
                state_dir,
                json,
            } => run_template_add(&TemplateAddOptions {
                id: &id,
                title: title.as_deref(),
                project: project.as_deref(),
                agent: &agent,
                model: model.as_deref(),
                effort: effort.as_deref(),
                task: &task,
                verify: &verify,
                draft_pr,
                state_dir: state_dir.as_deref(),
                json,
            })?,
            TemplateCommands::List {
                project,
                state_dir,
                json,
            } => run_template_list(project.as_deref(), state_dir.as_deref(), json)?,
            TemplateCommands::Run {
                id,
                project,
                task_id,
                agent,
                model,
                effort,
                state_dir,
                json,
                execute,
            } => run_template_run(&TemplateRunOptions {
                id: &id,
                project: project.as_deref(),
                task_id: task_id.as_deref(),
                agent: agent.as_deref(),
                model: model.as_deref(),
                effort: effort.as_deref(),
                state_dir: state_dir.as_deref(),
                json,
                execute,
            })?,
            TemplateCommands::Schedule {
                id,
                schedule_id,
                project,
                every,
                daily,
                weekly,
                branch,
                allow_battery,
                run_at_load,
                state_dir,
                execute,
            } => run_template_schedule(
                &id,
                schedule_id.as_deref(),
                &ScheduleAddOptions {
                    id: schedule_id.as_deref().unwrap_or(&id),
                    project: project.as_deref(),
                    every: every.as_deref(),
                    daily: daily.as_deref(),
                    weekly: weekly.as_deref(),
                    ci: None,
                    task: None,
                    agent: "claude",
                    model: None,
                    effort: None,
                    verify: "",
                    branch: &branch,
                    on_change: false,
                    draft_pr: false,
                    allow_battery,
                    run_at_load,
                    state_dir: state_dir.as_deref(),
                    execute,
                },
            )?,
            TemplateCommands::Remove { id, state_dir } => {
                run_template_remove(&id, state_dir.as_deref())?
            }
        },
        Commands::Serve {
            host,
            port,
            pixel_port,
            state_dir,
            workspaces_root,
        } => run_serve_cmd(
            &host,
            port,
            pixel_port,
            state_dir.as_deref(),
            workspaces_root.as_deref(),
        )?,
        Commands::Ui {
            port,
            pixel_port,
            state_dir,
            no_open,
            execute,
        } => run_ui(port, pixel_port, state_dir.as_deref(), no_open, execute)?,
        Commands::Jobs {
            project,
            state_dir,
            execute,
        } => run_jobs(project.as_deref(), state_dir.as_deref(), execute)?,
        Commands::Pull {
            id,
            project,
            state_dir,
            execute,
            merge,
            cleanup,
        } => run_pull(&PullOptions {
            project: project.as_deref(),
            id: &id,
            state_dir: state_dir.as_deref(),
            execute,
            merge,
            cleanup,
        })?,
        Commands::Projects { state_dir } => run_projects(state_dir.as_deref())?,
        Commands::Doctor {
            state_dir,
            remote,
            offline,
        } => run_doctor(state_dir.as_deref(), remote, offline)?,
        Commands::Arbiter {
            available_mb,
            ci_running,
            llm_loaded,
            llm_idle_secs,
            live,
            state_dir,
        } => run_arbiter(
            available_mb,
            ci_running,
            llm_loaded,
            llm_idle_secs,
            live,
            state_dir.as_deref(),
        )?,
        Commands::Schedule { action } => match action {
            ScheduleCommands::Add(args) => run_schedule_add(&ScheduleAddOptions {
                id: &args.id,
                project: args.project.as_deref(),
                every: args.every.as_deref(),
                daily: args.daily.as_deref(),
                weekly: args.weekly.as_deref(),
                ci: args.ci.as_deref(),
                task: args.task.as_deref(),
                agent: &args.agent,
                model: args.model.as_deref(),
                effort: args.effort.as_deref(),
                verify: &args.verify,
                branch: &args.branch,
                on_change: args.on_change,
                draft_pr: args.draft_pr,
                allow_battery: args.allow_battery,
                run_at_load: args.run_at_load,
                state_dir: args.state_dir.as_deref(),
                execute: args.execute,
            })?,
            ScheduleCommands::List {
                project,
                state_dir,
                execute,
            } => run_schedule_list(project.as_deref(), state_dir.as_deref(), execute)?,
            ScheduleCommands::RunNow {
                id,
                project,
                state_dir,
                execute,
            } => run_schedule_run_now(&id, project.as_deref(), state_dir.as_deref(), execute)?,
            ScheduleCommands::Remove {
                id,
                project,
                state_dir,
                execute,
            } => run_schedule_remove(&id, project.as_deref(), state_dir.as_deref(), execute)?,
        },
        Commands::Runs {
            project,
            id,
            limit,
            state_dir,
            execute,
        } => run_runs(&RunsOptions {
            project: project.as_deref(),
            id: id.as_deref(),
            limit,
            state_dir: state_dir.as_deref(),
            execute,
        })?,
        Commands::Restore {
            state_dir,
            unload_schedules,
            apply,
            remote,
        } => run_restore(state_dir.as_deref(), unload_schedules, apply, remote)?,
    }

    Ok(())
}

/// Runs a child process without a shell and returns `stdout` when exit status is `0`.
fn capture_command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

/// Runs a [`CommandSpec`] and captures its `stdout` when exit status is `0`.
fn capture_spec_stdout(spec: &CommandSpec) -> Option<String> {
    let output = Command::new(&spec.program).args(&spec.args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

/// Spawns a [`CommandSpec`] directly via `Command::args` with inherited terminal stdio.
fn execute_command_spec(spec: &CommandSpec) -> Result<(), Box<dyn std::error::Error>> {
    println!("  ==> {} ($ {})", spec.description, spec.display_command());
    let status = Command::new(&spec.program)
        .args(&spec.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| format!("failed to spawn '{}': {err}", spec.program))?;

    if !status.success() {
        if spec.allow_failure {
            println!("      [info] non-zero exit ({status}) tolerated for idempotent step");
        } else {
            return Err(
                format!("command '{}' exited with {status}", spec.display_command()).into(),
            );
        }
    }
    Ok(())
}

fn user_home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn current_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1_730_000_000, |d| d.as_secs())
}

fn discover_workspace(
    explicit_project: Option<&str>,
) -> Result<DetectedWorkspace, Box<dyn std::error::Error>> {
    let git_toplevel = capture_command_stdout("git", &["rev-parse", "--show-toplevel"]);
    let git_branch = capture_command_stdout("git", &["symbolic-ref", "--short", "-q", "HEAD"])
        .or_else(|| capture_command_stdout("git", &["rev-parse", "--abbrev-ref", "HEAD"]));
    let cwd = env::current_dir().ok();
    Ok(detect_workspace_from_probes(
        explicit_project,
        git_toplevel.as_deref(),
        git_branch.as_deref(),
        cwd.as_deref(),
    )?)
}

fn resolve_active_state_dir(explicit_override: Option<&Path>, repo_root: Option<&str>) -> PathBuf {
    let cwd_has_peer = Path::new(".cheap-otto").join("peer.json").exists();

    let repo_state_buf = repo_root.map(|root| Path::new(root).join(".cheap-otto"));
    let repo_state = repo_state_buf
        .as_deref()
        .map(|dir| (dir, dir.join("peer.json").exists()));

    let home_state_buf = user_home_dir().map(|home| home.join(".cheap-otto"));
    let home_state = home_state_buf
        .as_deref()
        .map(|dir| (dir, dir.join("peer.json").exists()));

    select_state_dir(explicit_override, cwd_has_peer, repo_state, home_state)
}

fn load_or_default_peer(state_dir: &Path) -> Result<(PeerNode, bool), Box<dyn std::error::Error>> {
    let peer_path = state_dir.join("peer.json");
    if peer_path.exists() {
        let raw = fs::read_to_string(&peer_path)?;
        let peer: PeerNode = serde_json::from_str(&raw)?;
        Ok((peer, true))
    } else {
        Ok((
            PeerNode::new("otto-mac", "100.101.102.103", "admin", 22)?,
            false,
        ))
    }
}

fn load_registry(state_dir: &Path) -> WorkspaceRegistry {
    let path = state_dir.join("projects.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<WorkspaceRegistry>(&raw).ok())
        .unwrap_or_default()
}

fn save_registry(
    state_dir: &Path,
    registry: &WorkspaceRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(state_dir)?;
    let path = state_dir.join("projects.json");
    fs::write(path, serde_json::to_string_pretty(registry)?)?;
    Ok(())
}

fn load_schedule_registry(state_dir: &Path) -> ScheduleRegistry {
    let path = state_dir.join("schedules.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<ScheduleRegistry>(&raw).ok())
        .unwrap_or_default()
}

fn save_schedule_registry(
    state_dir: &Path,
    registry: &ScheduleRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(state_dir)?;
    let path = state_dir.join("schedules.json");
    fs::write(path, serde_json::to_string_pretty(registry)?)?;
    Ok(())
}

fn load_agent_registry(state_dir: &Path) -> AgentSessionRegistry {
    let path = state_dir.join("agents.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<AgentSessionRegistry>(&raw).ok())
        .unwrap_or_default()
}

fn save_agent_registry(
    state_dir: &Path,
    registry: &AgentSessionRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(state_dir)?;
    let path = state_dir.join("agents.json");
    fs::write(path, serde_json::to_string_pretty(registry)?)?;
    Ok(())
}

fn load_template_registry(state_dir: &Path) -> TemplateRegistry {
    let path = state_dir.join("templates.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<TemplateRegistry>(&raw).ok())
        .unwrap_or_default()
}

fn save_template_registry(
    state_dir: &Path,
    registry: &TemplateRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(state_dir)?;
    let path = state_dir.join("templates.json");
    fs::write(path, serde_json::to_string_pretty(registry)?)?;
    Ok(())
}

/// Mirrors an [`AgentSessionRecord`] into `~/.cheap-otto/agents.json` on the paired MacBook so
/// `cheap-otto serve` and `pixel-agents` immediately track CLI-delegated agents.
fn sync_remote_agent_record(peer: &PeerNode, record: &AgentSessionRecord) {
    let raw_existing = capture_spec_stdout(&peer.build_ssh_probe_spec(
        "Read remote ~/.cheap-otto/agents.json",
        "cat ~/.cheap-otto/agents.json 2>/dev/null",
    ))
    .unwrap_or_default();
    let mut remote_reg =
        serde_json::from_str::<AgentSessionRegistry>(&raw_existing).unwrap_or_default();
    remote_reg.upsert(record.clone());
    if let Ok(serialized) = serde_json::to_string_pretty(&remote_reg) {
        let write_cmd = format!(
            "mkdir -p ~/.cheap-otto && printf '%s\\n' {} > ~/.cheap-otto/agents.json",
            posix_single_quote(&serialized)
        );
        let _ = capture_spec_stdout(
            &peer.build_ssh_probe_spec("Sync remote ~/.cheap-otto/agents.json", &write_cmd),
        );
    }
}

fn probe_hardware_and_pmset(
    peer: Option<&PeerNode>,
    ram_override: Option<u32>,
    offline: bool,
) -> (HardwareProfile, Option<String>, &'static str) {
    if !offline {
        if let Some(remote_peer) = peer {
            let sysctl_spec = remote_peer.build_ssh_probe_spec(
                "Probe remote MacBook sysctl hardware profile",
                "sysctl hw.memsize machdep.cpu.brand_string hw.model",
            );
            let pmset_spec = remote_peer.build_ssh_probe_spec(
                "Probe remote MacBook pmset configuration",
                "pmset -g custom && pmset -g",
            );
            let batt_spec = remote_peer.build_ssh_probe_spec(
                "Probe remote MacBook battery health",
                "ioreg -rn AppleSmartBattery | grep MaximumCapacityPercent",
            );

            if let Some(sysctl_out) = capture_spec_stdout(&sysctl_spec)
                && let Ok(mut profile) = HardwareProfile::from_macos_sysctl(&sysctl_out)
            {
                if let Some(explicit_ram) = ram_override {
                    profile.memory_gb = explicit_ram;
                }
                profile.battery_health_percent =
                    capture_spec_stdout(&batt_spec).and_then(|s| parse_battery_health_percent(&s));
                let pmset_out = capture_spec_stdout(&pmset_spec);
                return (profile, pmset_out, "Remote Paired MacBook (SSH)");
            }
        } else if cfg!(target_os = "macos")
            && let Some(sysctl_out) = capture_command_stdout(
                "sysctl",
                &["hw.memsize", "machdep.cpu.brand_string", "hw.model"],
            )
            && let Ok(mut profile) = HardwareProfile::from_macos_sysctl(&sysctl_out)
        {
            if let Some(explicit_ram) = ram_override {
                profile.memory_gb = explicit_ram;
            }
            profile.battery_health_percent =
                capture_command_stdout("ioreg", &["-rn", "AppleSmartBattery"])
                    .and_then(|s| parse_battery_health_percent(&s));
            let pmset_out = match (
                capture_command_stdout("pmset", &["-g", "custom"]),
                capture_command_stdout("pmset", &["-g"]),
            ) {
                (Some(custom), Some(global)) => Some(format!("{custom}\n{global}")),
                (Some(custom), None) => Some(custom),
                (None, other) => other,
            };
            return (profile, pmset_out, "Live Local macOS");
        }
    }

    let fallback = HardwareProfile {
        model_name: "MacBookAir10,1 (Apple M1)".to_string(),
        arch: CpuArch::AppleSilicon,
        memory_gb: ram_override.unwrap_or(16),
        has_battery: true,
        battery_health_percent: Some(88),
        fanless: true,
    };
    (fallback, None, "Baseline Appliance Profile")
}

fn run_init(opts: &InitOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let remote_peer = if opts.remote {
        let (peer, _) = load_or_default_peer(opts.out_dir)?;
        Some(peer)
    } else {
        None
    };

    let (profile, live_pmset, source_label) =
        probe_hardware_and_pmset(remote_peer.as_ref(), opts.ram_gb, opts.offline);
    let budget = profile.compute_memory_budget()?;

    let default_pmset = "\
AC Power:
 sleep                1
 disablesleep         0
 displaysleep         10
 autorestart          0
 womp                 0
 tcpkeepalive         1
";

    fs::create_dir_all(opts.out_dir)?;
    let backup_path = opts.out_dir.join("state-backup.json");

    // Preserve original factory pmset snapshot if state-backup.json already exists on re-runs.
    let base_backup = if backup_path.exists() {
        let existing_raw = fs::read_to_string(&backup_path)?;
        serde_json::from_str::<MacOsStateBackup>(&existing_raw)?
    } else {
        let pmset_source = live_pmset.as_deref().unwrap_or(default_pmset);
        MacOsStateBackup::from_pmset_custom(pmset_source, current_epoch_secs())?
    };

    let backup = base_backup.with_configured_ceiling(opts.battery_cap)?;
    let power_plan = backup.build_clamshell_plan(opts.battery_cap)?;

    // Safety invariant: always persist state-backup.json BEFORE executing any pmset mutation.
    fs::write(&backup_path, serde_json::to_string_pretty(&backup)?)?;

    println!("=== cheap-otto: Headless MacBook Appliance Setup ===");
    println!("Probe Source    : {source_label}");
    println!(
        "Target Hardware : {} ({} GB Unified RAM, CI/Agent Budget: {} MB)",
        profile.model_name, profile.memory_gb, budget.ci_runner_mb
    );
    println!("\n--- Reversible Clamshell & Battery Guardrails ---");
    for line in &power_plan.human_readable_diff {
        println!("  * {line}");
    }
    println!("\nCommands to apply on macOS:");
    for cmd in &power_plan.pmset_apply_commands {
        println!("  $ {cmd}");
    }
    println!("\nSaved state backup to: {}", backup_path.display());

    if opts.apply {
        println!("\n--- Applying Clamshell Power Plan ---");
        if let Some(peer) = remote_peer {
            let port_str = peer.ssh_port.to_string();
            let dest = peer.ssh_destination();
            for cmd in &power_plan.pmset_apply_commands {
                let remote_spec = CommandSpec::new(
                    "Apply pmset guardrail on paired MacBook over SSH",
                    "ssh",
                    ["-t", "-p", &port_str, &dest, cmd],
                );
                execute_command_spec(&remote_spec)?;
            }
        } else if cfg!(target_os = "macos") {
            for spec in &power_plan.apply_specs {
                execute_command_spec(spec)?;
            }
        } else {
            println!(
                "  [info] Local host is not macOS; pass `--remote --apply` to apply over SSH to your paired MacBook."
            );
        }
    }

    Ok(())
}

fn run_pair(opts: &PairOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let peer = PeerNode::new(opts.alias, opts.host, opts.user, opts.port)?;
    let tunnel = peer.build_port_tunnel(opts.ports)?;

    fs::create_dir_all(opts.out_dir)?;
    let peer_path = opts.out_dir.join("peer.json");
    fs::write(&peer_path, serde_json::to_string_pretty(&peer)?)?;

    let ssh_config_path = opts.out_dir.join("ssh-config.conf");
    fs::write(&ssh_config_path, &tunnel.ssh_config_snippet)?;

    println!("=== cheap-otto: Cross-OS Dev Bridge Paired (Windows <-> MacBook) ===");
    println!(
        "Paired MacBook  : {} ({} via {:?})",
        peer.alias,
        peer.ssh_destination(),
        peer.transport
    );
    println!("Workspace Root  : {}", peer.remote_workspace_root);
    println!("\n--- Localhost Port Forwarding Tunnel ---");
    println!("  $ {}", tunnel.ssh_tunnel_command);
    println!("\n--- Generated ~/.ssh/config Snippet ---");
    print!("{}", tunnel.ssh_config_snippet);
    println!("\nSaved peer profile to : {}", peer_path.display());
    println!("Saved SSH config to   : {}", ssh_config_path.display());

    if opts.apply {
        println!("\n--- Applying SSH Configuration & Connectivity Check ---");
        if let Some(home) = user_home_dir() {
            let ssh_dir = home.join(".ssh");
            fs::create_dir_all(&ssh_dir)?;
            let user_ssh_cfg = ssh_dir.join("config");
            let existing = fs::read_to_string(&user_ssh_cfg).unwrap_or_default();
            let merged =
                merge_ssh_config_block(&existing, &peer.alias, &tunnel.ssh_config_snippet)?;
            fs::write(&user_ssh_cfg, merged)?;
            println!("  Updated SSH config  : {}", user_ssh_cfg.display());
        }

        let verify_spec =
            peer.build_ssh_probe_spec("Verify SSH reachability to paired MacBook", "uname -mrs");
        execute_command_spec(&verify_spec)?;
    }

    if opts.tunnel
        && let Some(tunnel_spec) = &tunnel.tunnel_spec
    {
        println!("\n--- Starting Port Forwarding Tunnel (Ctrl+C to stop) ---");
        execute_command_spec(tunnel_spec)?;
    }

    Ok(())
}

fn run_sync(
    explicit_project: Option<&str>,
    explicit_state_dir: Option<&Path>,
    force: bool,
    execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(explicit_project)?;
    let state_dir = resolve_active_state_dir(explicit_state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let plan =
        peer.build_sync_plan_with_options(&ws.project_name, Some(&ws.current_branch), force)?;

    println!("=== cheap-otto: Cross-OS Workspace Sync (Windows -> MacBook) ===");
    println!(
        "Project         : {} (branch: {})",
        plan.project_name, ws.current_branch
    );
    println!("State Config    : {}", state_dir.display());
    println!("Remote Path     : {}", plan.remote_project_dir);
    println!("Git Remote URL  : {}", plan.git_remote_url);
    println!(
        "Manual Guard    : {}",
        if plan.force_overwrite {
            "disabled (--force will overwrite uncommitted remote edits)"
        } else {
            "enabled (protects uncommitted manual edits on MacBook)"
        }
    );
    println!("Excluded Paths  : {}", plan.excluded_paths.join(", "));
    println!("\nSync Commands:");
    for step in &plan.sync_commands {
        println!("  $ {step}");
    }

    let mut registry = load_registry(&state_dir);
    registry.record_sync(
        &plan.project_name,
        ws.repo_root.as_deref(),
        &plan.remote_project_dir,
        Some(&ws.current_branch),
    );

    if execute {
        println!("\n--- Executing Live Workspace Sync ---");
        for spec in &plan.sync_specs {
            execute_command_spec(spec)?;
        }
        save_registry(&state_dir, &registry)?;
        println!(
            "Synced '{}' to {}.",
            plan.project_name,
            peer.ssh_destination()
        );
    } else if state_dir.exists() {
        save_registry(&state_dir, &registry)?;
    }

    Ok(())
}

fn run_shell(opts: &ShellOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let discovered = discover_workspace(opts.project).ok();
    let repo_root = discovered.as_ref().and_then(|w| w.repo_root.as_deref());
    let state_dir = resolve_active_state_dir(opts.state_dir, repo_root);
    let (peer, _) = load_or_default_peer(&state_dir)?;

    let target = if opts.home {
        ShellTarget::HomeDirectory
    } else if let Some(explicit) = opts.project {
        ShellTarget::ProjectWorkspace(explicit.to_string())
    } else if let Some(ws) = &discovered {
        ShellTarget::ProjectWorkspace(ws.project_name.clone())
    } else {
        ShellTarget::HomeDirectory
    };

    let mode = if opts.plain {
        ShellMode::PlainLogin
    } else {
        ShellMode::PersistentTmux {
            session_name: opts.session.to_string(),
        }
    };

    let plan = build_interactive_shell_plan(&peer, &target, mode)?;

    println!("=== cheap-otto: Interactive M1 Terminal Session ===");
    println!("Target MacBook  : {}", peer.ssh_destination());
    println!("Remote Dir      : {}", plan.remote_dir);
    match &plan.mode {
        ShellMode::PersistentTmux { session_name } => println!(
            "Session Mode    : Persistent tmux (`{session_name}` — isolated from automated `otto-*` jobs)"
        ),
        ShellMode::PlainLogin => println!("Session Mode    : Direct `zsh -l` login shell"),
    }
    println!("SSH Command     :");
    println!("  $ {}", plan.display_command);

    if opts.execute {
        println!("\n--- Connecting to Paired MacBook Terminal ---");
        execute_command_spec(&plan.ssh_spec)?;
    } else {
        println!("\nPass `--execute` to open the live interactive terminal.");
    }

    Ok(())
}

fn run_attach(
    id: &str,
    read_only: bool,
    explicit_state_dir: Option<&Path>,
    execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(None).ok().and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let plan = build_attach_session_plan(&peer, id, read_only)?;

    println!("=== cheap-otto: Live tmux Job Visibility ===");
    println!("Target MacBook  : {}", peer.ssh_destination());
    println!(
        "Session Lookup  : {} (fallback: {})",
        plan.tmux_session_name, plan.scheduled_tmux_session_name
    );
    println!(
        "Attach Mode     : {}",
        if plan.read_only {
            "Read-Only (`-r` — safe observation)"
        } else {
            "Interactive (press Ctrl+B then D to detach cleanly)"
        }
    );
    println!("SSH Command     :");
    println!("  $ {}", plan.display_command);

    if execute {
        println!("\n--- Attaching to Remote tmux Pane ---");
        execute_command_spec(&plan.ssh_spec)?;
    } else {
        println!("\nPass `--execute` to attach to the live M1 terminal pane.");
    }

    Ok(())
}

fn run_logs(opts: &LogsOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(opts.project)?;
    let state_dir = resolve_active_state_dir(opts.state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let plan = build_job_logs_plan(&peer, &ws.project_name, opts.id, opts.lines, opts.follow)?;

    println!("=== cheap-otto: Persistent Job Logs ===");
    println!("Project         : {}", plan.project_name);
    println!("Job / Task ID   : {}", plan.task_id);
    println!(
        "Remote Log Path : {} (or {})",
        plan.remote_log_file, plan.scheduled_log_file
    );
    println!(
        "Mode            : last {} lines{}",
        plan.lines,
        if plan.follow {
            " (following live via `tail -f`)"
        } else {
            ""
        }
    );
    println!("SSH Command     :");
    println!("  $ {}", plan.display_command);

    if opts.execute {
        println!("\n--- Job Log Output ---");
        execute_command_spec(&plan.ssh_spec)?;
    }

    Ok(())
}

fn run_exec(
    explicit_project: Option<&str>,
    command: &str,
    explicit_state_dir: Option<&Path>,
    execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(explicit_project)?;
    let state_dir = resolve_active_state_dir(explicit_state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let remote_cmd = peer.build_remote_exec(&ws.project_name, command)?;
    let exec_spec = peer.build_remote_exec_spec(&ws.project_name, command)?;

    println!("=== cheap-otto: Cross-OS Remote Execution ===");
    println!("Project         : {}", ws.project_name);
    println!("Target MacBook  : {}", peer.ssh_destination());
    println!("Remote Command  :");
    println!("  $ {remote_cmd}");

    if execute {
        println!("\n--- Streaming Remote Output ---");
        execute_command_spec(&exec_spec)?;
    }

    Ok(())
}

fn run_delegate(opts: &DelegateOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(opts.project)?;
    let state_dir = resolve_active_state_dir(opts.state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let agent = AgentRunner::parse(opts.agent)?;
    let effort = match opts.effort {
        Some(raw) if !raw.trim().is_empty() => Some(ReasoningEffort::parse(raw)?),
        _ => None,
    };
    let assigned_session_id = if agent == AgentRunner::ClaudeCode {
        Some(generate_session_uuid())
    } else {
        None
    };
    let harness_cfg = HarnessRunConfig {
        runner: agent,
        model: opts
            .model
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
        effort,
        session_id: assigned_session_id,
        resume_existing: false,
        stream_json: matches!(agent, AgentRunner::ClaudeCode | AgentRunner::Antigravity),
    };
    let plan = build_delegate_plan_with_harness(
        &peer,
        &ws.project_name,
        opts.task,
        opts.id,
        &harness_cfg,
        opts.verify,
    )?;

    let session_rec = AgentSessionRecord::new_initial(
        &plan.project_name,
        &plan.task_id,
        &plan.remote_worktree_dir,
        &harness_cfg,
        &plan.verify_command,
        opts.task,
        current_epoch_secs(),
    );

    let remote_repo_dir = peer.remote_project_dir(&plan.project_name)?;
    let mut registry = load_registry(&state_dir);
    registry.record_delegated_task(
        &plan.project_name,
        ws.repo_root.as_deref(),
        &remote_repo_dir,
        &plan.task_id,
    );

    let mut agent_reg = load_agent_registry(&state_dir);
    agent_reg.upsert(session_rec.clone());

    if opts.execute {
        if !opts.json {
            println!("=== cheap-otto: Launching Addressable Agent Worktree ===");
        }
        for spec in &plan.execution_specs {
            if opts.json {
                let status = Command::new(&spec.program).args(&spec.args).status()?;
                if !status.success() && !spec.allow_failure {
                    return Err(format!(
                        "command '{}' exited with {status}",
                        spec.display_command()
                    )
                    .into());
                }
            } else {
                execute_command_spec(spec)?;
            }
        }
        save_registry(&state_dir, &registry)?;
        save_agent_registry(&state_dir, &agent_reg)?;
        sync_remote_agent_record(&peer, &session_rec);
    } else {
        save_agent_registry(&state_dir, &agent_reg)?;
        if state_dir.exists() {
            save_registry(&state_dir, &registry)?;
        }
    }

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "agent_id": plan.task_id,
                "project_name": plan.project_name,
                "branch_name": plan.branch_name,
                "remote_worktree_dir": plan.remote_worktree_dir,
                "tmux_session_name": plan.tmux_session_name,
                "harness": plan.agent.slug(),
                "model": plan.model,
                "effort": plan.effort,
                "provider_session_id": session_rec.provider_session_id,
                "executed": opts.execute,
                "execution_steps": plan.execution_steps,
            }))?
        );
        return Ok(());
    }

    println!("=== cheap-otto: Delegate Task to Parallel MacBook Worktree ===");
    println!(
        "Project         : {} (from {})",
        plan.project_name, ws.current_branch
    );
    println!("Agent ID        : {}", plan.task_id);
    println!("Worktree Branch : {}", plan.branch_name);
    println!("Remote Worktree : {}", plan.remote_worktree_dir);
    println!(
        "tmux Session    : {} (remain-on-exit enabled)",
        plan.tmux_session_name
    );
    println!("Remote Log File : {}", plan.remote_log_file);
    println!("Events Stream   : {}", plan.remote_events_file);
    println!(
        "Harness / Model : {} (model={}, effort={})",
        plan.agent.slug(),
        plan.model.as_deref().unwrap_or("default"),
        plan.effort.map_or("default", ReasoningEffort::as_flag_str)
    );
    if let Some(sid) = &session_rec.provider_session_id {
        println!("Session ID      : {sid}");
    }
    println!("Verify Gate     : {}", plan.verify_command);
    println!("\nExecution Steps:");
    for (idx, step) in plan.execution_steps.iter().enumerate() {
        println!("  [{}] $ {step}", idx + 1);
    }
    println!(
        "\nQuery or inspect this agent directly anytime:\n  $ cheap-otto inspect --id {} --execute\n  $ cheap-otto ask --id {} --prompt \"...\" --execute\n  $ cheap-otto attach --id {} --execute\n\nWhen finished, fetch the branch back to Windows with:\n  $ cheap-otto pull --id {} --execute",
        plan.task_id, plan.task_id, plan.task_id, plan.task_id
    );
    Ok(())
}

fn run_jobs(
    explicit_project: Option<&str>,
    explicit_state_dir: Option<&Path>,
    execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(explicit_project)?;
    let state_dir = resolve_active_state_dir(explicit_state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let cmd = build_jobs_status_command(&peer, &ws.project_name)?;
    let spec = build_jobs_status_spec(&peer, &ws.project_name)?;

    println!(
        "=== cheap-otto: Parallel MacBook Agent Worktrees ({}) ===",
        ws.project_name
    );
    println!("  $ {cmd}");

    if execute {
        println!("\n--- Live MacBook Worktree & tmux Status ---");
        execute_command_spec(&spec)?;
    }

    Ok(())
}

fn run_pull(opts: &PullOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(opts.project)?;
    let state_dir = resolve_active_state_dir(opts.state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let plan = build_pull_plan(&peer, &ws.project_name, opts.id)?;

    println!("=== cheap-otto: Pull MacBook Worktree Branch Back to Windows ===");
    println!("Project         : {}", plan.project_name);
    println!("Task ID         : {}", plan.task_id);
    println!("Branch          : {}", plan.branch_name);
    println!("\n1. Fetch branch from MacBook over SSH into your Windows repo:");
    println!("  $ {}", plan.git_fetch_command);
    println!("\n2. Inspect the agent's diff on Windows:");
    println!("  $ {}", plan.inspect_diff_command);
    println!("\n3. Merge into your active Windows branch:");
    println!("  $ {}", plan.merge_command);
    println!("\n4. Clean up the remote MacBook worktree (optional):");
    println!("  $ {}", plan.cleanup_remote_worktree_command);

    if opts.execute {
        println!("\n--- Executing Pull Workflow ---");
        if let Some(fetch_spec) = &plan.fetch_spec {
            execute_command_spec(fetch_spec)?;
        }
        if let Some(diff_spec) = &plan.diff_spec {
            execute_command_spec(diff_spec)?;
        }
        if opts.merge
            && let Some(merge_spec) = &plan.merge_spec
        {
            execute_command_spec(merge_spec)?;
        }
        if opts.cleanup
            && let Some(cleanup_spec) = &plan.cleanup_remote_worktree_spec
        {
            execute_command_spec(cleanup_spec)?;
        }

        let mut registry = load_registry(&state_dir);
        registry.mark_task_pulled(&plan.project_name, &plan.task_id);
        if state_dir.exists() {
            save_registry(&state_dir, &registry)?;
        }
    }

    Ok(())
}

fn run_projects(explicit_state_dir: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(None)?;
    let state_dir = resolve_active_state_dir(explicit_state_dir, ws.repo_root.as_deref());
    let (peer, has_peer_file) = load_or_default_peer(&state_dir)?;
    let registry = load_registry(&state_dir);

    println!("=== cheap-otto: Multi-Project & Config Discovery ===");
    println!("Active Project  : {}", ws.project_name);
    println!("Current Branch  : {}", ws.current_branch);
    println!(
        "Local Git Root  : {}",
        ws.repo_root
            .as_deref()
            .unwrap_or("(outside Git repository)")
    );
    println!("Resolved Config : {}", state_dir.display());
    println!(
        "Paired MacBook  : {} ({}{})",
        peer.alias,
        peer.ssh_destination(),
        if has_peer_file {
            ""
        } else {
            " [default - run `cheap-otto pair`]"
        }
    );

    println!("\n--- Tracked Workspaces ({}) ---", registry.projects.len());
    if registry.projects.is_empty() {
        println!("  (none recorded yet — run `cheap-otto sync` or `cheap-otto delegate`)");
    } else {
        for proj in &registry.projects {
            let branch = proj.last_synced_branch.as_deref().unwrap_or("-");
            let tasks = if proj.delegated_tasks.is_empty() {
                "none".to_string()
            } else {
                proj.delegated_tasks.join(", ")
            };
            println!(
                "  * {:<20} -> {} (branch: {branch}, active tasks: {tasks})",
                proj.project_name, proj.remote_project_dir
            );
        }
    }

    Ok(())
}

fn collect_local_macos_telemetry(configured_ceiling: Option<u8>) -> RawTelemetrySnapshot {
    let pmset_custom = match (
        capture_command_stdout("pmset", &["-g", "custom"]),
        capture_command_stdout("pmset", &["-g"]),
    ) {
        (Some(custom), Some(global)) => Some(format!("{custom}\n{global}")),
        (Some(custom), None) => Some(custom),
        (None, other) => other,
    };
    let pmset_batt = capture_command_stdout("pmset", &["-g", "batt"]);
    let thermal_raw = capture_command_stdout("sysctl", &["-n", "kern.thermalstate"]);
    let tailscale_ip_stdout = capture_command_stdout("tailscale", &["ip", "-4"]);
    let container_runtime_ready = capture_command_stdout("orb", &["status"]).is_some()
        || capture_command_stdout("docker", &["info"]).is_some();
    let df_root_stdout = capture_command_stdout("df", &["-P", "/"]);
    let local_ai_responding = capture_command_stdout("ollama", &["list"]).is_some();
    let active_tmux_sessions_stdout = capture_command_stdout("tmux", &["list-sessions"]);

    RawTelemetrySnapshot {
        pmset_custom,
        pmset_batt,
        configured_battery_ceiling: configured_ceiling,
        thermal_raw,
        tailscale_ip_stdout,
        container_runtime_ready,
        df_root_stdout,
        local_ai_responding,
        active_tmux_sessions_stdout,
    }
}

fn collect_remote_ssh_telemetry(
    peer: &PeerNode,
    configured_ceiling: Option<u8>,
) -> Option<RawTelemetrySnapshot> {
    let ping_spec = peer.build_ssh_probe_spec("Probe MacBook reachability", "pmset -g batt");
    let pmset_batt = capture_spec_stdout(&ping_spec)?;

    let pmset_custom = capture_spec_stdout(&peer.build_ssh_probe_spec(
        "Probe remote pmset custom and system-wide sleep settings",
        "pmset -g custom && pmset -g",
    ));
    let resolved_ceiling = configured_ceiling.or_else(|| {
        let raw_backup = capture_spec_stdout(&peer.build_ssh_probe_spec(
            "Probe remote state-backup.json battery ceiling",
            "cat ~/.cheap-otto/state-backup.json 2>/dev/null",
        ))?;
        let parsed = serde_json::from_str::<MacOsStateBackup>(&raw_backup).ok()?;
        parsed
            .configured_ceiling_percent
            .or(Some(parsed.battery_charge_ceiling_percent))
    });
    let thermal_raw = capture_spec_stdout(
        &peer.build_ssh_probe_spec("Probe remote thermal state", "sysctl -n kern.thermalstate"),
    );
    let tailscale_ip_stdout = capture_spec_stdout(
        &peer.build_ssh_probe_spec("Probe remote Tailscale IPv4", "tailscale ip -4"),
    );
    let container_runtime_ready = capture_spec_stdout(&peer.build_ssh_probe_spec(
        "Probe remote container runtime",
        "orb status >/dev/null 2>&1 || docker info >/dev/null 2>&1",
    ))
    .is_some();
    let df_root_stdout =
        capture_spec_stdout(&peer.build_ssh_probe_spec("Probe remote disk space", "df -P /"));
    let local_ai_responding = capture_spec_stdout(
        &peer.build_ssh_probe_spec("Probe remote Ollama gateway", "ollama list >/dev/null 2>&1"),
    )
    .is_some();
    let active_tmux_sessions_stdout = capture_spec_stdout(
        &peer.build_ssh_probe_spec("Probe remote tmux sessions", "tmux list-sessions"),
    );

    Some(RawTelemetrySnapshot {
        pmset_custom,
        pmset_batt: Some(pmset_batt),
        configured_battery_ceiling: resolved_ceiling,
        thermal_raw,
        tailscale_ip_stdout,
        container_runtime_ready,
        df_root_stdout,
        local_ai_responding,
        active_tmux_sessions_stdout,
    })
}

fn read_configured_ceiling(state_dir: &Path) -> Option<u8> {
    let backup_path = state_dir.join("state-backup.json");
    let raw = fs::read_to_string(backup_path).ok()?;
    let backup = serde_json::from_str::<MacOsStateBackup>(&raw).ok()?;
    backup
        .configured_ceiling_percent
        .or(Some(backup.battery_charge_ceiling_percent))
}

fn baseline_probe(configured_ceiling: Option<u8>) -> SystemProbeInput {
    SystemProbeInput {
        on_ac_power: true,
        lid_sleep_disabled: true,
        battery_ceiling_percent: configured_ceiling.unwrap_or(78),
        thermal_level: ThermalLevel::Nominal,
        tailscale_connected: true,
        tailscale_ip: Some("100.101.102.103".to_string()),
        container_runtime_ready: true,
        disk_free_percent: 45,
        local_ai_responding: true,
    }
}

fn run_doctor(
    explicit_state_dir: Option<&Path>,
    require_remote: bool,
    force_offline: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(None).ok().and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root.as_deref());
    let configured_ceiling = read_configured_ceiling(&state_dir);
    let (peer, has_peer_file) = load_or_default_peer(&state_dir)?;

    let (probe, source, active_sessions) = if force_offline {
        (
            baseline_probe(configured_ceiling),
            ProbeSource::SimulatedBaseline,
            Vec::new(),
        )
    } else if require_remote || (!cfg!(target_os = "macos") && has_peer_file) {
        if let Some(raw) = collect_remote_ssh_telemetry(&peer, configured_ceiling) {
            let sessions = parse_active_otto_sessions(
                raw.active_tmux_sessions_stdout.as_deref().unwrap_or(""),
            );
            (
                SystemProbeInput::from_raw_telemetry(&raw),
                ProbeSource::RemoteSsh {
                    alias: peer.alias.clone(),
                    destination: peer.ssh_destination(),
                },
                sessions,
            )
        } else if require_remote {
            return Err(format!(
                "unable to reach paired MacBook '{}' ({}) over SSH",
                peer.alias,
                peer.ssh_destination()
            )
            .into());
        } else {
            println!(
                "[info] Paired MacBook '{}' ({}) unreachable over SSH; showing baseline simulation.",
                peer.alias,
                peer.ssh_destination()
            );
            (
                baseline_probe(configured_ceiling),
                ProbeSource::SimulatedBaseline,
                Vec::new(),
            )
        }
    } else if cfg!(target_os = "macos") {
        let raw = collect_local_macos_telemetry(configured_ceiling);
        let sessions =
            parse_active_otto_sessions(raw.active_tmux_sessions_stdout.as_deref().unwrap_or(""));
        (
            SystemProbeInput::from_raw_telemetry(&raw),
            ProbeSource::LocalMacOs,
            sessions,
        )
    } else {
        (
            baseline_probe(configured_ceiling),
            ProbeSource::SimulatedBaseline,
            Vec::new(),
        )
    };

    let report = probe.evaluate_health();
    println!("=== cheap-otto doctor (8-Point Appliance Check) ===");
    match &source {
        ProbeSource::LocalMacOs => println!("Telemetry Source: Live Local macOS"),
        ProbeSource::RemoteSsh { alias, destination } => {
            println!("Telemetry Source: Live Paired MacBook `{alias}` ({destination})")
        }
        ProbeSource::SimulatedBaseline => {
            println!(
                "Telemetry Source: Baseline Simulation (pair a MacBook via `cheap-otto pair` for live SSH telemetry)"
            )
        }
    }

    for item in &report.checks {
        let badge = match item.status {
            CheckStatus::Pass => "[PASS]",
            CheckStatus::Warn => "[WARN]",
            CheckStatus::Fail => "[FAIL]",
        };
        println!("{badge} {:<30} - {}", item.name, item.detail);
        if let Some(fix) = &item.remediation {
            println!("       Fix: {fix}");
        }
    }

    if !active_sessions.is_empty() {
        println!(
            "\nActive Agent tmux Sessions ({}): {}",
            active_sessions.len(),
            active_sessions.join(", ")
        );
    }

    Ok(())
}

fn run_arbiter(
    available_mb: u32,
    ci_running: bool,
    llm_loaded: bool,
    llm_idle_secs: u32,
    live: bool,
    explicit_state_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut resolved_avail_mb = available_mb;
    let mut resolved_thermal = ThermalLevel::Nominal;
    let mut resolved_ci_running = ci_running;

    if live {
        let repo_root = discover_workspace(None).ok().and_then(|w| w.repo_root);
        let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root.as_deref());
        let (peer, has_peer_file) = load_or_default_peer(&state_dir)?;

        if cfg!(target_os = "macos") {
            if let Some(vm_out) = capture_command_stdout("vm_stat", &[])
                && let Ok(mb) = parse_macos_vm_stat_available_mb(&vm_out)
            {
                resolved_avail_mb = mb;
            }
            if let Some(th_out) = capture_command_stdout("sysctl", &["-n", "kern.thermalstate"])
                && let Some(level) = ThermalLevel::parse(&th_out)
            {
                resolved_thermal = level;
            }
            if let Some(tmux_out) = capture_command_stdout("tmux", &["list-sessions"])
                && !parse_active_otto_sessions(&tmux_out).is_empty()
            {
                resolved_ci_running = true;
            }
        } else if has_peer_file {
            if let Some(vm_out) =
                capture_spec_stdout(&peer.build_ssh_probe_spec("Probe remote vm_stat", "vm_stat"))
                && let Ok(mb) = parse_macos_vm_stat_available_mb(&vm_out)
            {
                resolved_avail_mb = mb;
            }
            if let Some(th_out) =
                capture_spec_stdout(&peer.build_ssh_probe_spec(
                    "Probe remote thermal state",
                    "sysctl -n kern.thermalstate",
                ))
                && let Some(level) = ThermalLevel::parse(&th_out)
            {
                resolved_thermal = level;
            }
            if let Some(tmux_out) = capture_spec_stdout(
                &peer.build_ssh_probe_spec("Probe remote otto tmux sessions", "tmux list-sessions"),
            ) && !parse_active_otto_sessions(&tmux_out).is_empty()
            {
                resolved_ci_running = true;
            }
        }
    }

    let policy = ArbiterPolicy::default();
    let snapshot = ArbiterSnapshot {
        thermal_level: resolved_thermal,
        available_memory_mb: resolved_avail_mb,
        ci_build_running: resolved_ci_running,
        local_llm_loaded: llm_loaded,
        local_llm_idle_secs: llm_idle_secs,
        kill_switch_engaged: false,
    };

    let actions = policy.evaluate(&snapshot);
    println!("=== cheap-otto Unified Memory & Thermal Arbiter ===");
    println!(
        "Telemetry       : {} MB free, thermal={:?}, ci_running={resolved_ci_running}",
        snapshot.available_memory_mb, snapshot.thermal_level
    );
    for action in actions {
        println!("  -> {action:?}");
    }
    Ok(())
}

fn resolve_schedule_cadence(
    every: Option<&str>,
    daily: Option<&str>,
    weekly: Option<&str>,
) -> Result<ScheduleCadence, Box<dyn std::error::Error>> {
    if let Some(interval) = every {
        Ok(ScheduleCadence::parse_interval(interval)?)
    } else if let Some(hh_mm) = daily {
        Ok(ScheduleCadence::parse_daily(hh_mm)?)
    } else if let Some(day_time) = weekly {
        Ok(ScheduleCadence::parse_weekly(day_time)?)
    } else {
        Err("must specify one cadence flag: `--every <dur>`, `--daily <HH:MM>`, or `--weekly <DAY@HH:MM>`".into())
    }
}

fn run_schedule_add(opts: &ScheduleAddOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(opts.project)?;
    let state_dir = resolve_active_state_dir(opts.state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;

    let cadence = resolve_schedule_cadence(opts.every, opts.daily, opts.weekly)?;
    let workload = if let Some(ci_cmd) = opts.ci {
        ScheduledWorkload::CiPipeline {
            branch: opts.branch.to_string(),
            command: ci_cmd.to_string(),
            only_on_change: opts.on_change,
        }
    } else if let Some(prompt) = opts.task {
        let agent = AgentRunner::parse(opts.agent)?;
        let effort = match opts.effort {
            Some(raw) if !raw.trim().is_empty() => Some(ReasoningEffort::parse(raw)?),
            _ => None,
        };
        ScheduledWorkload::AgentTask {
            branch: opts.branch.to_string(),
            agent,
            model: opts
                .model
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            effort,
            prompt: prompt.to_string(),
            verify_command: opts.verify.to_string(),
            create_draft_pr: opts.draft_pr,
        }
    } else {
        return Err(
            "must specify either `--ci <command>` or `--task <prompt>` for `schedule add`".into(),
        );
    };

    let plan = build_schedule_install_plan(
        &peer,
        &NewScheduleParams {
            job_id: opts.id,
            project_name: &ws.project_name,
            cadence,
            workload,
            allow_battery: opts.allow_battery,
            run_at_load: opts.run_at_load,
        },
    )?;

    println!("=== cheap-otto: Register Recurring M1 Schedule (launchd) ===");
    println!("Project         : {}", plan.spec.project_name);
    println!("Schedule ID     : {}", plan.spec.job_id);
    println!("Launchd Label   : {}", plan.spec.launchd_label);
    println!("Cadence         : {}", plan.spec.cadence.human_summary());
    println!("Workload Kind   : {}", plan.spec.workload.kind_label());
    println!("Remote Plist    : {}", plan.spec.remote_plist_path);
    println!("Remote Log      : {}", plan.spec.remote_log_file);
    println!("\nInstallation Plan:");
    for step in &plan.display_steps {
        println!("  * {step}");
    }

    let mut sched_reg = load_schedule_registry(&state_dir);
    sched_reg.upsert(plan.spec.clone());
    save_schedule_registry(&state_dir, &sched_reg)?;

    if opts.execute {
        println!("\n--- Bootstrapping LaunchAgent on Paired MacBook ---");
        for spec in &plan.install_specs {
            execute_command_spec(spec)?;
        }
        println!(
            "Scheduled '{}' ({}) on {}.",
            plan.spec.job_id,
            plan.spec.cadence.human_summary(),
            peer.ssh_destination()
        );
    } else {
        println!(
            "\nSaved schedule spec to {}. Pass `--execute` to install & bootstrap on your MacBook.",
            state_dir.join("schedules.json").display()
        );
    }

    Ok(())
}

fn run_schedule_list(
    explicit_project: Option<&str>,
    explicit_state_dir: Option<&Path>,
    execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let discovered = discover_workspace(explicit_project).ok();
    let repo_root = discovered.as_ref().and_then(|w| w.repo_root.as_deref());
    let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root);
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let sched_reg = load_schedule_registry(&state_dir);

    println!("=== cheap-otto: Configured M1 Recurring Schedules ===");
    println!("State Config    : {}", state_dir.display());
    println!("Paired MacBook  : {}", peer.ssh_destination());

    let filtered: Vec<_> = sched_reg
        .jobs
        .iter()
        .filter(|j| {
            explicit_project
                .map(str::trim)
                .is_none_or(|proj| j.project_name == proj)
        })
        .collect();

    if filtered.is_empty() {
        println!("  (no schedules configured — add one with `cheap-otto schedule add ...`)");
    } else {
        for job in filtered {
            let detail = match &job.workload {
                ScheduledWorkload::CiPipeline {
                    branch,
                    command,
                    only_on_change,
                } => format!(
                    "ci [branch={branch}{}] -> `{command}`",
                    if *only_on_change { ", on-change" } else { "" }
                ),
                ScheduledWorkload::AgentTask {
                    branch,
                    agent,
                    prompt,
                    create_draft_pr,
                    ..
                } => format!(
                    "agent({agent:?}) [branch={branch}{}] -> \"{prompt}\"",
                    if *create_draft_pr { ", draft-pr" } else { "" }
                ),
            };
            println!(
                "  * {}/{} ({}) — {detail}",
                job.project_name,
                job.job_id,
                job.cadence.human_summary()
            );
        }
    }

    if execute {
        println!("\n--- Live MacBook launchctl Status ---");
        let live_spec = build_live_launchctl_list_spec(&peer);
        execute_command_spec(&live_spec)?;
    }

    Ok(())
}

fn run_schedule_run_now(
    id: &str,
    explicit_project: Option<&str>,
    explicit_state_dir: Option<&Path>,
    execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(explicit_project)?;
    let state_dir = resolve_active_state_dir(explicit_state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let spec = build_schedule_kickstart_spec(&peer, &ws.project_name, id)?;

    println!("=== cheap-otto: Trigger Scheduled Job Immediately ===");
    println!("Project         : {}", ws.project_name);
    println!("Schedule ID     : {id}");
    println!("Command         : $ {}", spec.display_command());

    if execute {
        execute_command_spec(&spec)?;
        println!(
            "Triggered '{id}'. Watch live via `cheap-otto attach --id {id} --execute` or `cheap-otto logs --id {id} --follow --execute`."
        );
    }

    Ok(())
}

fn run_schedule_remove(
    id: &str,
    explicit_project: Option<&str>,
    explicit_state_dir: Option<&Path>,
    execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(explicit_project)?;
    let state_dir = resolve_active_state_dir(explicit_state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let plan = build_schedule_remove_plan(&peer, &ws.project_name, id)?;

    println!("=== cheap-otto: Remove Recurring M1 Schedule ===");
    println!("Launchd Label   : {}", plan.launchd_label);
    println!("Command         : $ {}", plan.display_command);

    let mut sched_reg = load_schedule_registry(&state_dir);
    let removed_local = sched_reg.remove(&plan.project_name, &plan.job_id);
    if removed_local && state_dir.exists() {
        save_schedule_registry(&state_dir, &sched_reg)?;
    }

    if execute {
        execute_command_spec(&plan.ssh_spec)?;
        println!(
            "Unloaded and removed '{}' from {}.",
            plan.launchd_label,
            peer.ssh_destination()
        );
    }

    Ok(())
}

fn run_runs(opts: &RunsOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(opts.project)?;
    let state_dir = resolve_active_state_dir(opts.state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let scan_lines = opts.limit.saturating_mul(5).max(50);
    let fetch_spec = build_fetch_runs_spec(&peer, &ws.project_name, scan_lines)?;

    println!(
        "=== cheap-otto: Scheduled CI & Agent Run History ({}) ===",
        ws.project_name
    );
    println!("Ledger Query    : $ {}", fetch_spec.display_command());

    if opts.execute {
        let raw = capture_spec_stdout(&fetch_spec).unwrap_or_default();
        let mut records = parse_run_history_jsonl(&raw);
        if let Some(filter_id) = opts.id {
            let trimmed = filter_id.trim();
            records.retain(|r| r.job_id == trimmed);
        }
        let start = records.len().saturating_sub(opts.limit);
        let recent = &records[start..];

        println!("\n--- Recent Executions ({}) ---", recent.len());
        if recent.is_empty() {
            println!("  (no recorded runs yet)");
        } else {
            for rec in recent {
                let badge = match rec.status {
                    RunStatus::Pass => "[PASS]",
                    RunStatus::Fail => "[FAIL]",
                    RunStatus::SkippedUnchanged => "[SKIP:UNCHANGED]",
                    RunStatus::SkippedHardwareGuard => "[SKIP:THERMAL/BATT]",
                };
                println!(
                    "  {badge:<19} {:<18} kind={:<6} sha={:<12} dur={}s  ts={}",
                    rec.job_id, rec.kind, rec.commit_sha, rec.duration_secs, rec.started_epoch_secs
                );
            }
        }
    } else {
        println!("\nPass `--execute` to query live run history from your paired MacBook.");
    }

    Ok(())
}

fn run_restore(
    explicit_state_dir: Option<&Path>,
    unload_schedules: bool,
    apply: bool,
    remote: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(None).ok().and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root.as_deref());
    let backup_path = state_dir.join("state-backup.json");
    let raw = fs::read_to_string(&backup_path)?;
    let backup: MacOsStateBackup = serde_json::from_str(&raw)?;
    let plan = backup.build_clamshell_plan(backup.battery_charge_ceiling_percent.clamp(50, 100))?;
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let unload_spec = build_unload_all_schedules_spec(&peer);

    println!("=== cheap-otto: Restore Original Laptop Behavior ===");
    for cmd in &plan.pmset_restore_commands {
        println!("  $ {cmd}");
    }
    if unload_schedules {
        println!("  $ {}", unload_spec.display_command());
    }

    if apply {
        println!("\n--- Applying Factory Restore Commands ---");
        if remote {
            let port_str = peer.ssh_port.to_string();
            let dest = peer.ssh_destination();
            for cmd in &plan.pmset_restore_commands {
                let remote_spec = CommandSpec::new(
                    "Restore pmset power configuration on paired MacBook over SSH",
                    "ssh",
                    ["-t", "-p", &port_str, &dest, cmd],
                );
                execute_command_spec(&remote_spec)?;
            }
            if unload_schedules {
                execute_command_spec(&unload_spec)?;
            }
        } else if cfg!(target_os = "macos") {
            for spec in &plan.restore_specs {
                execute_command_spec(spec)?;
            }
            if unload_schedules {
                let local_unload = CommandSpec::new(
                    "Unload and remove all local com.cheapotto LaunchAgents",
                    "sh",
                    [
                        "-c",
                        "for plist in \"$HOME\"/Library/LaunchAgents/com.cheapotto.*.plist; do [ -f \"$plist\" ] || continue; launchctl bootout \"gui/$(id -u)\" \"$plist\" 2>/dev/null || true; rm -f \"$plist\"; done",
                    ],
                );
                execute_command_spec(&local_unload)?;
            }
        } else {
            println!(
                "  [info] Local host is not macOS; pass `--remote --apply` to restore over SSH on your paired MacBook."
            );
        }
    }

    Ok(())
}

fn clean_version_line(raw: &str) -> Option<String> {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.contains("command not found") && !l.contains("not found"))?;
    Some(line.to_string())
}

fn probe_remote_harnesses(peer: &PeerNode) -> Option<Vec<HarnessCatalogEntry>> {
    let script = "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; \
        echo '===CLAUDE==='; (command -v claude >/dev/null 2>&1 && claude --version 2>&1 || true); \
        echo '===AGY==='; (command -v agy >/dev/null 2>&1 && agy --version 2>&1 || true); \
        echo '===AGY_MODELS==='; (command -v agy >/dev/null 2>&1 && agy models 2>&1 || true); \
        echo '===KIRO==='; (command -v kiro-cli >/dev/null 2>&1 && kiro-cli --version 2>&1 || true)";
    let out = capture_spec_stdout(
        &peer.build_ssh_probe_spec("Probe remote M1 coding harnesses and models", script),
    )?;

    let (_, after_claude) = out.split_once("===CLAUDE===")?;
    let (claude_sec, rest1) = after_claude.split_once("===AGY===")?;
    let (agy_sec, rest2) = rest1.split_once("===AGY_MODELS===")?;
    let (agy_models_sec, kiro_sec) = rest2.split_once("===KIRO===")?;

    let claude_ver = clean_version_line(claude_sec);
    let agy_ver = clean_version_line(agy_sec);
    let kiro_ver = clean_version_line(kiro_sec);

    let (agy_auth, agy_models) = if agy_ver.is_some() {
        parse_agy_models_output(agy_models_sec)
    } else {
        (false, default_agy_models())
    };

    Some(vec![
        HarnessCatalogEntry {
            id: "claude".to_string(),
            display_name: "Claude Code".to_string(),
            installed: claude_ver.is_some(),
            version: claude_ver.clone(),
            authenticated: claude_ver.is_some(),
            status_note: claude_ver.map(|v| format!("Ready ({v})")),
            supports_stream_json: true,
            supported_efforts: standard_efforts(),
            models: default_claude_models(),
        },
        HarnessCatalogEntry {
            id: "agy".to_string(),
            display_name: "Antigravity (AGY)".to_string(),
            installed: agy_ver.is_some(),
            version: agy_ver.clone(),
            authenticated: agy_auth,
            status_note: if agy_ver.is_none() {
                Some("Not installed on PATH".to_string())
            } else if !agy_auth {
                Some(
                    "Sign in once via `cheap-otto shell --execute` -> `agy` to unlock live models"
                        .to_string(),
                )
            } else {
                agy_ver.map(|v| format!("Authenticated ({v})"))
            },
            supports_stream_json: true,
            supported_efforts: standard_efforts(),
            models: agy_models,
        },
        HarnessCatalogEntry {
            id: "kiro".to_string(),
            display_name: "Kiro CLI".to_string(),
            installed: kiro_ver.is_some(),
            version: kiro_ver.clone(),
            authenticated: kiro_ver.is_some(),
            status_note: kiro_ver.map(|v| format!("Ready ({v})")),
            supports_stream_json: false,
            supported_efforts: standard_efforts(),
            models: default_kiro_models(),
        },
        HarnessCatalogEntry {
            id: "custom".to_string(),
            display_name: "Custom Shell / CI Script".to_string(),
            installed: true,
            version: Some("POSIX /bin/sh".to_string()),
            authenticated: true,
            status_note: Some("Runs arbitrary build/test shell pipelines".to_string()),
            supports_stream_json: false,
            supported_efforts: Vec::new(),
            models: Vec::new(),
        },
    ])
}

fn run_harnesses(
    require_remote: bool,
    json_out: bool,
    explicit_state_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(None).ok().and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root.as_deref());
    let (peer, has_peer_file) = load_or_default_peer(&state_dir)?;

    let (source_label, entries) = if require_remote || (!cfg!(target_os = "macos") && has_peer_file)
    {
        if let Some(remote_entries) = probe_remote_harnesses(&peer) {
            (
                format!(
                    "Paired MacBook `{}` ({})",
                    peer.alias,
                    peer.ssh_destination()
                ),
                remote_entries,
            )
        } else if require_remote {
            return Err(format!(
                "unable to probe harnesses on paired MacBook '{}' ({}) over SSH",
                peer.alias,
                peer.ssh_destination()
            )
            .into());
        } else {
            ("Local Host".to_string(), probe_local_harnesses())
        }
    } else {
        ("Local Host".to_string(), probe_local_harnesses())
    };

    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "source": source_label,
                "harnesses": entries,
            }))?
        );
        return Ok(());
    }

    println!("=== cheap-otto: Multi-Harness & Model Catalog ===");
    println!("Source          : {source_label}");
    for entry in &entries {
        let badge = if !entry.installed {
            "[MISSING]"
        } else if entry.authenticated {
            "[READY]"
        } else {
            "[SIGN-IN]"
        };
        println!(
            "\n{badge:<10} {} (`--agent {}`) — {}",
            entry.display_name,
            entry.id,
            entry.status_note.as_deref().unwrap_or("-")
        );
        if !entry.models.is_empty() {
            let model_ids: Vec<&str> = entry.models.iter().map(|m| m.id.as_str()).collect();
            println!("           Models  : {}", model_ids.join(", "));
        }
        if !entry.supported_efforts.is_empty() {
            let effs: Vec<&str> = entry
                .supported_efforts
                .iter()
                .map(|e| e.as_flag_str())
                .collect();
            println!("           Efforts : {}", effs.join(", "));
        }
    }
    Ok(())
}

fn load_or_synthesize_agent_record(
    reg: &AgentSessionRegistry,
    peer: &PeerNode,
    project_name: &str,
    raw_id: &str,
) -> Result<AgentSessionRecord, Box<dyn std::error::Error>> {
    let clean_id = normalize_agent_id(raw_id);
    if let Some(found) = reg
        .find(project_name, &clean_id)
        .or_else(|| reg.find_any_project(&clean_id))
    {
        return Ok(found.clone());
    }

    let remote_repo = peer.remote_project_dir(project_name)?;
    let wt_dir = format!("{remote_repo}/.otto-worktrees/{clean_id}");
    let default_cfg = HarnessRunConfig {
        runner: AgentRunner::ClaudeCode,
        model: None,
        effort: None,
        session_id: None,
        resume_existing: true,
        stream_json: true,
    };
    Ok(AgentSessionRecord::new_initial(
        project_name,
        &clean_id,
        &wt_dir,
        &default_cfg,
        "cargo test --all-targets",
        "(untracked worktree)",
        current_epoch_secs(),
    ))
}

fn split_inspect_sections(raw: &str) -> (&str, &str, &str, &str) {
    let after_tmux = raw.split_once("===TMUX===").map_or("", |(_, r)| r);
    let (tmux_sec, rest1) = after_tmux
        .split_once("===GIT===")
        .unwrap_or((after_tmux, ""));
    let (git_sec, rest2) = rest1.split_once("===NDJSON===").unwrap_or((rest1, ""));
    let (ndjson_sec, log_sec) = rest2.split_once("===LOG===").unwrap_or((rest2, ""));
    (
        tmux_sec.trim(),
        git_sec.trim(),
        ndjson_sec.trim(),
        log_sec.trim(),
    )
}

fn run_inspect(opts: &InspectOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(opts.project)?;
    let state_dir = resolve_active_state_dir(opts.state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;

    let mut agent_reg = load_agent_registry(&state_dir);
    let mut record = load_or_synthesize_agent_record(&agent_reg, &peer, &ws.project_name, opts.id)?;
    let plan = build_agent_inspect_plan(&peer, &record.project_name, &record.agent_id)?;

    if !opts.execute {
        if opts.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "record": record,
                    "probe_command": plan.probe_spec.display_command(),
                    "executed": false,
                }))?
            );
        } else {
            println!("=== cheap-otto: Inspect Individual Agent Worktree ===");
            println!("Project         : {}", record.project_name);
            println!("Agent ID        : {}", record.agent_id);
            println!("Worktree Branch : {}", record.branch_name);
            println!(
                "Harness / Model : {} (model={})",
                record.harness.slug(),
                record.model.as_deref().unwrap_or("default")
            );
            println!("SSH Probe       : $ {}", plan.probe_spec.display_command());
            println!("\nPass `--execute` to query live worktree diff & telemetry over SSH.");
        }
        return Ok(());
    }

    let raw_out = capture_spec_stdout(&plan.probe_spec).ok_or_else(|| {
        format!(
            "failed to probe agent '{}/{}' over SSH on {}",
            record.project_name,
            record.agent_id,
            peer.ssh_destination()
        )
    })?;

    let (tmux_sec, git_sec, ndjson_sec, log_sec) = split_inspect_sections(&raw_out);
    let fallback_sid = record
        .provider_session_id
        .clone()
        .unwrap_or_else(|| format!("otto-{}", record.agent_id));
    let summary = summarize_ndjson_and_log(ndjson_sec, log_sec, &fallback_sid);

    if let Some(sid) = summary.provider_session_id {
        record.provider_session_id = Some(sid);
    }
    if let Some(lbl) = summary.latest_activity_label {
        record.current_activity = Some(lbl);
    }
    if let Some(resp) = summary.latest_response {
        record.last_response = Some(resp);
    }
    if tmux_sec.starts_with("0:") {
        record.status = AgentSessionStatus::Active;
    } else if let Some(code) = summary.exit_code {
        record.status = if code == 0 {
            AgentSessionStatus::IdleDone
        } else {
            AgentSessionStatus::Failed
        };
    } else if let Some(dead_status) = tmux_sec.strip_prefix("1:") {
        record.status = if dead_status.trim() == "0" {
            AgentSessionStatus::IdleDone
        } else {
            AgentSessionStatus::Failed
        };
    }
    record.updated_epoch_secs = current_epoch_secs();

    agent_reg.upsert(record.clone());
    if state_dir.exists() {
        save_agent_registry(&state_dir, &agent_reg)?;
    }

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "record": record,
                "tmux_state": tmux_sec,
                "git_status": git_sec,
                "latest_response": record.last_response,
                "log_tail": log_sec,
            }))?
        );
        return Ok(());
    }

    println!(
        "=== cheap-otto: Live Agent Inspection ({}/{}) ===",
        record.project_name, record.agent_id
    );
    println!("Status          : {:?}", record.status);
    println!(
        "Harness / Model : {} (model={}, effort={})",
        record.harness.slug(),
        record.model.as_deref().unwrap_or("default"),
        record
            .effort
            .map_or("default", ReasoningEffort::as_flag_str)
    );
    println!(
        "Session ID      : {}",
        record.provider_session_id.as_deref().unwrap_or("(none)")
    );
    println!(
        "Activity        : {}",
        record.current_activity.as_deref().unwrap_or("-")
    );
    println!("\n--- Worktree Git Status & Diff ---\n{git_sec}");
    if let Some(resp) = &record.last_response {
        println!("\n--- Latest Agent Response ---\n{resp}");
    }
    Ok(())
}

fn run_ask(opts: &AskOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let ws = discover_workspace(opts.project)?;
    let state_dir = resolve_active_state_dir(opts.state_dir, ws.repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;

    let mut agent_reg = load_agent_registry(&state_dir);
    let mut record = load_or_synthesize_agent_record(&agent_reg, &peer, &ws.project_name, opts.id)?;

    let override_harness = match opts.agent {
        Some(raw) if !raw.trim().is_empty() => Some(AgentRunner::parse(raw)?),
        _ => None,
    };
    let override_effort = match opts.effort {
        Some(raw) if !raw.trim().is_empty() => Some(ReasoningEffort::parse(raw)?),
        _ => None,
    };
    let override_model = opts
        .model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    let plan = build_agent_ask_plan(
        &peer,
        &AgentAskParams {
            record: &record,
            prompt: opts.prompt,
            override_harness,
            override_model,
            override_effort,
            verify_and_commit: opts.verify,
            run_async: opts.run_async,
        },
    )?;

    if !opts.execute {
        if opts.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "agent_id": record.agent_id,
                    "project_name": record.project_name,
                    "harness": plan.harness_config.runner.slug(),
                    "model": plan.harness_config.model,
                    "effort": plan.harness_config.effort,
                    "resume_existing": plan.harness_config.resume_existing,
                    "session_id": plan.harness_config.session_id,
                    "command": plan.display_command,
                    "executed": false,
                }))?
            );
        } else {
            println!("=== cheap-otto: Direct Per-Agent Query (`ask`) ===");
            println!(
                "Target Agent    : {}/{}",
                record.project_name, record.agent_id
            );
            println!(
                "Harness / Model : {} (model={}, resume={})",
                plan.harness_config.runner.slug(),
                plan.harness_config.model.as_deref().unwrap_or("default"),
                plan.harness_config.resume_existing
            );
            println!("SSH Command     :\n  $ {}", plan.display_command);
            println!("\nPass `--execute` to send this query directly to the agent.");
        }
        return Ok(());
    }

    let now = current_epoch_secs();
    let next_turn = u32::try_from(record.turns.len()).unwrap_or(0) + 1;
    record.harness = plan.harness_config.runner;
    record.model.clone_from(&plan.harness_config.model);
    record.effort = plan.harness_config.effort;
    if plan.harness_config.session_id.is_some() {
        record
            .provider_session_id
            .clone_from(&plan.harness_config.session_id);
    }

    if opts.run_async {
        execute_command_spec(&plan.exec_spec)?;
        record.status = AgentSessionStatus::Active;
        record.current_activity = Some("Running async follow-up turn".to_string());
        record.turns.push(AgentTurnEntry {
            turn: next_turn,
            prompt: opts.prompt.trim().to_string(),
            harness: record.harness,
            model: record.model.clone(),
            effort: record.effort,
            response_summary: None,
            started_epoch_secs: now,
        });
        record.updated_epoch_secs = now;
        agent_reg.upsert(record.clone());
        save_agent_registry(&state_dir, &agent_reg)?;
        sync_remote_agent_record(&peer, &record);

        if opts.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "agent_id": record.agent_id,
                    "project_name": record.project_name,
                    "dispatched_async": true,
                    "turn": next_turn,
                }))?
            );
        } else {
            println!(
                "Dispatched async turn #{next_turn} to `otto-{}`. Inspect via `cheap-otto inspect --id {} --execute`.",
                record.agent_id, record.agent_id
            );
        }
        return Ok(());
    }

    let output = Command::new(&plan.exec_spec.program)
        .args(&plan.exec_spec.args)
        .output()?;
    let stdout_raw = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr_raw = String::from_utf8_lossy(&output.stderr).into_owned();

    let fallback_sid = record
        .provider_session_id
        .clone()
        .unwrap_or_else(|| format!("otto-{}", record.agent_id));
    let parsed = if plan.harness_config.stream_json {
        summarize_ndjson_and_log(&stdout_raw, "", &fallback_sid)
    } else {
        summarize_ndjson_and_log("", &stdout_raw, &fallback_sid)
    };

    if let Some(sid) = parsed.provider_session_id {
        record.provider_session_id = Some(sid);
    }
    let clean_answer = parsed
        .latest_response
        .unwrap_or_else(|| stdout_raw.trim().to_string());
    record.last_response = Some(clean_answer.clone());
    record.status = if output.status.success() {
        AgentSessionStatus::IdleDone
    } else {
        AgentSessionStatus::Failed
    };
    record.current_activity = Some(if output.status.success() {
        "Idle (follow-up turn complete)".to_string()
    } else {
        "Follow-up turn failed".to_string()
    });
    record.turns.push(AgentTurnEntry {
        turn: next_turn,
        prompt: opts.prompt.trim().to_string(),
        harness: record.harness,
        model: record.model.clone(),
        effort: record.effort,
        response_summary: Some(clean_answer.clone()),
        started_epoch_secs: now,
    });
    record.updated_epoch_secs = current_epoch_secs();

    agent_reg.upsert(record.clone());
    save_agent_registry(&state_dir, &agent_reg)?;
    sync_remote_agent_record(&peer, &record);

    if !output.status.success() {
        return Err(format!(
            "agent query exited with {}: {}",
            output.status,
            if !clean_answer.is_empty() {
                clean_answer
            } else {
                stderr_raw.trim().to_string()
            }
        )
        .into());
    }

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "agent_id": record.agent_id,
                "project_name": record.project_name,
                "turn": next_turn,
                "harness": record.harness.slug(),
                "model": record.model,
                "provider_session_id": record.provider_session_id,
                "response": clean_answer,
            }))?
        );
    } else {
        println!(
            "=== Response from {}/{} [{} / {}] ===",
            record.project_name,
            record.agent_id,
            record.harness.slug(),
            record.model.as_deref().unwrap_or("default")
        );
        println!("{clean_answer}");
    }
    Ok(())
}

fn run_template_add(opts: &TemplateAddOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(opts.project)
        .ok()
        .and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(opts.state_dir, repo_root.as_deref());
    let runner = AgentRunner::parse(opts.agent)?;
    let effort = match opts.effort {
        Some(raw) if !raw.trim().is_empty() => Some(ReasoningEffort::parse(raw)?),
        _ => None,
    };
    let model = opts
        .model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    let tpl = TaskTemplate::new(&NewTemplateParams {
        template_id: opts.id,
        title: opts.title,
        project_name: opts.project,
        harness: runner,
        model,
        effort,
        prompt: opts.task,
        verify_command: opts.verify,
        create_draft_pr: opts.draft_pr,
    })?;

    let mut reg = load_template_registry(&state_dir);
    reg.upsert(tpl.clone());
    save_template_registry(&state_dir, &reg)?;

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&tpl)?);
    } else {
        println!(
            "Saved repeatable template '{}' ({} / model={}) to {}.",
            tpl.template_id,
            tpl.harness.slug(),
            tpl.model.as_deref().unwrap_or("default"),
            state_dir.join("templates.json").display()
        );
    }
    Ok(())
}

fn run_template_list(
    explicit_project: Option<&str>,
    explicit_state_dir: Option<&Path>,
    json_out: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(explicit_project)
        .ok()
        .and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root.as_deref());
    let reg = load_template_registry(&state_dir);

    let filtered: Vec<&TaskTemplate> = reg
        .templates
        .iter()
        .filter(|t| {
            explicit_project
                .map(str::trim)
                .is_none_or(|proj| t.project_name.as_deref().is_none_or(|bound| bound == proj))
        })
        .collect();

    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "templates": filtered }))?
        );
        return Ok(());
    }

    println!(
        "=== cheap-otto: Repeatable Task Templates ({}) ===",
        filtered.len()
    );
    if filtered.is_empty() {
        println!("  (none saved yet — add one with `cheap-otto template add --id ... --task ...`)");
    } else {
        for t in filtered {
            println!(
                "  * {:<22} [{} / model={} / effort={}] -> \"{}\"",
                t.template_id,
                t.harness.slug(),
                t.model.as_deref().unwrap_or("default"),
                t.effort.map_or("default", ReasoningEffort::as_flag_str),
                t.title
            );
        }
    }
    Ok(())
}

fn run_template_run(opts: &TemplateRunOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(opts.project)
        .ok()
        .and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(opts.state_dir, repo_root.as_deref());
    let reg = load_template_registry(&state_dir);
    let tpl = reg.get(opts.id).ok_or_else(|| {
        format!(
            "template '{}' not found in {}",
            opts.id,
            state_dir.display()
        )
    })?;

    let override_runner = match opts.agent {
        Some(raw) if !raw.trim().is_empty() => Some(AgentRunner::parse(raw)?),
        _ => None,
    };
    let override_effort = match opts.effort {
        Some(raw) if !raw.trim().is_empty() => Some(ReasoningEffort::parse(raw)?),
        _ => None,
    };
    let override_model = opts
        .model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    let merged = tpl.to_harness_config(override_runner, override_model, override_effort, None);
    let target_project = opts.project.or(tpl.project_name.as_deref());

    run_delegate(&DelegateOptions {
        project: target_project,
        task: &tpl.prompt,
        id: opts.task_id.or(Some(&tpl.template_id)),
        agent: merged.runner.slug(),
        model: merged.model.as_deref(),
        effort: merged.effort.map(ReasoningEffort::as_flag_str),
        verify: &tpl.verify_command,
        state_dir: Some(&state_dir),
        json: opts.json,
        execute: opts.execute,
    })
}

fn run_template_schedule(
    template_id: &str,
    explicit_schedule_id: Option<&str>,
    base_opts: &ScheduleAddOptions<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(base_opts.project)
        .ok()
        .and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(base_opts.state_dir, repo_root.as_deref());
    let reg = load_template_registry(&state_dir);
    let tpl = reg.get(template_id).ok_or_else(|| {
        format!(
            "template '{template_id}' not found in {}",
            state_dir.display()
        )
    })?;

    let sched_id = explicit_schedule_id.unwrap_or(&tpl.template_id);
    run_schedule_add(&ScheduleAddOptions {
        id: sched_id,
        project: base_opts.project.or(tpl.project_name.as_deref()),
        every: base_opts.every,
        daily: base_opts.daily,
        weekly: base_opts.weekly,
        ci: None,
        task: Some(&tpl.prompt),
        agent: tpl.harness.slug(),
        model: tpl.model.as_deref(),
        effort: tpl.effort.map(ReasoningEffort::as_flag_str),
        verify: &tpl.verify_command,
        branch: base_opts.branch,
        on_change: false,
        draft_pr: tpl.create_draft_pr,
        allow_battery: base_opts.allow_battery,
        run_at_load: base_opts.run_at_load,
        state_dir: Some(&state_dir),
        execute: base_opts.execute,
    })
}

fn run_template_remove(
    template_id: &str,
    explicit_state_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(None).ok().and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root.as_deref());
    let mut reg = load_template_registry(&state_dir);
    if reg.remove(template_id) {
        save_template_registry(&state_dir, &reg)?;
        println!("Removed template '{template_id}'.");
    } else {
        println!("Template '{template_id}' was not present.");
    }
    Ok(())
}

fn run_serve_cmd(
    host: &str,
    port: u16,
    pixel_port: u16,
    explicit_state_dir: Option<&Path>,
    explicit_workspaces_root: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let home = user_home_dir();
    let state_dir = match explicit_state_dir {
        Some(p) => p.to_path_buf(),
        None => home
            .as_ref()
            .map_or_else(|| PathBuf::from(".cheap-otto"), |h| h.join(".cheap-otto")),
    };
    let workspaces_root = match explicit_workspaces_root {
        Some(p) => p.to_path_buf(),
        None => home.as_ref().map_or_else(
            || PathBuf::from("otto-workspaces"),
            |h| h.join("otto-workspaces"),
        ),
    };
    let (peer, _) = load_or_default_peer(&state_dir)?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_serve(ServeConfig {
        host: host.to_string(),
        port,
        pixel_port,
        state_dir,
        workspaces_root,
        peer,
    }))
}

fn is_local_port_open(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

fn open_system_browser(url: &str) {
    if cfg!(target_os = "windows") {
        let _ = Command::new("cmd")
            .args(["/c", "start", "", url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    } else if cfg!(target_os = "macos") {
        let _ = Command::new("open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    } else {
        let _ = Command::new("xdg-open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

fn run_ui(
    port: u16,
    pixel_port: u16,
    explicit_state_dir: Option<&Path>,
    no_open: bool,
    execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = discover_workspace(None).ok().and_then(|w| w.repo_root);
    let state_dir = resolve_active_state_dir(explicit_state_dir, repo_root.as_deref());
    let (peer, _) = load_or_default_peer(&state_dir)?;
    let tunnel = peer.build_port_tunnel(&[3000, 5173, 8080, port, pixel_port])?;

    let remote_start_script = format!(
        "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; \
         mkdir -p ~/.pixel-agents ~/.cheap-otto ~/otto-workspaces; \
         if command -v node >/dev/null 2>&1; then \
           node -e 'const fs=require(\"fs\"),p=process.env.HOME+\"/.pixel-agents/config.json\";let c={{}};try{{c=JSON.parse(fs.readFileSync(p,\"utf8\"))}}catch{{}};c.standalone=Object.assign({{}},c.standalone,{{watchAllSessions:true}});c.hooksConsent=Object.assign({{}},c.hooksConsent,{{claude:\"granted\"}});fs.writeFileSync(p,JSON.stringify(c,null,2));'; \
         fi; \
         if command -v pixel-agents >/dev/null 2>&1; then \
           tmux has-session -t cheapotto-pixel 2>/dev/null || tmux new-session -d -s cheapotto-pixel 'export PATH=\"/opt/homebrew/bin:/usr/local/bin:$PATH\"; exec pixel-agents --port {pixel_port} --host 127.0.0.1 --no-terminal'; \
         fi; \
         tmux kill-session -t cheapotto-serve 2>/dev/null || true; \
         tmux new-session -d -s cheapotto-serve 'export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; exec cheap-otto serve --host 127.0.0.1 --port {port} --pixel-port {pixel_port}'"
    );

    let start_spec = peer.build_ssh_probe_spec(
        "Start pixel-agents (:7421) and cheap-otto serve (:7420) on paired MacBook",
        &remote_start_script,
    );

    let ui_url = format!("http://localhost:{port}");
    let pixel_url = format!("http://localhost:{pixel_port}");

    println!("=== cheap-otto: Mission Control GUI & Pixel Office ===");
    println!(
        "Paired MacBook  : {} ({})",
        peer.alias,
        peer.ssh_destination()
    );
    println!("Mission Control : {ui_url}");
    println!("Pixel Office    : {pixel_url} (embedded + standalone Flat & Diorama views)");
    println!("Remote Startup  :\n  $ {}", start_spec.display_command());

    if !execute {
        println!(
            "\nPass `--execute` to start the remote M1 daemons, establish SSH port forwarding, and open Mission Control."
        );
        return Ok(());
    }

    println!("\n--- Starting M1 Daemons & Tunnel ---");
    execute_command_spec(&start_spec)?;

    if state_dir.join("peer.json").exists()
        && let Some(home) = user_home_dir()
    {
        let ssh_dir = home.join(".ssh");
        let ssh_cfg_path = ssh_dir.join("config");
        let existing = fs::read_to_string(&ssh_cfg_path).unwrap_or_default();
        if let Ok(merged) =
            merge_ssh_config_block(&existing, &peer.alias, &tunnel.ssh_config_snippet)
        {
            let _ = fs::write(&ssh_cfg_path, merged);
        }
    }

    if !is_local_port_open(port) || !is_local_port_open(pixel_port) {
        let port_str = peer.ssh_port.to_string();
        let dest = peer.ssh_destination();
        let fwd_ui = format!("127.0.0.1:{port}:127.0.0.1:{port}");
        let fwd_px = format!("127.0.0.1:{pixel_port}:127.0.0.1:{pixel_port}");
        println!("  ==> Spawning background SSH tunnel for :{port} and :{pixel_port}");
        let _ = Command::new("ssh")
            .args([
                "-N",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ServerAliveInterval=30",
                "-p",
                &port_str,
                "-L",
                &fwd_ui,
                "-L",
                &fwd_px,
                &dest,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        for _ in 0..12 {
            if is_local_port_open(port) {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    if !no_open {
        open_system_browser(&ui_url);
    }
    println!("\nMission Control is live at {ui_url}");
    Ok(())
}
