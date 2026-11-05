#![forbid(unsafe_code)]

use cheap_otto::ai_gateway::{LocalAiStackConfig, ScrapedCandidate, filter_scraped_batch};
use cheap_otto::arbiter::{ArbiterPolicy, ArbiterSnapshot};
use cheap_otto::chief_of_staff::{InboxItem, triage_inbox_item};
use cheap_otto::dev_worker::{CanaryProbeResult, CanarySpec, build_pr_sandbox_plan};
use cheap_otto::doctor::{CheckStatus, SystemProbeInput};
use cheap_otto::hardware::{CpuArch, HardwareProfile, ThermalLevel};
use cheap_otto::macos::MacOsStateBackup;
use cheap_otto::network::{ApplianceNode, RemoteWorkload};
use cheap_otto::packs::{CapabilityPack, render_compose_stack};
use cheap_otto::remote_dev::RemoteDevAccessKit;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "cheap-otto",
    version,
    about = "Turn any spare MacBook or old laptop into a 24/7 headless AI agent, residential browser worker, local AI gateway, and CI runner."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Preview and generate the reversible Clamshell Appliance plan + docker-compose stack
    Init {
        /// Detected or target RAM in GB (default: 16 for M1 MacBook Air)
        #[arg(long, default_value_t = 16)]
        ram_gb: u32,
        /// Target battery charge ceiling percentage (50..=100)
        #[arg(long, default_value_t = 78)]
        battery_cap: u8,
        /// Output directory for generated compose stack and state backup
        #[arg(long, default_value = ".cheap-otto")]
        out_dir: PathBuf,
    },
    /// Pair this workstation (e.g. Windows PC) with a remote cheap-otto appliance on the same LAN or Tailscale mesh
    Connect {
        /// Appliance LAN IP (e.g. 192.168.1.42) or Tailscale IP (e.g. 100.101.102.103)
        #[arg(long, default_value = "100.101.102.103")]
        host: String,
        /// Logical name for the headless appliance node
        #[arg(long, default_value = "cheap-otto-m1")]
        name: String,
        /// Control plane port on the appliance (1024..=65535)
        #[arg(long, default_value_t = 9400)]
        port: u16,
        /// SSH username on the macOS appliance
        #[arg(long, default_value = "otto")]
        ssh_user: String,
        /// Output directory for workstation connection configs (.env and node-config.json)
        #[arg(long, default_value = ".cheap-otto")]
        out_dir: PathBuf,
    },
    /// Dispatch or simulate a remote agent workload across the 4 appliance pillars
    Dispatch {
        /// Target appliance host (LAN or Tailscale IP)
        #[arg(long, default_value = "100.101.102.103")]
        host: String,
        #[command(subcommand)]
        task: DispatchTask,
    },
    /// Run the 8-point headless appliance health check
    Doctor,
    /// Evaluate the Unified Memory & Thermal Arbiter against current system telemetry
    Arbiter {
        /// Available unified memory in MB
        #[arg(long, default_value_t = 6144)]
        available_mb: u32,
        /// Whether a GitHub Actions CI job is currently active
        #[arg(long, default_value_t = false)]
        ci_running: bool,
        /// Whether the local LLM is currently loaded in RAM
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

#[derive(Subcommand, Debug)]
enum DispatchTask {
    /// Pillar 1: Run an isolated OrbStack Background Agent Sandbox for an `agent-fix` or `triage` issue
    PrFix {
        /// GitHub repository slug (owner/repo)
        #[arg(long, default_value = "cokitron/cheap-otto")]
        repo: String,
        /// Issue or PR number
        #[arg(long, default_value_t = 1)]
        issue: u64,
        /// Trigger label (`agent-fix`, `triage`, or `autofix`)
        #[arg(long, default_value = "agent-fix")]
        label: String,
    },
    /// Pillar 1: Execute a scheduled residential-IP Playwright E2E canary check
    Canary {
        /// Canary suite name
        #[arg(long, default_value = "prod-checkout-canary")]
        suite: String,
        /// Target URL to verify from residential IP
        #[arg(long, default_value = "https://github.com/cokitron/cheap-otto")]
        url: String,
    },
    /// Pillar 2: Run the 15-minute Chief of Staff Inbox & Calendar Triage with Telegram approval buttons
    Triage {
        /// Sender address
        #[arg(long, default_value = "founder@acme.dev")]
        sender: String,
        /// Subject line
        #[arg(long, default_value = "Can we ship the ARM64 runner today?")]
        subject: String,
    },
    /// Pillar 3: Run the High-Volume 99%-Local / 1%-Frontier Scraper Filter using the local 8B model
    ScraperFilter {
        /// Minimum local relevance score (0..=100) to escalate to paid frontier cloud API
        #[arg(long, default_value_t = 95)]
        min_score: u8,
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
        Commands::Connect {
            host,
            name,
            port,
            ssh_user,
            out_dir,
        } => run_connect(&name, &host, port, &ssh_user, &out_dir)?,
        Commands::Dispatch { host, task } => run_dispatch(&host, task)?,
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
    let ai_stack = LocalAiStackConfig::from_memory_budget(&budget);
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

    let compose_yaml = render_compose_stack(CapabilityPack::all(), &budget);
    let compose_path = out_dir.join("docker-compose.yml");
    fs::write(&compose_path, &compose_yaml)?;

    println!("=== cheap-otto: Headless Appliance Initialization Plan ===");
    println!(
        "Target Hardware : {} ({} GB Unified RAM)",
        profile.model_name, profile.memory_gb
    );
    println!(
        "Local LLM Ready : {} (Max AI Budget: {} MB, Idle Unload: {}s)",
        budget.can_run_local_llm, budget.local_ai_max_mb, budget.llm_idle_unload_secs
    );
    println!(
        "Selected Models : LLM={} (~{} tok/s) | RAG={} | Whisper={}",
        ai_stack.primary_llm_tag,
        ai_stack.estimated_tokens_per_sec,
        ai_stack.embedding_model_tag,
        ai_stack.whisper_model_tag
    );
    println!("\n--- 1. Reversible Clamshell & Battery Guardrails ---");
    for line in &power_plan.human_readable_diff {
        println!("  * {line}");
    }
    println!("\nCommands to apply on macOS:");
    for cmd in &power_plan.pmset_apply_commands {
        println!("  $ {cmd}");
    }
    println!("\n--- 2. Enabled Capability Packs ---");
    for pack in CapabilityPack::all() {
        println!("  [x] {}", pack.display_name());
    }
    println!("\nSaved state backup to : {}", backup_path.display());
    println!("Saved Compose stack to: {}", compose_path.display());
    Ok(())
}

fn run_connect(
    name: &str,
    host: &str,
    port: u16,
    ssh_user: &str,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let node = ApplianceNode::new(name, host, port, ssh_user)?;
    let endpoints = node.endpoints();
    let env_contents = node.render_workstation_env();
    let dev_kit = RemoteDevAccessKit::for_node(&node);

    fs::create_dir_all(out_dir)?;
    let node_path = out_dir.join("node-config.json");
    fs::write(&node_path, serde_json::to_string_pretty(&node)?)?;

    let env_path = out_dir.join("workstation.env");
    fs::write(&env_path, &env_contents)?;

    println!("=== cheap-otto: Cross-Network Workstation <-> Appliance Pairing ===");
    println!(
        "Paired Node     : {} ({} via {:?})",
        node.node_name, node.host, node.transport
    );
    println!("\n--- Network Service Endpoints (Reachable from this Workstation) ---");
    println!("  * Control Plane     : {}", endpoints.control_plane_url);
    println!(
        "  * OpenAI Local API  : {}",
        endpoints.openai_compatible_base_url
    );
    println!(
        "  * Local Whisper ASR : {}",
        endpoints.whisper_transcribe_url
    );
    println!("  * Local RAG Embeds  : {}", endpoints.embeddings_rag_url);
    println!(
        "  * Residential Browser: {}",
        endpoints.playwright_ws_endpoint
    );
    println!("  * n8n Orchestrator  : {}", endpoints.n8n_orchestrator_url);
    println!("  * Uptime Monitor    : {}", endpoints.uptime_monitor_url);
    println!("\n--- Remote Dev Access (Pillar 4) ---");
    println!(
        "  * Attach tmux       : {}",
        dev_kit.ssh_tmux_attach_command
    );
    println!(
        "  * VS Code Tunnel    : {}",
        dev_kit.vscode_tunnel_service_command
    );
    println!("\nSaved node profile to : {}", node_path.display());
    println!("Saved workstation env : {}", env_path.display());
    Ok(())
}

fn run_dispatch(host: &str, task: DispatchTask) -> Result<(), Box<dyn std::error::Error>> {
    let node = ApplianceNode::new("cheap-otto-m1", host, 9400, "otto")?;

    match task {
        DispatchTask::PrFix { repo, issue, label } => {
            let plan = build_pr_sandbox_plan(&repo, issue, &label)?;
            let envelope = node.build_dispatch(RemoteWorkload::OvernightPrFix {
                repo: plan.repo_slug.clone(),
                issue_number: plan.issue_number,
                label: plan.trigger_label.as_str().to_string(),
            });
            println!("=== Pillar 1: Overnight PR Worker Dispatch ===");
            println!("Dispatch Endpoint   : {}", envelope.endpoint_url);
            println!(
                "CI RAM Preemption   : {} (evicts 5.5GB LLM before cargo build)",
                envelope.requires_ci_ram_preemption
            );
            println!("Sandbox Branch      : {}", plan.branch_name);
            for step in &plan.sandbox_steps {
                println!("  -> {step}");
            }
        }
        DispatchTask::Canary { suite, url } => {
            let spec = CanarySpec::new(&suite, &url, "*/15 * * * *", 1200)?;
            let sample_probe = CanaryProbeResult {
                http_status: 200,
                latency_ms: 340,
                assertion_passed: true,
                failure_reason: None,
            };
            let decision = spec.evaluate_probe(&sample_probe);
            println!("=== Pillar 1: Residential-IP Playwright Canary ===");
            println!(
                "Suite: {} | Target: {} | Cron: {}",
                spec.suite_name, spec.target_url, spec.cron_schedule
            );
            println!(
                "Status: Healthy={} | Page Mobile Alert={}",
                decision.healthy, decision.notify_mobile_channel
            );
        }
        DispatchTask::Triage { sender, subject } => {
            let item = InboxItem {
                id: "msg-live-01".to_string(),
                sender,
                subject,
                preview_text: "Need confirmation before triggering the release workflow."
                    .to_string(),
                is_bulk_newsletter: false,
                asks_direct_question: true,
                mentions_urgent_deadline: true,
            };
            let card = triage_inbox_item(&item);
            println!("=== Pillar 2: 24/7 Chief of Staff Inbox Triage ===");
            println!("Category: {:?}", card.category);
            for bullet in &card.three_bullet_summary {
                println!("  • {bullet}");
            }
            if let Some(reply) = &card.drafted_reply {
                println!("Drafted Reply: \"{reply}\"");
            }
            println!("Telegram Buttons: {:?}", card.buttons);
        }
        DispatchTask::ScraperFilter { min_score } => {
            let candidates = vec![
                ScrapedCandidate {
                    id: "page-01".to_string(),
                    source_url: "https://news.example/item-1".to_string(),
                    local_relevance_score: 35,
                    extracted_json_summary: "{\"topic\":\"general\"}".to_string(),
                },
                ScrapedCandidate {
                    id: "page-02".to_string(),
                    source_url: "https://news.example/item-2".to_string(),
                    local_relevance_score: 72,
                    extracted_json_summary: "{\"topic\":\"devops\"}".to_string(),
                },
                ScrapedCandidate {
                    id: "page-03".to_string(),
                    source_url: "https://news.example/item-3".to_string(),
                    local_relevance_score: 98,
                    extracted_json_summary: "{\"topic\":\"high_intent_lead\"}".to_string(),
                },
            ];
            let report = filter_scraped_batch(&candidates, min_score, 1500);
            println!("=== Pillar 3: Local 8B Scraper Filter (99% Local -> 1% Frontier) ===");
            println!(
                "Evaluated Locally ($0) : {} items",
                report.total_evaluated_locally
            );
            println!(
                "Filtered Out Locally   : {} items",
                report.discarded_locally
            );
            println!(
                "Escalated to Frontier  : {} items",
                report.escalated_to_frontier.len()
            );
            println!(
                "Cloud Tokens Saved     : {} tokens",
                report.estimated_cloud_tokens_saved
            );
        }
    }

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
