//! Lightweight single-binary HTTP/JSON server (`127.0.0.1:7430`) and SSH-tunneled browser launcher (`code-reviewer ui`).

use crate::brains::{
    ADVERSARIAL_BRAIN, SPECIALIST_BRAINS, append_kb_entry, is_runner_authenticated,
};
use crate::config::{
    AccountPolicy, CliRunner, ReviewEventPolicy, WatchedRepoConfig, WorkspaceConfig, expand_tilde,
    home_dir, repo_fs_slug,
};
use crate::drive::sync_living_docs_to_drive;
use crate::state::{ActiveWorkerState, LedgerStore, TrackedPrState};
use crate::ui_html::DASHBOARD_HTML;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_UI_BIND: &str = "127.0.0.1:7430";
pub const DEFAULT_UI_PORT: u16 = 7430;

struct CachedHostProbe {
    captured_at: Instant,
    claude_ready: bool,
    agy_ready: bool,
    ac_power: bool,
    summary: String,
}

pub struct ServerContext {
    pub workspace: PathBuf,
    pub config_path: PathBuf,
    probe_cache: Mutex<Option<CachedHostProbe>>,
}

pub fn run_ui_server(
    workspace: &Path,
    config_path: &Path,
    bind_addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(bind_addr)?;
    println!(
        "[code-reviewer-ui] Dashboard listening on http://{} (workspace: {})",
        bind_addr,
        workspace.display()
    );

    let ctx = Arc::new(ServerContext {
        workspace: workspace.to_path_buf(),
        config_path: config_path.to_path_buf(),
        probe_cache: Mutex::new(None),
    });

    for incoming in listener.incoming() {
        let Ok(stream) = incoming else { continue };
        let ctx_clone = Arc::clone(&ctx);
        thread::spawn(move || {
            let _ = handle_http_connection(stream, &ctx_clone);
        });
    }
    Ok(())
}

/// Starts or reuses an SSH local port-forward (`127.0.0.1:<port> -> <host>:127.0.0.1:<port>`)
/// and opens the Internal CI Dashboard in the user's default browser.
pub fn open_tunneled_ui(
    workspace: &Path,
    host: &str,
    remote_repo: &str,
    port: u16,
    no_open: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let local_addr = format!("127.0.0.1:{}", port);
    let url = format!("http://{}", local_addr);

    if TcpStream::connect_timeout(&local_addr.parse()?, Duration::from_millis(350)).is_err() {
        // Ensure remote `com.cheapotto.code-reviewer.ui` (or detached `code-reviewer serve`) is running on M1
        let ensure_remote = format!(
            "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin\"\n\
             if ! nc -z 127.0.0.1 {port} 2>/dev/null; then\n\
               UID_NUM=$(id -u)\n\
               launchctl kickstart \"gui/$UID_NUM/com.cheapotto.code-reviewer.ui\" 2>/dev/null || \\\n\
               nohup code-reviewer --workspace \"{repo}\" serve --bind 127.0.0.1:{port} >> \"$HOME/.cheap-otto/logs/code-reviewer/ui-server.log\" 2>&1 &\n\
               sleep 1\n\
             fi\n",
            port = port,
            repo = remote_repo.replace('"', "\\\""),
        );
        let _ = crate::m1::run_ssh_script(host, &ensure_remote);

        println!(
            "[ui] Opening background SSH tunnel http://127.0.0.1:{port} -> {host} (127.0.0.1:{port})..."
        );
        let _child = Command::new("ssh")
            .current_dir(workspace)
            .args([
                "-N",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ServerAliveInterval=30",
                "-L",
                &format!("127.0.0.1:{0}:127.0.0.1:{0}", port),
                host,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        for _ in 0..20 {
            if TcpStream::connect_timeout(&local_addr.parse()?, Duration::from_millis(250)).is_ok()
            {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }
    }

    println!("[ui] Code Reviewer Dashboard ready at: {}", url);
    if !no_open {
        open_browser(&url);
    }
    Ok(())
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd")
            .args(["/C", "start", "", url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

fn handle_http_connection(
    mut stream: TcpStream,
    ctx: &ServerContext,
) -> Result<(), std::io::Error> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = vec![0u8; 65_536];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }

    let raw = String::from_utf8_lossy(&buf[..n]);
    let (head, body_part) = match raw.split_once("\r\n\r\n") {
        Some((h, b)) => (h, b.to_string()),
        None => (raw.as_ref(), String::new()),
    };

    let mut lines = head.lines();
    let req_line = lines.next().unwrap_or("");
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let full_target = parts.next().unwrap_or("/");

    let mut content_len: usize = 0;
    for header in lines {
        if let Some((k, v)) = header.split_once(':')
            && k.trim().eq_ignore_ascii_case("content-length")
        {
            content_len = v.trim().parse::<usize>().unwrap_or(0);
        }
    }

    let mut body = body_part.into_bytes();
    while body.len() < content_len && body.len() < 512_000 {
        let mut extra = vec![0u8; 8192];
        match stream.read(&mut extra) {
            Ok(0) | Err(_) => break,
            Ok(r) => body.extend_from_slice(&extra[..r]),
        }
    }
    let body_str = String::from_utf8_lossy(&body[..body.len().min(content_len)]).to_string();

    let (path, query) = match full_target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full_target, ""),
    };

    if method == "GET" && (path == "/" || path == "/index.html") {
        return write_http_response(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            DASHBOARD_HTML.as_bytes(),
        );
    }

    let result = dispatch_api(method, path, query, &body_str, ctx);
    match result {
        Ok(val) => {
            let bytes = serde_json::to_vec(&val).unwrap_or_else(|_| b"{}".to_vec());
            write_http_response(&mut stream, 200, "application/json; charset=utf-8", &bytes)
        }
        Err(msg) => {
            let bytes = serde_json::to_vec(&json!({ "error": msg })).unwrap_or_default();
            write_http_response(&mut stream, 400, "application/json; charset=utf-8", &bytes)
        }
    }
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), std::io::Error> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Server Error",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        reason,
        content_type,
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn dispatch_api(
    method: &str,
    path: &str,
    query: &str,
    body: &str,
    ctx: &ServerContext,
) -> Result<Value, String> {
    match (method, path) {
        ("GET", "/api/health") => Ok(json!({ "ok": true })),
        ("GET", "/api/overview") => build_overview_payload(ctx),
        ("GET", "/api/brain") => {
            let brain_id = parse_query_param(query, "id")
                .ok_or_else(|| "missing `id` query parameter".to_string())?;
            build_brain_payload(&ctx.workspace, &brain_id)
        }
        ("GET", "/api/living-docs") => {
            let repo = parse_query_param(query, "repo")
                .ok_or_else(|| "missing `repo` query parameter".to_string())?;
            build_living_docs_payload(&ctx.workspace, &repo)
        }
        ("POST", "/api/actions/poll") => {
            spawn_detached_self(&ctx.workspace, &["poll"])?;
            Ok(json!({ "ok": true, "message": "Dispatched parallel async poll batch!" }))
        }
        ("POST", "/api/actions/review") => {
            #[derive(Deserialize)]
            struct Req {
                repo: String,
                pr: u64,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let state_dir = expand_tilde(&cfg.state_dir);
            let pr_key = LedgerStore::pr_key(&req.repo, req.pr);
            let slot = LedgerStore::with_exclusive_lock(&state_dir, |store| {
                let s = store.provider_dispatch_counter;
                store.provider_dispatch_counter = store.provider_dispatch_counter.wrapping_add(1);
                if let Some(t) = store.pull_requests.get_mut(&pr_key) {
                    t.attempts_for_key = 0;
                    t.last_attempt_key = None;
                }
                s
            })
            .unwrap_or(0);
            let lead = if slot.is_multiple_of(2) {
                "claude"
            } else {
                "agy"
            };
            spawn_detached_self(
                &ctx.workspace,
                &[
                    "review",
                    "--repo",
                    &req.repo,
                    "--pr",
                    &req.pr.to_string(),
                    "--lead-runner",
                    lead,
                ],
            )?;
            Ok(json!({
                "ok": true,
                "message": format!("Started async session for {}#{} (lead runner: {})", req.repo, req.pr, lead)
            }))
        }
        ("POST", "/api/actions/sync-docs") => {
            #[derive(Deserialize)]
            struct Req {
                repo: String,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let state_dir = expand_tilde(&cfg.state_dir);
            let existing = LedgerStore::load_or_default(&state_dir).ok().and_then(|s| {
                s.repo_cursors
                    .get(&req.repo)
                    .and_then(|c| c.living_doc_gdrive_id.clone())
            });
            let (doc_id, doc_url) =
                sync_living_docs_to_drive(&ctx.workspace, &cfg, &req.repo, existing.as_deref())
                    .map_err(|e| e.to_string())?;
            let _ = LedgerStore::with_exclusive_lock(&state_dir, |store| {
                let cur = store.repo_cursors.entry(req.repo.clone()).or_default();
                cur.living_doc_gdrive_id = Some(doc_id);
                cur.living_doc_gdrive_url = Some(doc_url.clone());
            });
            Ok(json!({ "ok": true, "message": format!("Synced Living Docs -> {}", doc_url) }))
        }
        ("POST", "/api/settings/agents") => {
            #[derive(Deserialize)]
            struct Req {
                intercalate_providers: bool,
                max_parallel_prs: usize,
                max_parallel_brains_per_pr: usize,
                #[serde(default)]
                claude_model: Option<String>,
                #[serde(default)]
                reasoning_effort: Option<String>,
                #[serde(default)]
                poll_only_mode: Option<bool>,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let mut cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            cfg.agents.intercalate_providers = req.intercalate_providers;
            cfg.agents.max_parallel_prs = req.max_parallel_prs.clamp(1, 12);
            cfg.agents.max_parallel_brains_per_pr = req.max_parallel_brains_per_pr.clamp(1, 8);
            if let Some(m) = req.claude_model
                && !m.trim().is_empty()
            {
                cfg.agents.claude_model = m.trim().to_string();
            }
            if let Some(eff) = req.reasoning_effort
                && !eff.trim().is_empty()
            {
                cfg.agents.reasoning_effort = eff.trim().to_string();
            }
            if let Some(po) = req.poll_only_mode {
                cfg.agents.poll_only_mode = po;
            }
            cfg.save(&ctx.config_path).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true }))
        }
        ("POST", "/api/repos/add") => {
            #[derive(Deserialize)]
            struct Req {
                full_name: String,
                default_branch: String,
                account_policy: String,
                forced_account: Option<String>,
                review_event_policy: String,
                gdrive_folder_id: Option<String>,
                local_check_cmd: Option<String>,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let mut cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            cfg.upsert_repo(WatchedRepoConfig {
                full_name: req.full_name,
                default_branch: req.default_branch,
                language: "en".to_string(),
                account_policy: if req.account_policy.eq_ignore_ascii_case("forced") {
                    AccountPolicy::Forced
                } else {
                    AccountPolicy::Swap
                },
                forced_account: req.forced_account.filter(|s| !s.is_empty()),
                review_event_policy: if req.review_event_policy.eq_ignore_ascii_case("comment_only")
                {
                    ReviewEventPolicy::CommentOnly
                } else {
                    ReviewEventPolicy::FormalVerdict
                },
                gdrive_folder_id: req.gdrive_folder_id.filter(|s| !s.is_empty()),
                local_check_cmd: req.local_check_cmd.filter(|s| !s.is_empty()),
                enabled: true,
            })
            .map_err(|e| e.to_string())?;
            cfg.save(&ctx.config_path).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true }))
        }
        ("POST", "/api/repos/toggle") => {
            #[derive(Deserialize)]
            struct Req {
                full_name: String,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let mut cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            if let Some(r) = cfg
                .repos
                .iter_mut()
                .find(|r| r.full_name.eq_ignore_ascii_case(&req.full_name))
            {
                r.enabled = !r.enabled;
            }
            cfg.save(&ctx.config_path).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true }))
        }
        ("POST", "/api/repos/remove") => {
            #[derive(Deserialize)]
            struct Req {
                full_name: String,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let mut cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            cfg.remove_repo(&req.full_name);
            cfg.save(&ctx.config_path).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true }))
        }
        ("POST", "/api/kb/add") => {
            #[derive(Deserialize)]
            struct Req {
                brain: String,
                kind: String,
                rule: String,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            append_kb_entry(&ctx.workspace, &req.brain, &req.kind, &req.rule)
                .map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(format!("unknown endpoint `{} {}`", method, path)),
    }
}

fn build_overview_payload(ctx: &ServerContext) -> Result<Value, String> {
    let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
    let state_dir = expand_tilde(&cfg.state_dir);
    let ledger = LedgerStore::load_or_default(&state_dir).unwrap_or_default();

    let (claude_ready, agy_ready, ac_power, host_summary) = get_cached_host_probe(ctx);

    // Merge PR records from `state.json` + local `reviews/*/*/observations.json` so nothing is ever hidden.
    let mut pr_map: BTreeMap<String, TrackedPrState> = ledger.pull_requests.clone();
    if let Ok(repo_dirs) = fs::read_dir(ctx.workspace.join("reviews")) {
        for rd in repo_dirs.flatten() {
            if let Ok(pr_dirs) = fs::read_dir(rd.path()) {
                for pd in pr_dirs.flatten() {
                    let obs_file = pd.path().join("observations.json");
                    if let Ok(raw) = fs::read_to_string(&obs_file)
                        && let Ok(disk_pr) = serde_json::from_str::<TrackedPrState>(&raw)
                    {
                        let key = LedgerStore::pr_key(&disk_pr.repo, disk_pr.pr_number);
                        let replace = match pr_map.get(&key) {
                            Some(existing) => {
                                disk_pr.run_count > existing.run_count
                                    || (disk_pr.gdrive_doc_url.is_some()
                                        && existing.gdrive_doc_url.is_none())
                            }
                            None => true,
                        };
                        if replace {
                            pr_map.insert(key, disk_pr);
                        }
                    }
                }
            }
        }
    }

    let mut prs_json: Vec<Value> = pr_map
        .into_iter()
        .map(|(key, pr)| {
            let verdict = pr.compute_verdict().badge();
            let mut v = serde_json::to_value(&pr).unwrap_or_else(|_| json!({}));
            if let Some(obj) = v.as_object_mut() {
                obj.insert("key".to_string(), json!(key));
                obj.insert("verdict".to_string(), json!(verdict));
            }
            v
        })
        .collect();
    prs_json.sort_by(|a, b| {
        let b_num = b["pr_number"].as_u64().unwrap_or(0);
        let a_num = a["pr_number"].as_u64().unwrap_or(0);
        b_num.cmp(&a_num)
    });

    let mut active_workers = ActiveWorkerState::list_active(&state_dir);
    if active_workers.is_empty()
        && let Some(legacy) = detect_legacy_inflight_worker()
    {
        active_workers.push(legacy);
    }

    let next_lead = if cfg.agents.intercalate_providers {
        if ledger.provider_dispatch_counter.is_multiple_of(2) {
            CliRunner::Claude.slug()
        } else {
            CliRunner::Agy.slug()
        }
    } else {
        cfg.agents.primary_runner.slug()
    };

    let all_brains: Vec<&str> = SPECIALIST_BRAINS
        .iter()
        .copied()
        .chain(std::iter::once(ADVERSARIAL_BRAIN))
        .collect();

    let recent_logs = read_tail_logs(&home_dir().join(".cheap-otto/logs/code-reviewer"), 45);
    let run_history = read_recent_history(
        &home_dir().join(".cheap-otto/runs/code-reviewer/history.jsonl"),
        12,
    );

    Ok(json!({
        "runners": {
            "claude_ready": claude_ready,
            "agy_ready": agy_ready,
        },
        "host": {
            "ac_power": ac_power,
            "summary": host_summary,
        },
        "agents": cfg.agents,
        "next_lead_runner": next_lead,
        "active_workers": active_workers,
        "pull_requests": prs_json,
        "repos": cfg.repos,
        "brains": all_brains,
        "recent_logs": recent_logs,
        "run_history": run_history,
    }))
}

fn get_cached_host_probe(ctx: &ServerContext) -> (bool, bool, bool, String) {
    if let Ok(guard) = ctx.probe_cache.lock()
        && let Some(cached) = &*guard
        && cached.captured_at.elapsed() < Duration::from_secs(300)
    {
        return (
            cached.claude_ready,
            cached.agy_ready,
            cached.ac_power,
            cached.summary.clone(),
        );
    }

    let claude_ready = is_runner_authenticated(CliRunner::Claude);
    let agy_ready = is_runner_authenticated(CliRunner::Agy);
    let (ac_power, summary) = probe_host_power();

    if let Ok(mut guard) = ctx.probe_cache.lock() {
        *guard = Some(CachedHostProbe {
            captured_at: Instant::now(),
            claude_ready,
            agy_ready,
            ac_power,
            summary: summary.clone(),
        });
    }
    (claude_ready, agy_ready, ac_power, summary)
}

fn probe_host_power() -> (bool, String) {
    #[cfg(target_os = "macos")]
    {
        let batt = Command::new("pmset")
            .args(["-g", "batt"])
            .stdin(Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        let on_ac = batt.contains("AC Power");
        let therm = Command::new("sysctl")
            .args(["-n", "kern.thermalstate"])
            .stdin(Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "0".to_string());
        (
            on_ac,
            format!(
                "otto-mac • {} • thermal={}",
                if on_ac { "AC Power" } else { "Battery" },
                therm
            ),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        (true, "Local Workstation".to_string())
    }
}

/// Fallback detector if an older pre-worker-json `code-reviewer poll` process is still finishing on M1.
fn detect_legacy_inflight_worker() -> Option<ActiveWorkerState> {
    #[cfg(unix)]
    {
        let out = Command::new("ps")
            .args(["-ax", "-o", "pid=,command="])
            .stdin(Stdio::null())
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut poll_pid: Option<u32> = None;
        let mut active_brain = "analyzing".to_string();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.contains("code-reviewer") && trimmed.contains(" poll") {
                poll_pid = trimmed
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u32>().ok());
            }
            if let Some(idx) = trimmed.find("# Brain ") {
                active_brain = trimmed[idx + 2..].chars().take(28).collect();
            }
        }
        let pid = poll_pid?;
        let mut brains = BTreeMap::new();
        brains.insert(
            active_brain,
            crate::state::BrainLiveState {
                runner: "claude".to_string(),
                status: "running".to_string(),
            },
        );
        Some(ActiveWorkerState {
            pid,
            repo: "acme-org/web-app".to_string(),
            pr_number: 1,
            title: "(legacy poll tick finishing in-flight review)".to_string(),
            head_sha: "in-flight".to_string(),
            mode: "INITIAL_FULL".to_string(),
            reviewer_account: "bob-reviewer".to_string(),
            lead_runner: "claude".to_string(),
            alt_runner: "claude".to_string(),
            phase: "specialist_brains".to_string(),
            started_at_utc: String::new(),
            updated_at_utc: String::new(),
            brains,
        })
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn build_brain_payload(workspace: &Path, brain_id: &str) -> Result<Value, String> {
    let valid = SPECIALIST_BRAINS
        .iter()
        .copied()
        .chain(std::iter::once(ADVERSARIAL_BRAIN))
        .any(|b| b == brain_id);
    if !valid {
        return Err(format!("invalid brain id `{}`", brain_id));
    }
    let dir = workspace.join("brains").join(brain_id);
    let prompt = fs::read_to_string(dir.join("PROMPT.md")).unwrap_or_default();
    let lessons = fs::read_to_string(dir.join("kb/lessons.md")).unwrap_or_default();
    let fps = fs::read_to_string(dir.join("kb/false-positives.md")).unwrap_or_default();
    let exemplars = fs::read_to_string(dir.join("kb/exemplars.md")).unwrap_or_default();
    Ok(json!({
        "id": brain_id,
        "prompt": prompt,
        "lessons": lessons,
        "false_positives": fps,
        "exemplars": exemplars,
    }))
}

fn build_living_docs_payload(workspace: &Path, repo: &str) -> Result<Value, String> {
    let dir = workspace.join("living-docs").join(repo_fs_slug(repo));
    let mut files = Vec::new();
    for name in [
        "00-system-map.md",
        "01-product-flows.md",
        "02-contracts-schema.md",
        "03-invariants-adrs.md",
        "04-merge-ledger.jsonl",
    ] {
        let content =
            fs::read_to_string(dir.join(name)).unwrap_or_else(|_| "(not created yet)".to_string());
        files.push(json!({ "name": name, "content": content }));
    }
    Ok(json!({ "repo": repo, "files": files }))
}

fn spawn_detached_self(workspace: &Path, args: &[&str]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let logs_dir = home_dir().join(".cheap-otto/logs/code-reviewer");
    let _ = fs::create_dir_all(&logs_dir);
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs_dir.join("sched-pr-poller.log"))
        .map_err(|e| e.to_string())?;
    let err_file = log_file.try_clone().map_err(|e| e.to_string())?;

    let mut cmd = Command::new(exe);
    cmd.arg("--workspace")
        .arg(workspace)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn read_tail_logs(logs_dir: &Path, max_lines: usize) -> String {
    let mut combined = Vec::new();
    if let Ok(raw) = fs::read_to_string(logs_dir.join("sched-pr-poller.log")) {
        let lines: Vec<&str> = raw.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        combined.extend(lines[start..].iter().map(|s| (*s).to_string()));
    }
    combined.join("\n")
}

fn read_recent_history(history_path: &Path, limit: usize) -> Vec<Value> {
    let Ok(raw) = fs::read_to_string(history_path) else {
        return Vec::new();
    };
    let mut items: Vec<Value> = raw
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    items.reverse();
    items.truncate(limit);
    items
}

fn parse_query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=')
            && k == key
        {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(val) = u8::from_str_radix(hex, 16)
        {
            out.push(val);
            i += 3;
            continue;
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
