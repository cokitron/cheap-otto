//! `cheap-otto serve` HTTP/JSON Control Plane, Mission Control UI & `pixel-agents` Bridge.
//!
//! Exposes a single shared REST API (`axum`) consumed identically by both the browser-based
//! Mission Control UI (`GET /`) and external AI agents / CLI automation:
//! - Multi-harness & model catalog discovery (`GET /api/harnesses`)
//! - Addressable per-agent lifecycle & direct 1-on-1 querying (`GET/POST /api/agents`,
//!   `GET/DELETE /api/agents/{project}/{id}`, `POST /api/agents/{project}/{id}/ask`)
//! - Repeatable task templates (`GET/POST /api/templates`, `POST /api/templates/{id}/run`)
//! - Native M1 `launchd` schedules & run history (`GET/POST /api/schedules`, `GET /api/runs`)
//! - Interactive M1 `tmux` pane inspection & command input (`GET/POST /api/terminal/{session}`)
//! - Real-time `stream-json` -> `pixel-agents` (`v1.6.0`) HTTP `HookEvent` forwarder so every
//!   harness (`claude`, `agy`, `kiro`, `custom`) animates inside the Flat & Diorama Pixel Office.

use crate::agent_session::{
    AgentSessionRecord, AgentSessionRegistry, AgentSessionStatus, AgentTurnEntry,
    normalize_agent_id, summarize_ndjson_and_log,
};
use crate::bridge::{PeerNode, validate_safe_identifier};
use crate::doctor::parse_df_free_percent;
use crate::hardware::{ThermalLevel, parse_macos_vm_stat_available_mb};
use crate::harness::{
    HarnessCatalogEntry, HarnessRunConfig, PixelHookPayload, ReasoningEffort, default_agy_models,
    default_claude_models, default_kiro_models, generate_session_uuid, parse_agy_models_output,
    parse_stream_json_line, standard_efforts,
};
use crate::macos::{MacOsStateBackup, parse_pmset_batt_ac_power};
use crate::schedule::{
    NewScheduleParams, ScheduleCadence, ScheduleRegistry, ScheduledWorkload,
    build_schedule_install_plan, parse_run_history_jsonl,
};
use crate::template::{NewTemplateParams, TaskTemplate, TemplateRegistry};
use crate::ui_assets::MISSION_CONTROL_HTML;
use crate::worktree::{AgentRunner, posix_single_quote, slugify_task_prompt};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tower_http::cors::CorsLayer;

/// Runtime configuration for `cheap-otto serve`.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Bind address (defaults to `127.0.0.1`).
    pub host: String,
    /// HTTP port for `cheap-otto serve` (defaults to `7420`).
    pub port: u16,
    /// Expected port where `pixel-agents` standalone runs on the M1 (defaults to `7421`).
    pub pixel_port: u16,
    /// State directory holding `agents.json`, `templates.json`, `schedules.json`, `state-backup.json`.
    pub state_dir: PathBuf,
    /// Root directory containing synced project repositories (`~/otto-workspaces`).
    pub workspaces_root: PathBuf,
    /// Paired or local peer descriptor used for generating `launchd` paths.
    pub peer: PeerNode,
}

/// Shared thread-safe state passed to `axum` handlers and the background event forwarder.
pub struct ControlPlaneState {
    /// Immutable server paths and ports.
    pub config: ServeConfig,
    /// Byte offsets already forwarded per `.ndjson` file so hook events are never duplicated.
    pub forwarded_offsets: Mutex<HashMap<PathBuf, u64>>,
    /// Serializes writes to `agents.json`, `templates.json`, and `schedules.json`.
    pub store_lock: Mutex<()>,
}

#[derive(Debug, Deserialize)]
struct PixelServerJson {
    port: Option<u16>,
    token: Option<String>,
}

/// Summary serialized in `GET /api/agents`.
#[derive(Debug, Serialize)]
pub struct AgentApiSummary {
    /// Agent identifier (`<id>`).
    pub agent_id: String,
    /// Project name (`<project>`).
    pub project_name: String,
    /// Worktree branch (`otto/<id>`).
    pub branch_name: String,
    /// Remote worktree filesystem path.
    pub remote_worktree_dir: String,
    /// Detached `tmux` session (`otto-<id>`).
    pub tmux_session_name: String,
    /// Canonical harness slug (`claude`, `agy`, `kiro`, `custom`).
    pub harness_slug: String,
    /// Model identifier, if set.
    pub model: Option<String>,
    /// Reasoning effort string, if set.
    pub effort: Option<String>,
    /// Bound provider session/conversation ID.
    pub provider_session_id: Option<String>,
    /// Live status (`active`, `idle_done`, `waiting_input`, `failed`).
    pub status: AgentSessionStatus,
    /// Current tool/activity label.
    pub current_activity: Option<String>,
    /// Turn history.
    pub turns: Vec<AgentTurnEntry>,
}

/// Request payload for `POST /api/agents`.
#[derive(Debug, Deserialize)]
pub struct LaunchAgentRequest {
    /// Target project workspace name.
    pub project_name: String,
    /// Optional explicit agent ID (`None` auto-slugifies `prompt`).
    pub task_id: Option<String>,
    /// Selected harness (`claude`, `agy`, `kiro`, `custom`).
    pub harness: String,
    /// Optional `--model` identifier.
    pub model: Option<String>,
    /// Optional `--effort` level (`low`..`max`).
    pub effort: Option<String>,
    /// Task prompt or shell command.
    pub prompt: String,
    /// Verification gate command (`cargo test` default).
    pub verify_command: Option<String>,
}

/// Request payload for `POST /api/agents/{project}/{id}/ask`.
#[derive(Debug, Deserialize)]
pub struct AskAgentRequest {
    /// Question or follow-up coding instruction directed to this specific agent.
    pub prompt: String,
    /// Optional harness override (`claude`, `agy`, `kiro`, `custom`).
    pub harness: Option<String>,
    /// Optional model override.
    pub model: Option<String>,
    /// Optional reasoning effort override.
    pub effort: Option<String>,
    /// Optional verification command to run after the follow-up turn.
    pub verify_command: Option<String>,
    /// When `true`, dispatches inside detached `tmux` instead of waiting synchronously.
    #[serde(default)]
    pub run_async: bool,
}

/// Request payload for `POST /api/templates`.
#[derive(Debug, Deserialize)]
pub struct UpsertTemplateRequest {
    /// Unique template slug.
    pub template_id: String,
    /// Optional display title.
    pub title: Option<String>,
    /// Optional project filter.
    pub project_name: Option<String>,
    /// Default harness (`claude`, `agy`, `kiro`, `custom`).
    pub harness: String,
    /// Optional default model.
    pub model: Option<String>,
    /// Optional default effort.
    pub effort: Option<String>,
    /// Task prompt.
    pub prompt: String,
    /// Verification command.
    pub verify_command: Option<String>,
    /// Whether scheduled runs open a Draft PR.
    #[serde(default)]
    pub create_draft_pr: bool,
}

/// Request payload for `POST /api/templates/{id}/run`.
#[derive(Debug, Deserialize)]
pub struct RunTemplateRequest {
    /// Target project workspace if the template is project-agnostic.
    pub project_name: Option<String>,
    /// Optional custom agent ID override.
    pub task_id: Option<String>,
    /// Optional model override.
    pub model: Option<String>,
}

/// Request payload for `POST /api/schedules`.
#[derive(Debug, Deserialize)]
pub struct CreateScheduleApiRequest {
    /// Target project workspace.
    pub project_name: String,
    /// Schedule identifier.
    pub job_id: String,
    /// Cadence expression (`every:15m`, `15m`, `daily:02:00`, `weekly:MON@03:00`).
    pub cadence: String,
    /// Harness (`ci` / `custom` for CI pipeline, or `claude` / `agy` / `kiro` for agent schedule).
    pub harness: String,
    /// Optional model for scheduled agent runs.
    pub model: Option<String>,
    /// Optional reasoning effort for scheduled agent runs.
    pub effort: Option<String>,
    /// CI shell command or agent task prompt.
    pub prompt: String,
    /// Verification command for agent runs.
    pub verify_command: Option<String>,
    /// Branch to track (`main` default).
    pub branch: Option<String>,
    /// Skip unchanged commits for CI schedules.
    #[serde(default)]
    pub on_change: bool,
    /// Open Draft PR on scheduled agent commits.
    #[serde(default)]
    pub create_draft_pr: bool,
}

/// Request payload for `POST /api/terminal/{session}`.
#[derive(Debug, Deserialize)]
pub struct TerminalKeysRequest {
    /// Keys or command text to send to the `tmux` session.
    pub keys: String,
    /// Whether to append `Enter` (`C-m`).
    #[serde(default = "default_true")]
    pub enter: bool,
}

const fn default_true() -> bool {
    true
}

/// Builds the complete `axum::Router` for `cheap-otto serve` (used both in production and integration tests).
pub fn build_router(state: Arc<ControlPlaneState>) -> Router {
    Router::new()
        .route("/", get(handle_index))
        .route("/api/pixel-office", get(handle_pixel_office))
        .route("/api/health", get(handle_health))
        .route("/api/harnesses", get(handle_harnesses))
        .route("/api/projects", get(handle_projects))
        .route(
            "/api/agents",
            get(handle_list_agents).post(handle_launch_agent),
        )
        .route(
            "/api/agents/{project}/{id}",
            get(handle_inspect_agent).delete(handle_dismiss_agent),
        )
        .route("/api/agents/{project}/{id}/ask", post(handle_ask_agent))
        .route(
            "/api/templates",
            get(handle_list_templates).post(handle_upsert_template),
        )
        .route("/api/templates/{id}", delete(handle_delete_template))
        .route("/api/templates/{id}/run", post(handle_run_template))
        .route(
            "/api/schedules",
            get(handle_list_schedules).post(handle_create_schedule),
        )
        .route(
            "/api/schedules/{project}/{id}",
            delete(handle_remove_schedule),
        )
        .route(
            "/api/schedules/{project}/{id}/run-now",
            post(handle_run_schedule_now),
        )
        .route("/api/runs", get(handle_list_runs))
        .route(
            "/api/terminal/{session}",
            get(handle_capture_terminal).post(handle_send_terminal_keys),
        )
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Starts `cheap-otto serve` on `host:port` alongside the background `pixel-agents` NDJSON hook forwarder.
///
/// # Errors
/// Returns an error if the TCP listener cannot bind to `host:port`.
pub async fn run_serve(config: ServeConfig) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(&config.state_dir)?;
    fs::create_dir_all(&config.workspaces_root)?;

    let bind_addr = format!("{}:{}", config.host, config.port);
    let state = Arc::new(ControlPlaneState {
        config,
        forwarded_offsets: Mutex::new(HashMap::new()),
        store_lock: Mutex::new(()),
    });

    spawn_pixel_bridge_loop(Arc::clone(&state));

    let app = build_router(state);
    let listener = TcpListener::bind(&bind_addr).await?;
    println!("=== cheap-otto serve (Mission Control & Agent API) ===");
    println!("Listening on    : http://{bind_addr}");
    println!(
        "API Endpoints   : http://{bind_addr}/api/harnesses | /api/agents | /api/templates | /api/schedules"
    );
    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_pixel_bridge_loop(state: Arc<ControlPlaneState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(650));
        loop {
            interval.tick().await;
            sync_ndjson_events_to_pixel_agents(&state).await;
        }
    });
}

async fn sync_ndjson_events_to_pixel_agents(state: &ControlPlaneState) {
    let reg =
        read_json_or_default::<AgentSessionRegistry>(&state.config.state_dir.join("agents.json"));
    if reg.agents.is_empty() {
        return;
    }

    let pixel_auth = read_pixel_server_auth(state.config.pixel_port);
    let home = home_dir_path();

    for agent in &reg.agents {
        let events_path = expand_tilde_path(&agent.remote_events_file, home.as_deref());
        let Ok(meta) = fs::metadata(&events_path) else {
            continue;
        };
        let file_len = meta.len();
        let prev_offset = {
            let Ok(guard) = state.forwarded_offsets.lock() else {
                continue;
            };
            guard.get(&events_path).copied().unwrap_or(0)
        };
        if file_len <= prev_offset {
            continue;
        }

        let Ok(raw_bytes) = fs::read(&events_path) else {
            continue;
        };
        let start = usize::try_from(prev_offset)
            .unwrap_or(0)
            .min(raw_bytes.len());
        let chunk = String::from_utf8_lossy(&raw_bytes[start..]).into_owned();

        {
            if let Ok(mut guard) = state.forwarded_offsets.lock() {
                guard.insert(events_path.clone(), file_len);
            }
        }

        let fallback_sid = agent
            .provider_session_id
            .clone()
            .unwrap_or_else(|| format!("otto-{}-{}", agent.project_name, agent.agent_id));

        for line in chunk.lines() {
            if let Some(upd) = parse_stream_json_line(line, &fallback_sid)
                && let (Some((p_port, p_token)), Some(hook)) = (&pixel_auth, upd.pixel_hook)
            {
                let _ = post_pixel_hook(*p_port, p_token, &hook).await;
            }
        }
    }
}

fn read_pixel_server_auth(fallback_port: u16) -> Option<(u16, String)> {
    let home = home_dir_path()?;
    let server_json_path = home.join(".pixel-agents").join("server.json");
    let raw = fs::read_to_string(server_json_path).ok()?;
    let parsed: PixelServerJson = serde_json::from_str(&raw).ok()?;
    let token = parsed.token?;
    let port = parsed.port.unwrap_or(fallback_port);
    Some((port, token))
}

async fn post_pixel_hook(
    port: u16,
    token: &str,
    payload: &PixelHookPayload,
) -> Result<(), std::io::Error> {
    let body = serde_json::to_string(payload).unwrap_or_default();
    let req = format!(
        "POST /api/hooks/claude HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Authorization: Bearer {token}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {body}",
        body.len()
    );
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
    stream.write_all(req.as_bytes()).await?;
    let mut discard = [0_u8; 256];
    let _ = stream.read(&mut discard).await;
    Ok(())
}

async fn handle_index() -> Html<&'static str> {
    Html(MISSION_CONTROL_HTML)
}

async fn handle_pixel_office(State(state): State<Arc<ControlPlaneState>>) -> impl IntoResponse {
    if let Some((port, token)) = read_pixel_server_auth(state.config.pixel_port) {
        let reachable = TcpStream::connect(("127.0.0.1", port)).await.is_ok();
        Json(json!({
            "available": reachable,
            "pixel_port": port,
            "token": token,
        }))
    } else {
        let reachable = TcpStream::connect(("127.0.0.1", state.config.pixel_port))
            .await
            .is_ok();
        Json(json!({
            "available": reachable,
            "pixel_port": state.config.pixel_port,
            "token": Value::Null,
        }))
    }
}

async fn handle_health(State(state): State<Arc<ControlPlaneState>>) -> impl IntoResponse {
    let ceiling = read_ceiling_from_state(&state.config.state_dir).unwrap_or(78);
    if cfg!(target_os = "macos") {
        let batt = run_local_stdout("pmset", &["-g", "batt"]).unwrap_or_default();
        let pmset = format!(
            "{}\n{}",
            run_local_stdout("pmset", &["-g", "custom"]).unwrap_or_default(),
            run_local_stdout("pmset", &["-g"]).unwrap_or_default()
        );
        let on_ac = parse_pmset_batt_ac_power(&batt).unwrap_or(true);
        let lid_disabled = MacOsStateBackup::from_pmset_custom(&pmset, 0)
            .map(|b| b.ac_disablesleep)
            .unwrap_or(true);
        let thermal = run_local_stdout("sysctl", &["-n", "kern.thermalstate"])
            .and_then(|s| ThermalLevel::parse(&s))
            .unwrap_or(ThermalLevel::Nominal);
        let avail_mb = run_local_stdout("vm_stat", &[])
            .and_then(|s| parse_macos_vm_stat_available_mb(&s).ok())
            .unwrap_or(4096);
        let disk_free = run_local_stdout("df", &["-P", "/"])
            .and_then(|s| parse_df_free_percent(&s))
            .unwrap_or(80);

        Json(json!({
            "on_ac_power": on_ac,
            "lid_sleep_disabled": lid_disabled,
            "battery_ceiling_percent": ceiling,
            "thermal_level": format!("{thermal:?}"),
            "available_memory_mb": avail_mb,
            "disk_free_percent": disk_free,
        }))
    } else {
        Json(json!({
            "on_ac_power": true,
            "lid_sleep_disabled": true,
            "battery_ceiling_percent": ceiling,
            "thermal_level": "Nominal",
            "available_memory_mb": 4096,
            "disk_free_percent": 80,
        }))
    }
}

/// Probes local CLI harnesses and returns structured [`HarnessCatalogEntry`] items.
#[must_use = "catalog powers both GET /api/harnesses and local cheap-otto harnesses"]
pub fn probe_local_harnesses() -> Vec<HarnessCatalogEntry> {
    let claude_ver = run_local_combined("claude", &["--version"]);
    let agy_ver = run_local_combined("agy", &["--version"]);
    let kiro_ver = run_local_combined("kiro-cli", &["--version"]);

    let (agy_auth, agy_models) = if agy_ver.is_some() {
        let raw_models = run_local_combined("agy", &["models"]).unwrap_or_default();
        parse_agy_models_output(&raw_models)
    } else {
        (false, default_agy_models())
    };

    vec![
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
                    "Sign in once via `cheap-otto shell` -> `agy` to unlock live models"
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
    ]
}

async fn handle_harnesses() -> impl IntoResponse {
    let entries = tokio::task::spawn_blocking(probe_local_harnesses)
        .await
        .unwrap_or_default();
    Json(json!({ "harnesses": entries }))
}

async fn handle_projects(State(state): State<Arc<ControlPlaneState>>) -> impl IntoResponse {
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(&state.config.workspaces_root) {
        for entry in entries.flatten() {
            if entry.path().is_dir()
                && let Some(name) = entry.file_name().to_str()
                && !name.starts_with('.')
                && validate_safe_identifier(name)
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    Json(json!({ "projects": names }))
}

async fn handle_list_agents(State(state): State<Arc<ControlPlaneState>>) -> impl IntoResponse {
    let mut reg = load_agents_registry(&state);
    let mut changed = false;
    let home = home_dir_path();

    for rec in &mut reg.agents {
        if reconcile_local_agent_record(rec, home.as_deref()) {
            changed = true;
        }
    }
    if changed {
        let _ = save_agents_registry(&state, &reg);
    }

    let items: Vec<AgentApiSummary> = reg.agents.iter().map(to_agent_summary).collect();
    Json(json!({ "agents": items }))
}

async fn handle_launch_agent(
    State(state): State<Arc<ControlPlaneState>>,
    Json(req): Json<LaunchAgentRequest>,
) -> impl IntoResponse {
    match launch_local_or_recorded_agent(&state, &req).await {
        Ok(rec) => (
            StatusCode::CREATED,
            Json(json!({
                "agent_id": rec.agent_id,
                "project_name": rec.project_name,
                "branch_name": rec.branch_name,
                "tmux_session_name": rec.tmux_session_name,
                "harness_slug": rec.harness.slug(),
                "model": rec.model,
                "provider_session_id": rec.provider_session_id,
            })),
        )
            .into_response(),
        Err(msg) => error_response(StatusCode::BAD_REQUEST, &msg),
    }
}

async fn handle_inspect_agent(
    State(state): State<Arc<ControlPlaneState>>,
    AxumPath((project, id)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let clean_id = normalize_agent_id(&id);
    let mut reg = load_agents_registry(&state);
    let home = home_dir_path();

    let Some(rec) = reg
        .agents
        .iter_mut()
        .find(|a| a.project_name == project.trim() && a.agent_id == clean_id)
    else {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("agent '{project}/{clean_id}' not found"),
        );
    };

    let _ = reconcile_local_agent_record(rec, home.as_deref());
    let rec_snapshot = rec.clone();
    let _ = save_agents_registry(&state, &reg);

    let wt_path = expand_tilde_path(&rec_snapshot.remote_worktree_dir, home.as_deref());
    let git_status = if wt_path.exists() {
        let st = run_local_stdout_in(&wt_path, "git", &["status", "-sb"]).unwrap_or_default();
        let df = run_local_stdout_in(&wt_path, "git", &["diff", "--stat"]).unwrap_or_default();
        format!("{st}\n{df}").trim().to_string()
    } else {
        "(worktree directory not created locally in test/remote mode)".to_string()
    };

    let log_path = expand_tilde_path(&rec_snapshot.remote_log_file, home.as_deref());
    let log_tail = tail_file_lines(&log_path, 40);

    (
        StatusCode::OK,
        Json(json!({
            "record": rec_snapshot,
            "harness_slug": rec_snapshot.harness.slug(),
            "git_status": git_status,
            "latest_response": rec_snapshot.last_response,
            "log_tail": log_tail,
        })),
    )
        .into_response()
}

async fn handle_ask_agent(
    State(state): State<Arc<ControlPlaneState>>,
    AxumPath((project, id)): AxumPath<(String, String)>,
    Json(req): Json<AskAgentRequest>,
) -> impl IntoResponse {
    if req.prompt.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "prompt cannot be empty");
    }
    let clean_id = normalize_agent_id(&id);
    let reg = load_agents_registry(&state);
    let Some(mut rec) = reg.find(&project, &clean_id).cloned() else {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("agent '{project}/{clean_id}' not found"),
        );
    };

    let override_runner = match req.harness.as_deref() {
        Some(h) if !h.trim().is_empty() => match AgentRunner::parse(h) {
            Ok(r) => Some(r),
            Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
        },
        _ => None,
    };
    let override_effort = match req.effort.as_deref() {
        Some(e) if !e.trim().is_empty() => match ReasoningEffort::parse(e) {
            Ok(ef) => Some(ef),
            Err(err) => return error_response(StatusCode::BAD_REQUEST, &err.to_string()),
        },
        _ => None,
    };

    let target_runner = override_runner.unwrap_or(rec.harness);
    let same_runner = target_runner == rec.harness;
    let target_model = req
        .model
        .clone()
        .filter(|m| !m.trim().is_empty())
        .or_else(|| same_runner.then(|| rec.model.clone()).flatten());
    let target_effort = override_effort.or_else(|| same_runner.then_some(rec.effort).flatten());

    let session_id = if same_runner {
        rec.provider_session_id.clone()
    } else if target_runner == AgentRunner::ClaudeCode {
        Some(generate_session_uuid())
    } else {
        None
    };

    let stream_json = matches!(
        target_runner,
        AgentRunner::ClaudeCode | AgentRunner::Antigravity
    );
    let h_cfg = HarnessRunConfig {
        runner: target_runner,
        model: target_model.clone(),
        effort: target_effort,
        session_id: session_id.clone(),
        resume_existing: same_runner,
        stream_json,
    };
    let safe_cmd = match h_cfg.build_remote_command(&req.prompt) {
        Ok(c) => c,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };

    let home = home_dir_path();
    let wt_path = expand_tilde_path(&rec.remote_worktree_dir, home.as_deref());
    let events_path = expand_tilde_path(&rec.remote_events_file, home.as_deref());
    let log_path = expand_tilde_path(&rec.remote_log_file, home.as_deref());
    if let Some(parent) = events_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let next_turn = u32::try_from(rec.turns.len() + 1).unwrap_or(1);
    let now = now_epoch_secs();
    rec.harness = target_runner;
    rec.model = target_model.clone();
    rec.effort = target_effort;
    if session_id.is_some() {
        rec.provider_session_id = session_id;
    }
    rec.updated_epoch_secs = now;

    if !wt_path.exists() {
        // Test / dry environment where the M1 worktree directory is not mounted locally:
        rec.turns.push(AgentTurnEntry {
            turn: next_turn,
            prompt: req.prompt.trim().to_string(),
            harness: target_runner,
            model: target_model.clone(),
            effort: target_effort,
            response_summary: Some(format!("[queued command] {safe_cmd}")),
            started_epoch_secs: now,
        });
        let mut reg_mut = load_agents_registry(&state);
        reg_mut.upsert(rec.clone());
        let _ = save_agents_registry(&state, &reg_mut);
        return (
            StatusCode::OK,
            Json(json!({
                "agent_id": rec.agent_id,
                "project_name": rec.project_name,
                "turn": next_turn,
                "harness_slug": target_runner.slug(),
                "model": target_model,
                "status": rec.status,
                "command": safe_cmd,
                "response": format!("[queued command] {safe_cmd}"),
            })),
        )
            .into_response();
    }

    let post_verify = match req.verify_command.as_deref() {
        Some(v) if !v.trim().is_empty() => format!(
            " && {} && (git diff --quiet && git diff --cached --quiet || (git add -A && git commit -m \"otto({}): follow-up turn\"))",
            v.trim(),
            rec.agent_id
        ),
        _ => String::new(),
    };

    let events_q = posix_single_quote(&events_path.to_string_lossy());
    let log_q = posix_single_quote(&log_path.to_string_lossy());
    let wt_q = posix_single_quote(&wt_path.to_string_lossy());

    if req.run_async {
        let inner = format!(
            "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; \
             cd {wt_q} && {{ {safe_cmd}{post_verify}; }} 2>&1 | tee -a {events_q} {log_q}; \
             rc=$?; printf '\\n[OTTO_STATUS exit=%s]\\n' \"$rc\" >> {log_q}; exit \"$rc\""
        );
        let tmux_name = rec.tmux_session_name.clone();
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &tmux_name])
            .output();
        let _ = Command::new("tmux")
            .args(["new-session", "-d", "-s", &tmux_name, &inner])
            .output();
        let _ = Command::new("tmux")
            .args(["set-option", "-t", &tmux_name, "remain-on-exit", "on"])
            .output();

        rec.status = AgentSessionStatus::Active;
        rec.current_activity = Some("Executing async follow-up query".to_string());
        rec.turns.push(AgentTurnEntry {
            turn: next_turn,
            prompt: req.prompt.trim().to_string(),
            harness: target_runner,
            model: target_model.clone(),
            effort: target_effort,
            response_summary: None,
            started_epoch_secs: now,
        });
        let mut reg_mut = load_agents_registry(&state);
        reg_mut.upsert(rec.clone());
        let _ = save_agents_registry(&state, &reg_mut);

        return (
            StatusCode::ACCEPTED,
            Json(json!({
                "agent_id": rec.agent_id,
                "project_name": rec.project_name,
                "turn": next_turn,
                "harness_slug": target_runner.slug(),
                "model": target_model,
                "status": rec.status,
                "response": "Dispatched asynchronously in tmux session.",
            })),
        )
            .into_response();
    }

    // Synchronous direct query: run inside worktree, tee NDJSON, and return the agent's exact answer!
    let shell_line = format!(
        "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; \
         cd {wt_q} && {safe_cmd}{post_verify}"
    );
    let out = tokio::process::Command::new("/bin/sh")
        .args(["-c", &shell_line])
        .output()
        .await;

    match out {
        Ok(output) => {
            let stdout_str = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr_str = String::from_utf8_lossy(&output.stderr).into_owned();
            let _ = append_file(&events_path, &stdout_str);
            let _ = append_file(&log_path, &format!("{stdout_str}\n{stderr_str}\n"));

            let fallback_sid = rec
                .provider_session_id
                .clone()
                .unwrap_or_else(|| format!("otto-{}-{}", rec.project_name, rec.agent_id));
            let parsed = summarize_ndjson_and_log(&stdout_str, &stderr_str, &fallback_sid);
            if let Some(discovered_sid) = parsed.provider_session_id {
                rec.provider_session_id = Some(discovered_sid);
            }
            let answer = parsed
                .latest_response
                .unwrap_or_else(|| stdout_str.trim().to_string());
            rec.status = if output.status.success() {
                AgentSessionStatus::IdleDone
            } else {
                AgentSessionStatus::Failed
            };
            rec.current_activity = Some("Turn completed".to_string());
            rec.last_response = Some(answer.clone());
            rec.turns.push(AgentTurnEntry {
                turn: next_turn,
                prompt: req.prompt.trim().to_string(),
                harness: target_runner,
                model: target_model.clone(),
                effort: target_effort,
                response_summary: Some(answer.clone()),
                started_epoch_secs: now,
            });
            let mut reg_mut = load_agents_registry(&state);
            reg_mut.upsert(rec.clone());
            let _ = save_agents_registry(&state, &reg_mut);

            (
                StatusCode::OK,
                Json(json!({
                    "agent_id": rec.agent_id,
                    "project_name": rec.project_name,
                    "turn": next_turn,
                    "harness_slug": target_runner.slug(),
                    "model": target_model,
                    "status": rec.status,
                    "response": answer,
                })),
            )
                .into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn handle_dismiss_agent(
    State(state): State<Arc<ControlPlaneState>>,
    AxumPath((project, id)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let clean_id = normalize_agent_id(&id);
    let mut reg = load_agents_registry(&state);
    if let Some(rec) = reg.find(&project, &clean_id).cloned() {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &rec.tmux_session_name])
            .output();
        if rec.harness != AgentRunner::ClaudeCode
            && let Some((p_port, p_token)) = read_pixel_server_auth(state.config.pixel_port)
        {
            let sid = rec
                .provider_session_id
                .unwrap_or_else(|| format!("otto-{}-{}", rec.project_name, rec.agent_id));
            let _ = post_pixel_hook(
                p_port,
                &p_token,
                &PixelHookPayload {
                    hook_event_name: "SessionEnd".to_string(),
                    session_id: sid,
                    cwd: None,
                    tool_name: None,
                    tool_input: None,
                },
            )
            .await;
        }
    }
    let removed = reg.remove(&project, &clean_id);
    let _ = save_agents_registry(&state, &reg);
    Json(json!({ "removed": removed }))
}

async fn handle_list_templates(State(state): State<Arc<ControlPlaneState>>) -> impl IntoResponse {
    let reg = load_templates_registry(&state);
    let items: Vec<Value> = reg
        .templates
        .iter()
        .map(|t| {
            json!({
                "template_id": t.template_id,
                "title": t.title,
                "project_name": t.project_name,
                "harness_slug": t.harness.slug(),
                "model": t.model,
                "effort": t.effort.map(ReasoningEffort::as_flag_str),
                "prompt": t.prompt,
                "verify_command": t.verify_command,
                "create_draft_pr": t.create_draft_pr,
            })
        })
        .collect();
    Json(json!({ "templates": items }))
}

async fn handle_upsert_template(
    State(state): State<Arc<ControlPlaneState>>,
    Json(req): Json<UpsertTemplateRequest>,
) -> impl IntoResponse {
    let runner = match AgentRunner::parse(&req.harness) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    let effort = match req.effort.as_deref() {
        Some(e) if !e.trim().is_empty() => match ReasoningEffort::parse(e) {
            Ok(ef) => Some(ef),
            Err(err) => return error_response(StatusCode::BAD_REQUEST, &err.to_string()),
        },
        _ => None,
    };
    let tpl = match TaskTemplate::new(&NewTemplateParams {
        template_id: &req.template_id,
        title: req.title.as_deref(),
        project_name: req.project_name.as_deref(),
        harness: runner,
        model: req.model.filter(|m| !m.trim().is_empty()),
        effort,
        prompt: &req.prompt,
        verify_command: req.verify_command.as_deref().unwrap_or("cargo test"),
        create_draft_pr: req.create_draft_pr,
    }) {
        Ok(t) => t,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };

    let mut reg = load_templates_registry(&state);
    reg.upsert(tpl.clone());
    let _ = save_templates_registry(&state, &reg);
    (StatusCode::CREATED, Json(json!({ "template": tpl }))).into_response()
}

async fn handle_delete_template(
    State(state): State<Arc<ControlPlaneState>>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let mut reg = load_templates_registry(&state);
    let removed = reg.remove(&id);
    let _ = save_templates_registry(&state, &reg);
    Json(json!({ "removed": removed }))
}

async fn handle_run_template(
    State(state): State<Arc<ControlPlaneState>>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<RunTemplateRequest>,
) -> impl IntoResponse {
    let reg = load_templates_registry(&state);
    let Some(tpl) = reg.get(&id).cloned() else {
        return error_response(StatusCode::NOT_FOUND, &format!("template '{id}' not found"));
    };
    let project = tpl
        .project_name
        .or(req.project_name)
        .unwrap_or_else(|| "cheapotto".to_string());
    let launch_req = LaunchAgentRequest {
        project_name: project,
        task_id: req
            .task_id
            .or_else(|| Some(format!("{}-{}", tpl.template_id, now_epoch_secs() % 10_000))),
        harness: tpl.harness.slug().to_string(),
        model: req.model.or(tpl.model),
        effort: tpl.effort.map(|e| e.as_flag_str().to_string()),
        prompt: tpl.prompt,
        verify_command: Some(tpl.verify_command),
    };
    match launch_local_or_recorded_agent(&state, &launch_req).await {
        Ok(rec) => (
            StatusCode::CREATED,
            Json(json!({
                "agent_id": rec.agent_id,
                "project_name": rec.project_name,
                "branch_name": rec.branch_name,
                "harness_slug": rec.harness.slug(),
            })),
        )
            .into_response(),
        Err(msg) => error_response(StatusCode::BAD_REQUEST, &msg),
    }
}

async fn handle_list_schedules(State(state): State<Arc<ControlPlaneState>>) -> impl IntoResponse {
    let reg = load_schedules_registry(&state);
    let jobs: Vec<Value> = reg
        .jobs
        .iter()
        .map(|j| {
            json!({
                "job_id": j.job_id,
                "project_name": j.project_name,
                "launchd_label": j.launchd_label,
                "cadence_summary": j.cadence.human_summary(),
                "workload": j.workload,
            })
        })
        .collect();
    Json(json!({ "jobs": jobs }))
}

async fn handle_create_schedule(
    State(state): State<Arc<ControlPlaneState>>,
    Json(req): Json<CreateScheduleApiRequest>,
) -> impl IntoResponse {
    let cadence = match parse_flexible_cadence(&req.cadence) {
        Ok(c) => c,
        Err(msg) => return error_response(StatusCode::BAD_REQUEST, &msg),
    };
    let branch = req.branch.unwrap_or_else(|| "main".to_string());
    let workload = if req.harness == "ci" || req.harness == "custom" {
        ScheduledWorkload::CiPipeline {
            branch,
            command: req.prompt,
            only_on_change: req.on_change,
        }
    } else {
        let runner = match AgentRunner::parse(&req.harness) {
            Ok(r) => r,
            Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
        };
        let effort = match req.effort.as_deref() {
            Some(e) if !e.trim().is_empty() => match ReasoningEffort::parse(e) {
                Ok(ef) => Some(ef),
                Err(err) => return error_response(StatusCode::BAD_REQUEST, &err.to_string()),
            },
            _ => None,
        };
        ScheduledWorkload::AgentTask {
            branch,
            agent: runner,
            model: req.model.filter(|m| !m.trim().is_empty()),
            effort,
            prompt: req.prompt,
            verify_command: req
                .verify_command
                .unwrap_or_else(|| "cargo test".to_string()),
            create_draft_pr: req.create_draft_pr,
        }
    };

    let plan = match build_schedule_install_plan(
        &state.config.peer,
        &NewScheduleParams {
            job_id: &req.job_id,
            project_name: &req.project_name,
            cadence,
            workload,
            allow_battery: false,
            run_at_load: false,
        },
    ) {
        Ok(p) => p,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };

    if cfg!(target_os = "macos") {
        let script_path = PathBuf::from(&plan.spec.remote_script_path);
        let plist_path = PathBuf::from(&plan.spec.remote_plist_path);
        if let Some(p) = script_path.parent() {
            let _ = fs::create_dir_all(p);
        }
        if let Some(p) = plist_path.parent() {
            let _ = fs::create_dir_all(p);
        }
        let _ = fs::write(&script_path, &plan.runner_script);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755));
        }
        let _ = fs::write(&plist_path, &plan.plist_xml);
        if let Some(uid) = run_local_stdout("id", &["-u"]) {
            let domain = format!("gui/{}", uid.trim());
            let plist_str = plist_path.to_string_lossy().into_owned();
            let _ = Command::new("launchctl")
                .args(["bootout", &domain, &plist_str])
                .output();
            let _ = Command::new("launchctl")
                .args(["bootstrap", &domain, &plist_str])
                .output();
        }
    }

    let mut reg = load_schedules_registry(&state);
    reg.upsert(plan.spec.clone());
    let _ = save_schedules_registry(&state, &reg);

    (
        StatusCode::CREATED,
        Json(json!({
            "job_id": plan.spec.job_id,
            "launchd_label": plan.spec.launchd_label,
            "cadence_summary": plan.spec.cadence.human_summary(),
        })),
    )
        .into_response()
}

async fn handle_run_schedule_now(
    AxumPath((project, id)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    if !validate_safe_identifier(&project) || !validate_safe_identifier(&id) {
        return error_response(StatusCode::BAD_REQUEST, "invalid schedule identifier");
    }
    let label = format!("com.cheapotto.{project}.{id}");
    if cfg!(target_os = "macos")
        && let Some(uid) = run_local_stdout("id", &["-u"])
    {
        let target = format!("gui/{}/{label}", uid.trim());
        let _ = Command::new("launchctl")
            .args(["kickstart", "-k", &target])
            .output();
    }
    (StatusCode::OK, Json(json!({ "triggered": label }))).into_response()
}

async fn handle_remove_schedule(
    State(state): State<Arc<ControlPlaneState>>,
    AxumPath((project, id)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let mut reg = load_schedules_registry(&state);
    if let Some(spec) = reg
        .jobs
        .iter()
        .find(|j| j.project_name == project && j.job_id == id)
        .cloned()
        && cfg!(target_os = "macos")
    {
        if let Some(uid) = run_local_stdout("id", &["-u"]) {
            let domain = format!("gui/{}", uid.trim());
            let _ = Command::new("launchctl")
                .args(["bootout", &domain, &spec.remote_plist_path])
                .output();
        }
        let _ = fs::remove_file(&spec.remote_plist_path);
        let _ = fs::remove_file(&spec.remote_script_path);
    }
    let removed = reg.remove(&project, &id);
    let _ = save_schedules_registry(&state, &reg);
    Json(json!({ "removed": removed }))
}

async fn handle_list_runs() -> impl IntoResponse {
    let mut all_records = Vec::new();
    if let Some(home) = home_dir_path() {
        let runs_root = home.join(".cheap-otto").join("runs");
        if let Ok(entries) = fs::read_dir(runs_root) {
            for entry in entries.flatten() {
                let hist = entry.path().join("history.jsonl");
                if let Ok(raw) = fs::read_to_string(hist) {
                    all_records.extend(parse_run_history_jsonl(&raw));
                }
            }
        }
    }
    all_records.sort_by_key(|a| std::cmp::Reverse(a.started_epoch_secs));
    all_records.truncate(50);
    Json(json!({ "records": all_records }))
}

async fn handle_capture_terminal(AxumPath(session): AxumPath<String>) -> impl IntoResponse {
    if !validate_safe_identifier(&session) {
        return error_response(StatusCode::BAD_REQUEST, "invalid tmux session identifier");
    }
    let out = run_local_stdout("tmux", &["capture-pane", "-pt", &session, "-S", "-120"])
        .unwrap_or_else(|| format!("(tmux session '{session}' is not currently running)"));
    (
        StatusCode::OK,
        Json(json!({ "session": session, "output": out })),
    )
        .into_response()
}

async fn handle_send_terminal_keys(
    AxumPath(session): AxumPath<String>,
    Json(req): Json<TerminalKeysRequest>,
) -> impl IntoResponse {
    if !validate_safe_identifier(&session) {
        return error_response(StatusCode::BAD_REQUEST, "invalid tmux session identifier");
    }
    if req.keys == "C-c" && !req.enter {
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &session, "C-c"])
            .output();
    } else if req.enter {
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &session, "-l", &req.keys])
            .output();
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &session, "Enter"])
            .output();
    } else {
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &session, "-l", &req.keys])
            .output();
    }
    (StatusCode::OK, Json(json!({ "sent": true }))).into_response()
}

async fn launch_local_or_recorded_agent(
    state: &ControlPlaneState,
    req: &LaunchAgentRequest,
) -> Result<AgentSessionRecord, String> {
    if !validate_safe_identifier(&req.project_name) {
        return Err(format!("invalid project name '{}'", req.project_name));
    }
    if req.prompt.trim().is_empty() {
        return Err("prompt cannot be empty".to_string());
    }
    let runner = AgentRunner::parse(&req.harness).map_err(|e| e.to_string())?;
    let effort = match req.effort.as_deref() {
        Some(e) if !e.trim().is_empty() => {
            Some(ReasoningEffort::parse(e).map_err(|err| err.to_string())?)
        }
        _ => None,
    };

    let clean_project = req.project_name.trim().to_string();
    let agent_id = match req.task_id.as_deref() {
        Some(id) if !id.trim().is_empty() => {
            let normalized = normalize_agent_id(id);
            if !validate_safe_identifier(&normalized) {
                return Err(format!("invalid task ID '{id}'"));
            }
            normalized
        }
        _ => slugify_task_prompt(&req.prompt),
    };

    let assigned_sid = if runner == AgentRunner::ClaudeCode {
        Some(generate_session_uuid())
    } else {
        None
    };

    let stream_json = matches!(runner, AgentRunner::ClaudeCode | AgentRunner::Antigravity);
    let h_cfg = HarnessRunConfig {
        runner,
        model: req.model.clone().filter(|m| !m.trim().is_empty()),
        effort,
        session_id: assigned_sid,
        resume_existing: false,
        stream_json,
    };
    let safe_cmd = h_cfg
        .build_remote_command(&req.prompt)
        .map_err(|e| e.to_string())?;

    let verify_cmd = req
        .verify_command
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("cargo test");
    let remote_wt_str = format!("~/otto-workspaces/{clean_project}/.otto-worktrees/{agent_id}");
    let rec = AgentSessionRecord::new_initial(
        &clean_project,
        &agent_id,
        &remote_wt_str,
        &h_cfg,
        verify_cmd,
        &req.prompt,
        now_epoch_secs(),
    );

    let base_repo = state.config.workspaces_root.join(&clean_project);
    if base_repo.join(".git").exists() {
        let wt_dir = base_repo.join(".otto-worktrees").join(&agent_id);
        if let Some(parent) = wt_dir.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let branch = format!("otto/{agent_id}");
        let wt_str = wt_dir.to_string_lossy().into_owned();
        let _ = Command::new("git")
            .current_dir(&base_repo)
            .args(["worktree", "add", "--force", "-B", &branch, &wt_str, "HEAD"])
            .output();

        let home = home_dir_path();
        let events_path = expand_tilde_path(&rec.remote_events_file, home.as_deref());
        let log_path = expand_tilde_path(&rec.remote_log_file, home.as_deref());
        if let Some(p) = events_path.parent() {
            let _ = fs::create_dir_all(p);
        }
        if let Some(p) = log_path.parent() {
            let _ = fs::create_dir_all(p);
        }

        let events_q = posix_single_quote(&events_path.to_string_lossy());
        let log_q = posix_single_quote(&log_path.to_string_lossy());
        let wt_q = posix_single_quote(&wt_str);
        let rc_file = format!("/tmp/otto-{clean_project}-{agent_id}.rc");

        let agent_stage = if stream_json {
            format!(
                "rm -f {rc_file}; {{ {safe_cmd}; printf '%s' \"$?\" > {rc_file}; }} | tee -a {events_q}; test \"$(cat {rc_file} 2>/dev/null || echo 1)\" = \"0\""
            )
        } else {
            safe_cmd
        };

        let inner = format!(
            "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; \
             cd {wt_q} && {{ {{ {agent_stage} && {verify_cmd} && (git diff --quiet && git diff --cached --quiet || (git add -A && git commit -m \"otto({agent_id}): completed task\")); printf '%s' \"$?\" > {rc_file}.all; }} 2>&1 | tee -a {log_q}; }}; \
             rc=$(cat {rc_file}.all 2>/dev/null || echo 1); rm -f {rc_file} {rc_file}.all; printf '\\n[OTTO_STATUS exit=%s]\\n' \"$rc\" | tee -a {log_q}; exit \"$rc\""
        );

        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &rec.tmux_session_name])
            .output();
        let _ = Command::new("tmux")
            .args(["new-session", "-d", "-s", &rec.tmux_session_name, &inner])
            .output();
        let _ = Command::new("tmux")
            .args([
                "set-option",
                "-t",
                &rec.tmux_session_name,
                "remain-on-exit",
                "on",
            ])
            .output();

        // Notify pixel-agents immediately for non-Claude harnesses so the character spawns right away
        if runner != AgentRunner::ClaudeCode
            && let Some((p_port, p_token)) = read_pixel_server_auth(state.config.pixel_port)
        {
            let sid = rec
                .provider_session_id
                .clone()
                .unwrap_or_else(|| format!("otto-{clean_project}-{agent_id}"));
            let _ = post_pixel_hook(
                p_port,
                &p_token,
                &PixelHookPayload {
                    hook_event_name: "SessionStart".to_string(),
                    session_id: sid,
                    cwd: Some(wt_str),
                    tool_name: None,
                    tool_input: None,
                },
            )
            .await;
        }
    }

    let mut reg = load_agents_registry(state);
    reg.upsert(rec.clone());
    save_agents_registry(state, &reg).map_err(|e| e.to_string())?;
    Ok(rec)
}

fn reconcile_local_agent_record(rec: &mut AgentSessionRecord, home: Option<&Path>) -> bool {
    let events_path = expand_tilde_path(&rec.remote_events_file, home);
    let log_path = expand_tilde_path(&rec.remote_log_file, home);
    if !events_path.exists() && !log_path.exists() {
        return false;
    }

    let ndjson = fs::read_to_string(&events_path).unwrap_or_default();
    let log_tail = tail_file_lines(&log_path, 40);
    let fallback_sid = rec
        .provider_session_id
        .clone()
        .unwrap_or_else(|| format!("otto-{}-{}", rec.project_name, rec.agent_id));
    let parsed = summarize_ndjson_and_log(&ndjson, &log_tail, &fallback_sid);

    let mut changed = false;
    if let Some(sid) = parsed.provider_session_id
        && rec.provider_session_id.as_deref() != Some(sid.as_str())
    {
        rec.provider_session_id = Some(sid);
        changed = true;
    }
    if let Some(lbl) = parsed.latest_activity_label
        && rec.current_activity.as_deref() != Some(lbl.as_str())
    {
        rec.current_activity = Some(lbl);
        changed = true;
    }
    if let Some(resp) = parsed.latest_response
        && rec.last_response.as_deref() != Some(resp.as_str())
    {
        rec.last_response = Some(resp);
        changed = true;
    }
    if let Some(code) = parsed.exit_code {
        let next_status = if code == 0 {
            AgentSessionStatus::IdleDone
        } else {
            AgentSessionStatus::Failed
        };
        if rec.status != next_status {
            rec.status = next_status;
            changed = true;
        }
    }
    changed
}

fn to_agent_summary(rec: &AgentSessionRecord) -> AgentApiSummary {
    AgentApiSummary {
        agent_id: rec.agent_id.clone(),
        project_name: rec.project_name.clone(),
        branch_name: rec.branch_name.clone(),
        remote_worktree_dir: rec.remote_worktree_dir.clone(),
        tmux_session_name: rec.tmux_session_name.clone(),
        harness_slug: rec.harness.slug().to_string(),
        model: rec.model.clone(),
        effort: rec.effort.map(|e| e.as_flag_str().to_string()),
        provider_session_id: rec.provider_session_id.clone(),
        status: rec.status,
        current_activity: rec.current_activity.clone(),
        turns: rec.turns.clone(),
    }
}

fn parse_flexible_cadence(raw: &str) -> Result<ScheduleCadence, String> {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("every:") {
        ScheduleCadence::parse_interval(rest).map_err(|e| e.to_string())
    } else if let Some(rest) = trimmed.strip_prefix("daily:") {
        ScheduleCadence::parse_daily(rest).map_err(|e| e.to_string())
    } else if let Some(rest) = trimmed.strip_prefix("weekly:") {
        ScheduleCadence::parse_weekly(rest).map_err(|e| e.to_string())
    } else if trimmed.contains('@') {
        ScheduleCadence::parse_weekly(trimmed).map_err(|e| e.to_string())
    } else if trimmed.contains(':') {
        ScheduleCadence::parse_daily(trimmed).map_err(|e| e.to_string())
    } else {
        ScheduleCadence::parse_interval(trimmed).map_err(|e| e.to_string())
    }
}

fn load_agents_registry(state: &ControlPlaneState) -> AgentSessionRegistry {
    let _guard = state.store_lock.lock().ok();
    read_json_or_default(&state.config.state_dir.join("agents.json"))
}

fn save_agents_registry(
    state: &ControlPlaneState,
    reg: &AgentSessionRegistry,
) -> std::io::Result<()> {
    let _guard = state.store_lock.lock().ok();
    fs::create_dir_all(&state.config.state_dir)?;
    let raw = serde_json::to_string_pretty(reg).unwrap_or_default();
    fs::write(state.config.state_dir.join("agents.json"), raw)
}

fn load_templates_registry(state: &ControlPlaneState) -> TemplateRegistry {
    let _guard = state.store_lock.lock().ok();
    read_json_or_default(&state.config.state_dir.join("templates.json"))
}

fn save_templates_registry(
    state: &ControlPlaneState,
    reg: &TemplateRegistry,
) -> std::io::Result<()> {
    let _guard = state.store_lock.lock().ok();
    fs::create_dir_all(&state.config.state_dir)?;
    let raw = serde_json::to_string_pretty(reg).unwrap_or_default();
    fs::write(state.config.state_dir.join("templates.json"), raw)
}

fn load_schedules_registry(state: &ControlPlaneState) -> ScheduleRegistry {
    let _guard = state.store_lock.lock().ok();
    read_json_or_default(&state.config.state_dir.join("schedules.json"))
}

fn save_schedules_registry(
    state: &ControlPlaneState,
    reg: &ScheduleRegistry,
) -> std::io::Result<()> {
    let _guard = state.store_lock.lock().ok();
    fs::create_dir_all(&state.config.state_dir)?;
    let raw = serde_json::to_string_pretty(reg).unwrap_or_default();
    fs::write(state.config.state_dir.join("schedules.json"), raw)
}

fn read_json_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<T>(&s).ok())
        .unwrap_or_default()
}

fn read_ceiling_from_state(state_dir: &Path) -> Option<u8> {
    let raw = fs::read_to_string(state_dir.join("state-backup.json")).ok()?;
    let b: MacOsStateBackup = serde_json::from_str(&raw).ok()?;
    b.configured_ceiling_percent
        .or(Some(b.battery_charge_ceiling_percent))
}

fn home_dir_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn expand_tilde_path(raw: &str, home: Option<&Path>) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/")
        && let Some(h) = home
    {
        return h.join(rest);
    }
    PathBuf::from(raw)
}

fn tail_file_lines(path: &Path, max_lines: usize) -> String {
    let Ok(raw) = fs::read_to_string(path) else {
        return String::new();
    };
    let lines: Vec<&str> = raw.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

fn append_file(path: &Path, content: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(content.as_bytes())
}

fn run_local_stdout(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .env(
            "PATH",
            format!(
                "/opt/homebrew/bin:/usr/local/bin:{}/.cargo/bin:{}/.local/bin:{}",
                home_dir_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                home_dir_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn run_local_combined(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .env(
            "PATH",
            format!(
                "/opt/homebrew/bin:/usr/local/bin:{}/.cargo/bin:{}/.local/bin:{}",
                home_dir_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                home_dir_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .ok()?;
    let merged = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_string();
    if merged.is_empty() && !out.status.success() {
        None
    } else {
        Some(merged)
    }
}

fn run_local_stdout_in(dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn error_response(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    fn test_router(tmp_dir: &Path) -> Router {
        let peer =
            PeerNode::new("otto-mac", "127.0.0.1", "m", 22).unwrap_or_else(|_| unreachable!());
        let state = Arc::new(ControlPlaneState {
            config: ServeConfig {
                host: "127.0.0.1".to_string(),
                port: 7420,
                pixel_port: 7421,
                state_dir: tmp_dir.join("state"),
                workspaces_root: tmp_dir.join("workspaces"),
                peer,
            },
            forwarded_offsets: Mutex::new(HashMap::new()),
            store_lock: Mutex::new(()),
        });
        build_router(state)
    }

    #[tokio::test]
    async fn api_serves_ui_launches_agent_queries_it_and_manages_templates() {
        let tmp =
            std::env::temp_dir().join(format!("cheap-otto-srv-test-{}", generate_session_uuid()));
        let _ = fs::create_dir_all(&tmp);
        let app = test_router(&tmp);

        // 1. GET / serves Mission Control HTML
        let res_root = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res_root.status(), StatusCode::OK);

        // 2. POST /api/agents creates an addressable Antigravity agent with Gemini model
        let create_body = serde_json::to_vec(&json!({
            "project_name": "cheapotto",
            "task_id": "review-auth",
            "harness": "agy",
            "model": "gemini-4-carbon-high",
            "effort": "high",
            "prompt": "Audit auth edge cases",
            "verify_command": "cargo test"
        }))
        .unwrap();

        let res_create = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/agents")
                    .header("content-type", "application/json")
                    .body(Body::from(create_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_create.status(), StatusCode::CREATED);

        // 3. POST /api/agents/cheapotto/review-auth/ask queries that specific agent (and switches to Claude Opus!)
        let ask_body = serde_json::to_vec(&json!({
            "prompt": "Summarize your findings in 3 bullet points",
            "harness": "claude",
            "model": "opus",
            "effort": "max"
        }))
        .unwrap();

        let res_ask = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/agents/cheapotto/review-auth/ask")
                    .header("content-type", "application/json")
                    .body(Body::from(ask_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_ask.status(), StatusCode::OK);
        let ask_bytes = axum::body::to_bytes(res_ask.into_body(), 65_536)
            .await
            .unwrap();
        let ask_val: Value = serde_json::from_slice(&ask_bytes).unwrap();
        assert_eq!(ask_val["harness_slug"], "claude");
        assert_eq!(ask_val["model"], "opus");
        assert_eq!(ask_val["turn"], 2);

        // 4. POST /api/templates saves a repeatable playbook & runs it
        let tpl_body = serde_json::to_vec(&json!({
            "template_id": "nightly-clippy",
            "harness": "claude",
            "model": "sonnet",
            "effort": "medium",
            "prompt": "Fix any clippy pedantic warnings",
            "verify_command": "cargo clippy --all-targets -- -D warnings"
        }))
        .unwrap();

        let res_tpl = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/templates")
                    .header("content-type", "application/json")
                    .body(Body::from(tpl_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_tpl.status(), StatusCode::CREATED);

        let _ = fs::remove_dir_all(&tmp);
    }
}
