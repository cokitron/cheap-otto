#![forbid(unsafe_code)]

mod brains;
mod config;
mod diff;
mod drive;
mod github;
mod m1;
mod server;
mod state;
mod tender;
mod ui_html;

use crate::brains::{
    ADVERSARIAL_BRAIN, RunnerPair, SPECIALIST_BRAINS, append_kb_entry, checkout_pinned_worktree,
    execute_initial_full_review, execute_stateful_re_review, is_runner_authenticated,
    resolve_active_runners, resolve_intercalated_runners, update_living_docs_on_merge,
};
use crate::config::{
    AccountPolicy, CliRunner, ReviewEventPolicy, WatchedRepoConfig, WorkspaceConfig, expand_tilde,
    home_dir, repo_fs_slug,
};
use crate::diff::ParsedDiffMap;
use crate::drive::{resolve_plugin_cli, sync_living_docs_to_drive, sync_pr_review_artifacts};
use crate::github::{AtomicPrReviewRequest, DetailedOpenPr, GitHubClient};
use crate::m1::{
    DEFAULT_M1_HOST, DEFAULT_M1_REPO_PATH, M1SyncOptions, kickstart_m1_poller,
    print_m1_live_status, push_bytes_to_m1, run_ssh_script, sync_everything_to_m1,
    sync_github_tokens_to_m1, sync_m1_reviews_to_drive,
};
use crate::server::{DEFAULT_UI_BIND, DEFAULT_UI_PORT, open_tunneled_ui, run_ui_server};
use crate::state::{
    ActiveTenderWorkerState, ActiveWorkerState, CompletedBannerOutcome, LedgerStore,
    StatusBannerContext, TenderPrSnapshot, TenderPrState, TenderTriggerDecision, TrackedPrState,
    TriggerDecision, build_github_review_artifacts, evaluate_pr_trigger, evaluate_tender_trigger,
    format_completed_status_banner, format_interrupted_status_banner, format_queued_status_banner,
    format_running_status_banner, format_superseded_status_banner, is_non_actionable_tender_item,
    is_process_alive_str,
};
use crate::tender::{execute_tender_pr_cycle, sync_watched_prs_from_snapshots_with_gh};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Parser)]
#[command(
    name = "code-reviewer",
    version,
    about = "Autonomous multi-lens Internal CI code reviewer for Claude Code & Antigravity"
)]
struct Cli {
    /// Workspace root containing `config/repos.toml`, `brains/`, and `living-docs/`.
    #[arg(long, global = true, default_value = ".")]
    workspace: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Check GitHub dual-account auth, Claude/Antigravity CLI auth, Google Drive auth, and brain files.
    Doctor,
    /// Inspect or modify the watched repository registry (`config/repos.toml`).
    Watch {
        #[command(subcommand)]
        action: WatchAction,
    },
    /// Poll all enabled watched repositories and execute pending initial reviews, push/reply re-reviews, and post-merge Living Docs updates.
    Poll {
        /// Preview trigger decisions without invoking LLMs or posting to GitHub/Drive.
        #[arg(long)]
        dry_run: bool,
        /// Optional filter to poll only a single `<owner>/<repo>`.
        #[arg(long)]
        repo: Option<String>,
    },
    /// Run or dry-run a review on a specific Pull Request immediately in an isolated worker session.
    Review {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        pr: u64,
        #[arg(long)]
        dry_run: bool,
        /// Optional preferred lead runner (`claude` or `agy`) for intercalated load splitting.
        #[arg(long)]
        lead_runner: Option<String>,
    },
    /// Poll all enabled repositories for unhandled PR code reviews (0 AI tokens when idle) and dispatch Code Tender async workers.
    TendPoll {
        /// Preview tender trigger decisions without invoking LLMs or pushing commits.
        #[arg(long)]
        dry_run: bool,
        /// Optional filter to poll only a single `<owner>/<repo>`.
        #[arg(long)]
        repo: Option<String>,
    },
    /// Handle PR review feedback for a specific Pull Request immediately in an isolated writable Code Tender worker session.
    Tend {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        pr: u64,
        #[arg(long)]
        dry_run: bool,
        /// Optional preferred lead runner (`claude` or `agy`) for intercalated load splitting.
        #[arg(long)]
        lead_runner: Option<String>,
        /// Stage the generated patch and draft replies in the Code Tender UI instead of pushing directly.
        #[arg(long)]
        stage_only: bool,
    },
    /// Publish/refresh a repository's Living Documentation bundle in Google Drive (`00 Living Docs`).
    SyncDocs {
        #[arg(long)]
        repo: String,
    },
    /// Add a lesson or false-positive suppression guardrail to a specific review brain's Knowledge Base.
    Kb {
        #[command(subcommand)]
        action: KbAction,
    },
    /// Print current PR tracking ledger, verdicts, open observation counts, and Google Drive Doc links.
    Status,
    /// Start the local Web Dashboard HTTP/JSON server (`127.0.0.1:7430`).
    Serve {
        #[arg(long, default_value = DEFAULT_UI_BIND)]
        bind: String,
    },
    /// Ensure the M1 Web Dashboard server is running, open an SSH tunnel (`127.0.0.1:7430`), and launch the GUI in your browser.
    Ui {
        #[arg(long, default_value = DEFAULT_M1_HOST)]
        host: String,
        #[arg(long, default_value = DEFAULT_M1_REPO_PATH)]
        remote_repo: String,
        #[arg(long, default_value_t = DEFAULT_UI_PORT)]
        port: u16,
        #[arg(long)]
        no_open: bool,
    },
    /// Remotely update, provision, push files/packages/tokens, bridge Google Drive, or inspect the headless M1 (`otto-mac`).
    M1 {
        #[arg(long, default_value = DEFAULT_M1_HOST)]
        host: String,
        #[arg(long, default_value = DEFAULT_M1_REPO_PATH)]
        remote_repo: String,
        #[command(subcommand)]
        action: M1Action,
    },
}

#[derive(Debug, Subcommand)]
enum M1Action {
    /// Push workspace git HEAD, compile & install release binary, sync global skills/symlinks/launchd, and optionally sync tokens/Drive/packages.
    Sync {
        #[arg(long)]
        sync_tokens: bool,
        #[arg(long)]
        sync_drive: bool,
        /// Comma-separated Homebrew formulas to ensure installed on M1 (e.g. `jq,ripgrep`).
        #[arg(long, default_value = "")]
        brew: String,
        /// Comma-separated Cargo crates to ensure installed on M1.
        #[arg(long, default_value = "")]
        cargo: String,
        /// Comma-separated global npm packages to install (`--ignore-scripts`) on M1.
        #[arg(long, default_value = "")]
        npm: String,
    },
    /// Show live M1 AC/thermal telemetry, running brain PID, launchd state, PR ledger, and poller log tail.
    Status,
    /// Refresh isolated `primary` & `secondary` GitHub tokens on M1 from local `gh auth token`.
    SyncTokens,
    /// Pull M1 review outputs (`observations.json`), upload/update Google Docs via local `gdrive`+`gdocs`, and write links back to M1.
    SyncDrive,
    /// Atomically copy any local file to an arbitrary path on M1 (`~/` supported) with exact octal permissions.
    PushFile {
        #[arg(long)]
        local: PathBuf,
        #[arg(long)]
        remote: String,
        #[arg(long, default_value = "644")]
        mode: String,
    },
    /// Run any shell command safely on M1 over SSH stdin (`sh -s`) with full Homebrew/Cargo/Antigravity PATH.
    Exec {
        #[arg(long)]
        cmd: String,
    },
    /// Trigger an immediate `launchd` poll tick on M1 (safely skipped if a review is already running).
    Kickstart,
}

#[derive(Debug, Subcommand)]
enum WatchAction {
    List,
    Add {
        /// Repository slug `<owner>/<repo>`.
        full_name: String,
        #[arg(long, default_value = "main")]
        default_branch: String,
        /// `swap` (primary <-> secondary) or `forced`.
        #[arg(long, default_value = "swap")]
        account_policy: String,
        #[arg(long)]
        forced_account: Option<String>,
        /// `comment_only` or `formal_verdict`.
        #[arg(long, default_value = "formal_verdict")]
        event_policy: String,
        #[arg(long)]
        gdrive_folder_id: Option<String>,
        #[arg(long)]
        local_repo_path: Option<String>,
        #[arg(long)]
        local_check_cmd: Option<String>,
        #[arg(long)]
        no_review: bool,
        #[arg(long)]
        no_tender: bool,
    },
    Remove {
        full_name: String,
    },
}

#[derive(Debug, Subcommand)]
enum KbAction {
    Add {
        /// Brain folder name (e.g. `02-correctness-domain`, `06-ux-a11y-frontend`).
        #[arg(long)]
        brain: String,
        /// `lesson` or `false-positive`.
        #[arg(long, default_value = "lesson")]
        kind: String,
        /// Imperative rule text to append.
        #[arg(long)]
        rule: String,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let raw_workspace = cli.workspace.canonicalize().unwrap_or(cli.workspace);
    let workspace = if raw_workspace.join("brains").is_dir() {
        raw_workspace
    } else if raw_workspace.join("crates/code-reviewer/brains").is_dir() {
        raw_workspace.join("crates/code-reviewer")
    } else {
        raw_workspace
    };
    let config_path = workspace.join("config").join("repos.toml");

    match cli.command {
        Commands::Doctor => run_doctor(&workspace, &config_path)?,
        Commands::Watch { action } => run_watch(&config_path, action)?,
        Commands::Poll { dry_run, repo } => {
            run_poll(&workspace, &config_path, dry_run, repo.as_deref())?
        }
        Commands::Review {
            repo,
            pr,
            dry_run,
            lead_runner,
        } => run_single_pr_review(
            &workspace,
            &config_path,
            &repo,
            pr,
            dry_run,
            lead_runner.as_deref(),
        )?,
        Commands::TendPoll { dry_run, repo } => {
            run_tender_poll(&workspace, &config_path, dry_run, repo.as_deref())?
        }
        Commands::Tend {
            repo,
            pr,
            dry_run,
            lead_runner,
            stage_only,
        } => run_single_pr_tend(
            &workspace,
            &config_path,
            &repo,
            pr,
            dry_run,
            lead_runner.as_deref(),
            stage_only,
        )?,
        Commands::SyncDocs { repo } => run_sync_docs(&workspace, &config_path, &repo)?,
        Commands::Kb { action } => match action {
            KbAction::Add { brain, kind, rule } => {
                let p = append_kb_entry(&workspace, &brain, &kind, &rule)?;
                println!("Added {} entry to {}", kind, p.display());
            }
        },
        Commands::Status => run_status(&config_path)?,
        Commands::Serve { bind } => run_ui_server(&workspace, &config_path, &bind)?,
        Commands::Ui {
            host,
            remote_repo,
            port,
            no_open,
        } => open_tunneled_ui(&workspace, &host, &remote_repo, port, no_open)?,
        Commands::M1 {
            host,
            remote_repo,
            action,
        } => run_m1_command(&workspace, &config_path, &host, &remote_repo, action)?,
    }

    Ok(())
}

fn run_m1_command(
    workspace: &Path,
    config_path: &Path,
    host: &str,
    remote_repo: &str,
    action: M1Action,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        M1Action::Sync {
            sync_tokens,
            sync_drive,
            brew,
            cargo,
            npm,
        } => {
            let cfg = WorkspaceConfig::load(config_path)?;
            sync_everything_to_m1(
                workspace,
                &cfg,
                &M1SyncOptions {
                    host,
                    remote_repo_path: remote_repo,
                    sync_tokens,
                    sync_drive,
                    brew_pkgs: &brew,
                    cargo_pkgs: &cargo,
                    npm_pkgs: &npm,
                },
            )?;
        }
        M1Action::Status => {
            print_m1_live_status(host, remote_repo)?;
        }
        M1Action::SyncTokens => {
            let cfg = WorkspaceConfig::load(config_path)?;
            sync_github_tokens_to_m1(&cfg, host)?;
        }
        M1Action::SyncDrive => {
            let cfg = WorkspaceConfig::load(config_path)?;
            sync_m1_reviews_to_drive(workspace, &cfg, host, remote_repo)?;
        }
        M1Action::PushFile {
            local,
            remote,
            mode,
        } => {
            let bytes = fs::read(&local)?;
            push_bytes_to_m1(host, &remote, &bytes, &mode)?;
            println!(
                "[m1-push-file] Copied {} ({} bytes, mode={}) -> {}:{}",
                local.display(),
                bytes.len(),
                mode,
                host,
                remote
            );
        }
        M1Action::Exec { cmd } => {
            let script = format!(
                "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:/usr/bin:/bin:/usr/sbin:/sbin\"\n{}\n",
                cmd
            );
            let out = run_ssh_script(host, &script)?;
            print!("{}", out);
        }
        M1Action::Kickstart => {
            kickstart_m1_poller(host)?;
        }
    }
    Ok(())
}

fn run_doctor(workspace: &Path, config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = WorkspaceConfig::load(config_path)?;
    let gh = GitHubClient::new(&cfg);

    println!("=== code-reviewer Appliance Doctor ===");
    println!("Workspace : {}", workspace.display());
    println!("State Dir : {}", expand_tilde(&cfg.state_dir).display());

    // 1. GitHub accounts
    for acct in [&cfg.accounts.primary, &cfg.accounts.secondary] {
        match gh.verify_account(acct) {
            Ok(login) => println!(
                "[PASS] GitHub Account `{}` authenticated (login: {})",
                acct, login
            ),
            Err(e) => println!("[FAIL] GitHub Account `{}`: {}", acct, e),
        }
    }

    // 2. Claude Code + Antigravity CLIs
    for runner in [CliRunner::Claude, CliRunner::Agy] {
        if is_runner_authenticated(runner) {
            println!(
                "[PASS] Agent Runner `{}` authenticated & headless-ready",
                runner.slug()
            );
        } else {
            println!(
                "[WARN] Agent Runner `{}` requires interactive sign-in on this host",
                runner.slug()
            );
        }
    }

    // 3. Google Drive + Google Docs CLIs
    let gdrive_bin = resolve_plugin_cli(&cfg.gdrive.gdrive_bin, "gdrive");
    let gdrive_ok = Command::new(&gdrive_bin)
        .args(["readonly", "ls", &cfg.gdrive.root_folder_id, "--max", "1"])
        .stdin(Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if gdrive_ok {
        println!(
            "[PASS] Google Drive CLI (`{}`) authenticated & folder `{}` reachable",
            gdrive_bin.display(),
            cfg.gdrive.root_folder_id
        );
    } else {
        println!(
            "[WARN] Google Drive CLI (`{}`) needs one-time OAuth on this host (`{} auth login` and `{} auth login`)",
            gdrive_bin.display(),
            gdrive_bin.display(),
            resolve_plugin_cli(&cfg.gdrive.gdocs_bin, "gdocs").display()
        );
    }

    // 4. Brain definitions & KBs
    let mut all_brains_ok = true;
    for brain in SPECIALIST_BRAINS
        .iter()
        .copied()
        .chain(std::iter::once(ADVERSARIAL_BRAIN))
    {
        let p = workspace.join("brains").join(brain).join("PROMPT.md");
        if !p.exists() {
            println!("[FAIL] Missing brain prompt: {}", p.display());
            all_brains_ok = false;
        }
    }
    if all_brains_ok {
        println!(
            "[PASS] All {} review brains + isolated KBs verified",
            SPECIALIST_BRAINS.len() + 1
        );
    }

    // 5. Watched repositories poll check
    for repo in cfg.repos.iter().filter(|r| r.enabled) {
        let probe_acct = repo
            .forced_account
            .as_deref()
            .unwrap_or(cfg.accounts.primary.as_str());
        match gh.poll_repository(probe_acct, &repo.full_name) {
            Ok(batch) => println!(
                "[PASS] Watched Repo `{}` reachable via `{}` (open PRs: {}, recent merged: {})",
                repo.full_name,
                probe_acct,
                batch.open_prs.len(),
                batch.merged_prs.len()
            ),
            Err(e) => println!(
                "[FAIL] Watched Repo `{}` via `{}`: {}",
                repo.full_name, probe_acct, e
            ),
        }
    }

    Ok(())
}

fn run_watch(config_path: &Path, action: WatchAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = WorkspaceConfig::load(config_path)?;
    match action {
        WatchAction::List => {
            for r in &cfg.repos {
                println!(
                    "- {} [enabled={}] branch={} account_policy={:?} forced_account={:?} event_policy={:?} drive_folder={:?}",
                    r.full_name,
                    r.enabled,
                    r.default_branch,
                    r.account_policy,
                    r.forced_account,
                    r.review_event_policy,
                    r.gdrive_folder_id
                );
            }
        }
        WatchAction::Add {
            full_name,
            default_branch,
            account_policy,
            forced_account,
            event_policy,
            gdrive_folder_id,
            local_repo_path,
            local_check_cmd,
            no_review,
            no_tender,
        } => {
            let acct_pol = if account_policy.eq_ignore_ascii_case("forced") {
                AccountPolicy::Forced
            } else {
                AccountPolicy::Swap
            };
            let ev_pol = if event_policy.eq_ignore_ascii_case("comment_only") {
                ReviewEventPolicy::CommentOnly
            } else {
                ReviewEventPolicy::FormalVerdict
            };
            cfg.upsert_repo(WatchedRepoConfig {
                full_name: full_name.clone(),
                default_branch,
                language: "en".to_string(),
                account_policy: acct_pol,
                forced_account,
                review_event_policy: ev_pol,
                gdrive_folder_id,
                local_repo_path,
                local_check_cmd,
                enabled: !no_review,
                tender_enabled: !no_tender,
            })?;
            cfg.save(config_path)?;
            println!("Registered watched repository `{}`.", full_name);
        }
        WatchAction::Remove { full_name } => {
            if cfg.remove_repo(&full_name) {
                cfg.save(config_path)?;
                println!("Removed `{}` from watched repositories.", full_name);
            } else {
                println!("Repository `{}` was not in the watch list.", full_name);
            }
        }
    }
    Ok(())
}

struct PollLockGuard {
    lock_path: PathBuf,
}

impl PollLockGuard {
    fn try_acquire(state_dir: &Path) -> Result<Option<Self>, std::io::Error> {
        try_acquire_named_lock(state_dir, "poll.lock")
    }
}

impl Drop for PollLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

struct WorkerCleanupGuard {
    state_dir: PathBuf,
    repo: String,
    pr_number: u64,
}

impl Drop for WorkerCleanupGuard {
    fn drop(&mut self) {
        ActiveWorkerState::remove(&self.state_dir, &self.repo, self.pr_number);
    }
}

fn run_poll(
    workspace: &Path,
    config_path: &Path,
    dry_run: bool,
    only_repo: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = WorkspaceConfig::load(config_path)?;
    let state_dir = expand_tilde(&cfg.state_dir);
    let _poll_lock = if !dry_run {
        match PollLockGuard::try_acquire(&state_dir)? {
            Some(guard) => Some(guard),
            None => {
                println!(
                    "[SKIP-ACTIVE] Another `code-reviewer poll` dispatch pass is active (lock: {}).",
                    state_dir.join("poll.lock").display()
                );
                return Ok(());
            }
        }
    } else {
        None
    };

    let gh = GitHubClient::new(&cfg);
    let max_parallel = cfg.agents.max_parallel_prs.clamp(1, 12);
    let mut active_workers_count = ActiveWorkerState::list_active(&state_dir).len();

    for repo_cfg in cfg.repos.iter().filter(|r| r.enabled) {
        if let Some(filter) = only_repo
            && !repo_cfg.full_name.eq_ignore_ascii_case(filter)
        {
            continue;
        }

        let poll_account = repo_cfg
            .forced_account
            .as_deref()
            .unwrap_or(cfg.accounts.primary.as_str());
        let batch = gh.poll_repository(poll_account, &repo_cfg.full_name)?;

        // Seed merged cursor atomically on first-ever poll so historical merges aren't replayed retroactively.
        if !dry_run {
            let latest_merge_ts = batch.merged_prs.last().map(|m| m.merged_at.clone());
            LedgerStore::with_exclusive_lock(&state_dir, |store| {
                let cursor = store
                    .repo_cursors
                    .entry(repo_cfg.full_name.clone())
                    .or_default();
                cursor.last_poll_at = Some(Utc::now());
                if cursor.last_merged_at_cursor.is_none()
                    && let Some(ts) = latest_merge_ts
                {
                    cursor.last_merged_at_cursor = Some(ts);
                }
            })?;
        }

        let ledger_snapshot = LedgerStore::load_or_default(&state_dir)?;

        // 1. Evaluate open PRs & dispatch independent async PR worker sessions in parallel (intercalating claude <-> agy)
        let mut overflow_queue_index = 0usize;
        for pr_detail in &batch.open_prs {
            if let Some(running) = ActiveWorkerState::is_pr_worker_running(
                &state_dir,
                &repo_cfg.full_name,
                pr_detail.snapshot.pr_number,
            ) {
                println!(
                    "[IN-FLIGHT] {}#{} already running in async session PID {} (phase={}, runners={} ⇄ {})",
                    repo_cfg.full_name,
                    pr_detail.snapshot.pr_number,
                    running.pid,
                    running.phase,
                    running.lead_runner,
                    running.alt_runner
                );
                continue;
            }

            let key = LedgerStore::pr_key(&repo_cfg.full_name, pr_detail.snapshot.pr_number);
            let decision =
                evaluate_pr_trigger(ledger_snapshot.pull_requests.get(&key), &pr_detail.snapshot);
            let reviewer = cfg.select_reviewer_account(
                repo_cfg,
                &pr_detail.snapshot.author,
                pr_detail.snapshot.latest_committer.as_deref(),
            );
            let gh_event = WorkspaceConfig::select_github_review_event(
                repo_cfg,
                reviewer,
                &pr_detail.snapshot.author,
                true,
            );

            match &decision {
                TriggerDecision::SkipDraft => {
                    println!(
                        "[SKIP-DRAFT] {}#{} ({})",
                        repo_cfg.full_name, pr_detail.snapshot.pr_number, pr_detail.snapshot.title
                    );
                }
                TriggerDecision::UpToDate => {
                    println!(
                        "[UP-TO-DATE] {}#{} @ {}",
                        repo_cfg.full_name,
                        pr_detail.snapshot.pr_number,
                        &pr_detail.snapshot.head_sha[..pr_detail.snapshot.head_sha.len().min(7)]
                    );
                }
                TriggerDecision::InitialFullReview { head_sha } => {
                    println!(
                        "[TRIGGER:INITIAL_FULL] {}#{} @ {} (reviewer=`{}`, sample_event_if_blocking=`{}`)",
                        repo_cfg.full_name,
                        pr_detail.snapshot.pr_number,
                        &head_sha[..head_sha.len().min(7)],
                        reviewer,
                        gh_event
                    );
                    if !dry_run {
                        dispatch_or_queue_pr_session(
                            workspace,
                            &cfg,
                            repo_cfg,
                            &state_dir,
                            &gh,
                            pr_detail,
                            reviewer,
                            &decision,
                            max_parallel,
                            &mut active_workers_count,
                            &mut overflow_queue_index,
                        )?;
                    }
                }
                TriggerDecision::ReReview {
                    head_sha,
                    sha_changed,
                    new_replies,
                    ..
                } => {
                    println!(
                        "[TRIGGER:RE_REVIEW] {}#{} @ {} (sha_changed={}, new_replies={}, reviewer=`{}`)",
                        repo_cfg.full_name,
                        pr_detail.snapshot.pr_number,
                        &head_sha[..head_sha.len().min(7)],
                        sha_changed,
                        new_replies.len(),
                        reviewer
                    );
                    if !dry_run {
                        dispatch_or_queue_pr_session(
                            workspace,
                            &cfg,
                            repo_cfg,
                            &state_dir,
                            &gh,
                            pr_detail,
                            reviewer,
                            &decision,
                            max_parallel,
                            &mut active_workers_count,
                            &mut overflow_queue_index,
                        )?;
                    }
                }
            }
        }

        // 2. Evaluate newly merged PRs for Living Docs + KB updates
        let merged_cursor = ledger_snapshot
            .repo_cursors
            .get(&repo_cfg.full_name)
            .and_then(|c| c.last_merged_at_cursor.clone())
            .unwrap_or_default();

        for merged in &batch.merged_prs {
            if merged.merged_at <= merged_cursor {
                continue;
            }
            if cfg.agents.poll_only_mode {
                println!(
                    "[POLL-ONLY-NO-AI] {}#{} merged at {} — deferring AI Living Docs update while poll_only_mode=true",
                    repo_cfg.full_name, merged.pr_number, merged.merged_at
                );
                continue;
            }
            println!(
                "[TRIGGER:POST_MERGE_DOCS] {}#{} merged at {} ({})",
                repo_cfg.full_name,
                merged.pr_number,
                merged.merged_at,
                &merged.merge_commit_sha[..merged.merge_commit_sha.len().min(7)]
            );
            if !dry_run {
                if let Err(e) = resolve_active_runners(&cfg) {
                    println!(
                        "[WAIT-AUTH] Deferring post-merge Living Docs update for {}#{} ({})",
                        repo_cfg.full_name, merged.pr_number, e
                    );
                    continue;
                }
                let token = gh.resolve_token(poll_account)?;
                let wt = checkout_pinned_worktree(
                    &state_dir,
                    &repo_cfg.full_name,
                    &merged.merge_commit_sha,
                    Some(merged.pr_number),
                    &token,
                )?;
                update_living_docs_on_merge(
                    workspace,
                    &cfg,
                    &wt.path,
                    &repo_cfg.full_name,
                    merged,
                )?;
                let existing_doc_id = ledger_snapshot
                    .repo_cursors
                    .get(&repo_cfg.full_name)
                    .and_then(|c| c.living_doc_gdrive_id.clone());
                let synced_drive = sync_living_docs_to_drive(
                    workspace,
                    &cfg,
                    &repo_cfg.full_name,
                    existing_doc_id.as_deref(),
                )
                .ok();
                LedgerStore::with_exclusive_lock(&state_dir, |store| {
                    let cur = store
                        .repo_cursors
                        .entry(repo_cfg.full_name.clone())
                        .or_default();
                    if let Some((doc_id, doc_url)) = synced_drive {
                        cur.living_doc_gdrive_id = Some(doc_id);
                        cur.living_doc_gdrive_url = Some(doc_url);
                    }
                    cur.last_merged_at_cursor = Some(merged.merged_at.clone());
                })?;
            }
        }
    }

    Ok(())
}

/// Launches an independent detached OS session for a triggered PR (intercalating `claude` <-> `agy`
/// across concurrent sessions) up to `max_parallel_prs`, or posts an overflow status banner if all
/// parallel slots are currently busy.
#[allow(clippy::too_many_arguments)]
fn dispatch_or_queue_pr_session(
    workspace: &Path,
    cfg: &WorkspaceConfig,
    repo_cfg: &WatchedRepoConfig,
    state_dir: &Path,
    gh: &GitHubClient,
    pr_detail: &DetailedOpenPr,
    reviewer: &str,
    decision: &TriggerDecision,
    max_parallel: usize,
    active_workers_count: &mut usize,
    overflow_queue_index: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if cfg.agents.poll_only_mode {
        println!(
            "[POLL-ONLY-NO-AI] {}#{} @ {} needs review, but `poll_only_mode = true` (0 AI tokens consumed).",
            repo_cfg.full_name,
            pr_detail.snapshot.pr_number,
            &pr_detail.snapshot.head_sha[..pr_detail.snapshot.head_sha.len().min(7)]
        );
        return Ok(());
    }

    let latest_reply_ts = pr_detail
        .snapshot
        .human_replies
        .iter()
        .filter(|r| !r.body.contains(crate::state::BOT_COMMENT_MARKER))
        .map(|r| r.created_at.as_str())
        .max()
        .unwrap_or("");
    let attempt_key = format!("{}:{}", pr_detail.snapshot.head_sha, latest_reply_ts);
    let pr_key = LedgerStore::pr_key(&repo_cfg.full_name, pr_detail.snapshot.pr_number);

    // Token cost circuit-breaker: never auto-dispatch AI more than twice for the exact same (SHA + reply_cursor).
    let existing_attempts = LedgerStore::load_or_default(state_dir)?
        .pull_requests
        .get(&pr_key)
        .and_then(|t| {
            if t.last_attempt_key.as_deref() == Some(attempt_key.as_str()) {
                Some(t.attempts_for_key)
            } else {
                None
            }
        })
        .unwrap_or(0);
    if existing_attempts >= 2 {
        println!(
            "[CIRCUIT-BREAKER:NO-AI] {}#{} @ {} hit {} prior incomplete attempts for this SHA/comment cursor; halting auto-AI retries (0 tokens) until a new push/comment or manual UI trigger.",
            repo_cfg.full_name,
            pr_detail.snapshot.pr_number,
            &pr_detail.snapshot.head_sha[..pr_detail.snapshot.head_sha.len().min(7)],
            existing_attempts
        );
        return Ok(());
    }

    if let Err(e) = resolve_active_runners(cfg) {
        println!(
            "[WAIT-AUTH] Deferring {}#{} until agent CLI sign-in completes ({})",
            repo_cfg.full_name, pr_detail.snapshot.pr_number, e
        );
        return Ok(());
    }

    if *active_workers_count < max_parallel {
        let slot = LedgerStore::with_exclusive_lock(state_dir, |store| {
            let s = store.provider_dispatch_counter;
            store.provider_dispatch_counter = store.provider_dispatch_counter.wrapping_add(1);
            let tracked = store
                .pull_requests
                .entry(pr_key.clone())
                .or_insert_with(|| {
                    TrackedPrState::new(
                        &repo_cfg.full_name,
                        pr_detail.snapshot.pr_number,
                        &pr_detail.snapshot.title,
                        &pr_detail.snapshot.author,
                        &pr_detail.snapshot.base_ref,
                        &pr_detail.snapshot.head_ref,
                    )
                });
            if tracked.last_attempt_key.as_deref() == Some(attempt_key.as_str()) {
                tracked.attempts_for_key = tracked.attempts_for_key.saturating_add(1);
            } else {
                tracked.last_attempt_key = Some(attempt_key.clone());
                tracked.attempts_for_key = 1;
            }
            s
        })?;
        let preferred_lead = if cfg.agents.intercalate_providers {
            if slot.is_multiple_of(2) {
                CliRunner::Claude
            } else {
                CliRunner::Agy
            }
        } else {
            cfg.agents.primary_runner
        };
        let runners = resolve_intercalated_runners(cfg, Some(preferred_lead))?;
        let child_pid = spawn_async_pr_worker(
            workspace,
            state_dir,
            &repo_cfg.full_name,
            pr_detail,
            reviewer,
            decision,
            runners,
        )?;
        *active_workers_count += 1;
        println!(
            "[DISPATCHED-ASYNC] {}#{} -> independent session PID {} (runners=`{}`, active={}/{})",
            repo_cfg.full_name,
            pr_detail.snapshot.pr_number,
            child_pid,
            runners.label(),
            *active_workers_count,
            max_parallel
        );
    } else {
        *overflow_queue_index += 1;
        announce_overflow_queued_pr(
            repo_cfg,
            state_dir,
            gh,
            pr_detail,
            reviewer,
            decision,
            *overflow_queue_index,
            max_parallel,
        )?;
    }

    Ok(())
}

fn spawn_async_pr_worker(
    workspace: &Path,
    state_dir: &Path,
    repo_full_name: &str,
    pr_detail: &DetailedOpenPr,
    reviewer: &str,
    decision: &TriggerDecision,
    runners: RunnerPair,
) -> Result<u32, Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let log_dir = home_dir().join(".cheap-otto/logs/code-reviewer");
    fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join(format!(
        "pr-{}-{}.log",
        repo_fs_slug(repo_full_name),
        pr_detail.snapshot.pr_number
    ));
    let out_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err_file = out_file.try_clone()?;

    let mut cmd = Command::new(exe);
    cmd.current_dir(workspace)
        .arg("--workspace")
        .arg(workspace)
        .arg("review")
        .arg("--repo")
        .arg(repo_full_name)
        .arg("--pr")
        .arg(pr_detail.snapshot.pr_number.to_string())
        .arg("--lead-runner")
        .arg(runners.lead.slug())
        .stdin(Stdio::null())
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn()?;

    let pid = child.id();
    let now = Utc::now().to_rfc3339();
    let mode_short = match decision {
        TriggerDecision::InitialFullReview { .. } => "INITIAL_FULL",
        TriggerDecision::ReReview { .. } => "RE_REVIEW",
        _ => "REVIEW",
    };
    let mut initial_worker = ActiveWorkerState {
        pid,
        repo: repo_full_name.to_string(),
        pr_number: pr_detail.snapshot.pr_number,
        title: pr_detail.snapshot.title.clone(),
        head_sha: pr_detail.snapshot.head_sha.clone(),
        mode: mode_short.to_string(),
        reviewer_account: reviewer.to_string(),
        lead_runner: runners.lead.slug().to_string(),
        alt_runner: runners.alt.slug().to_string(),
        phase: "spawning".to_string(),
        started_at_utc: now.clone(),
        updated_at_utc: now,
        brains: BTreeMap::new(),
    };
    initial_worker.save(state_dir);

    Ok(pid)
}

#[allow(clippy::too_many_arguments)]
fn announce_overflow_queued_pr(
    repo_cfg: &WatchedRepoConfig,
    state_dir: &Path,
    gh: &GitHubClient,
    pr_detail: &DetailedOpenPr,
    reviewer_account: &str,
    decision: &TriggerDecision,
    overflow_pos: usize,
    max_parallel: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let key = LedgerStore::pr_key(&repo_cfg.full_name, pr_detail.snapshot.pr_number);
    let latest_reply_ts = pr_detail
        .snapshot
        .human_replies
        .iter()
        .map(|r| r.created_at.as_str())
        .max()
        .unwrap_or("");
    let dedup_key = format!("{}:{}", pr_detail.snapshot.head_sha, latest_reply_ts);

    let ledger_snap = LedgerStore::load_or_default(state_dir)?;
    if let Some(existing) = ledger_snap.pull_requests.get(&key)
        && existing.active_status_comment_id.is_some()
        && existing.active_status_key.as_deref() == Some(dedup_key.as_str())
    {
        return Ok(());
    }

    let upcoming_run = ledger_snap
        .pull_requests
        .get(&key)
        .map(|t| t.run_count + 1)
        .unwrap_or(1);
    let mode_label = match decision {
        TriggerDecision::InitialFullReview { .. } => {
            "INITIAL_FULL — 7 specialist lenses + adversarial verifier"
        }
        TriggerDecision::ReReview { .. } => "RE_REVIEW — stateful compliance & reply adjudication",
        _ => "REVIEW",
    };
    let queued_at = Utc::now().to_rfc3339();
    let ctx = StatusBannerContext {
        run_number: upcoming_run,
        mode_label,
        head_sha: &pr_detail.snapshot.head_sha,
        reviewer_account,
        runners_label: "claude ⇄ agy",
        started_at_utc: &queued_at,
    };
    let queued_body = format_queued_status_banner(&ctx, overflow_pos, max_parallel + overflow_pos);
    if let Ok(posted) = gh.create_issue_comment(
        reviewer_account,
        &repo_cfg.full_name,
        pr_detail.snapshot.pr_number,
        &queued_body,
    ) {
        println!(
            "[OVERFLOW-SLOT-WAIT] {}#{} (max_parallel_prs={} busy): {}",
            repo_cfg.full_name, pr_detail.snapshot.pr_number, max_parallel, posted.html_url
        );
        LedgerStore::with_exclusive_lock(state_dir, |store| {
            let tracked = store.pull_requests.entry(key).or_insert_with(|| {
                TrackedPrState::new(
                    &repo_cfg.full_name,
                    pr_detail.snapshot.pr_number,
                    &pr_detail.snapshot.title,
                    &pr_detail.snapshot.author,
                    &pr_detail.snapshot.base_ref,
                    &pr_detail.snapshot.head_ref,
                )
            });
            tracked.active_status_comment_id = Some(posted.id);
            tracked.active_status_key = Some(dedup_key);
        })?;
    }
    Ok(())
}

fn run_single_pr_review(
    workspace: &Path,
    config_path: &Path,
    repo_name: &str,
    pr_number: u64,
    dry_run: bool,
    lead_runner_arg: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = WorkspaceConfig::load(config_path)?;
    let repo_cfg = cfg
        .repos
        .iter()
        .find(|r| r.full_name.eq_ignore_ascii_case(repo_name))
        .ok_or_else(|| {
            format!(
                "repo `{}` is not registered in config/repos.toml",
                repo_name
            )
        })?;
    let state_dir = expand_tilde(&cfg.state_dir);

    if !dry_run
        && let Some(existing) =
            ActiveWorkerState::is_pr_worker_running(&state_dir, &repo_cfg.full_name, pr_number)
        && existing.pid != std::process::id()
    {
        println!(
            "[SKIP-IN-FLIGHT] {}#{} is already being reviewed by async worker PID {} (phase={}).",
            repo_cfg.full_name, pr_number, existing.pid, existing.phase
        );
        return Ok(());
    }

    let _worker_cleanup = if !dry_run {
        Some(WorkerCleanupGuard {
            state_dir: state_dir.clone(),
            repo: repo_cfg.full_name.clone(),
            pr_number,
        })
    } else {
        None
    };

    let gh = GitHubClient::new(&cfg);
    let poll_account = repo_cfg
        .forced_account
        .as_deref()
        .unwrap_or(cfg.accounts.primary.as_str());
    let batch = gh.poll_repository(poll_account, &repo_cfg.full_name)?;
    let pr_detail = batch
        .open_prs
        .iter()
        .find(|p| p.snapshot.pr_number == pr_number)
        .ok_or_else(|| format!("open PR #{} not found on {}", pr_number, repo_cfg.full_name))?;

    let ledger_snap = LedgerStore::load_or_default(&state_dir)?;
    let key = LedgerStore::pr_key(&repo_cfg.full_name, pr_number);
    let mut decision =
        evaluate_pr_trigger(ledger_snap.pull_requests.get(&key), &pr_detail.snapshot);
    if matches!(decision, TriggerDecision::UpToDate) {
        // Explicit manual `review` command forces a compliance re-check at current SHA.
        decision = TriggerDecision::ReReview {
            previous_sha: pr_detail.snapshot.head_sha.clone(),
            head_sha: pr_detail.snapshot.head_sha.clone(),
            sha_changed: false,
            new_replies: vec![],
        };
    }

    println!(
        "Decision for {}#{}: {:?}",
        repo_cfg.full_name, pr_number, decision
    );
    if !dry_run {
        let preferred_lead = lead_runner_arg.and_then(CliRunner::from_slug);
        let runners = resolve_intercalated_runners(&cfg, preferred_lead)?;
        let reviewer_account = cfg.select_reviewer_account(
            repo_cfg,
            &pr_detail.snapshot.author,
            pr_detail.snapshot.latest_committer.as_deref(),
        );
        let now = Utc::now().to_rfc3339();
        let mode_short = match &decision {
            TriggerDecision::InitialFullReview { .. } => "INITIAL_FULL",
            TriggerDecision::ReReview { .. } => "RE_REVIEW",
            _ => "REVIEW",
        };
        let mut init_state = ActiveWorkerState {
            pid: std::process::id(),
            repo: repo_cfg.full_name.clone(),
            pr_number,
            title: pr_detail.snapshot.title.clone(),
            head_sha: pr_detail.snapshot.head_sha.clone(),
            mode: mode_short.to_string(),
            reviewer_account: reviewer_account.to_string(),
            lead_runner: runners.lead.slug().to_string(),
            alt_runner: runners.alt.slug().to_string(),
            phase: "starting".to_string(),
            started_at_utc: now.clone(),
            updated_at_utc: now,
            brains: BTreeMap::new(),
        };
        init_state.save(&state_dir);
        let live_worker = Mutex::new(init_state);

        process_pr_decision(
            workspace,
            &cfg,
            repo_cfg,
            &state_dir,
            &gh,
            pr_detail,
            decision,
            runners,
            &live_worker,
        )?;
    }
    Ok(())
}

enum ReviewPipelineOutcome {
    Posted { review_url: String },
    DeferredMovedSha { live_head_sha: String },
}

#[allow(clippy::too_many_arguments)]
fn process_pr_decision(
    workspace: &Path,
    cfg: &WorkspaceConfig,
    repo_cfg: &WatchedRepoConfig,
    state_dir: &Path,
    gh: &GitHubClient,
    pr_detail: &DetailedOpenPr,
    decision: TriggerDecision,
    runners: RunnerPair,
    live_worker: &Mutex<ActiveWorkerState>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(
        decision,
        TriggerDecision::InitialFullReview { .. } | TriggerDecision::ReReview { .. }
    ) {
        return Ok(());
    }

    let reviewer_account = cfg.select_reviewer_account(
        repo_cfg,
        &pr_detail.snapshot.author,
        pr_detail.snapshot.latest_committer.as_deref(),
    );

    let key = LedgerStore::pr_key(&repo_cfg.full_name, pr_detail.snapshot.pr_number);
    let mut tracked = LedgerStore::load_or_default(state_dir)?
        .pull_requests
        .get(&key)
        .cloned()
        .unwrap_or_else(|| {
            TrackedPrState::new(
                &repo_cfg.full_name,
                pr_detail.snapshot.pr_number,
                &pr_detail.snapshot.title,
                &pr_detail.snapshot.author,
                &pr_detail.snapshot.base_ref,
                &pr_detail.snapshot.head_ref,
            )
        });
    tracked.title = pr_detail.snapshot.title.clone();
    tracked.head_ref = pr_detail.snapshot.head_ref.clone();
    tracked.base_ref = pr_detail.snapshot.base_ref.clone();

    let upcoming_run = tracked.run_count + 1;
    let mode_label = match &decision {
        TriggerDecision::InitialFullReview { .. } => {
            "INITIAL_FULL — 7 specialist lenses + adversarial verifier"
        }
        TriggerDecision::ReReview { .. } => "RE_REVIEW — stateful compliance & reply adjudication",
        _ => "REVIEW",
    };
    let latest_reply_ts = pr_detail
        .snapshot
        .human_replies
        .iter()
        .map(|r| r.created_at.as_str())
        .max()
        .unwrap_or("");
    let dedup_key = format!("{}:{}", pr_detail.snapshot.head_sha, latest_reply_ts);
    let reusing_banner = tracked.active_status_comment_id.is_some()
        && tracked.active_status_key.as_deref() == Some(dedup_key.as_str());
    let existing_banner_id = if reusing_banner {
        tracked.active_status_comment_id
    } else {
        None
    };

    let started_at_utc = Utc::now().to_rfc3339();
    let started_instant = Instant::now();
    let runners_label = runners.label();
    let banner_ctx = StatusBannerContext {
        run_number: upcoming_run,
        mode_label,
        head_sha: &pr_detail.snapshot.head_sha,
        reviewer_account,
        runners_label: &runners_label,
        started_at_utc: &started_at_utc,
    };

    // 1. Immediately leave/update the pickup status banner on GitHub BEFORE worktree checkout & LLM brains
    // so anyone viewing the PR knows right away that `otto-mac` caught it in an independent async session.
    let running_banner = format_running_status_banner(&banner_ctx);
    match gh.upsert_status_banner_comment(
        reviewer_account,
        &repo_cfg.full_name,
        pr_detail.snapshot.pr_number,
        existing_banner_id,
        &running_banner,
    ) {
        Ok(posted) => {
            println!(
                "[PICKED-UP] {}#{} status banner active ({}, runners={}): {}",
                repo_cfg.full_name,
                pr_detail.snapshot.pr_number,
                if reusing_banner { "updated" } else { "created" },
                runners_label,
                posted.html_url
            );
            tracked.active_status_comment_id = Some(posted.id);
            tracked.active_status_key = Some(dedup_key);
        }
        Err(e) => {
            eprintln!(
                "[WARN] Could not post pickup status banner on {}#{}: {}",
                repo_cfg.full_name, pr_detail.snapshot.pr_number, e
            );
        }
    }

    // Persist `active_status_comment_id` atomically under exclusive lock so concurrent workers never clobber `state.json`.
    let snapshot_before_run = tracked.clone();
    LedgerStore::with_exclusive_lock(state_dir, |store| {
        store.pull_requests.insert(key.clone(), snapshot_before_run);
    })?;

    // 2. Run worktree checkout, multi-lens brains, Drive sync, and atomic GitHub review submission
    let pipeline_res = execute_pr_review_pipeline(
        workspace,
        cfg,
        repo_cfg,
        state_dir,
        gh,
        pr_detail,
        reviewer_account,
        decision,
        runners,
        &mut tracked,
        live_worker,
    );

    match pipeline_res {
        Ok(ReviewPipelineOutcome::Posted { review_url }) => {
            if let Some(cid) = tracked.active_status_comment_id {
                let completed_body = format_completed_status_banner(
                    &banner_ctx,
                    &CompletedBannerOutcome {
                        verdict: tracked.compute_verdict(),
                        open_count: tracked.open_observations().len(),
                        duration_secs: started_instant.elapsed().as_secs(),
                        review_url: &review_url,
                        drive_url: tracked.gdrive_doc_url.as_deref(),
                    },
                );
                let _ = gh.update_issue_comment(
                    reviewer_account,
                    &repo_cfg.full_name,
                    cid,
                    &completed_body,
                );
            }
            tracked.active_status_comment_id = None;
            tracked.active_status_key = None;
            tracked.last_attempt_key = None;
            tracked.attempts_for_key = 0;
            LedgerStore::with_exclusive_lock(state_dir, |store| {
                store.pull_requests.insert(key, tracked);
            })?;
            Ok(())
        }
        Ok(ReviewPipelineOutcome::DeferredMovedSha { live_head_sha }) => {
            if let Some(cid) = tracked.active_status_comment_id {
                let superseded_body = format_superseded_status_banner(&banner_ctx, &live_head_sha);
                let _ = gh.update_issue_comment(
                    reviewer_account,
                    &repo_cfg.full_name,
                    cid,
                    &superseded_body,
                );
            }
            tracked.active_status_comment_id = None;
            tracked.active_status_key = None;
            tracked.last_attempt_key = None;
            tracked.attempts_for_key = 0;
            LedgerStore::with_exclusive_lock(state_dir, |store| {
                store.pull_requests.insert(key, tracked);
            })?;
            Ok(())
        }
        Err(err) => {
            if let Some(cid) = tracked.active_status_comment_id {
                let interrupted_body =
                    format_interrupted_status_banner(&banner_ctx, &err.to_string());
                let _ = gh.update_issue_comment(
                    reviewer_account,
                    &repo_cfg.full_name,
                    cid,
                    &interrupted_body,
                );
            }
            // Keep `active_status_comment_id` + `active_status_key` set so next poll reuses the banner!
            LedgerStore::with_exclusive_lock(state_dir, |store| {
                store.pull_requests.insert(key, tracked);
            })?;
            Err(err)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_pr_review_pipeline(
    workspace: &Path,
    cfg: &WorkspaceConfig,
    repo_cfg: &WatchedRepoConfig,
    state_dir: &Path,
    gh: &GitHubClient,
    pr_detail: &DetailedOpenPr,
    reviewer_account: &str,
    decision: TriggerDecision,
    runners: RunnerPair,
    tracked: &mut TrackedPrState,
    live_worker: &Mutex<ActiveWorkerState>,
) -> Result<ReviewPipelineOutcome, Box<dyn std::error::Error>> {
    if let Ok(mut guard) = live_worker.lock() {
        guard.phase = "worktree".to_string();
        guard.save(state_dir);
    }

    let token = gh.resolve_token(reviewer_account)?;
    let full_diff = gh.fetch_pr_diff(
        reviewer_account,
        &repo_cfg.full_name,
        pr_detail.snapshot.pr_number,
    )?;
    let diff_map = ParsedDiffMap::parse(&full_diff);
    let changed_files = diff_map.changed_files();

    let wt = checkout_pinned_worktree(
        state_dir,
        &repo_cfg.full_name,
        &pr_detail.snapshot.head_sha,
        Some(pr_detail.snapshot.pr_number),
        &token,
    )?;

    let (is_re_review, newly_added_codes, adjudication_notes) = match decision {
        TriggerDecision::InitialFullReview { .. } => {
            let codes = execute_initial_full_review(
                workspace,
                state_dir,
                cfg,
                runners,
                &wt.path,
                pr_detail,
                &changed_files,
                &full_diff,
                tracked,
                Some(live_worker),
            )?;
            (false, codes, Vec::new())
        }
        TriggerDecision::ReReview {
            previous_sha,
            head_sha,
            sha_changed,
            new_replies,
        } => {
            let delta_diff = if sha_changed && !previous_sha.is_empty() {
                gh.fetch_compare_diff(
                    reviewer_account,
                    &repo_cfg.full_name,
                    &previous_sha,
                    &head_sha,
                )
                .unwrap_or_else(|_| full_diff.clone())
            } else {
                String::from(
                    "(No new commits pushed; adjudicating developer replies against pinned SHA.)",
                )
            };
            let (codes, notes) = execute_stateful_re_review(
                workspace,
                state_dir,
                cfg,
                runners,
                &wt.path,
                pr_detail,
                &delta_diff,
                &full_diff,
                &new_replies,
                tracked,
                Some(live_worker),
            )?;
            (true, codes, notes)
        }
        _ => {
            return Ok(ReviewPipelineOutcome::Posted {
                review_url: String::new(),
            });
        }
    };

    if let Ok(mut guard) = live_worker.lock() {
        guard.phase = "drive_sync".to_string();
        guard.save(state_dir);
    }

    // Sync local M1 markdown/JSON + private Google Drive Doc BEFORE posting to GitHub so the
    // GitHub summary includes the direct link to the PR's living Google Doc!
    let drive_res = sync_pr_review_artifacts(workspace, cfg, repo_cfg, tracked)?;
    if let Some(warn) = &drive_res.drive_warning {
        eprintln!(
            "[WARN] Local review saved to {}, Google Drive sync deferred: {}",
            drive_res.local_markdown_path.display(),
            warn
        );
    } else if let Some(url) = &drive_res.gdrive_doc_url {
        println!("[OK] Synced Google Drive PR Review Doc: {}", url);
    }

    if let Ok(mut guard) = live_worker.lock() {
        guard.phase = "posting_github".to_string();
        guard.save(state_dir);
    }

    // Verify PR tip hasn't moved while brains were running so inline line numbers match `commit_id`.
    let live_head_sha = gh.fetch_current_pr_head_sha(
        reviewer_account,
        &repo_cfg.full_name,
        pr_detail.snapshot.pr_number,
    )?;
    if live_head_sha != pr_detail.snapshot.head_sha {
        println!(
            "[DEFER-POST] {}#{} moved from {} to {} during analysis; next poll will re-verify against new SHA.",
            repo_cfg.full_name,
            pr_detail.snapshot.pr_number,
            &pr_detail.snapshot.head_sha[..7.min(pr_detail.snapshot.head_sha.len())],
            &live_head_sha[..7.min(live_head_sha.len())]
        );
        return Ok(ReviewPipelineOutcome::DeferredMovedSha { live_head_sha });
    }

    let (gh_body, gh_inline_comments) = build_github_review_artifacts(
        tracked,
        &pr_detail.snapshot.head_sha,
        is_re_review,
        &diff_map,
        &newly_added_codes,
        &adjudication_notes,
    );
    let gh_event = WorkspaceConfig::select_github_review_event(
        repo_cfg,
        reviewer_account,
        &pr_detail.snapshot.author,
        tracked.has_blocking_open_observations(),
    );

    let review_url = gh.post_atomic_pr_review(&AtomicPrReviewRequest {
        account: reviewer_account,
        repo_full_name: &repo_cfg.full_name,
        pr_number: pr_detail.snapshot.pr_number,
        commit_id: &pr_detail.snapshot.head_sha,
        event: gh_event,
        body: &gh_body,
        comments: &gh_inline_comments,
    })?;
    println!(
        "[POSTED] {}#{} as `{}` (event={}, inline_comments={}): {}",
        repo_cfg.full_name,
        pr_detail.snapshot.pr_number,
        reviewer_account,
        gh_event,
        gh_inline_comments.len(),
        review_url
    );

    Ok(ReviewPipelineOutcome::Posted { review_url })
}

fn run_sync_docs(
    workspace: &Path,
    config_path: &Path,
    repo_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = WorkspaceConfig::load(config_path)?;
    let state_dir = expand_tilde(&cfg.state_dir);
    let ledger_snap = LedgerStore::load_or_default(&state_dir)?;
    let existing_id = ledger_snap
        .repo_cursors
        .get(repo_name)
        .and_then(|c| c.living_doc_gdrive_id.clone());

    let (doc_id, doc_url) =
        sync_living_docs_to_drive(workspace, &cfg, repo_name, existing_id.as_deref())?;
    LedgerStore::with_exclusive_lock(&state_dir, |store| {
        let cur = store.repo_cursors.entry(repo_name.to_string()).or_default();
        cur.living_doc_gdrive_id = Some(doc_id);
        cur.living_doc_gdrive_url = Some(doc_url.clone());
    })?;
    println!("Synced Living Docs for `{}` -> {}", repo_name, doc_url);
    Ok(())
}

fn run_status(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = WorkspaceConfig::load(config_path)?;
    let state_dir = expand_tilde(&cfg.state_dir);
    let ledger = LedgerStore::load_or_default(&state_dir)?;

    println!(
        "=== Tracked Pull Requests ({}) ===",
        ledger.pull_requests.len()
    );
    for pr in ledger.pull_requests.values() {
        println!(
            "- {}#{} [{}] runs={} sha={} open_obs={} drive={}",
            pr.repo,
            pr.pr_number,
            pr.compute_verdict().badge(),
            pr.run_count,
            pr.last_reviewed_sha.as_deref().unwrap_or("-"),
            pr.open_observations().len(),
            pr.gdrive_doc_url.as_deref().unwrap_or("(local-only)")
        );
    }
    println!(
        "\n=== Code Tender PRs ({}) ===",
        ledger.tender_pull_requests.len()
    );
    for t in ledger.tender_pull_requests.values() {
        println!(
            "- {}#{} [{}] cycles={} sha={} handled={} staged={}",
            t.repo,
            t.pr_number,
            t.last_status.badge(),
            t.runs_count,
            t.last_tended_sha.as_deref().unwrap_or("-"),
            t.handled_items.len(),
            t.staged_patch.is_some()
        );
    }
    Ok(())
}

struct TenderWorkerCleanupGuard {
    state_dir: PathBuf,
    repo: String,
    pr_number: u64,
}

impl Drop for TenderWorkerCleanupGuard {
    fn drop(&mut self) {
        ActiveTenderWorkerState::remove(&self.state_dir, &self.repo, self.pr_number);
    }
}

fn try_acquire_named_lock(
    state_dir: &Path,
    filename: &str,
) -> Result<Option<PollLockGuard>, std::io::Error> {
    fs::create_dir_all(state_dir)?;
    let lock_path = state_dir.join(filename);
    if lock_path.exists() {
        let stale = match fs::read_to_string(&lock_path) {
            Ok(raw) => {
                let pid_str = raw.lines().next().unwrap_or("").trim();
                !is_process_alive_str(pid_str)
            }
            Err(_) => true,
        };
        if stale {
            let _ = fs::remove_file(&lock_path);
        } else {
            return Ok(None);
        }
    }
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut f) => {
            let _ = writeln!(f, "{}", std::process::id());
            Ok(Some(PollLockGuard { lock_path }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(e) => Err(e),
    }
}

fn run_tender_poll(
    workspace: &Path,
    config_path: &Path,
    dry_run: bool,
    only_repo: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = WorkspaceConfig::load(config_path)?;
    if !cfg.tender.enabled {
        println!("[TENDER-DISABLED] `tender.enabled = false` in config/repos.toml (0 AI tokens).");
        return Ok(());
    }

    let state_dir = expand_tilde(&cfg.state_dir);
    let _lock = if !dry_run {
        match try_acquire_named_lock(&state_dir, "tender-poll.lock")? {
            Some(g) => Some(g),
            None => {
                println!("[SKIP-ACTIVE] Another `code-reviewer tend-poll` is active.");
                return Ok(());
            }
        }
    } else {
        None
    };

    let gh = GitHubClient::new(&cfg);
    let max_parallel = cfg.tender.max_parallel_prs.clamp(1, 12);
    let mut active_count = ActiveTenderWorkerState::list_active(&state_dir).len();
    let initial_ledger = LedgerStore::load_or_default(&state_dir)?;

    for repo_cfg in &cfg.repos {
        let has_explicit_watched_pr = initial_ledger
            .tender_pull_requests
            .values()
            .any(|t| t.repo.eq_ignore_ascii_case(&repo_cfg.full_name) && t.watched && t.is_open);
        if !repo_cfg.tender_enabled && !has_explicit_watched_pr {
            continue;
        }
        if let Some(filter) = only_repo
            && !repo_cfg.full_name.eq_ignore_ascii_case(filter)
        {
            continue;
        }
        let poll_account = repo_cfg
            .forced_account
            .as_deref()
            .unwrap_or(cfg.accounts.primary.as_str());
        let snapshots = gh.poll_tender_repository(poll_account, &repo_cfg.full_name)?;
        if !dry_run {
            let _ = sync_watched_prs_from_snapshots_with_gh(
                &cfg,
                repo_cfg,
                &state_dir,
                Some(&gh),
                &snapshots,
            );
        }
        let ledger_snap = LedgerStore::load_or_default(&state_dir)?;

        for snap in &snapshots {
            if let Some(running) = ActiveTenderWorkerState::is_pr_worker_running(
                &state_dir,
                &repo_cfg.full_name,
                snap.pr_number,
            ) {
                println!(
                    "[TENDER-IN-FLIGHT] {}#{} already running PID {} (phase={})",
                    repo_cfg.full_name, snap.pr_number, running.pid, running.phase
                );
                continue;
            }

            let key = LedgerStore::pr_key(&repo_cfg.full_name, snap.pr_number);
            let existing = ledger_snap.tender_pull_requests.get(&key);
            if existing.is_some_and(|t| !t.watched) {
                continue;
            }
            let explicitly_watched = existing.is_some_and(|t| t.watched);
            if !repo_cfg.tender_enabled && !explicitly_watched {
                continue;
            }
            let allowed =
                explicitly_watched || cfg.is_tender_author_allowed(repo_cfg, &snap.author);
            let decision = evaluate_tender_trigger(
                existing,
                snap,
                allowed,
                cfg.tender.max_autonomous_cycles_per_pr.clamp(1, 10),
                cfg.tender.ignore_clean_pass_reviews,
            );

            match decision {
                TenderTriggerDecision::SkipDraft => println!(
                    "[TENDER-SKIP-DRAFT] {}#{} ({})",
                    repo_cfg.full_name, snap.pr_number, snap.title
                ),
                TenderTriggerDecision::SkipFork => println!(
                    "[TENDER-SKIP-FORK] {}#{} is a cross-repository fork",
                    repo_cfg.full_name, snap.pr_number
                ),
                TenderTriggerDecision::SkipUnownedAuthor => println!(
                    "[TENDER-SKIP-AUTHOR] {}#{} author `{}` not in Code Tender allowlist",
                    repo_cfg.full_name, snap.pr_number, snap.author
                ),
                TenderTriggerDecision::SkipMaxCyclesReached => println!(
                    "[TENDER-MAX-CYCLES] {}#{} reached max_autonomous_cycles_per_pr ({})",
                    repo_cfg.full_name, snap.pr_number, cfg.tender.max_autonomous_cycles_per_pr
                ),
                TenderTriggerDecision::UpToDate => println!(
                    "[TENDER-UP-TO-DATE] {}#{} @ {}",
                    repo_cfg.full_name,
                    snap.pr_number,
                    &snap.head_sha[..snap.head_sha.len().min(7)]
                ),
                TenderTriggerDecision::HandleFeedback {
                    head_sha,
                    items,
                    latest_feedback_at,
                } => {
                    println!(
                        "[TENDER-TRIGGER] {}#{} @ {} ({} feedback item(s))",
                        repo_cfg.full_name,
                        snap.pr_number,
                        &head_sha[..head_sha.len().min(7)],
                        items.len()
                    );
                    if dry_run {
                        continue;
                    }
                    if cfg.tender.poll_only_mode {
                        println!(
                            "[TENDER-POLL-ONLY-NO-AI] {}#{} needs tending, but `tender.poll_only_mode = true` (0 AI tokens).",
                            repo_cfg.full_name, snap.pr_number
                        );
                        continue;
                    }
                    if let Err(e) = resolve_active_runners(&cfg) {
                        println!(
                            "[TENDER-WAIT-AUTH] Deferring {}#{} ({})",
                            repo_cfg.full_name, snap.pr_number, e
                        );
                        continue;
                    }

                    let attempt_key =
                        format!("{}:{}:{}", head_sha, latest_feedback_at, items.len());
                    let prior_attempts = existing
                        .and_then(|t| {
                            (t.last_attempt_key.as_deref() == Some(attempt_key.as_str()))
                                .then_some(t.attempts_for_key)
                        })
                        .unwrap_or(0);
                    if prior_attempts >= 2 {
                        println!(
                            "[TENDER-CIRCUIT-BREAKER] {}#{} hit {} failed attempts for `{}`; pausing auto retries (0 AI tokens).",
                            repo_cfg.full_name, snap.pr_number, prior_attempts, attempt_key
                        );
                        continue;
                    }

                    if active_count >= max_parallel {
                        println!(
                            "[TENDER-SLOT-WAIT] {}#{} waiting (active {}/{})",
                            repo_cfg.full_name, snap.pr_number, active_count, max_parallel
                        );
                        continue;
                    }

                    let slot = LedgerStore::with_exclusive_lock(&state_dir, |store| {
                        let s = store.tender_dispatch_counter;
                        store.tender_dispatch_counter =
                            store.tender_dispatch_counter.wrapping_add(1);
                        let t = store
                            .tender_pull_requests
                            .entry(key.clone())
                            .or_insert_with(|| {
                                TenderPrState::new(
                                    &repo_cfg.full_name,
                                    snap.pr_number,
                                    &snap.title,
                                    &snap.author,
                                    &snap.base_ref,
                                    &snap.head_ref,
                                )
                            });
                        if t.last_attempt_key.as_deref() == Some(attempt_key.as_str()) {
                            t.attempts_for_key = t.attempts_for_key.saturating_add(1);
                        } else {
                            t.last_attempt_key = Some(attempt_key);
                            t.attempts_for_key = 1;
                        }
                        s
                    })?;

                    let preferred_lead = if cfg.tender.intercalate_providers {
                        if slot.is_multiple_of(2) {
                            CliRunner::Claude
                        } else {
                            CliRunner::Agy
                        }
                    } else {
                        cfg.agents.primary_runner
                    };
                    let runners = resolve_intercalated_runners(&cfg, Some(preferred_lead))?;
                    let tender_account = cfg.select_tender_account(repo_cfg, &snap.author);
                    let pid = spawn_async_tender_worker(
                        workspace,
                        &state_dir,
                        &repo_cfg.full_name,
                        snap,
                        tender_account,
                        runners,
                    )?;
                    active_count += 1;
                    println!(
                        "[TENDER-DISPATCHED] {}#{} -> async PID {} (runners=`{}`, active={}/{})",
                        repo_cfg.full_name,
                        snap.pr_number,
                        pid,
                        runners.label(),
                        active_count,
                        max_parallel
                    );
                }
            }
        }
    }

    Ok(())
}

fn spawn_async_tender_worker(
    workspace: &Path,
    state_dir: &Path,
    repo_full_name: &str,
    snap: &TenderPrSnapshot,
    tender_account: &str,
    runners: RunnerPair,
) -> Result<u32, Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let log_dir = home_dir().join(".cheap-otto/logs/code-reviewer");
    fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join(format!(
        "tender-{}-{}.log",
        repo_fs_slug(repo_full_name),
        snap.pr_number
    ));
    let out_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err_file = out_file.try_clone()?;

    let mut cmd = Command::new(exe);
    cmd.current_dir(workspace)
        .arg("--workspace")
        .arg(workspace)
        .arg("tend")
        .arg("--repo")
        .arg(repo_full_name)
        .arg("--pr")
        .arg(snap.pr_number.to_string())
        .arg("--lead-runner")
        .arg(runners.lead.slug())
        .stdin(Stdio::null())
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn()?;
    let pid = child.id();
    let now = Utc::now().to_rfc3339();
    let mut initial = ActiveTenderWorkerState {
        pid,
        repo: repo_full_name.to_string(),
        pr_number: snap.pr_number,
        title: snap.title.clone(),
        head_sha: snap.head_sha.clone(),
        tender_account: tender_account.to_string(),
        lead_runner: runners.lead.slug().to_string(),
        alt_runner: runners.alt.slug().to_string(),
        phase: "spawning".to_string(),
        packets_total: 0,
        packets_done: 0,
        started_at_utc: now.clone(),
        updated_at_utc: now,
        workers: BTreeMap::new(),
    };
    initial.save(state_dir);
    Ok(pid)
}

fn run_single_pr_tend(
    workspace: &Path,
    config_path: &Path,
    repo_name: &str,
    pr_number: u64,
    dry_run: bool,
    lead_runner_arg: Option<&str>,
    stage_only: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = WorkspaceConfig::load(config_path)?;
    let repo_cfg = cfg
        .repos
        .iter()
        .find(|r| r.full_name.eq_ignore_ascii_case(repo_name))
        .ok_or_else(|| {
            format!(
                "repo `{}` is not registered in config/repos.toml",
                repo_name
            )
        })?;
    let state_dir = expand_tilde(&cfg.state_dir);

    if !dry_run
        && let Some(existing) = ActiveTenderWorkerState::is_pr_worker_running(
            &state_dir,
            &repo_cfg.full_name,
            pr_number,
        )
        && existing.pid != std::process::id()
    {
        println!(
            "[TENDER-SKIP-IN-FLIGHT] {}#{} already running PID {} (phase={}).",
            repo_cfg.full_name, pr_number, existing.pid, existing.phase
        );
        return Ok(());
    }

    let _cleanup = if !dry_run {
        Some(TenderWorkerCleanupGuard {
            state_dir: state_dir.clone(),
            repo: repo_cfg.full_name.clone(),
            pr_number,
        })
    } else {
        None
    };

    let gh = GitHubClient::new(&cfg);
    let poll_account = repo_cfg
        .forced_account
        .as_deref()
        .unwrap_or(cfg.accounts.primary.as_str());
    let snapshots = gh.poll_tender_repository(poll_account, &repo_cfg.full_name)?;
    if !dry_run {
        let _ = sync_watched_prs_from_snapshots_with_gh(
            &cfg,
            repo_cfg,
            &state_dir,
            Some(&gh),
            &snapshots,
        );
    }
    let snap = snapshots
        .iter()
        .find(|p| p.pr_number == pr_number)
        .ok_or_else(|| format!("open PR #{} not found on {}", pr_number, repo_cfg.full_name))?;

    let ledger_snap = LedgerStore::load_or_default(&state_dir)?;
    let key = LedgerStore::pr_key(&repo_cfg.full_name, pr_number);
    let existing = ledger_snap.tender_pull_requests.get(&key);

    let items = match evaluate_tender_trigger(
        existing,
        snap,
        true,
        u32::MAX,
        cfg.tender.ignore_clean_pass_reviews,
    ) {
        TenderTriggerDecision::HandleFeedback { items, .. } => items,
        _ => {
            // Manual UI trigger: collect unresolved actionable threads that have NOT been answered by us
            snap.feedback_items
                .iter()
                .filter(|f| {
                    !is_non_actionable_tender_item(
                        f,
                        &snap.author,
                        cfg.tender.ignore_clean_pass_reviews,
                    )
                })
                .filter(|f| !f.is_resolved && !f.answered_by_pr_author && !f.answered_by_tender)
                .cloned()
                .collect()
        }
    };

    if items.is_empty() {
        println!(
            "[TENDER-NOTHING-TO-DO] {}#{} has no open review feedback.",
            repo_cfg.full_name, pr_number
        );
        return Ok(());
    }

    let latest_feedback_at = items
        .iter()
        .map(|i| i.created_at.as_str())
        .max()
        .unwrap_or("")
        .to_string();

    if dry_run {
        println!(
            "[TENDER-DRY-RUN] Would tend {}#{} ({} feedback items, latest={})",
            repo_cfg.full_name,
            pr_number,
            items.len(),
            latest_feedback_at
        );
        return Ok(());
    }

    let preferred = lead_runner_arg.and_then(CliRunner::from_slug);
    let runners = resolve_intercalated_runners(&cfg, preferred)?;
    let tender_account = cfg.select_tender_account(repo_cfg, &snap.author);
    let now = Utc::now().to_rfc3339();
    let mut init_worker = ActiveTenderWorkerState {
        pid: std::process::id(),
        repo: repo_cfg.full_name.clone(),
        pr_number,
        title: snap.title.clone(),
        head_sha: snap.head_sha.clone(),
        tender_account: tender_account.to_string(),
        lead_runner: runners.lead.slug().to_string(),
        alt_runner: runners.alt.slug().to_string(),
        phase: "starting".to_string(),
        packets_total: 0,
        packets_done: 0,
        started_at_utc: now.clone(),
        updated_at_utc: now,
        workers: BTreeMap::new(),
    };
    init_worker.save(&state_dir);
    let live_worker = Mutex::new(init_worker);

    let outcome = execute_tender_pr_cycle(
        workspace,
        &cfg,
        repo_cfg,
        &state_dir,
        &gh,
        snap,
        &items,
        &latest_feedback_at,
        tender_account,
        preferred,
        stage_only,
        Some(&live_worker),
    )?;
    println!(
        "[TENDER-DONE] {}#{} status={} pushed={}",
        repo_cfg.full_name,
        pr_number,
        outcome.last_status.badge(),
        outcome.last_commit_pushed.as_deref().unwrap_or("(none)")
    );
    Ok(())
}
