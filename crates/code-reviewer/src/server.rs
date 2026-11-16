//! Lightweight single-binary HTTP/JSON server (`127.0.0.1:7430`) and SSH-tunneled browser launcher (`code-reviewer ui`).
//!
//! The server has no authentication and its `POST` routes act for real, so every request passes
//! `request_guard` before routing: the `Host` must name this server, a state-changing request
//! that carries an `Origin` must come from an allowlisted one, and a `POST` body must be
//! `application/json`. No CORS headers are sent.

use crate::brains::{
    ADVERSARIAL_BRAIN, SPECIALIST_BRAINS, append_kb_entry, is_runner_authenticated,
    load_brain_kb_section, read_pixel_server_auth,
};
use crate::config::{
    AccountPolicy, CliRunner, ReviewEventPolicy, TenderDeliveryMode, WatchedRepoConfig,
    WorkspaceConfig, expand_tilde, home_dir, normalize_repo_input, parse_pr_reference_input,
    repo_fs_slug,
};
use crate::drive::sync_living_docs_to_drive;
use crate::github::GitHubClient;
use crate::m1::{DEFAULT_M1_HOST, remote_shell_path_expr, validate_ssh_host};
use crate::request_guard::{RequestGuard, host_name_of, require_json_body, require_loopback_bind};
use crate::state::{
    ActiveTenderWorkerState, ActiveWorkerState, LedgerStore, TenderPrState, TenderRunStatus,
    TenderTriggerDecision, TrackedPrState, evaluate_tender_trigger,
};
use crate::tender::{
    TENDER_AGENTS, append_tender_kb_entry, approve_and_push_staged_patch, browse_m1_directories,
    detect_local_repo_checkout, infer_local_check_cmd, inspect_local_git_repo,
    load_tender_agent_lessons, load_tender_agent_notes, load_tender_kb_payload,
    respond_and_resolve_tender_item, save_tender_agent_lessons, save_tender_agent_notes,
    save_tender_kb_file, sync_watched_prs_from_snapshots_with_gh,
};
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
pub const DEFAULT_PIXEL_PORT: u16 = 7421;
/// Mission Control (`cheap-otto serve`), which `DASHBOARD_HTML` links to. The ports above and
/// this one mirror the registry in `src/dashboards.rs` of the root crate, the source of truth;
/// `tests/dashboard_ports.rs` there fails if they drift.
pub const DEFAULT_MISSION_CONTROL_PORT: u16 = 7420;
/// The Pixel Office LaunchAgent that `cheap-otto dashboards install` loads on the Mac. It mirrors
/// the registry in `src/dashboards.rs` of the root crate, the source of truth;
/// `tests/dashboard_labels.rs` there fails if they drift.
pub const PIXEL_OFFICE_LAUNCHD_LABEL: &str = "com.cheapotto.dashboards.pixel-office";

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
    /// `Host` names and web origins allowed to reach the router, checked on every request.
    guard: RequestGuard,
    probe_cache: Mutex<Option<CachedHostProbe>>,
}

pub fn run_ui_server(
    workspace: &Path,
    config_path: &Path,
    bind_addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    require_loopback_bind(&host_name_of(bind_addr).unwrap_or_default())?;
    let listener = TcpListener::bind(bind_addr)?;
    // The origin allowlist is keyed on the port browsers see, so read it back from the socket.
    let port = listener
        .local_addr()
        .map_or(DEFAULT_UI_PORT, |addr| addr.port());
    let guard = RequestGuard::for_listener(port);
    println!(
        "[code-reviewer-ui] Dashboard listening on http://{} (workspace: {})",
        bind_addr,
        workspace.display()
    );
    println!(
        "[code-reviewer-ui] Trusted origins: {}",
        guard.trusted_origins().join(", ")
    );
    println!(
        "[code-reviewer-ui] Mission Control (the dashboard hub): http://127.0.0.1:{DEFAULT_MISSION_CONTROL_PORT}/"
    );

    let ctx = Arc::new(ServerContext {
        workspace: workspace.to_path_buf(),
        config_path: config_path.to_path_buf(),
        guard,
        probe_cache: Mutex::new(None),
    });

    for incoming in listener.incoming() {
        let Ok(mut stream) = incoming else { continue };
        let ctx_clone = Arc::clone(&ctx);
        thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = handle_http_connection(&mut stream, &ctx_clone);
        });
    }
    Ok(())
}

/// How long `code-reviewer ui` waits for its fallback tunnel to answer before giving up.
const FALLBACK_TUNNEL_DEADLINE: Duration = Duration::from_secs(5);

/// What `code-reviewer ui` was asked to open.
pub struct TunneledUiOptions<'a> {
    /// `--host`; `None` means the peer recorded in `peer.json`, else `otto-mac`.
    pub host: Option<&'a str>,
    pub remote_repo: &'a str,
    pub port: u16,
    /// `--state-dir`, handed on to `cheap-otto ui` and used to find `peer.json`.
    pub state_dir: Option<&'a Path>,
    pub no_open: bool,
}

/// Reuses a tunnel already answering on `127.0.0.1:<port>`, otherwise reaches the Reviewer through
/// the one shared `cheap-otto ui` tunnel, and opens the dashboard in the default browser.
///
/// Only when `cheap-otto ui` cannot express the request (a custom host or port) or `cheap-otto` is
/// not installed does it open its own fallback tunnel, which forwards only the Reviewer port:
/// Pixel Office belongs to the shared tunnel, and forwarding it here as well would collide.
pub fn open_tunneled_ui(
    workspace: &Path,
    opts: &TunneledUiOptions<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(host) = opts.host {
        validate_ssh_host(host)?;
    }
    let port = opts.port;
    let local_addr: std::net::SocketAddr = ([127, 0, 0, 1], port).into();
    let url = format!("http://{}", local_addr);

    if TcpStream::connect_timeout(&local_addr, Duration::from_millis(350)).is_err() {
        let peer_hosts = read_peer_hosts(opts.state_dir);
        // Prefer the peer entry the operator named, so `--host <peer address>` still delegates.
        let peer_host = peer_hosts
            .iter()
            .map(String::as_str)
            .find(|peer| Some(*peer) == opts.host)
            .or_else(|| peer_hosts.first().map(String::as_str));
        let host = opts.host.or(peer_host).unwrap_or(DEFAULT_M1_HOST);
        validate_ssh_host(host)?;
        let remote_repo = opts.remote_repo;

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
            repo = remote_shell_path_expr(remote_repo),
        );
        let _ = crate::m1::run_ssh_script(host, &ensure_remote);

        let cheap_otto = if should_delegate_to_cheap_otto(opts.host, peer_host, port) {
            let found = find_cheap_otto_binary();
            if found.is_none() {
                println!(
                    "[ui] `cheap-otto` was not found; falling back to a Reviewer-only tunnel."
                );
            }
            found
        } else {
            println!(
                "[ui] No paired peer in `peer.json` lets `cheap-otto ui` reach {host}:{port}; \
                 using a Reviewer-only tunnel."
            );
            None
        };

        if let Some(bin) = cheap_otto {
            println!("[ui] Sharing the dashboard tunnel through `cheap-otto ui`...");
            // Inherited stdio, so the install and tunnel progress of `cheap-otto ui` stays visible.
            let exit_ok = cheap_otto_ui_command(&bin, opts.state_dir)
                .status()?
                .success();
            // `cheap-otto ui` only warns about a dashboard that is not listening, so check the port.
            let port_answers =
                exit_ok && TcpStream::connect_timeout(&local_addr, Duration::from_secs(1)).is_ok();
            delegated_ui_check(exit_ok, port_answers, port)?;
        } else {
            println!("[ui] Opening background SSH tunnel http://127.0.0.1:{port} -> {host}...");
            let resolved = resolve_ssh_effective_config(host)?;
            let mut command =
                reviewer_fallback_tunnel_command_with_config(host, port, Some(&resolved));
            let mut child = command.current_dir(workspace).spawn()?;
            wait_for_fallback_tunnel(&mut child, &local_addr, port)?;
        }
    }

    println!("[ui] Code Reviewer Dashboard ready at: {}", url);
    if !opts.no_open {
        open_browser(&url);
    }
    Ok(())
}

/// Whether `cheap-otto ui` can serve this request. It has no `--host` and always forwards the
/// Reviewer on `DEFAULT_UI_PORT`, so it qualifies only when `peer.json` records a peer, the
/// requested host is none or that peer, and the port is 7430. Without a recorded peer
/// `cheap-otto ui` would fall back to its own default host, which may not be the Mac meant here.
fn should_delegate_to_cheap_otto(
    requested_host: Option<&str>,
    peer_host: Option<&str>,
    port: u16,
) -> bool {
    let Some(peer) = peer_host else {
        return false;
    };
    requested_host.is_none_or(|requested| requested == peer) && port == DEFAULT_UI_PORT
}

/// Judges a finished delegated `cheap-otto ui`: it must exit 0 and leave the Reviewer port
/// answering, or the browser would open a dead page.
fn delegated_ui_check(exit_ok: bool, port_answers: bool, port: u16) -> Result<(), String> {
    let reason = if !exit_ok {
        "`cheap-otto ui` failed"
    } else if !port_answers {
        "`cheap-otto ui` finished but the port does not answer"
    } else {
        return Ok(());
    };
    Err(format!(
        "{reason}, so the Reviewer dashboard on 127.0.0.1:{port} is not reachable; \
         check the services with `cheap-otto dashboards status --execute`"
    ))
}

/// `cheap-otto` beside this executable, else `cheap-otto` on `PATH` when `--version` runs.
fn find_cheap_otto_binary() -> Option<PathBuf> {
    let own_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    if let Some(dir) = own_dir {
        let beside = ["cheap-otto", "cheap-otto.exe"]
            .into_iter()
            .map(|name| dir.join(name))
            .find(|candidate| candidate.is_file());
        if beside.is_some() {
            return beside;
        }
    }
    let on_path = cheap_otto_version_probe()
        .status()
        .is_ok_and(|status| status.success());
    on_path.then(|| PathBuf::from("cheap-otto"))
}

/// The silent `cheap-otto --version` probe, without `GH_TOKEN` or `GITHUB_TOKEN`.
fn cheap_otto_version_probe() -> Command {
    let mut cmd = Command::new("cheap-otto");
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::brains::strip_github_credentials(&mut cmd);
    cmd
}

/// `cheap-otto ui --execute --no-open [--state-dir <dir>]`, without `GH_TOKEN` or `GITHUB_TOKEN`.
/// `--no-open` because this command opens the Reviewer page itself, not Mission Control.
fn cheap_otto_ui_command(bin: &Path, state_dir: Option<&Path>) -> Command {
    let mut cmd = Command::new(bin);
    cmd.args(["ui", "--execute", "--no-open"]);
    if let Some(dir) = state_dir {
        cmd.arg("--state-dir").arg(dir);
    }
    crate::brains::strip_github_credentials(&mut cmd);
    cmd
}

/// The `alias` and `host` of the peer in `cheap-otto`'s `peer.json`, alias first, or empty.
///
/// Uses `--state-dir` alone when given. Otherwise the first `peer.json` in `./.cheap-otto`, the git
/// repo root's `.cheap-otto`, then `~/.cheap-otto`: the order of `cheap-otto`'s
/// `resolve_active_state_dir`, so both binaries read the same peer.
fn read_peer_hosts(state_dir: Option<&Path>) -> Vec<String> {
    let candidates: Vec<PathBuf> = match state_dir {
        Some(dir) => vec![dir.to_path_buf()],
        None => {
            let mut dirs = vec![PathBuf::from(".cheap-otto")];
            dirs.extend(git_repo_root().map(|root| root.join(".cheap-otto")));
            dirs.push(home_dir().join(".cheap-otto"));
            dirs
        }
    };
    candidates
        .iter()
        .find_map(|dir| fs::read_to_string(dir.join("peer.json")).ok())
        .map(|text| parse_peer_hosts(&text))
        .unwrap_or_default()
}

/// `git rev-parse --show-toplevel`, without `GH_TOKEN` or `GITHUB_TOKEN`.
fn git_repo_root_command() -> Command {
    let mut cmd = Command::new(crate::brains::git_binary());
    cmd.args(["rev-parse", "--show-toplevel"])
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    crate::brains::strip_github_credentials(&mut cmd);
    cmd
}

/// The current git repository's root, if the current directory is inside one.
fn git_repo_root() -> Option<PathBuf> {
    let output = git_repo_root_command().output().ok()?;
    let root = String::from_utf8(output.stdout).ok()?;
    let root = root.trim();
    (output.status.success() && !root.is_empty()).then(|| PathBuf::from(root))
}

/// Reads `alias` and `host` from `peer.json` text, dropping any value `ssh` must not receive.
fn parse_peer_hosts(json: &str) -> Vec<String> {
    let Ok(peer) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    ["alias", "host"]
        .into_iter()
        .filter_map(|key| peer.get(key).and_then(Value::as_str))
        .filter(|host| validate_ssh_host(host).is_ok())
        .map(String::from)
        .collect()
}

/// Connection-only settings resolved from `ssh -G -- <host>`, excluding all port forwardings so
/// `-F none` can connect through aliases without inheriting `LocalForward` entries (such as `7421`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ResolvedSshConfig {
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_files: Vec<String>,
    identities_only: bool,
    proxy_jump: Option<String>,
    proxy_command: Option<String>,
    host_key_alias: Option<String>,
    strict_host_key_checking: Option<String>,
}

/// Pure parser for `ssh -G` output. Keeps connection settings and drops `localforward`,
/// `remoteforward` and `dynamicforward` so bound sibling ports (like `7421`) never collide.
fn parse_ssh_effective_config(raw: &str) -> ResolvedSshConfig {
    let mut cfg = ResolvedSshConfig::default();
    for line in raw.lines() {
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once(char::is_whitespace) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || value.chars().any(char::is_control) {
            continue;
        }
        match key.to_ascii_lowercase().as_str() {
            "hostname" => cfg.hostname = Some(value.to_string()),
            "user" => cfg.user = Some(value.to_string()),
            "port" => {
                if let Ok(port) = value.parse::<u16>()
                    && port > 0
                {
                    cfg.port = Some(port);
                }
            }
            "identityfile" => cfg.identity_files.push(value.to_string()),
            "identitiesonly" => cfg.identities_only = value.eq_ignore_ascii_case("yes"),
            "proxyjump" if !value.eq_ignore_ascii_case("none") => {
                cfg.proxy_jump = Some(value.to_string());
            }
            "proxycommand" if !value.eq_ignore_ascii_case("none") => {
                cfg.proxy_command = Some(value.to_string());
            }
            "hostkeyalias" if !value.eq_ignore_ascii_case("none") => {
                cfg.host_key_alias = Some(value.to_string());
            }
            "stricthostkeychecking" if !value.eq_ignore_ascii_case("ask") => {
                cfg.strict_host_key_checking = Some(value.to_string());
            }
            // `localforward`, `remoteforward`, `dynamicforward` and all other directives are
            // deliberately ignored so `-F none` carries only the single Reviewer `-L` forward.
            _ => {}
        }
    }
    cfg
}

/// Builds `ssh -G -- <host>` with GitHub credentials stripped. Opens no network socket.
fn ssh_effective_config_command(host: &str) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args(["-G", "--", host])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::brains::strip_github_credentials(&mut cmd);
    cmd
}

/// Evaluates local OpenSSH configuration for `host` via `ssh -G -- <host>`.
fn resolve_ssh_effective_config(host: &str) -> Result<ResolvedSshConfig, String> {
    validate_ssh_host(host)?;
    let out = ssh_effective_config_command(host)
        .output()
        .map_err(|err| format!("failed to run `ssh -G` for `{host}`: {err}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            format!("`ssh -G -- {host}` exited with {}", out.status)
        } else {
            format!("`ssh -G -- {host}` failed: {detail}")
        });
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(parse_ssh_effective_config(&stdout))
}

/// The `ssh` arguments for the fallback tunnel with `-F none` so `~/.ssh/config` `LocalForward`
/// entries (such as port `7421` when Pixel Office is already bound) are never inherited, plus one
/// `-L` for the Reviewer port and any resolved connection settings.
fn reviewer_fallback_tunnel_args_with_config(
    host: &str,
    port: u16,
    resolved: Option<&ResolvedSshConfig>,
) -> Vec<String> {
    let mut args: Vec<String> = [
        "-F",
        "none",
        "-N",
        "-o",
        "ExitOnForwardFailure=yes",
        "-o",
        "ServerAliveInterval=30",
        "-o",
        "ServerAliveCountMax=3",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "BatchMode=yes",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    if let Some(cfg) = resolved {
        if let Some(hostname) = &cfg.hostname {
            args.push("-o".to_string());
            args.push(format!("HostName={hostname}"));
        }
        if let Some(user) = &cfg.user {
            args.push("-o".to_string());
            args.push(format!("User={user}"));
        }
        if let Some(ssh_port) = cfg.port {
            args.push("-p".to_string());
            args.push(ssh_port.to_string());
        }
        for identity in &cfg.identity_files {
            args.push("-i".to_string());
            args.push(identity.clone());
        }
        if cfg.identities_only {
            args.push("-o".to_string());
            args.push("IdentitiesOnly=yes".to_string());
        }
        if let Some(jump) = &cfg.proxy_jump {
            args.push("-J".to_string());
            args.push(jump.clone());
        }
        if let Some(proxy_cmd) = &cfg.proxy_command {
            args.push("-o".to_string());
            args.push(format!("ProxyCommand={proxy_cmd}"));
        }
        if let Some(alias) = &cfg.host_key_alias {
            args.push("-o".to_string());
            args.push(format!("HostKeyAlias={alias}"));
        }
        if let Some(strict) = &cfg.strict_host_key_checking {
            args.push("-o".to_string());
            args.push(format!("StrictHostKeyChecking={strict}"));
        }
    }

    args.push("-L".to_string());
    args.push(format!("127.0.0.1:{port}:127.0.0.1:{port}"));
    // `--` ends option parsing so the host can never be read as an ssh flag.
    args.push("--".to_string());
    args.push(host.to_string());
    args
}

/// Pure fallback tunnel argument builder without pre-resolved `ssh -G` output.
#[cfg(test)]
fn reviewer_fallback_tunnel_args(host: &str, port: u16) -> Vec<String> {
    reviewer_fallback_tunnel_args_with_config(host, port, None)
}

/// The fallback `ssh` tunnel process, with no stdio and without `GH_TOKEN` or `GITHUB_TOKEN`.
fn reviewer_fallback_tunnel_command_with_config(
    host: &str,
    port: u16,
    resolved: Option<&ResolvedSshConfig>,
) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args(reviewer_fallback_tunnel_args_with_config(
        host, port, resolved,
    ))
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    crate::brains::strip_github_credentials(&mut cmd);
    cmd
}

#[cfg(test)]
fn reviewer_fallback_tunnel_command(host: &str, port: u16) -> Command {
    reviewer_fallback_tunnel_command_with_config(host, port, None)
}

/// What one tick of the fallback tunnel poll decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FallbackStep {
    Ready,
    Wait,
    FailExited,
    FailTimedOut,
}

/// Decides one poll tick. An open port wins; then an exited `ssh`; then the deadline.
fn fallback_poll_step(port_open: bool, child_exited: bool, deadline_passed: bool) -> FallbackStep {
    if port_open {
        FallbackStep::Ready
    } else if child_exited {
        FallbackStep::FailExited
    } else if deadline_passed {
        FallbackStep::FailTimedOut
    } else {
        FallbackStep::Wait
    }
}

/// Polls the local port until the tunnel answers. On failure the `ssh` child is killed if it is
/// still running and always reaped, so no stray process is left holding the port.
fn wait_for_fallback_tunnel(
    child: &mut std::process::Child,
    local_addr: &std::net::SocketAddr,
    port: u16,
) -> Result<(), String> {
    let deadline = Instant::now() + FALLBACK_TUNNEL_DEADLINE;
    loop {
        let port_open = TcpStream::connect_timeout(local_addr, Duration::from_millis(250)).is_ok();
        // A `try_wait` error means the child can no longer be tracked: treat it as gone.
        let child_exited = !matches!(child.try_wait(), Ok(None));
        let reason = match fallback_poll_step(port_open, child_exited, Instant::now() >= deadline) {
            FallbackStep::Ready => return Ok(()),
            FallbackStep::Wait => {
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            FallbackStep::FailExited => "ssh exited before the tunnel came up",
            FallbackStep::FailTimedOut => "the tunnel did not answer within 5 s",
        };
        if matches!(child.try_wait(), Ok(None)) {
            let _ = child.kill();
        }
        let _ = child.wait();
        return Err(format!(
            "could not tunnel the Reviewer dashboard on 127.0.0.1:{port}: {reason}; \
             check the services with `cheap-otto dashboards status --execute`"
        ));
    }
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

/// The parts of an HTTP request head the server acts on.
#[derive(Debug, PartialEq, Eq)]
struct RequestHead<'a> {
    method: &'a str,
    target: &'a str,
    host: Option<&'a str>,
    origin: Option<&'a str>,
    content_type: Option<&'a str>,
    content_length: usize,
}

impl<'a> RequestHead<'a> {
    /// Parses the request line and the headers the guard and the body reader need. Never fails:
    /// a missing request line reads as `GET /` and unknown headers are ignored.
    fn parse(head: &'a str) -> Self {
        let mut lines = head.lines();
        let mut request_line = lines.next().unwrap_or("").split_whitespace();
        let mut parsed = Self {
            method: request_line.next().unwrap_or("GET"),
            target: request_line.next().unwrap_or("/"),
            host: None,
            origin: None,
            content_type: None,
            content_length: 0,
        };
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim());
            if name.eq_ignore_ascii_case("host") {
                record_single_valued(&mut parsed.host, value);
            } else if name.eq_ignore_ascii_case("origin") {
                record_single_valued(&mut parsed.origin, value);
            } else if name.eq_ignore_ascii_case("content-type") {
                record_single_valued(&mut parsed.content_type, value);
            } else if name.eq_ignore_ascii_case("content-length") {
                parsed.content_length = value.parse::<usize>().unwrap_or(0);
            }
        }
        parsed
    }
}

/// Stores a header that must appear once. A repeat with a different value blanks it, so the
/// guard refuses the request instead of trusting whichever copy came first.
fn record_single_valued<'a>(slot: &mut Option<&'a str>, value: &'a str) {
    match *slot {
        None => *slot = Some(value),
        Some(previous) if previous != value => *slot = Some(""),
        Some(_) => {}
    }
}

fn handle_http_connection<S: Read + Write>(
    stream: &mut S,
    ctx: &ServerContext,
) -> Result<(), std::io::Error> {
    let mut buf = vec![0u8; 65_536];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }

    let raw = String::from_utf8_lossy(&buf[..n]);
    let (head_text, body_part) = match raw.split_once("\r\n\r\n") {
        Some((h, b)) => (h, b.to_string()),
        None => (raw.as_ref(), String::new()),
    };
    let head = RequestHead::parse(head_text);
    let method = head.method;
    let content_len = head.content_length;

    // Nothing below this point runs for a request the guard refuses: not the page, not a route.
    let verdict = ctx
        .guard
        .evaluate(method, head.host, head.origin)
        .and_then(|()| require_json_body(method, head.content_type, content_len));
    if let Err(rejection) = verdict {
        let bytes =
            serde_json::to_vec(&json!({ "error": rejection.to_string() })).unwrap_or_default();
        return write_http_response(
            stream,
            rejection.http_status(),
            "application/json; charset=utf-8",
            &bytes,
        );
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

    let (path, query) = match head.target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (head.target, ""),
    };

    if method == "GET"
        && (path == "/"
            || path == "/index.html"
            || path == "/reviewer"
            || path == "/tender"
            || path == "/pixel")
    {
        return write_http_response(
            stream,
            200,
            "text/html; charset=utf-8",
            DASHBOARD_HTML.as_bytes(),
        );
    }

    let result = dispatch_api(method, path, query, &body_str, ctx);
    match result {
        Ok(val) => {
            let bytes = serde_json::to_vec(&val).unwrap_or_else(|_| b"{}".to_vec());
            write_http_response(stream, 200, "application/json; charset=utf-8", &bytes)
        }
        Err(msg) => {
            let bytes = serde_json::to_vec(&json!({ "error": msg })).unwrap_or_default();
            write_http_response(stream, 400, "application/json; charset=utf-8", &bytes)
        }
    }
}

fn write_http_response<S: Write>(
    stream: &mut S,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), std::io::Error> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        415 => "Unsupported Media Type",
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
        ("GET", "/api/pixel-office") => Ok(build_pixel_office_payload()),
        ("POST", "/api/pixel-office/start") => start_pixel_agents_server(),
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
        ("GET", "/api/tender-agent") => {
            let agent_id = parse_query_param(query, "id")
                .ok_or_else(|| "missing `id` query parameter".to_string())?;
            build_tender_agent_payload(&ctx.workspace, &agent_id)
        }
        ("POST", "/api/tender-agent/save") => {
            #[derive(Deserialize)]
            struct Req {
                agent: String,
                prompt: Option<String>,
                notes: Option<String>,
                #[serde(default)]
                lessons: Option<String>,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            save_tender_agent_notes(
                &ctx.workspace,
                &req.agent,
                req.prompt.as_deref(),
                req.notes.as_deref(),
            )?;
            if let Some(lessons) = req.lessons {
                save_tender_agent_lessons(&ctx.workspace, &req.agent, &lessons)?;
            }
            Ok(json!({ "ok": true }))
        }
        ("GET", "/api/tender-kb") => Ok(load_tender_kb_payload(&ctx.workspace)),
        ("POST", "/api/tender-kb/add") => {
            #[derive(Deserialize)]
            struct Req {
                target: String,
                #[serde(default)]
                target_agent: Option<String>,
                rule: String,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let added = append_tender_kb_entry(
                &ctx.workspace,
                &req.target,
                req.target_agent.as_deref(),
                &req.rule,
            )?;
            Ok(json!({ "ok": true, "added": added }))
        }
        ("POST", "/api/tender-kb/save") => {
            #[derive(Deserialize)]
            struct Req {
                file_key: String,
                content: String,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            save_tender_kb_file(&ctx.workspace, &req.file_key, &req.content)?;
            Ok(json!({ "ok": true }))
        }
        ("GET", "/api/repos/detect-local") => {
            let raw_repo = parse_query_param(query, "repo")
                .ok_or_else(|| "missing `repo` query parameter".to_string())?;
            let repo = normalize_repo_input(&raw_repo).unwrap_or(raw_repo);
            let detected = detect_local_repo_checkout(&repo);
            let inferred_check_cmd = detected.as_deref().and_then(infer_local_check_cmd);
            let git_info = detected
                .as_deref()
                .and_then(|p| inspect_local_git_repo(&p.display().to_string()).ok());
            Ok(json!({
                "repo": repo,
                "local_repo_path": detected.map(|p| p.display().to_string()),
                "inferred_check_cmd": inferred_check_cmd,
                "default_branch": git_info.as_ref().and_then(|v| v["default_branch"].as_str()),
            }))
        }
        ("GET", "/api/m1/browse") => {
            let path = parse_query_param(query, "path");
            browse_m1_directories(path.as_deref())
        }
        ("GET", "/api/m1/git-info") => {
            let path = parse_query_param(query, "path")
                .ok_or_else(|| "missing `path` query parameter".to_string())?;
            inspect_local_git_repo(&path)
        }
        ("POST", "/api/actions/poll") => {
            spawn_detached_self(&ctx.workspace, "sched-pr-poller.log", &["poll"])?;
            Ok(json!({ "ok": true, "message": "Dispatched parallel async poll batch!" }))
        }
        ("POST", "/api/actions/tend-poll") => {
            spawn_detached_self(&ctx.workspace, "sched-code-tender.log", &["tend-poll"])?;
            Ok(json!({ "ok": true, "message": "Dispatched Code Tender GraphQL poll batch!" }))
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
                "sched-pr-poller.log",
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
        ("POST", "/api/actions/tend") => {
            #[derive(Deserialize)]
            struct Req {
                repo: String,
                pr: u64,
                #[serde(default)]
                stage_only: bool,
                #[serde(default)]
                lead_runner: Option<String>,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let state_dir = expand_tilde(&cfg.state_dir);
            let pr_key = LedgerStore::pr_key(&req.repo, req.pr);
            let slot = LedgerStore::with_exclusive_lock(&state_dir, |store| {
                let s = store.tender_dispatch_counter;
                store.tender_dispatch_counter = store.tender_dispatch_counter.wrapping_add(1);
                if let Some(t) = store.tender_pull_requests.get_mut(&pr_key) {
                    t.attempts_for_key = 0;
                    t.last_attempt_key = None;
                }
                s
            })
            .unwrap_or(0);
            let lead = req
                .lead_runner
                .as_deref()
                .and_then(CliRunner::from_slug)
                .map(CliRunner::slug)
                .unwrap_or(if slot.is_multiple_of(2) {
                    "claude"
                } else {
                    "agy"
                });
            let pr_str = req.pr.to_string();
            let mut args = vec![
                "tend",
                "--repo",
                &req.repo,
                "--pr",
                &pr_str,
                "--lead-runner",
                lead,
            ];
            if req.stage_only {
                args.push("--stage-only");
            }
            spawn_detached_self(&ctx.workspace, "sched-code-tender.log", &args)?;
            Ok(json!({
                "ok": true,
                "message": format!("Started Code Tender async session for {}#{} (lead: {})", req.repo, req.pr, lead)
            }))
        }
        ("POST", "/api/actions/tend/approve-push") => {
            #[derive(Deserialize)]
            struct Req {
                repo: String,
                pr: u64,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let repo_cfg = cfg
                .repos
                .iter()
                .find(|r| r.full_name.eq_ignore_ascii_case(&req.repo))
                .ok_or_else(|| format!("repo `{}` not configured", req.repo))?;
            let state_dir = expand_tilde(&cfg.state_dir);
            let gh = GitHubClient::new(&cfg);
            let sha = approve_and_push_staged_patch(&cfg, repo_cfg, &state_dir, &gh, req.pr)
                .map_err(|e| e.to_string())?;
            Ok(json!({
                "ok": true,
                "sha": sha,
                "message": format!("Pushed staged fix commit {} for {}#{}", &sha[..7.min(sha.len())], req.repo, req.pr)
            }))
        }
        ("POST", "/api/actions/tend/discard-patch") => {
            #[derive(Deserialize)]
            struct Req {
                repo: String,
                pr: u64,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let state_dir = expand_tilde(&cfg.state_dir);
            let key = LedgerStore::pr_key(&req.repo, req.pr);
            LedgerStore::with_exclusive_lock(&state_dir, |store| {
                if let Some(t) = store.tender_pull_requests.get_mut(&key) {
                    t.staged_patch = None;
                    t.staged_commit_message = None;
                    t.pending_items.clear();
                    if t.last_status == TenderRunStatus::StagedPatchReady
                        || t.last_status == TenderRunStatus::CheckFailed
                    {
                        t.last_status = TenderRunStatus::Idle;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "message": format!("Discarded staged patch for {}", key) }))
        }
        ("GET", "/api/tender/open-prs") => {
            let repo_filter = parse_query_param(query, "repo")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let state_dir = expand_tilde(&cfg.state_dir);
            let gh = GitHubClient::new(&cfg);
            let mut open_prs = Vec::new();
            let mut errors = Vec::new();

            let initial_ledger = LedgerStore::load_or_default(&state_dir).unwrap_or_default();
            for repo_cfg in &cfg.repos {
                if let Some(ref rf) = repo_filter
                    && !repo_cfg.full_name.eq_ignore_ascii_case(rf)
                {
                    continue;
                }
                let explicit_watched_prs: Vec<u64> = if repo_cfg.tender_allowed_authors.is_empty() {
                    initial_ledger
                        .tender_pull_requests
                        .values()
                        .filter(|t| {
                            t.repo.eq_ignore_ascii_case(&repo_cfg.full_name)
                                && t.watched
                                && t.is_open
                        })
                        .map(|t| t.pr_number)
                        .collect()
                } else {
                    Vec::new()
                };
                let poll_account = repo_cfg
                    .forced_account
                    .as_deref()
                    .unwrap_or(&cfg.accounts.primary);
                match gh.poll_tender_repository_for_repo(
                    poll_account,
                    &cfg,
                    repo_cfg,
                    &explicit_watched_prs,
                ) {
                    Ok(outcome) => {
                        let snaps = outcome.snapshots;
                        let _ = sync_watched_prs_from_snapshots_with_gh(
                            &cfg,
                            repo_cfg,
                            &state_dir,
                            Some(&gh),
                            &snaps,
                            outcome
                                .open_list_complete
                                .then_some(outcome.open_pr_numbers.as_slice()),
                        );
                        let ledger = LedgerStore::load_or_default(&state_dir).unwrap_or_default();
                        for snap in snaps {
                            let key = LedgerStore::pr_key(&repo_cfg.full_name, snap.pr_number);
                            let existing = ledger.tender_pull_requests.get(&key);
                            let watched = existing.map(|e| e.watched).unwrap_or(false);
                            let feedback_count =
                                existing.map(|e| e.open_feedback_count).unwrap_or(0);
                            open_prs.push(json!({
                                "key": key,
                                "repo": repo_cfg.full_name,
                                "pr_number": snap.pr_number,
                                "title": snap.title,
                                "author": snap.author,
                                "base_ref": snap.base_ref,
                                "head_ref": snap.head_ref,
                                "head_sha": snap.head_sha,
                                "is_draft": snap.is_draft,
                                "is_fork": snap.is_fork,
                                "watched": watched,
                                "open_feedback_count": feedback_count,
                            }));
                        }
                    }
                    Err(e) => {
                        errors.push(format!("{}: {}", repo_cfg.full_name, e));
                    }
                }
            }
            open_prs.sort_by(|a, b| {
                let b_num = b["pr_number"].as_u64().unwrap_or(0);
                let a_num = a["pr_number"].as_u64().unwrap_or(0);
                b_num.cmp(&a_num)
            });
            Ok(json!({
                "ok": true,
                "prs": open_prs,
                "errors": errors,
            }))
        }
        ("POST", "/api/tender/watch-pr") => {
            #[derive(Deserialize)]
            struct Req {
                #[serde(default)]
                input: Option<String>,
                #[serde(default)]
                repo: Option<String>,
                #[serde(default)]
                pr: Option<u64>,
                #[serde(default)]
                watched: Option<bool>,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let (raw_repo, pr_num) = if let Some(raw_in) = req
                .input
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                parse_pr_reference_input(raw_in)?
            } else {
                let r = req.repo.as_deref().ok_or_else(|| {
                    "provide a PR URL (`owner/repo#123`) or `repo` + `pr`".to_string()
                })?;
                let norm = normalize_repo_input(r).map_err(|e| e.to_string())?;
                let p = req
                    .pr
                    .filter(|&n| n > 0)
                    .ok_or_else(|| "PR number must be greater than zero".to_string())?;
                (norm, p)
            };

            let watch_flag = req.watched.unwrap_or(true);
            let mut cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let state_dir = expand_tilde(&cfg.state_dir);

            let canonical_repo = cfg
                .repos
                .iter()
                .find(|r| r.full_name.eq_ignore_ascii_case(&raw_repo))
                .map(|r| r.full_name.clone())
                .unwrap_or_else(|| raw_repo.clone());
            let key = LedgerStore::pr_key(&canonical_repo, pr_num);

            if !watch_flag {
                LedgerStore::with_exclusive_lock(&state_dir, |store| {
                    if let Some(tpr) = store.tender_pull_requests.get_mut(&key) {
                        tpr.watched = false;
                    }
                })
                .map_err(|e| e.to_string())?;
                return Ok(json!({
                    "ok": true,
                    "watched": false,
                    "key": key,
                    "message": format!("Paused watching {}#{}", canonical_repo, pr_num),
                }));
            }

            if !cfg
                .repos
                .iter()
                .any(|r| r.full_name.eq_ignore_ascii_case(&canonical_repo))
            {
                let detected = detect_local_repo_checkout(&canonical_repo);
                let inferred_check_cmd = detected.as_deref().and_then(infer_local_check_cmd);
                let default_branch = detected
                    .as_deref()
                    .and_then(|p| inspect_local_git_repo(&p.display().to_string()).ok())
                    .and_then(|v| v["default_branch"].as_str().map(str::to_string))
                    .unwrap_or_else(|| "main".to_string());
                cfg.upsert_repo(WatchedRepoConfig {
                    full_name: canonical_repo.clone(),
                    default_branch,
                    language: "en".to_string(),
                    account_policy: AccountPolicy::Swap,
                    forced_account: None,
                    review_event_policy: ReviewEventPolicy::FormalVerdict,
                    gdrive_folder_id: None,
                    local_repo_path: detected.map(|p| p.display().to_string()),
                    local_check_cmd: inferred_check_cmd,
                    enabled: false,
                    tender_enabled: false,
                    tender_allowed_authors: Vec::new(),
                    review_allowed_authors: Vec::new(),
                    skip_fork_prs: true,
                })
                .map_err(|e| e.to_string())?;
                cfg.save(&ctx.config_path).map_err(|e| e.to_string())?;
            }

            let repo_cfg = cfg
                .repos
                .iter()
                .find(|r| r.full_name.eq_ignore_ascii_case(&canonical_repo))
                .cloned()
                .ok_or_else(|| format!("repo `{}` not found", canonical_repo))?;

            let gh = GitHubClient::new(&cfg);
            let poll_account = repo_cfg
                .forced_account
                .as_deref()
                .unwrap_or(&cfg.accounts.primary);
            let explicit_for_watch: Vec<u64> = if repo_cfg.tender_allowed_authors.is_empty() {
                vec![pr_num]
            } else {
                Vec::new()
            };
            let matched_snap = match gh.poll_tender_repository_for_repo(
                poll_account,
                &cfg,
                &repo_cfg,
                &explicit_for_watch,
            ) {
                Ok(outcome) => {
                    if outcome.skipped_forks.contains(&pr_num) {
                        return Err(format!(
                            "cannot watch {}#{}: cross-repository fork PRs are blocked from Code Tender",
                            repo_cfg.full_name, pr_num
                        ));
                    }
                    if let Some((_, blocked_author)) =
                        outcome.skipped_authors.iter().find(|(n, _)| *n == pr_num)
                    {
                        return Err(format!(
                            "cannot watch {}#{}: author `{}` is not in `tender_allowed_authors` ({:?})",
                            repo_cfg.full_name,
                            pr_num,
                            blocked_author,
                            repo_cfg.tender_allowed_authors
                        ));
                    }
                    let snaps = outcome.snapshots;
                    let _ = sync_watched_prs_from_snapshots_with_gh(
                        &cfg,
                        &repo_cfg,
                        &state_dir,
                        Some(&gh),
                        &snaps,
                        outcome
                            .open_list_complete
                            .then_some(outcome.open_pr_numbers.as_slice()),
                    );
                    snaps.into_iter().find(|s| s.pr_number == pr_num)
                }
                Err(_) => None,
            };

            let mut ready_feedback_count = 0usize;
            LedgerStore::with_exclusive_lock(&state_dir, |store| {
                if let Some(ref snap) = matched_snap {
                    ready_feedback_count = match evaluate_tender_trigger(
                        store.tender_pull_requests.get(&key),
                        snap,
                        true,
                        u32::MAX,
                        cfg.tender.ignore_clean_pass_reviews,
                    ) {
                        TenderTriggerDecision::HandleFeedback { items, .. } => items.len(),
                        _ => 0,
                    };
                }

                if let Some(existing) = store.tender_pull_requests.get_mut(&key) {
                    existing.watched = true;
                    existing.is_open = true;
                    if let Some(ref snap) = matched_snap {
                        existing.title = snap.title.clone();
                        existing.author = snap.author.clone();
                        existing.base_ref = snap.base_ref.clone();
                        existing.head_ref = snap.head_ref.clone();
                        existing.head_sha = Some(snap.head_sha.clone());
                        existing.open_feedback_count = ready_feedback_count;
                    }
                } else {
                    let (title, author, base_ref, head_ref, head_sha) =
                        if let Some(ref snap) = matched_snap {
                            (
                                snap.title.clone(),
                                snap.author.clone(),
                                snap.base_ref.clone(),
                                snap.head_ref.clone(),
                                Some(snap.head_sha.clone()),
                            )
                        } else {
                            (
                                format!("Pull Request #{}", pr_num),
                                String::new(),
                                repo_cfg.default_branch.clone(),
                                String::new(),
                                None,
                            )
                        };
                    let mut fresh = TenderPrState::new(
                        &repo_cfg.full_name,
                        pr_num,
                        &title,
                        &author,
                        &base_ref,
                        &head_ref,
                    );
                    fresh.watched = true;
                    fresh.is_open = true;
                    fresh.head_sha = head_sha;
                    fresh.open_feedback_count = ready_feedback_count;
                    store.tender_pull_requests.insert(key.clone(), fresh);
                }
            })
            .map_err(|e| e.to_string())?;

            let already_running = ActiveTenderWorkerState::list_active(&state_dir)
                .iter()
                .any(|w| w.repo.eq_ignore_ascii_case(&repo_cfg.full_name) && w.pr_number == pr_num);
            let mut dispatched = false;
            if ready_feedback_count > 0
                && cfg.tender.enabled
                && !cfg.tender.poll_only_mode
                && !already_running
            {
                let pr_str = pr_num.to_string();
                let _ = spawn_detached_self(
                    &ctx.workspace,
                    "sched-code-tender.log",
                    &["tend", "--repo", &repo_cfg.full_name, "--pr", &pr_str],
                );
                dispatched = true;
            }

            let msg = if dispatched {
                format!(
                    "Watching {}#{} — found {} review comment(s) and started tending immediately!",
                    repo_cfg.full_name, pr_num, ready_feedback_count
                )
            } else {
                format!(
                    "Watching {}#{} for review comments (checked every 2m)",
                    repo_cfg.full_name, pr_num
                )
            };
            Ok(json!({
                "ok": true,
                "watched": true,
                "key": key,
                "dispatched": dispatched,
                "open_feedback_count": ready_feedback_count,
                "message": msg,
            }))
        }
        ("POST", "/api/tender/remove-pr") => {
            #[derive(Deserialize)]
            struct Req {
                repo: String,
                pr: u64,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let state_dir = expand_tilde(&cfg.state_dir);
            let repo_auto_watches = cfg
                .repos
                .iter()
                .find(|r| r.full_name.eq_ignore_ascii_case(&req.repo))
                .map(|r| r.tender_enabled)
                .unwrap_or(false);
            let key = LedgerStore::pr_key(&req.repo, req.pr);
            let mut paused_instead = false;
            LedgerStore::with_exclusive_lock(&state_dir, |store| {
                if let Some(existing) = store.tender_pull_requests.get_mut(&key)
                    && repo_auto_watches
                    && existing.is_open
                {
                    existing.watched = false;
                    paused_instead = true;
                    return;
                }
                store.tender_pull_requests.remove(&key);
            })
            .map_err(|e| e.to_string())?;
            let msg = if paused_instead {
                format!("Paused {} (repo auto-watch is enabled)", key)
            } else {
                format!("Removed {} from list", key)
            };
            Ok(json!({ "ok": true, "paused": paused_instead, "message": msg }))
        }
        ("POST", "/api/tender/respond-item") => {
            #[derive(Deserialize)]
            struct Req {
                repo: String,
                pr: u64,
                feedback_id: String,
                #[serde(default)]
                reply_text: String,
                #[serde(default)]
                create_issue: bool,
                #[serde(default)]
                issue_title: Option<String>,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            let repo_cfg = cfg
                .repos
                .iter()
                .find(|r| r.full_name.eq_ignore_ascii_case(&req.repo))
                .ok_or_else(|| format!("repo `{}` not configured", req.repo))?;
            let state_dir = expand_tilde(&cfg.state_dir);
            let gh = GitHubClient::new(&cfg);
            let (issue_number, issue_url) = respond_and_resolve_tender_item(
                &cfg,
                repo_cfg,
                &state_dir,
                &gh,
                req.pr,
                &req.feedback_id,
                &req.reply_text,
                req.create_issue,
                req.issue_title.as_deref(),
            )
            .map_err(|e| e.to_string())?;
            let msg = match issue_number {
                Some(num) => format!(
                    "Created issue #{}, linked it on the PR thread, and resolved the thread!",
                    num
                ),
                None => "Posted reply on PR thread and resolved the thread!".to_string(),
            };
            Ok(json!({
                "ok": true,
                "issue_number": issue_number,
                "issue_url": issue_url,
                "message": msg,
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
        ("POST", "/api/settings/tender") => {
            #[derive(Deserialize)]
            struct Req {
                enabled: bool,
                poll_only_mode: bool,
                delivery_mode: String,
                intercalate_providers: bool,
                max_parallel_prs: usize,
                max_parallel_fixers_per_pr: usize,
                max_autonomous_cycles_per_pr: u32,
                resolve_threads_on_fix: bool,
                ignore_clean_pass_reviews: bool,
                #[serde(default)]
                allowed_authors: Vec<String>,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let mut cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            cfg.tender.enabled = req.enabled;
            cfg.tender.poll_only_mode = req.poll_only_mode;
            cfg.tender.delivery_mode = if req.delivery_mode.eq_ignore_ascii_case("stage_patch_only")
            {
                TenderDeliveryMode::StagePatchOnly
            } else {
                TenderDeliveryMode::AutoPush
            };
            cfg.tender.intercalate_providers = req.intercalate_providers;
            cfg.tender.max_parallel_prs = req.max_parallel_prs.clamp(1, 12);
            cfg.tender.max_parallel_fixers_per_pr = req.max_parallel_fixers_per_pr.clamp(1, 8);
            cfg.tender.max_autonomous_cycles_per_pr = req.max_autonomous_cycles_per_pr.clamp(1, 10);
            cfg.tender.resolve_threads_on_fix = req.resolve_threads_on_fix;
            cfg.tender.ignore_clean_pass_reviews = req.ignore_clean_pass_reviews;
            cfg.tender.allowed_authors = req
                .allowed_authors
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
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
                #[serde(default)]
                local_repo_path: Option<String>,
                local_check_cmd: Option<String>,
                #[serde(default)]
                enabled: Option<bool>,
                #[serde(default)]
                tender_enabled: Option<bool>,
                #[serde(default)]
                tender_allowed_authors: Option<Vec<String>>,
                #[serde(default)]
                review_allowed_authors: Option<Vec<String>>,
                #[serde(default)]
                skip_fork_prs: Option<bool>,
            }
            let req: Req = serde_json::from_str(body).map_err(|e| e.to_string())?;
            let mut cfg = WorkspaceConfig::load(&ctx.config_path).map_err(|e| e.to_string())?;
            // Omitted allowlist/fork fields keep the stored values; an empty review allowlist means everyone is allowed.
            let stored = cfg
                .repos
                .iter()
                .find(|r| r.full_name.eq_ignore_ascii_case(&req.full_name))
                .cloned();
            let clean_authors = |list: Vec<String>| -> Vec<String> {
                list.into_iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            cfg.upsert_repo(WatchedRepoConfig {
                full_name: req.full_name,
                default_branch: req.default_branch,
                language: "en".to_string(),
                account_policy: if req.account_policy.eq_ignore_ascii_case("forced") {
                    AccountPolicy::Forced
                } else {
                    AccountPolicy::Swap
                },
                forced_account: req.forced_account.filter(|s| !s.trim().is_empty()),
                review_event_policy: if req.review_event_policy.eq_ignore_ascii_case("comment_only")
                {
                    ReviewEventPolicy::CommentOnly
                } else {
                    ReviewEventPolicy::FormalVerdict
                },
                gdrive_folder_id: req.gdrive_folder_id.filter(|s| !s.trim().is_empty()),
                local_repo_path: req.local_repo_path.filter(|s| !s.trim().is_empty()),
                local_check_cmd: req.local_check_cmd.filter(|s| !s.trim().is_empty()),
                enabled: req.enabled.unwrap_or(true),
                tender_enabled: req.tender_enabled.unwrap_or(true),
                tender_allowed_authors: req.tender_allowed_authors.map_or_else(
                    || {
                        stored
                            .as_ref()
                            .map(|r| r.tender_allowed_authors.clone())
                            .unwrap_or_default()
                    },
                    clean_authors,
                ),
                review_allowed_authors: req.review_allowed_authors.map_or_else(
                    || {
                        stored
                            .as_ref()
                            .map(|r| r.review_allowed_authors.clone())
                            .unwrap_or_default()
                    },
                    clean_authors,
                ),
                skip_fork_prs: req
                    .skip_fork_prs
                    .or_else(|| stored.as_ref().map(|r| r.skip_fork_prs))
                    .unwrap_or(true),
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
        ("POST", "/api/repos/tender-toggle") => {
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
                r.tender_enabled = !r.tender_enabled;
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

    let recent_logs = read_tail_logs(
        &home_dir().join(".cheap-otto/logs/code-reviewer"),
        "sched-pr-poller.log",
        45,
    );
    let run_history = read_recent_history(
        &home_dir().join(".cheap-otto/runs/code-reviewer/history.jsonl"),
        12,
    );

    let tender_active_workers = ActiveTenderWorkerState::list_active(&state_dir);
    let mut tender_prs_json: Vec<Value> = ledger
        .tender_pull_requests
        .into_iter()
        .map(|(key, tpr)| {
            let mut v = serde_json::to_value(&tpr).unwrap_or_else(|_| json!({}));
            if let Some(obj) = v.as_object_mut() {
                obj.insert("key".to_string(), json!(key));
                obj.insert("status_badge".to_string(), json!(tpr.last_status.badge()));
            }
            v
        })
        .collect();
    tender_prs_json.sort_by(|a, b| {
        let rank = |v: &Value| -> u8 {
            let is_open = v["is_open"].as_bool().unwrap_or(true);
            let watched = v["watched"].as_bool().unwrap_or(true);
            match (is_open, watched) {
                (true, true) => 0,
                (true, false) => 1,
                (false, _) => 2,
            }
        };
        let ra = rank(a);
        let rb = rank(b);
        ra.cmp(&rb).then_with(|| {
            let b_num = b["pr_number"].as_u64().unwrap_or(0);
            let a_num = a["pr_number"].as_u64().unwrap_or(0);
            b_num.cmp(&a_num)
        })
    });

    let tender_next_lead = if cfg.tender.intercalate_providers {
        if ledger.tender_dispatch_counter.is_multiple_of(2) {
            CliRunner::Claude.slug()
        } else {
            CliRunner::Agy.slug()
        }
    } else {
        cfg.agents.primary_runner.slug()
    };

    let tender_logs = read_tail_logs(
        &home_dir().join(".cheap-otto/logs/code-reviewer"),
        "sched-code-tender.log",
        45,
    );
    let tender_history = read_recent_history(
        &home_dir().join(".cheap-otto/runs/code-reviewer/tender-history.jsonl"),
        12,
    );

    Ok(json!({
        "accounts": {
            "primary": cfg.accounts.primary,
            "secondary": cfg.accounts.secondary,
        },
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
        "tender": cfg.tender,
        "tender_next_lead_runner": tender_next_lead,
        "tender_active_workers": tender_active_workers,
        "tender_pull_requests": tender_prs_json,
        "tender_agents": TENDER_AGENTS,
        "tender_logs": tender_logs,
        "tender_history": tender_history,
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
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()));
        let power_label = if on_ac { "AC Power" } else { "Battery" };
        let summary = match therm {
            Some(t) => format!("M1 Host • {} • thermal={}", power_label, t),
            None => format!("M1 Host • {}", power_label),
        };
        (on_ac, summary)
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
    let lessons = load_brain_kb_section(workspace, brain_id, "lessons.md");
    let fps = load_brain_kb_section(workspace, brain_id, "false-positives.md");
    let exemplars = load_brain_kb_section(workspace, brain_id, "exemplars.md");
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

fn build_tender_agent_payload(workspace: &Path, agent_id: &str) -> Result<Value, String> {
    if !TENDER_AGENTS.contains(&agent_id) {
        return Err(format!("invalid tender agent id `{}`", agent_id));
    }
    let dir = workspace.join("tender-agents").join(agent_id);
    let prompt = fs::read_to_string(dir.join("PROMPT.md")).unwrap_or_default();
    let notes = load_tender_agent_notes(workspace, agent_id);
    let lessons = load_tender_agent_lessons(workspace, agent_id);
    Ok(json!({
        "id": agent_id,
        "prompt": prompt,
        "notes": notes,
        "lessons": lessons,
    }))
}

fn spawn_detached_self(workspace: &Path, log_filename: &str, args: &[&str]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let logs_dir = home_dir().join(".cheap-otto/logs/code-reviewer");
    let _ = fs::create_dir_all(&logs_dir);
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs_dir.join(log_filename))
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

fn read_tail_logs(logs_dir: &Path, filename: &str, max_lines: usize) -> String {
    let mut combined = Vec::new();
    if let Ok(raw) = fs::read_to_string(logs_dir.join(filename)) {
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

fn build_pixel_office_payload() -> Value {
    if let Some((port, token)) = read_pixel_server_auth(DEFAULT_PIXEL_PORT) {
        let addr: std::net::SocketAddr = ([127, 0, 0, 1], port).into();
        let reachable = TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok();
        json!({
            "available": reachable,
            "pixel_port": port,
            "token": token,
        })
    } else {
        let addr: std::net::SocketAddr = ([127, 0, 0, 1], DEFAULT_PIXEL_PORT).into();
        let reachable = TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok();
        json!({
            "available": reachable,
            "pixel_port": DEFAULT_PIXEL_PORT,
            "token": Value::Null,
        })
    }
}

/// What the Pixel Office start button does once the port is known to be closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PixelStartAction {
    /// `launchd` accepted the kickstart, so wait for the port.
    WaitForService,
    /// The kickstart failed on macOS: report it and never spawn an orphan.
    ReportNotInstalled,
    /// No `launchd` on this host, so spawn `pixel-agents` directly.
    SpawnDirectly,
}

/// Decides the start button's action. On macOS only the LaunchAgent may own the Pixel Office
/// port: a process spawned here would live outside `launchd` and hold the port the service
/// needs, so a failed (or missing) kickstart is reported instead of falling back to a spawn.
fn pixel_start_action(is_macos: bool, kickstart_ok: Option<bool>) -> PixelStartAction {
    match (is_macos, kickstart_ok) {
        (false, _) => PixelStartAction::SpawnDirectly,
        (true, Some(true)) => PixelStartAction::WaitForService,
        (true, Some(false) | None) => PixelStartAction::ReportNotInstalled,
    }
}

/// The error the start button returns when `launchctl kickstart` fails on macOS.
fn pixel_office_not_installed_error(detail: &str) -> String {
    format!(
        "the Pixel Office service ({PIXEL_OFFICE_LAUNCHD_LABEL}) is not installed or could not be started ({detail}); install it with `cheap-otto dashboards install --execute`"
    )
}

/// Runs `launchctl kickstart gui/<uid>/<label>` for the Pixel Office LaunchAgent, with the uid
/// from `id -u`. Both spawns take discrete args and run without GitHub credentials.
fn kickstart_pixel_office_service() -> Result<(), String> {
    let mut id_cmd = Command::new("id");
    id_cmd.arg("-u").stdin(Stdio::null()).stderr(Stdio::null());
    crate::brains::strip_github_credentials(&mut id_cmd);
    let id_out = id_cmd
        .output()
        .map_err(|e| format!("`id -u` failed to run: {e}"))?;
    let uid = String::from_utf8_lossy(&id_out.stdout).trim().to_string();
    // The uid lands in a launchctl domain target, so accept digits only.
    if !id_out.status.success() || uid.is_empty() || !uid.bytes().all(|b| b.is_ascii_digit()) {
        return Err("`id -u` did not print a numeric uid".to_string());
    }

    let mut cmd = Command::new("launchctl");
    cmd.args([
        "kickstart".to_string(),
        format!("gui/{uid}/{PIXEL_OFFICE_LAUNCHD_LABEL}"),
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    crate::brains::strip_github_credentials(&mut cmd);
    let status = cmd
        .status()
        .map_err(|e| format!("`launchctl kickstart` failed to run: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`launchctl kickstart` exited with {status}"))
    }
}

fn start_pixel_agents_server() -> Result<Value, String> {
    let existing = build_pixel_office_payload();
    if existing["available"].as_bool() == Some(true) {
        return Ok(json!({
            "ok": true,
            "message": format!("pixel-agents is already listening on 127.0.0.1:{}", existing["pixel_port"])
        }));
    }

    let is_macos = cfg!(target_os = "macos");
    let kickstart = is_macos.then(kickstart_pixel_office_service);
    match pixel_start_action(is_macos, kickstart.as_ref().map(Result::is_ok)) {
        PixelStartAction::WaitForService => {}
        PixelStartAction::ReportNotInstalled => {
            let detail = match kickstart {
                Some(Err(detail)) => detail,
                Some(Ok(())) | None => "kickstart was not attempted".to_string(),
            };
            return Err(pixel_office_not_installed_error(&detail));
        }
        PixelStartAction::SpawnDirectly => spawn_pixel_agents_directly()?,
    }

    for _ in 0..15 {
        thread::sleep(Duration::from_millis(200));
        let p = build_pixel_office_payload();
        if p["available"].as_bool() == Some(true) {
            return Ok(json!({
                "ok": true,
                "message": format!("pixel-agents started on 127.0.0.1:{}", p["pixel_port"])
            }));
        }
    }

    Ok(json!({
        "ok": true,
        "message": "Started pixel-agents; refreshing connection..."
    }))
}

/// Spawns `pixel-agents` directly. Only used off macOS, where there is no LaunchAgent.
fn spawn_pixel_agents_directly() -> Result<(), String> {
    let candidate_bins = ["/opt/homebrew/bin/pixel-agents", "pixel-agents"];
    let bin = candidate_bins
        .iter()
        .find(|p| Path::new(p).exists() || *p == &"pixel-agents")
        .copied()
        .unwrap_or("pixel-agents");

    let mut cmd = Command::new(bin);
    cmd.args([
        "--port",
        &DEFAULT_PIXEL_PORT.to_string(),
        "--host",
        "127.0.0.1",
        "--no-terminal",
    ])
    .env(
        "PATH",
        format!(
            "/opt/homebrew/bin:/usr/local/bin:{}",
            std::env::var("PATH").unwrap_or_default()
        ),
    )
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    crate::brains::strip_github_credentials(&mut cmd);

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to launch `{}`: {}", bin, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const OWN_HOST: &str = "127.0.0.1:7430";
    const OWN_ORIGIN: &str = "http://127.0.0.1:7430";
    const FOREIGN_ORIGIN: &str = "http://evil.example";

    #[test]
    fn fallback_tunnel_forwards_only_the_reviewer_port() {
        let args = reviewer_fallback_tunnel_args("otto-mac", 7430);
        let forwards: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| arg.as_str() == "-L")
            .map(|(i, _)| args.get(i + 1).cloned().unwrap_or_default())
            .collect();
        assert_eq!(forwards, vec!["127.0.0.1:7430:127.0.0.1:7430".to_string()]);
        assert!(!args.iter().any(|arg| arg.contains("7421")));
        for option in [
            "ServerAliveCountMax=3",
            "ConnectTimeout=10",
            "ExitOnForwardFailure=yes",
            "BatchMode=yes",
        ] {
            assert!(args.iter().any(|arg| arg == option), "missing {option}");
        }
        assert_eq!(args[..3], ["-F", "none", "-N"]);
        assert_eq!(args.last().map(String::as_str), Some("otto-mac"));
        assert_eq!(
            args.get(args.len().saturating_sub(2)).map(String::as_str),
            Some("--")
        );

        let custom = reviewer_fallback_tunnel_args("otto-mac", 8430);
        assert_eq!(custom[..2], ["-F", "none"]);
        assert!(custom.contains(&"127.0.0.1:8430:127.0.0.1:8430".to_string()));
        assert!(!custom.iter().any(|arg| arg.contains("7430")));

        let cmd = reviewer_fallback_tunnel_command("otto-mac", 7430);
        assert_eq!(cmd.get_program(), "ssh");
        let removed: Vec<_> = cmd
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_string_lossy().to_string())
            .collect();
        assert!(removed.contains(&"GH_TOKEN".to_string()));
        assert!(removed.contains(&"GITHUB_TOKEN".to_string()));
    }

    #[test]
    fn fallback_tunnel_resolves_ssh_config_without_inheriting_forwards() {
        // Simulates `ssh -G -- otto-mac` when `~/.ssh/config` still contains a `LocalForward`
        // for Pixel Office (`7421` already bound by another tunnel), plus `RemoteForward` and
        // `DynamicForward`: none may enter the fallback args, so `ExitOnForwardFailure=yes`
        // checks only the requested Reviewer port.
        let ssh_g = "\
host otto-mac
user admin
hostname 192.0.2.42
port 2222
identityfile ~/.ssh/id_ed25519
identityfile ~/.ssh/id_rsa
identitiesonly yes
proxyjump bastion.example:22
proxycommand none
hostkeyalias otto-mac-key
stricthostkeychecking accept-new
localforward [127.0.0.1]:7421 [127.0.0.1]:7421
localforward [127.0.0.1]:5173 [127.0.0.1]:5173
remoteforward 9000 127.0.0.1:9000
dynamicforward 1080
";
        let resolved = parse_ssh_effective_config(ssh_g);
        assert_eq!(
            resolved,
            ResolvedSshConfig {
                hostname: Some("192.0.2.42".to_string()),
                user: Some("admin".to_string()),
                port: Some(2222),
                identity_files: vec!["~/.ssh/id_ed25519".to_string(), "~/.ssh/id_rsa".to_string(),],
                identities_only: true,
                proxy_jump: Some("bastion.example:22".to_string()),
                proxy_command: None,
                host_key_alias: Some("otto-mac-key".to_string()),
                strict_host_key_checking: Some("accept-new".to_string()),
            }
        );

        let args = reviewer_fallback_tunnel_args_with_config("otto-mac", 7430, Some(&resolved));
        assert_eq!(args[..2], ["-F", "none"]);
        let l_forwards: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| arg.as_str() == "-L")
            .map(|(i, _)| args.get(i + 1).cloned().unwrap_or_default())
            .collect();
        assert_eq!(
            l_forwards,
            vec!["127.0.0.1:7430:127.0.0.1:7430".to_string()]
        );
        for forbidden in ["7421", "5173", "9000", "1080", "-R", "-D", "LocalForward"] {
            assert!(
                !args.iter().any(|arg| arg.contains(forbidden)),
                "unexpected {forbidden} in {args:?}"
            );
        }
        for expected in [
            "HostName=192.0.2.42",
            "User=admin",
            "2222",
            "~/.ssh/id_ed25519",
            "~/.ssh/id_rsa",
            "IdentitiesOnly=yes",
            "bastion.example:22",
            "HostKeyAlias=otto-mac-key",
            "StrictHostKeyChecking=accept-new",
        ] {
            assert!(args.iter().any(|arg| arg == expected), "missing {expected}");
        }
        assert_eq!(args.last().map(String::as_str), Some("otto-mac"));
    }

    #[test]
    fn pixel_start_never_spawns_directly_on_macos() {
        assert_eq!(
            pixel_start_action(true, Some(true)),
            PixelStartAction::WaitForService
        );
        assert_eq!(
            pixel_start_action(true, Some(false)),
            PixelStartAction::ReportNotInstalled
        );
        assert_eq!(
            pixel_start_action(true, None),
            PixelStartAction::ReportNotInstalled
        );
        for kickstart_ok in [None, Some(true), Some(false)] {
            assert_eq!(
                pixel_start_action(false, kickstart_ok),
                PixelStartAction::SpawnDirectly
            );
        }
        let error = pixel_office_not_installed_error("exit status: 113");
        assert!(error.contains("cheap-otto dashboards install --execute"));
        assert!(error.contains(PIXEL_OFFICE_LAUNCHD_LABEL));
    }

    #[test]
    fn ssh_host_validation_accepts_aliases_and_addresses() {
        for host in [
            "otto-mac",
            "user@otto-mac",
            "otto-mac.example.test",
            "192.0.2.10",
        ] {
            assert_eq!(validate_ssh_host(host), Ok(()), "{host} should pass");
        }
    }

    #[test]
    fn ssh_host_validation_rejects_option_and_shell_injection() {
        for host in [
            "-oProxyCommand=touch /tmp/x",
            "-F/tmp/cfg",
            "",
            "a;b",
            "a$(id)",
            "a'b",
            "a b",
        ] {
            assert!(validate_ssh_host(host).is_err(), "{host:?} should fail");
        }
    }

    #[test]
    fn open_tunneled_ui_rejects_an_option_looking_host_before_any_ssh() {
        let result = open_tunneled_ui(
            Path::new("."),
            &TunneledUiOptions {
                host: Some("-oProxyCommand=touch /tmp/x"),
                remote_repo: "~/otto-workspaces/cheap-otto",
                port: 7430,
                state_dir: None,
                no_open: true,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn delegation_is_skipped_for_custom_host_or_port() {
        // No recorded peer: `cheap-otto ui` would pick its own default host, so never delegate.
        assert!(!should_delegate_to_cheap_otto(None, None, 7430));
        assert!(should_delegate_to_cheap_otto(None, Some("otto-mac"), 7430));
        assert!(should_delegate_to_cheap_otto(
            Some("otto-mac"),
            Some("otto-mac"),
            7430
        ));
        assert!(!should_delegate_to_cheap_otto(
            Some("other-mac"),
            Some("otto-mac"),
            7430
        ));
        assert!(!should_delegate_to_cheap_otto(Some("otto-mac"), None, 7430));
        assert!(!should_delegate_to_cheap_otto(None, Some("otto-mac"), 8430));
        assert!(!should_delegate_to_cheap_otto(
            Some("otto-mac"),
            Some("otto-mac"),
            7421
        ));
    }

    #[test]
    fn delegation_and_fallback_commands_strip_github_credentials() {
        let state_dir = Path::new("state");
        let ui = cheap_otto_ui_command(Path::new("cheap-otto"), Some(state_dir));
        let ui_args: Vec<_> = ui
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            ui_args,
            ["ui", "--execute", "--no-open", "--state-dir", "state"]
        );
        let bare = cheap_otto_ui_command(Path::new("cheap-otto"), None);
        assert_eq!(bare.get_args().count(), 3);

        let probe = cheap_otto_version_probe();
        assert_eq!(probe.get_program(), "cheap-otto");
        let fallback = reviewer_fallback_tunnel_command("otto-mac", 7430);
        let ssh_cfg = ssh_effective_config_command("otto-mac");
        let cfg_args: Vec<_> = ssh_cfg
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cfg_args, ["-G", "--", "otto-mac"]);
        let repo_root = git_repo_root_command();
        for cmd in [&probe, &ui, &bare, &fallback, &ssh_cfg, &repo_root] {
            let removed: Vec<_> = cmd
                .get_envs()
                .filter(|(_, v)| v.is_none())
                .map(|(k, _)| k.to_string_lossy().to_string())
                .collect();
            assert!(removed.contains(&"GH_TOKEN".to_string()), "{cmd:?}");
            assert!(removed.contains(&"GITHUB_TOKEN".to_string()), "{cmd:?}");
        }
    }

    #[test]
    fn delegated_ui_is_ready_only_when_it_exits_zero_and_the_port_answers() {
        assert_eq!(delegated_ui_check(true, true, 7430), Ok(()));
        for (exit_ok, port_answers) in [(true, false), (false, true), (false, false)] {
            let error = delegated_ui_check(exit_ok, port_answers, 7430).unwrap_err();
            assert!(error.contains("127.0.0.1:7430"), "{error}");
            assert!(error.contains("cheap-otto dashboards status --execute"));
        }
        let dead_port = delegated_ui_check(true, false, 7430).unwrap_err();
        assert!(dead_port.contains("does not answer"));
    }

    #[test]
    fn peer_hosts_come_from_peer_json_and_drop_unsafe_values() {
        let peer = r#"{"alias":"otto-mac","host":"192.0.2.10","ssh_user":"otto","ssh_port":22}"#;
        assert_eq!(parse_peer_hosts(peer), ["otto-mac", "192.0.2.10"]);
        let hostile = r#"{"alias":"-oProxyCommand=x","host":"a;$(id)"}"#;
        assert!(parse_peer_hosts(hostile).is_empty());
        assert!(parse_peer_hosts("").is_empty());
        assert!(parse_peer_hosts("not json").is_empty());
    }

    #[test]
    fn fallback_poll_step_decides_ready_wait_exited_or_timed_out() {
        use FallbackStep::{FailExited, FailTimedOut, Ready, Wait};
        let cases = [
            ((false, false, false), Wait),
            ((false, false, true), FailTimedOut),
            ((false, true, false), FailExited),
            ((false, true, true), FailExited),
            ((true, false, false), Ready),
            ((true, false, true), Ready),
            ((true, true, false), Ready),
            ((true, true, true), Ready),
        ];
        for ((port_open, child_exited, deadline_passed), expected) in cases {
            assert_eq!(
                fallback_poll_step(port_open, child_exited, deadline_passed),
                expected,
                "port_open={port_open} child_exited={child_exited} deadline_passed={deadline_passed}"
            );
        }
    }

    /// In-memory connection: the request is read from `input`, the response lands in `output`.
    struct FakeStream {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for FakeStream {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for FakeStream {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A context whose workspace and config do not exist, so no route can touch real state.
    fn test_context() -> ServerContext {
        let workspace = std::env::temp_dir().join("code-reviewer-guard-test-missing-workspace");
        ServerContext {
            config_path: workspace.join("config").join("repos.toml"),
            workspace,
            guard: RequestGuard::for_listener(7430),
            probe_cache: Mutex::new(None),
        }
    }

    fn request(method: &str, target: &str, headers: &[(&str, &str)], body: &str) -> String {
        let mut raw = format!("{method} {target} HTTP/1.1\r\n");
        for (name, value) in headers {
            raw.push_str(&format!("{name}: {value}\r\n"));
        }
        if !body.is_empty() {
            raw.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        raw.push_str("\r\n");
        raw.push_str(body);
        raw
    }

    /// Sends one raw request through the connection handler and returns `(status, response)`.
    fn exchange(raw_request: &str) -> (u16, String) {
        let mut stream = FakeStream {
            input: Cursor::new(raw_request.as_bytes().to_vec()),
            output: Vec::new(),
        };
        handle_http_connection(&mut stream, &test_context()).unwrap();
        let response = String::from_utf8_lossy(&stream.output).to_string();
        let status = response
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap();
        (status, response)
    }

    #[test]
    fn parses_request_line_and_guard_headers_case_insensitively() {
        let head = RequestHead::parse(
            "POST /api/repos/add?x=1 HTTP/1.1\r\nHOST: localhost:7430\r\norigin:http://localhost:7430\r\nContent-type: application/json; charset=utf-8\r\ncontent-LENGTH: 42\r\nX-Other: ignored",
        );
        assert_eq!(
            head,
            RequestHead {
                method: "POST",
                target: "/api/repos/add?x=1",
                host: Some("localhost:7430"),
                origin: Some("http://localhost:7430"),
                content_type: Some("application/json; charset=utf-8"),
                content_length: 42,
            }
        );
    }

    #[test]
    fn parses_empty_and_headerless_heads_without_panicking() {
        for head in [
            "",
            "\r\n",
            "GET",
            "GET / HTTP/1.1\r\nno-colon-line\r\n: empty-name",
        ] {
            let parsed = RequestHead::parse(head);
            assert_eq!((parsed.host, parsed.origin), (None, None), "{head:?}");
            assert_eq!(parsed.content_length, 0);
        }
    }

    #[test]
    fn blanks_conflicting_duplicate_host_and_origin_headers() {
        let head = RequestHead::parse(
            "POST / HTTP/1.1\r\nHost: 127.0.0.1:7430\r\nHost: evil.example\r\nOrigin: http://127.0.0.1:7430\r\nOrigin: http://evil.example",
        );
        assert_eq!((head.host, head.origin), (Some(""), Some("")));

        let repeated =
            RequestHead::parse("GET / HTTP/1.1\r\nHost: 127.0.0.1:7430\r\nHost: 127.0.0.1:7430");
        assert_eq!(repeated.host, Some(OWN_HOST));
    }

    #[test]
    fn serves_health_and_the_page_to_a_loopback_host() {
        let (status, response) =
            exchange(&request("GET", "/api/health", &[("Host", OWN_HOST)], ""));
        assert_eq!(status, 200);
        assert!(response.ends_with(r#"{"ok":true}"#), "{response}");

        for host in [OWN_HOST, "localhost:7430", "[::1]:7430"] {
            let (status, response) = exchange(&request("GET", "/", &[("Host", host)], ""));
            assert_eq!(status, 200, "{host}");
            assert!(response.contains("Content-Type: text/html"), "{host}");
        }
    }

    #[test]
    fn refuses_rebound_or_missing_host_before_serving_page_or_api() {
        for target in ["/", "/api/health", "/api/overview"] {
            for headers in [&[("Host", "evil.example:7430")][..], &[][..]] {
                let (status, response) = exchange(&request("GET", target, headers, ""));
                assert_eq!(status, 403, "{target} with {headers:?}");
                assert!(response.starts_with("HTTP/1.1 403 Forbidden\r\n"));
            }
        }
    }

    #[test]
    fn refuses_cross_origin_simple_post_before_routing() {
        // A `text/plain` JSON body is the request a foreign page can send without a preflight.
        // `{}` fails to parse in the route, so a regression here could still not flip a repo.
        for origin in [FOREIGN_ORIGIN, "null", "http://127.0.0.1:7420"] {
            let (status, response) = exchange(&request(
                "POST",
                "/api/repos/toggle",
                &[
                    ("Host", OWN_HOST),
                    ("Origin", origin),
                    ("Content-Type", "text/plain;charset=UTF-8"),
                ],
                "{}",
            ));
            assert_eq!(status, 403, "{origin}");
            assert!(
                response.contains("cross-origin request refused"),
                "{origin}"
            );
            assert!(!response.to_ascii_lowercase().contains("access-control-"));
        }
    }

    #[test]
    fn refuses_cross_origin_post_without_a_body() {
        // An unknown route stands in for the body-less action routes: the guard never sees the
        // path, and a regression must not be able to start a real poll from the test suite.
        let (status, _) = exchange(&request(
            "POST",
            "/api/guard-probe",
            &[("Host", OWN_HOST), ("Origin", FOREIGN_ORIGIN)],
            "",
        ));
        assert_eq!(status, 403);
    }

    #[test]
    fn passes_same_origin_json_post_and_originless_post_to_the_router() {
        for headers in [
            &[
                ("Host", OWN_HOST),
                ("Origin", OWN_ORIGIN),
                ("Content-Type", "application/json"),
            ][..],
            &[("Host", OWN_HOST), ("Content-Type", "application/json")][..],
        ] {
            let (status, response) = exchange(&request("POST", "/api/guard-probe", headers, "{}"));
            assert_eq!(status, 400, "{headers:?}");
            assert!(
                response.contains("unknown endpoint `POST /api/guard-probe`"),
                "{response}"
            );
        }
    }

    #[test]
    fn refuses_post_body_that_is_not_declared_json() {
        for headers in [
            &[("Host", OWN_HOST), ("Content-Type", "text/plain")][..],
            &[("Host", OWN_HOST)][..],
            &[
                ("Host", OWN_HOST),
                ("Origin", OWN_ORIGIN),
                ("Content-Type", "application/x-www-form-urlencoded"),
            ][..],
        ] {
            let (status, response) = exchange(&request("POST", "/api/repos/toggle", headers, "{}"));
            assert_eq!(status, 415, "{headers:?}");
            assert!(response.starts_with("HTTP/1.1 415 Unsupported Media Type\r\n"));
        }
    }

    #[test]
    fn preflight_from_foreign_origin_gets_no_cors_allow_header() {
        let (status, response) = exchange(&request(
            "OPTIONS",
            "/api/actions/review",
            &[
                ("Host", OWN_HOST),
                ("Origin", FOREIGN_ORIGIN),
                ("Access-Control-Request-Method", "POST"),
                ("Access-Control-Request-Headers", "content-type"),
            ],
            "",
        ));
        assert_eq!(status, 400);
        assert!(
            !response
                .to_ascii_lowercase()
                .contains("access-control-allow")
        );
    }

    #[test]
    fn rejects_conflicting_duplicate_content_type_headers() {
        let raw = request(
            "POST",
            "/api/repos/add",
            &[
                ("Host", OWN_HOST),
                ("Content-Type", "application/json"),
                ("Content-Type", "text/plain"),
            ],
            "{}",
        );
        let (status, _) = exchange(&raw);
        assert_eq!(status, 415);
    }

    #[test]
    fn accepts_identical_duplicate_content_type_headers() {
        let head = RequestHead::parse(
            "POST / HTTP/1.1\r\nContent-Type: application/json\r\nContent-Type: application/json",
        );
        assert_eq!(head.content_type, Some("application/json"));
        let raw = request(
            "POST",
            "/api/unknown-route",
            &[
                ("Host", OWN_HOST),
                ("Content-Type", "application/json"),
                ("Content-Type", "application/json"),
            ],
            "{}",
        );
        let (status, _) = exchange(&raw);
        assert_ne!(status, 415);
        assert_ne!(status, 403);
    }

    /// Counts `read` calls and serves the same bytes for the first one only.
    struct CountingStream {
        first: Vec<u8>,
        reads: usize,
        output: Vec<u8>,
    }

    impl Read for CountingStream {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.reads += 1;
            if self.reads > 1 {
                // Pretend the peer is still sending, so an unwanted body read would show up.
                buf.fill(b'x');
                return Ok(buf.len());
            }
            let n = self.first.len().min(buf.len());
            buf[..n].copy_from_slice(&self.first[..n]);
            Ok(n)
        }
    }

    impl Write for CountingStream {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn refused_cross_origin_post_reads_no_body() {
        let head = format!(
            "POST /api/repos/add HTTP/1.1\r\nHost: {OWN_HOST}\r\nOrigin: {FOREIGN_ORIGIN}\r\nContent-Type: application/json\r\nContent-Length: 400000\r\n\r\n"
        );
        let mut stream = CountingStream {
            first: head.into_bytes(),
            reads: 0,
            output: Vec::new(),
        };
        handle_http_connection(&mut stream, &test_context()).unwrap();
        assert_eq!(stream.reads, 1);
        assert!(String::from_utf8_lossy(&stream.output).starts_with("HTTP/1.1 403 Forbidden\r\n"));
    }

    #[test]
    fn dashboard_html_links_mission_control_on_the_shared_port() {
        let html = crate::ui_html::DASHBOARD_HTML;
        assert!(html.contains("id=\"mission-control-link\""));
        assert!(html.contains("MISSION CONTROL ↗"));
        assert!(html.contains(&format!(":{DEFAULT_MISSION_CONTROL_PORT}/")));
    }
}
