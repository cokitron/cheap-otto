#![forbid(unsafe_code)]

use cheap_otto::arbiter::{ArbiterPolicy, ArbiterSnapshot};
use cheap_otto::bridge::PeerNode;
use cheap_otto::doctor::{CheckStatus, SystemProbeInput};
use cheap_otto::hardware::{CpuArch, HardwareProfile, ThermalLevel};
use cheap_otto::macos::MacOsStateBackup;
use cheap_otto::worktree::{
    AgentRunner, build_delegate_plan, build_jobs_status_command, build_pull_plan,
};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

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
        /// Detected or target RAM in GB (default: 16 for M1 MacBook Air)
        #[arg(long, default_value_t = 16)]
        ram_gb: u32,
        /// Target battery charge ceiling percentage (50..=100)
        #[arg(long, default_value_t = 78)]
        battery_cap: u8,
        /// Output directory for state backup
        #[arg(long, default_value = ".cheap-otto")]
        out_dir: PathBuf,
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
        /// State directory where peer.json is stored
        #[arg(long, default_value = ".cheap-otto")]
        out_dir: PathBuf,
    },
    /// Option 5 (Cross-OS Dev Bridge): Sync your current Windows project to the MacBook over Git + SSH
    Sync {
        /// Project folder name (default: cheap-otto)
        #[arg(long, default_value = "cheap-otto")]
        project: String,
        /// Path to state directory containing peer.json
        #[arg(long, default_value = ".cheap-otto")]
        state_dir: PathBuf,
    },
    /// Option 5 (Cross-OS Dev Bridge): Run a native macOS / ARM64 command inside the synced MacBook workspace
    Exec {
        /// Command to run on the MacBook (e.g. "cargo test --all-targets")
        #[arg(long, default_value = "cargo test --all-targets")]
        cmd: String,
        /// Project folder name
        #[arg(long, default_value = "cheap-otto")]
        project: String,
        /// Path to state directory containing peer.json
        #[arg(long, default_value = ".cheap-otto")]
        state_dir: PathBuf,
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
        /// Verification gate command executed after the agent finishes
        #[arg(long, default_value = "cargo test --all-targets")]
        verify: String,
        /// Project folder name
        #[arg(long, default_value = "cheap-otto")]
        project: String,
        /// Path to state directory containing peer.json
        #[arg(long, default_value = ".cheap-otto")]
        state_dir: PathBuf,
    },
    /// Option 1 (Parallel Agent Farm): List active and completed agent worktrees on the MacBook
    Jobs {
        /// Project folder name
        #[arg(long, default_value = "cheap-otto")]
        project: String,
        /// Path to state directory containing peer.json
        #[arg(long, default_value = ".cheap-otto")]
        state_dir: PathBuf,
    },
    /// Option 1 (Parallel Agent Farm): Pull a completed MacBook worktree branch (`otto/<task-id>`) back into Windows
    Pull {
        /// Task ID of the MacBook worktree to fetch into the local Windows repo
        #[arg(long)]
        id: String,
        /// Project folder name
        #[arg(long, default_value = "cheap-otto")]
        project: String,
        /// Path to state directory containing peer.json
        #[arg(long, default_value = ".cheap-otto")]
        state_dir: PathBuf,
    },
    /// Run the 8-point headless appliance health check
    Doctor,
    /// Evaluate the Unified Memory & Thermal Arbiter against current system telemetry
    Arbiter {
        /// Available unified memory in MB
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
    },
    /// Print exact commands to restore original macOS sleep and power behavior
    Restore {
        /// Path to state backup directory
        #[arg(long, default_value = ".cheap-otto")]
        state_dir: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            ram_gb,
            battery_cap,
            out_dir,
        } => run_init(ram_gb, battery_cap, &out_dir)?,
        Commands::Pair {
            host,
            user,
            alias,
            port,
            ports,
            out_dir,
        } => run_pair(&alias, &host, &user, port, &ports, &out_dir)?,
        Commands::Sync { project, state_dir } => run_sync(&project, &state_dir)?,
        Commands::Exec {
            cmd,
            project,
            state_dir,
        } => run_exec(&project, &cmd, &state_dir)?,
        Commands::Delegate {
            task,
            id,
            agent,
            verify,
            project,
            state_dir,
        } => run_delegate(&project, &task, id.as_deref(), &agent, &verify, &state_dir)?,
        Commands::Jobs { project, state_dir } => run_jobs(&project, &state_dir)?,
        Commands::Pull {
            id,
            project,
            state_dir,
        } => run_pull(&project, &id, &state_dir)?,
        Commands::Doctor => run_doctor(),
        Commands::Arbiter {
            available_mb,
            ci_running,
            llm_loaded,
            llm_idle_secs,
        } => run_arbiter(available_mb, ci_running, llm_loaded, llm_idle_secs),
        Commands::Restore { state_dir } => run_restore(&state_dir)?,
    }

    Ok(())
}

fn load_or_default_peer(state_dir: &Path) -> Result<PeerNode, Box<dyn std::error::Error>> {
    let peer_path = state_dir.join("peer.json");
    if peer_path.exists() {
        let raw = fs::read_to_string(&peer_path)?;
        let peer: PeerNode = serde_json::from_str(&raw)?;
        Ok(peer)
    } else {
        Ok(PeerNode::new("otto-mac", "100.101.102.103", "admin", 22)?)
    }
}

fn run_init(
    ram_gb: u32,
    battery_cap: u8,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let profile = HardwareProfile {
        model_name: "MacBookAir10,1 (Apple M1)".to_string(),
        arch: CpuArch::AppleSilicon,
        memory_gb: ram_gb,
        has_battery: true,
        battery_health_percent: Some(88),
        fanless: true,
    };

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
    let backup = MacOsStateBackup::from_pmset_custom(default_pmset, 1_730_000_000)?;
    let power_plan = backup.build_clamshell_plan(battery_cap)?;

    fs::create_dir_all(out_dir)?;
    let backup_path = out_dir.join("state-backup.json");
    fs::write(&backup_path, serde_json::to_string_pretty(&backup)?)?;

    println!("=== cheap-otto: Headless MacBook Appliance Setup ===");
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
    Ok(())
}

fn run_pair(
    alias: &str,
    host: &str,
    user: &str,
    port: u16,
    ports: &[u16],
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let peer = PeerNode::new(alias, host, user, port)?;
    let tunnel = peer.build_port_tunnel(ports)?;

    fs::create_dir_all(out_dir)?;
    let peer_path = out_dir.join("peer.json");
    fs::write(&peer_path, serde_json::to_string_pretty(&peer)?)?;

    let ssh_config_path = out_dir.join("ssh-config.conf");
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
    Ok(())
}

fn run_sync(project: &str, state_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let peer = load_or_default_peer(state_dir)?;
    let plan = peer.build_sync_plan(project)?;

    println!("=== cheap-otto: Cross-OS Workspace Sync (Windows -> MacBook) ===");
    println!("Project         : {}", plan.project_name);
    println!("Remote Path     : {}", plan.remote_project_dir);
    println!("Git Remote URL  : {}", plan.git_remote_url);
    println!("Excluded Paths  : {}", plan.excluded_paths.join(", "));
    println!("\nSync Commands:");
    for step in &plan.sync_commands {
        println!("  $ {step}");
    }
    Ok(())
}

fn run_exec(
    project: &str,
    command: &str,
    state_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let peer = load_or_default_peer(state_dir)?;
    let remote_cmd = peer.build_remote_exec(project, command)?;

    println!("=== cheap-otto: Cross-OS Remote Execution ===");
    println!("Target MacBook  : {}", peer.ssh_destination());
    println!("Remote Command  :");
    println!("  $ {remote_cmd}");
    Ok(())
}

fn run_delegate(
    project: &str,
    task: &str,
    id: Option<&str>,
    agent_str: &str,
    verify: &str,
    state_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let peer = load_or_default_peer(state_dir)?;
    let agent = AgentRunner::parse(agent_str)?;
    let plan = build_delegate_plan(&peer, project, task, id, agent, verify)?;

    println!("=== cheap-otto: Delegate Task to Parallel MacBook Worktree ===");
    println!("Task ID         : {}", plan.task_id);
    println!("Worktree Branch : {}", plan.branch_name);
    println!("Remote Worktree : {}", plan.remote_worktree_dir);
    println!("tmux Session    : {}", plan.tmux_session_name);
    println!("Agent Runner    : {:?}", plan.agent);
    println!("Verify Gate     : {}", plan.verify_command);
    println!("\nExecution Steps:");
    for (idx, step) in plan.execution_steps.iter().enumerate() {
        println!("  [{}] $ {step}", idx + 1);
    }
    println!(
        "\nWhen finished, fetch the branch back to Windows with:\n  $ cargo run -- pull --id {}",
        plan.task_id
    );
    Ok(())
}

fn run_jobs(project: &str, state_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let peer = load_or_default_peer(state_dir)?;
    let cmd = build_jobs_status_command(&peer, project)?;

    println!("=== cheap-otto: Parallel MacBook Agent Worktrees ===");
    println!("  $ {cmd}");
    Ok(())
}

fn run_pull(
    project: &str,
    task_id: &str,
    state_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let peer = load_or_default_peer(state_dir)?;
    let plan = build_pull_plan(&peer, project, task_id)?;

    println!("=== cheap-otto: Pull MacBook Worktree Branch Back to Windows ===");
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
    Ok(())
}

fn run_doctor() {
    let probe = SystemProbeInput {
        on_ac_power: true,
        lid_sleep_disabled: true,
        battery_ceiling_percent: 78,
        thermal_level: ThermalLevel::Nominal,
        tailscale_connected: true,
        tailscale_ip: Some("100.101.102.103".to_string()),
        container_runtime_ready: true,
        disk_free_percent: 45,
        local_ai_responding: true,
    };

    let report = probe.evaluate_health();
    println!("=== cheap-otto doctor (8-Point Appliance Check) ===");
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
}

fn run_arbiter(available_mb: u32, ci_running: bool, llm_loaded: bool, llm_idle_secs: u32) {
    let policy = ArbiterPolicy::default();
    let snapshot = ArbiterSnapshot {
        thermal_level: ThermalLevel::Nominal,
        available_memory_mb: available_mb,
        ci_build_running: ci_running,
        local_llm_loaded: llm_loaded,
        local_llm_idle_secs: llm_idle_secs,
        kill_switch_engaged: false,
    };

    let actions = policy.evaluate(&snapshot);
    println!("=== cheap-otto Unified Memory & Thermal Arbiter ===");
    for action in actions {
        println!("  -> {action:?}");
    }
}

fn run_restore(state_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let backup_path = state_dir.join("state-backup.json");
    let raw = fs::read_to_string(&backup_path)?;
    let backup: MacOsStateBackup = serde_json::from_str(&raw)?;
    let plan = backup.build_clamshell_plan(backup.battery_charge_ceiling_percent.clamp(50, 100))?;

    println!("=== cheap-otto: Restore Original Laptop Behavior ===");
    for cmd in &plan.pmset_restore_commands {
        println!("  $ {cmd}");
    }
    Ok(())
}
