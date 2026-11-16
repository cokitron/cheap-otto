//! Dashboard registry: the one place that names every dashboard served from the Mac.
//!
//! Mission Control, the Pixel Office and the Reviewer each run as a `launchd` service on the
//! paired MacBook and reach the workstation through one SSH tunnel. Install, uninstall, status,
//! the tunnel, `cheap-otto ui` and the hub API all read ids, ports and LaunchAgent labels from
//! [`DashboardSet`], so they can never disagree. Pure module: no I/O and no clock.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::bridge::PeerNode;
use crate::command::CommandSpec;
use crate::worktree::posix_single_quote;

/// Default port of Mission Control (`cheap-otto serve`).
pub const MISSION_CONTROL_PORT: u16 = 7420;
/// Default port of the Pixel Office (`pixel-agents`).
pub const PIXEL_OFFICE_PORT: u16 = 7421;
/// Port of the Reviewer and Code Tender dashboard (`code-reviewer serve`).
pub const REVIEWER_PORT: u16 = 7430;
/// Pseudo-project that owns dashboard LaunchAgent labels. Schedules may not use it.
pub const RESERVED_PROJECT_NAME: &str = "dashboards";
/// LaunchAgent label of the Reviewer, installed by `crates/code-reviewer/scripts/m1-provision.sh`.
pub const REVIEWER_LAUNCHD_LABEL: &str = "com.cheapotto.code-reviewer.ui";

/// Errors returned when building or parsing the dashboard registry.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DashboardError {
    /// A port of 0 cannot be listened on or forwarded.
    #[error("invalid dashboard port {0}")]
    InvalidPort(u16),
    /// Two dashboards would collide on one port, locally or through the tunnel.
    #[error("dashboard port {0} is used twice")]
    DuplicatePort(u16),
    /// A dashboard id given on the command line or in a request is not known.
    #[error("unknown dashboard '{0}' (expected mission-control, pixel-office or reviewer)")]
    UnknownDashboard(String),
    /// A deserialized set differs from the registry in anything but ports: ids, order, names,
    /// LaunchAgent labels or managers.
    #[error(
        "a dashboard set must match the registry apart from ports: mission-control, pixel-office and reviewer once each, in that order, with the registry's names, labels and managers"
    )]
    UnexpectedServices,
}

/// Identifies one dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DashboardId {
    /// Mission Control, the `cheap-otto serve` control plane.
    MissionControl,
    /// The Pixel Office, the third-party `pixel-agents` page.
    PixelOffice,
    /// The Reviewer and Code Tender dashboard, `code-reviewer serve`.
    Reviewer,
}

impl DashboardId {
    /// Every dashboard, in registry order.
    #[must_use = "returns every dashboard id in registry order"]
    pub const fn all() -> &'static [DashboardId] {
        &[
            DashboardId::MissionControl,
            DashboardId::PixelOffice,
            DashboardId::Reviewer,
        ]
    }

    /// Kebab-case id used in labels, log paths, CLI flags and JSON.
    #[must_use = "returns the kebab-case id"]
    pub const fn slug(self) -> &'static str {
        match self {
            DashboardId::MissionControl => "mission-control",
            DashboardId::PixelOffice => "pixel-office",
            DashboardId::Reviewer => "reviewer",
        }
    }

    /// Human-readable name shown in plans, status rows and links.
    #[must_use = "returns the display name"]
    pub const fn display_name(self) -> &'static str {
        match self {
            DashboardId::MissionControl => "Mission Control",
            DashboardId::PixelOffice => "Pixel Office",
            DashboardId::Reviewer => "Reviewer",
        }
    }

    /// Parses a kebab-case id such as `pixel-office`, ignoring surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`DashboardError::UnknownDashboard`] for anything that is not one of the three ids.
    pub fn parse(raw: &str) -> Result<Self, DashboardError> {
        let wanted = raw.trim();
        DashboardId::all()
            .iter()
            .copied()
            .find(|id| id.slug() == wanted)
            .ok_or_else(|| DashboardError::UnknownDashboard(raw.to_string()))
    }
}

/// Who installs and owns a dashboard's LaunchAgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceManager {
    /// LaunchAgent is rendered and installed by cheap-otto.
    CheapOtto,
    /// LaunchAgent is installed by crates/code-reviewer/scripts/m1-provision.sh.
    CodeReviewerProvisioner,
}

/// One dashboard: where it listens and which LaunchAgent runs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardService {
    /// Which dashboard this is.
    pub id: DashboardId,
    /// Human-readable name.
    pub display_name: String,
    /// Port it listens on, on `127.0.0.1` on the Mac and through the tunnel on the workstation.
    pub port: u16,
    /// `launchd` label of the LaunchAgent that runs it.
    pub launchd_label: String,
    /// Who installs that LaunchAgent.
    pub manager: ServiceManager,
}

/// The full set of dashboards, in [`DashboardId::all`] order.
///
/// The field is private and deserialization goes through [`DashboardSet::try_from`], so every
/// set that exists has passed the same checks as [`DashboardSet::with_ports`]. [R2.2]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DashboardSetFields")]
pub struct DashboardSet {
    services: Vec<DashboardService>,
}

/// The shape of a serialized [`DashboardSet`], before validation.
#[derive(Deserialize)]
struct DashboardSetFields {
    services: Vec<DashboardService>,
}

impl TryFrom<DashboardSetFields> for DashboardSet {
    type Error = DashboardError;

    fn try_from(fields: DashboardSetFields) -> Result<Self, Self::Error> {
        let ids: Vec<DashboardId> = fields.services.iter().map(|service| service.id).collect();
        if ids != DashboardId::all() {
            return Err(DashboardError::UnexpectedServices);
        }
        let ports: Vec<u16> = fields.services.iter().map(|service| service.port).collect();
        validate_ports(&ports)?;
        let &[mission_control, pixel_office, reviewer] = ports.as_slice() else {
            return Err(DashboardError::UnexpectedServices);
        };
        // Labels, managers and display names come from the registry, so a payload that
        // disagrees with it must not select the wrong LaunchAgents in `managed()`.
        let set = Self::from_ports(mission_control, pixel_office, reviewer);
        if set.services != fields.services {
            return Err(DashboardError::UnexpectedServices);
        }
        Ok(set)
    }
}

/// Rejects port 0 and any port used twice. [R2.2]
fn validate_ports(ports: &[u16]) -> Result<(), DashboardError> {
    for (index, port) in ports.iter().enumerate() {
        if *port == 0 {
            return Err(DashboardError::InvalidPort(*port));
        }
        if ports[..index].contains(port) {
            return Err(DashboardError::DuplicatePort(*port));
        }
    }
    Ok(())
}

impl DashboardSet {
    /// Every dashboard, in [`DashboardId::all`] order.
    #[must_use = "returns every dashboard entry"]
    pub fn services(&self) -> &[DashboardService] {
        &self.services
    }

    /// The three dashboards on their default ports.
    #[must_use = "returns the default dashboard set"]
    pub fn standard() -> Self {
        // The constants are non-zero and distinct (checked by a test), so this skips the
        // validation in `with_ports` and needs no `.expect()`.
        Self::from_ports(MISSION_CONTROL_PORT, PIXEL_OFFICE_PORT, REVIEWER_PORT)
    }

    /// The three dashboards on the given ports.
    ///
    /// # Errors
    ///
    /// Returns [`DashboardError::InvalidPort`] for a port of 0 and
    /// [`DashboardError::DuplicatePort`] when two dashboards share a port.
    pub fn with_ports(
        mission_control: u16,
        pixel_office: u16,
        reviewer: u16,
    ) -> Result<Self, DashboardError> {
        validate_ports(&[mission_control, pixel_office, reviewer])?;
        Ok(Self::from_ports(mission_control, pixel_office, reviewer))
    }

    fn from_ports(mission_control: u16, pixel_office: u16, reviewer: u16) -> Self {
        let services = DashboardId::all()
            .iter()
            .map(|&id| {
                let (port, launchd_label, manager) = match id {
                    DashboardId::MissionControl => (
                        mission_control,
                        managed_label(id),
                        ServiceManager::CheapOtto,
                    ),
                    DashboardId::PixelOffice => {
                        (pixel_office, managed_label(id), ServiceManager::CheapOtto)
                    }
                    DashboardId::Reviewer => (
                        reviewer,
                        REVIEWER_LAUNCHD_LABEL.to_string(),
                        ServiceManager::CodeReviewerProvisioner,
                    ),
                };
                DashboardService {
                    id,
                    display_name: id.display_name().to_string(),
                    port,
                    launchd_label,
                    manager,
                }
            })
            .collect();
        Self { services }
    }

    /// The entry for one dashboard.
    #[must_use = "returns the dashboard entry if present"]
    pub fn get(&self, id: DashboardId) -> Option<&DashboardService> {
        self.services.iter().find(|service| service.id == id)
    }

    /// Every port, in set order.
    #[must_use = "returns every dashboard port in set order"]
    pub fn ports(&self) -> Vec<u16> {
        self.services.iter().map(|service| service.port).collect()
    }

    /// The dashboards whose LaunchAgents cheap-otto installs (not the Reviewer).
    pub fn managed(&self) -> impl Iterator<Item = &DashboardService> {
        self.services
            .iter()
            .filter(|service| service.manager == ServiceManager::CheapOtto)
    }
}

fn managed_label(id: DashboardId) -> String {
    format!("com.cheapotto.{RESERVED_PROJECT_NAME}.{}", id.slug())
}

/// `PATH` for the dashboard LaunchAgents: `launchd` starts jobs with a minimal one.
const SERVICE_PATH_XML: &str = "/opt/homebrew/bin:/usr/local/bin:$HOME_XML/.cargo/bin:$HOME_XML/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin";

/// Renders the LaunchAgent plist for a dashboard that cheap-otto manages. [R1.1, R1.2, R1.3, R1.4, R1.8]
///
/// Returns `None` for the Reviewer, whose LaunchAgent `m1-provision.sh` installs. The text is a
/// template for an unquoted heredoc in the install script: `$HOME_XML` and, for the Pixel Office,
/// `$PIXEL_BIN_XML` stay literal, and the Mac's shell fills them with XML-escaped values, because
/// `launchd` expands no variables. Only `service.id` is read: ports come from `set` and the label
/// is rebuilt from the id, because `DashboardService` fields are public and a hand-built value
/// must not put its own text into the heredoc or the XML.
#[must_use = "the plist template should be written by the install script"]
pub fn render_service_plist_template(
    service: &DashboardService,
    set: &DashboardSet,
) -> Option<String> {
    let port = set.get(service.id)?.port;
    let program_arguments: Vec<String> = match service.id {
        DashboardId::MissionControl => {
            let pixel_port = set.get(DashboardId::PixelOffice)?.port;
            vec![
                "$HOME_XML/.cargo/bin/cheap-otto".to_string(),
                "serve".to_string(),
                "--host".to_string(),
                "127.0.0.1".to_string(),
                "--port".to_string(),
                port.to_string(),
                "--pixel-port".to_string(),
                pixel_port.to_string(),
            ]
        }
        // The flags `run_ui` passes to `pixel-agents` today.
        DashboardId::PixelOffice => vec![
            "$PIXEL_BIN_XML".to_string(),
            "--port".to_string(),
            port.to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--no-terminal".to_string(),
        ],
        DashboardId::Reviewer => return None,
    };
    let label = managed_label(service.id);
    let log = format!(
        "$HOME_XML/.cheap-otto/logs/dashboards/{}.log",
        service.id.slug()
    );
    let arguments: String = program_arguments
        .iter()
        .map(|argument| format!("    <string>{argument}</string>\n"))
        .collect();
    Some(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>{SERVICE_PATH_XML}</string>
  </dict>
  <key>ProgramArguments</key>
  <array>
{arguments}  </array>
  <key>WorkingDirectory</key>
  <string>$HOME_XML</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ThrottleInterval</key>
  <integer>10</integer>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#
    ))
}

// ---------------------------------------------------------------------------------------------
// Tunnel: the one SSH process that forwards every dashboard port (design 3.5). The process work
// (spawning, probing ports, reading `tunnel.json`) lives in `src/main.rs`; this part only plans
// and decides.
// ---------------------------------------------------------------------------------------------

/// `-o` options every dashboard tunnel carries, in order. [R3.3]
///
/// `BatchMode=yes` stops a detached process from hanging on a password prompt and
/// `ConnectTimeout=10` stops it from hanging on an unreachable host.
const TUNNEL_SSH_OPTIONS: [&str; 5] = [
    "ExitOnForwardFailure=yes",
    "ServerAliveInterval=30",
    "ServerAliveCountMax=3",
    "BatchMode=yes",
    "ConnectTimeout=10",
];

/// The `-L` value for one port: loopback on the workstation and on the Mac. [R3.3]
fn loopback_forward(port: u16) -> String {
    format!("127.0.0.1:{port}:127.0.0.1:{port}")
}

/// Builds the one SSH tunnel that forwards every dashboard port. [R3.1, R3.3]
///
/// `ssh -N -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 -o ServerAliveCountMax=3
/// -o BatchMode=yes -o ConnectTimeout=10 -p <ssh_port> -L 127.0.0.1:P:127.0.0.1:P … <user@host>`,
/// with one forward per port in set order. Every port is forwarded whether or not its service is
/// installed. The destination is `user@host`, not the SSH alias, so no `Host` block can add
/// forwards of its own. The arguments are discrete and no remote command is sent (`-N`), so
/// nothing here is interpreted by a shell.
#[must_use = "the tunnel command should be displayed or spawned"]
pub fn build_tunnel_spec(peer: &PeerNode, set: &DashboardSet) -> CommandSpec {
    CommandSpec::new(
        "Forward every dashboard port from the Mac to 127.0.0.1 on this workstation",
        "ssh",
        tunnel_args(&peer.ssh_destination(), peer.ssh_port, &set.ports()),
    )
}

/// The tunnel's argv after the program, in the one order [`build_tunnel_spec`] spawns and
/// [`command_line_is_recorded_tunnel`] verifies.
fn tunnel_args(destination: &str, ssh_port: u16, ports: &[u16]) -> Vec<String> {
    let mut args = Vec::with_capacity(4 + TUNNEL_SSH_OPTIONS.len() * 2 + ports.len() * 2);
    args.push("-N".to_string());
    for option in TUNNEL_SSH_OPTIONS {
        args.push("-o".to_string());
        args.push(option.to_string());
    }
    args.push("-p".to_string());
    args.push(ssh_port.to_string());
    for &port in ports {
        args.push("-L".to_string());
        args.push(loopback_forward(port));
    }
    args.push(destination.to_string());
    args
}

/// What `tunnel.json` in the state directory records about the tunnel cheap-otto started. [R3.4]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelRecord {
    /// Process id of the `ssh` process.
    pub pid: u32,
    /// Every forwarded port, in set order.
    pub ports: Vec<u16>,
    /// The `user@host` the tunnel was opened to.
    pub destination: String,
    /// The SSH port the tunnel was opened to.
    pub ssh_port: u16,
    /// When the tunnel was started, in seconds since the Unix epoch.
    pub started_epoch_secs: u64,
    /// OS process start-time stamp (`Win32_Process.CreationDate` in UTC ISO-8601 on Windows,
    /// `ps -o lstart=` run under `TZ=UTC` on Unix, so DST or timezone changes cannot alter it)
    /// captured at spawn time to narrow PID-reuse races before signalling. macOS `lstart` has
    /// one-second resolution, so this is a second check on top of the exact argv match; a stronger
    /// token is tracked in a follow-up issue. Older `tunnel.json` files omit this field and fall back to command-line verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_started_at: Option<String>,
}

/// What to do about the tunnel, given which dashboard ports answer on the workstation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelDecision {
    /// Every port answers locally and the recorded tunnel is a live ssh process that covers them.
    AlreadyUp,
    /// No port answers locally.
    Start,
    /// Some ports answer and some do not, or every port answers without a covering recorded
    /// tunnel (then `held` is every port and `missing` is empty).
    Conflict {
        /// Ports that answer locally, in input order.
        held: Vec<u16>,
        /// Ports that do not answer locally, in input order.
        missing: Vec<u16>,
    },
}

/// True only when `record` exists, its process is a live ssh process (`ssh_alive`, from
/// [`command_line_is_recorded_tunnel`]), it was opened to the current peer's `user@host` and SSH
/// port, and it forwards every port in `set`. A record made for another Mac is never reused.
/// [R3.2, R3.5]
#[must_use = "the result decides whether a new tunnel is started"]
pub fn recorded_tunnel_covers(
    record: Option<&TunnelRecord>,
    ssh_alive: bool,
    set: &DashboardSet,
    peer: &PeerNode,
) -> bool {
    let Some(record) = record else {
        return false;
    };
    ssh_alive
        && record.destination == peer.ssh_destination()
        && record.ssh_port == peer.ssh_port
        && set.ports().iter().all(|port| record.ports.contains(port))
}

/// Decides between reusing, starting and refusing, from `(port, answers locally)` pairs and the
/// result of [`recorded_tunnel_covers`]. Every port answering is proof of a tunnel only when the
/// recorded tunnel covers them; otherwise unrelated listeners hold them. [R3.2, R3.5]
#[must_use = "the decision says whether to start, reuse or refuse a tunnel"]
pub fn decide_tunnel(local_ports: &[(u16, bool)], recorded_tunnel_covers: bool) -> TunnelDecision {
    let ports_where = |wanted: bool| -> Vec<u16> {
        local_ports
            .iter()
            .filter(|(_, answers)| *answers == wanted)
            .map(|(port, _)| *port)
            .collect()
    };
    let held = ports_where(true);
    let missing = ports_where(false);
    if held.is_empty() {
        TunnelDecision::Start
    } else if missing.is_empty() && recorded_tunnel_covers {
        TunnelDecision::AlreadyUp
    } else {
        TunnelDecision::Conflict { held, missing }
    }
}

/// Why a freshly started tunnel did not come up. [R3.6]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelFailure {
    /// The ssh process exited before every forward answered.
    ExitedEarly {
        /// The exit status as the OS reported it.
        status: String,
    },
    /// The ssh process was still running, but not every forward answered in time.
    TimedOut {
        /// How long the shell waited.
        waited_secs: u64,
    },
}

/// One failure message for both kinds: what happened, every port that did not answer, and, last,
/// the command that tests SSH by hand. [R3.6]
#[must_use = "the failure message should be reported to the operator"]
pub fn describe_tunnel_failure(
    failure: &TunnelFailure,
    unanswered: &[u16],
    ssh_test_command: &str,
) -> String {
    let what = match failure {
        TunnelFailure::ExitedEarly { status } => {
            format!("The SSH tunnel exited before opening every forward ({status}).")
        }
        TunnelFailure::TimedOut { waited_secs } => {
            format!("The SSH tunnel did not open every forward within {waited_secs} s.")
        }
    };
    let ports = if unanswered.is_empty() {
        "none".to_string()
    } else {
        unanswered
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "{what} Ports not answering on 127.0.0.1: {ports}. Test SSH by hand: {ssh_test_command}"
    )
}

/// True only when `command_line` (as `ps -o args=` or Win32 `CommandLine` reports it) is `ssh`
/// followed by exactly the argv [`build_tunnel_spec`] gives for the record: `-N`, the tunnel
/// options, `-p <record.ssh_port>`, one `127.0.0.1:<port>:127.0.0.1:<port>` forward per recorded
/// port in order, and the record's destination last, so no remote command can follow it.
///
/// Guards against a recycled pid: after a reboot the recorded pid can belong to someone's ssh
/// session, which must never be signalled. A record with no ports never matches. [R3.5]
#[must_use = "the result decides whether a recorded pid may be signalled"]
pub fn command_line_is_recorded_tunnel(command_line: &str, record: &TunnelRecord) -> bool {
    let line = command_line.trim();
    // The program may be quoted, with spaces, as Windows reports it: `"C:\Program Files\…\ssh.exe"`.
    let (program, rest) = match line.strip_prefix('"') {
        Some(quoted) => quoted.split_once('"').unwrap_or((quoted, "")),
        None => line.split_once(char::is_whitespace).unwrap_or((line, "")),
    };
    let file_name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let is_ssh = file_name.eq_ignore_ascii_case("ssh") || file_name.eq_ignore_ascii_case("ssh.exe");
    let args: Vec<&str> = rest
        .split_whitespace()
        .map(|arg| arg.trim_matches('"'))
        .collect();
    let expected = tunnel_args(&record.destination, record.ssh_port, &record.ports);
    is_ssh && !record.ports.is_empty() && args == expected
}

/// Verifies both the live command line and, when present on `record`, the OS process start-time
/// stamp (`Win32_Process.CreationDate` or `ps -o lstart=` captured under `TZ=UTC`).
///
/// The Unix stamp has one-second resolution, so it is a second check on top of the exact argv
/// match rather than a unique identity; a stronger token is tracked in a follow-up issue.
///
/// Records written by older builds carry `process_started_at: None` and fall back to
/// [`command_line_is_recorded_tunnel`]. When `record.process_started_at` is `Some`, `live_started_at`
/// must match after trimming surrounding whitespace; a missing, empty or differing start time is
/// treated as a recycled PID so nothing is signalled. [R3.5]
#[must_use = "the result decides whether a recorded pid may be signalled"]
pub fn process_matches_recorded_tunnel(
    command_line: &str,
    live_started_at: Option<&str>,
    record: &TunnelRecord,
) -> bool {
    if !command_line_is_recorded_tunnel(command_line, record) {
        return false;
    }
    match &record.process_started_at {
        None => true,
        Some(recorded) => {
            let recorded = recorded.trim();
            !recorded.is_empty() && live_started_at.map(str::trim) == Some(recorded)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Install: one POSIX `sh` script that installs the dashboard LaunchAgents on the Mac (design 3.3)
// and the SSH plan that runs it from the workstation.
// ---------------------------------------------------------------------------------------------

/// Writes the Pixel Office settings `run_ui` used to write before starting `pixel-agents`.
///
/// A raw string, so the braces of the inline JavaScript need no escaping. The install script runs
/// under `set -e`, so a failed write is reported as a warning instead of aborting the install.
const PIXEL_AGENTS_CONFIG_SNIPPET: &str = r#"if command -v node >/dev/null 2>&1; then
    mkdir -p "$HOME/.pixel-agents" &&
      node -e 'const fs=require("fs"),p=process.env.HOME+"/.pixel-agents/config.json";let c={};try{c=JSON.parse(fs.readFileSync(p,"utf8"))}catch{};c.standalone=Object.assign({},c.standalone,{watchAllSessions:true});c.hooksConsent=Object.assign({},c.hooksConsent,{claude:"granted"});fs.writeFileSync(p,JSON.stringify(c,null,2));' ||
      echo "dashboard=pixel-office warning=pixel-agents-config-not-written"
  fi"#;

/// Header of the install script: environment, `xml_escape`, the GUI-session guard and the
/// cleanup of tmux sessions left by older versions of `cheap-otto ui`. [R1.7, R1.9]
const INSTALL_SCRIPT_HEADER: &str = r#"set -eu
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
UID_NUM=$(id -u)
AGENTS_DIR="$HOME/Library/LaunchAgents"
LOG_DIR="$HOME/.cheap-otto/logs/dashboards"
# `set -e` would end the script here with exit 1 and no cause, so the failure is handled explicitly.
if ! mkdir -p "$AGENTS_DIR" "$LOG_DIR"; then echo "error=dashboard-directories-not-created path=$AGENTS_DIR log=$LOG_DIR"; exit 4; fi
FAILED=0
# Values written into plist XML must be escaped; plain "$HOME" stays for shell use.
xml_escape() { printf '%s' "$1" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }
HOME_XML=$(xml_escape "$HOME")

# [R1.7] LaunchAgents need the user's GUI session.
if ! launchctl print "gui/$UID_NUM" >/dev/null 2>&1; then
  echo "error=no-gui-session"
  echo "hint=Log in to the Mac desktop once, or enable automatic login, then run this again."
  exit 3
fi

# [R1.9] Sessions started by older versions of `cheap-otto ui`.
if command -v tmux >/dev/null 2>&1; then
  tmux kill-session -t cheapotto-serve 2>/dev/null || true
  tmux kill-session -t cheapotto-pixel 2>/dev/null || true
fi
"#;

/// `install_service` and `wait_port`, the two shell functions every service block calls.
/// [R1.5, R1.9, R1.10]
const INSTALL_SCRIPT_FUNCTIONS: &str = r#"
# launchd_last_exit <label>: what launchd last saw, for the failure reports ("-" when unknown).
launchd_last_exit() {
  launchctl print "gui/$UID_NUM/$1" 2>/dev/null | awk -F'= ' '/last exit code = /{print $2; exit}' | tr ' ' '_'
}

# install_service <id> <label> <port>   (plist text on stdin)
install_service() {
  id="$1"; label="$2"; port="$3"
  plist="$AGENTS_DIR/$label.plist"
  tmp="$plist.tmp.$$"
  # This function is called as an `if` condition, and POSIX turns `set -e` off inside a condition,
  # so every step that can fail is checked here and reported with a cause.
  if ! cat > "$tmp"; then
    last_exit=$(launchd_last_exit "$label")
    echo "dashboard=$id listening=0 cause=plist-write-failed last_exit=${last_exit:--} log=$LOG_DIR/$id.log"
    FAILED=1; rm -f "$tmp"; return 1
  fi
  # [R1.5] Unchanged and loaded: leave it alone.
  if [ -f "$plist" ] && cmp -s "$tmp" "$plist" && launchctl print "gui/$UID_NUM/$label" >/dev/null 2>&1; then
    rm -f "$tmp"
    echo "dashboard=$id action=unchanged"
    return 0
  fi
  launchctl bootout "gui/$UID_NUM/$label" 2>/dev/null || true
  # bootout is asynchronous; bootstrapping too early fails with "Input/output error".
  n=0
  while launchctl print "gui/$UID_NUM/$label" >/dev/null 2>&1 && [ "$n" -lt 10 ]; do
    n=$((n + 1)); sleep 0.5
  done
  # [R1.9] Whatever still listens on the port is a stray. Found by port, never by a name pattern:
  # this script's own text is in the remote shell's argv, so a pattern would match the installer.
  for pid in $(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true); do
    kill "$pid" 2>/dev/null || true
    echo "dashboard=$id stray_killed=$pid"
  done
  # The killed listener may still be exiting; wait for the port to close before bootstrapping.
  n=0
  while [ -n "$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)" ] && [ "$n" -lt 10 ]; do
    n=$((n + 1)); sleep 0.5
  done
  held=$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)
  if [ -n "$held" ]; then
    for pid in $held; do kill -9 "$pid" 2>/dev/null || true; done
    n=0
    while [ -n "$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)" ] && [ "$n" -lt 4 ]; do
      n=$((n + 1)); sleep 0.5
    done
    held=$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)
    if [ -n "$held" ]; then
      # [R1.9, R1.10] The port cannot be freed. Say why, where the log is and what launchd last saw,
      # and return non-zero so the caller skips wait_port: the foreign listener would answer the
      # port probe and this service would be reported as listening.
      held=$(printf '%s' "$held" | tr '\n' ',')
      last_exit=$(launchd_last_exit "$label")
      echo "dashboard=$id listening=0 cause=port-held pid=$held last_exit=${last_exit:--} log=$LOG_DIR/$id.log"
      FAILED=1
      rm -f "$tmp"
      return 1
    fi
  fi
  if ! mv "$tmp" "$plist"; then
    last_exit=$(launchd_last_exit "$label")
    echo "dashboard=$id listening=0 cause=plist-write-failed last_exit=${last_exit:--} log=$LOG_DIR/$id.log"
    FAILED=1; rm -f "$tmp"; return 1
  fi
  if ! launchctl bootstrap "gui/$UID_NUM" "$plist"; then
    last_exit=$(launchd_last_exit "$label")
    echo "dashboard=$id listening=0 cause=bootstrap-failed last_exit=${last_exit:--} log=$LOG_DIR/$id.log"
    FAILED=1; return 1
  fi
  echo "dashboard=$id action=installed"
}

# wait_port <id> <label> <port>
# The LaunchAgent runs the binary directly, so the listener is the launchd service pid or its child.
# A port that answers for any other process (a foreign listener) is not this dashboard.
wait_port() {
  i=0
  listeners=""
  while [ "$i" -lt 20 ]; do
    if nc -z 127.0.0.1 "$3" 2>/dev/null; then
      svc_pid=$(launchctl print "gui/$UID_NUM/$2" 2>/dev/null | awk '/^[ \t]*pid = /{print $3; exit}')
      listeners=$(lsof -nP -iTCP:"$3" -sTCP:LISTEN -t 2>/dev/null || true)
      if [ -n "$svc_pid" ]; then
        for lpid in $listeners; do
          parent=$(ps -o ppid= -p "$lpid" 2>/dev/null | tr -d ' ' || true)
          if [ "$lpid" = "$svc_pid" ] || [ "$parent" = "$svc_pid" ]; then
            echo "dashboard=$1 listening=1"; return 0
          fi
        done
      fi
    fi
    i=$((i + 1)); sleep 0.5
  done
  # [R1.10] A service that never listens is a failure the caller must be able to diagnose:
  # say why, where its log is, and what launchd last saw.
  last_exit=$(launchd_last_exit "$2")
  if [ -n "$listeners" ]; then
    listeners=$(printf '%s' "$listeners" | tr '\n' ',')
    echo "dashboard=$1 listening=0 cause=foreign-listener pid=$listeners last_exit=${last_exit:--} log=$LOG_DIR/$1.log"
  else
    echo "dashboard=$1 listening=0 cause=not-listening-after-10s last_exit=${last_exit:--} log=$LOG_DIR/$1.log"
  fi
  FAILED=1
}
"#;

/// Renders the POSIX `sh` script that installs every dashboard LaunchAgent cheap-otto manages.
/// [R1.5, R1.6, R1.7, R1.9, R1.10]
///
/// It runs under `/bin/sh` with `set -eu` and exits 0 when every step succeeded (skips are
/// reported), 3 without a GUI session, and 4 when the dashboard directories cannot be created
/// (at once) or when a service could not be installed or never listens (the other services are
/// still attempted). Each service block is
/// `if install_service <id> <label> <port> <<PLIST … PLIST then wait_port …; fi`, with the plist
/// from [`render_service_plist_template`] in an unquoted heredoc, so the Mac's shell fills in the
/// XML-escaped `$HOME_XML` and `$PIXEL_BIN_XML`. Ids are slugs, labels are rebuilt from the id
/// and ports are `u16`, so nothing interpolated here needs quoting.
#[must_use = "the install script should be run on the Mac or printed"]
pub fn render_install_script(set: &DashboardSet) -> String {
    let mut script = String::with_capacity(8192);
    script.push_str(INSTALL_SCRIPT_HEADER);
    script.push_str(INSTALL_SCRIPT_FUNCTIONS);
    for service in set.managed() {
        let Some(template) = render_service_plist_template(service, set) else {
            continue;
        };
        let id = service.id.slug();
        let label = managed_label(service.id);
        let port = service.port;
        script.push_str(&format!("\n# --- {id} ---\n"));
        let (guard, preamble, skip_reason) = match service.id {
            DashboardId::MissionControl => (
                r#"if [ -x "$HOME/.cargo/bin/cheap-otto" ]; then"#,
                String::new(),
                "cheap-otto-not-installed",
            ),
            // [R1.6] Pixel Office is optional: without `pixel-agents` it is skipped, not failed.
            DashboardId::PixelOffice => (
                "PIXEL_BIN=$(command -v pixel-agents 2>/dev/null || true)\n\
                 if [ -n \"$PIXEL_BIN\" ]; then",
                format!(
                    "  PIXEL_BIN_XML=$(xml_escape \"$PIXEL_BIN\")\n  {PIXEL_AGENTS_CONFIG_SNIPPET}\n"
                ),
                "pixel-agents-not-installed",
            ),
            DashboardId::Reviewer => continue,
        };
        // install_service returns non-zero when the service could not be installed (port held,
        // plist not written, bootstrap failed); calling it as an `if` condition lets the other
        // services proceed and skips wait_port for a service that did not start.
        script.push_str(&format!(
            "{guard}\n{preamble}  if install_service {id} {label} {port} <<PLIST\n{template}PLIST\n  then\n    wait_port {id} {label} {port}\n  fi\nelse\n  echo \"dashboard={id} action=skipped reason={skip_reason}\"\nfi\n"
        ));
    }
    script.push_str("\nif [ \"$FAILED\" -ne 0 ]; then exit 4; fi\n");
    script
}

/// A dashboard install, uninstall or restart plan: the script, what it will do and how to run it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardPlan {
    /// The dashboards the plan was built for.
    pub set: DashboardSet,
    /// The POSIX script that runs on the Mac.
    pub script: String,
    /// Human-readable steps for the dry-run printout.
    pub display_steps: Vec<String>,
    /// `ssh -o BatchMode=yes -o ConnectTimeout=10 -p <port> <user@host> "/bin/sh -s"` with the
    /// script streamed on standard input.
    pub spec: CommandSpec,
}

/// Builds the plan that installs the managed dashboard LaunchAgents over SSH. [R4.4]
///
/// The remote command is `/bin/sh -s` and the script travels on `spec.stdin` (LF-normalised):
/// the Mac's login shell is zsh and the script must run under `sh`, while streaming over stdin
/// keeps multi-line scripts out of command-line arguments. `BatchMode=yes` keeps an authentication
/// prompt from stalling the install. `display_steps` has one
/// `Ensure LaunchAgent <label> -> 127.0.0.1:<port>` line per managed service.
#[must_use = "the install plan should be printed or executed"]
pub fn build_install_plan(peer: &PeerNode, set: &DashboardSet) -> DashboardPlan {
    let script = render_install_script(set);
    let display_steps = set
        .managed()
        .map(|service| {
            format!(
                "Ensure LaunchAgent {} -> 127.0.0.1:{}",
                managed_label(service.id),
                service.port
            )
        })
        .collect();
    let spec = remote_sh_stdin_spec("Install dashboard services on MacBook", peer, &script);
    DashboardPlan {
        set: set.clone(),
        script,
        display_steps,
        spec,
    }
}

/// `ssh -o BatchMode=yes -o ConnectTimeout=10 -p <port> <user@host> "/bin/sh -s"` with `script`
/// LF-normalised on standard input, so no multi-line script appears in an `ssh` argument.
fn remote_sh_stdin_spec(description: &str, peer: &PeerNode, script: &str) -> CommandSpec {
    CommandSpec::new(
        description,
        "ssh",
        [
            "-o".to_string(),
            "BatchMode=yes".to_string(),
            "-o".to_string(),
            "ConnectTimeout=10".to_string(),
            "-p".to_string(),
            peer.ssh_port.to_string(),
            peer.ssh_destination(),
            "/bin/sh -s".to_string(),
        ],
    )
    .with_stdin(script)
}

/// `ssh -o BatchMode=yes -o ConnectTimeout=10 -p <port> <user@host> /bin/sh -c '<command>'`
/// for single-line remote commands such as `launchctl kickstart`.
fn remote_sh_spec(description: &str, peer: &PeerNode, command: &str) -> CommandSpec {
    CommandSpec::new(
        description,
        "ssh",
        [
            "-o".to_string(),
            "BatchMode=yes".to_string(),
            "-o".to_string(),
            "ConnectTimeout=10".to_string(),
            "-p".to_string(),
            peer.ssh_port.to_string(),
            peer.ssh_destination(),
            format!("/bin/sh -c {}", posix_single_quote(command)),
        ],
    )
}

/// Renders the POSIX `sh` script that unloads and deletes every LaunchAgent cheap-otto manages
/// and prints `dashboard=<id> action=removed` for each. [R8.2]
///
/// Only [`DashboardSet::managed`] services are touched, so the Reviewer's provisioner-owned
/// LaunchAgent survives. Unloading something that is not loaded is not an error, so the script
/// can run any number of times. Labels come from the registry, so nothing here needs quoting.
#[must_use = "the uninstall script should be run on the Mac or printed"]
pub fn render_uninstall_script(set: &DashboardSet) -> String {
    let mut script =
        String::from("set -eu\nUID_NUM=$(id -u)\nAGENTS_DIR=\"$HOME/Library/LaunchAgents\"\n");
    for service in set.managed() {
        let (id, label) = (service.id.slug(), &service.launchd_label);
        script.push_str(&format!(
            "launchctl bootout \"gui/$UID_NUM/{label}\" 2>/dev/null || true\n\
             rm -f \"$AGENTS_DIR/{label}.plist\"\n\
             echo \"dashboard={id} action=removed\"\n"
        ));
    }
    script
}

/// Builds the plan that removes the managed dashboard LaunchAgents over SSH. [R8.2]
///
/// `display_steps` has one `Unload and delete LaunchAgent <label>` line per managed service.
#[must_use = "the uninstall plan should be printed or executed"]
pub fn build_uninstall_plan(peer: &PeerNode, set: &DashboardSet) -> DashboardPlan {
    let script = render_uninstall_script(set);
    let display_steps = set
        .managed()
        .map(|service| format!("Unload and delete LaunchAgent {}", service.launchd_label))
        .collect();
    let spec = remote_sh_stdin_spec("Remove dashboard services from MacBook", peer, &script);
    DashboardPlan {
        set: set.clone(),
        script,
        display_steps,
        spec,
    }
}

/// Builds `launchctl kickstart -k "gui/$(id -u)/<label>"` over SSH, which restarts the service
/// whether or not it is running. Works for all three dashboards, the Reviewer included.
///
/// Returns `None` only when `id` is not in `set`.
#[must_use = "the restart command should be printed or executed"]
pub fn build_restart_spec(
    peer: &PeerNode,
    set: &DashboardSet,
    id: DashboardId,
) -> Option<CommandSpec> {
    let service = set.get(id)?;
    let script = format!(
        "launchctl kickstart -k \"gui/$(id -u)/{}\"",
        service.launchd_label
    );
    let description = format!("Restart {} on MacBook", service.display_name);
    Some(remote_sh_spec(&description, peer, &script))
}

// Status (design 3.4): a read-only probe run on the Mac, a tolerant parser for what it prints,
// and the diagnosis of why a dashboard is unavailable. `src/main.rs` runs the probe.

/// Longest value the parser keeps for free-text fields such as `last_exit` and `autologin_user`.
const STATUS_VALUE_MAX_CHARS: usize = 64;

/// Renders the read-only status probe: one `dashboard=<id> …` line per dashboard in set order,
/// then one `host …` line. [R5.1, R5.2, R5.5]
///
/// It only runs `launchctl print`, `nc -z`, `command -v` and `defaults read`; it never starts,
/// stops or changes anything. The text is built from constants, registry labels and `u16` ports,
/// so no operator input reaches the Mac's shell. Every value it prints is a single token, so
/// [`parse_status_output`] can split on whitespace.
#[must_use = "the status script should be run on the Mac"]
pub fn render_status_script(set: &DashboardSet) -> String {
    let mut script = String::from(
        r#"set -u
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
UID_NUM=$(id -u)

# status_line <id> <label> <port>
status_line() {
  loaded=0; pid=""; last_exit=""; listening=0
  if info=$(launchctl print "gui/$UID_NUM/$2" 2>/dev/null); then
    loaded=1
    pid=$(printf '%s\n' "$info" | awk '/^[[:space:]]*pid = /{print $3; exit}')
    last_exit=$(printf '%s\n' "$info" | awk -F'= ' '/last exit code = /{print $2; exit}' | tr ' ' '_')
  fi
  if nc -z 127.0.0.1 "$3" >/dev/null 2>&1; then listening=1; fi
  echo "dashboard=$1 loaded=$loaded pid=${pid:--} listening=$listening last_exit=${last_exit:--}"
}

"#,
    );
    for service in set.services() {
        script.push_str(&format!(
            "status_line {} {} {}\n",
            service.id.slug(),
            service.launchd_label,
            service.port
        ));
    }
    script.push_str(
        r#"
if [ -x "$HOME/.cargo/bin/cheap-otto" ]; then cheap_otto_bin=1; else cheap_otto_bin=0; fi
if command -v pixel-agents >/dev/null 2>&1; then pixel_agents_bin=1; else pixel_agents_bin=0; fi
if launchctl print "gui/$UID_NUM" >/dev/null 2>&1; then gui_session=1; else gui_session=0; fi
autologin_user=$(defaults read /Library/Preferences/com.apple.loginwindow autoLoginUser 2>/dev/null || true)
autologin_user=$(printf '%s' "$autologin_user" | tr -s ' \t\n' '_')
echo "host cheap_otto_bin=$cheap_otto_bin pixel_agents_bin=$pixel_agents_bin gui_session=$gui_session autologin_user=${autologin_user:--}"
"#,
    );
    script
}

/// Builds `ssh -o BatchMode=yes -o ConnectTimeout=3 -p <port> <user@host> "/bin/sh -s"` with
/// [`render_status_script`] piped on standard input. [R5.5]
///
/// The Mac's login shell is zsh, so the remote command invokes `/bin/sh -s` and reads the
/// multi-line script from stdin rather than passing it as a quoted command-line argument.
#[must_use = "the status probe should be displayed or executed"]
pub fn build_status_spec(peer: &PeerNode, set: &DashboardSet) -> CommandSpec {
    peer.build_ssh_probe_spec(
        "Read every dashboard's LaunchAgent, pid and port on the Mac (read-only)",
        "/bin/sh -s",
    )
    .with_stdin(render_status_script(set))
}

/// What the probe saw for one dashboard on the Mac. [R5.1]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardRemoteStatus {
    /// Which dashboard the line describes.
    pub id: DashboardId,
    /// `launchctl print` found the LaunchAgent in the user's GUI domain.
    pub loaded: bool,
    /// The running process id, when `launchd` reports one.
    pub pid: Option<u32>,
    /// The port answers on `127.0.0.1` on the Mac.
    pub listening: bool,
    /// `launchd`'s last exit status (spaces turned into `_`), when there is one.
    pub last_exit: Option<String>,
}

/// Facts about the Mac that explain why a dashboard cannot run. [R5.2]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardHostFacts {
    /// `~/.cargo/bin/cheap-otto` exists and is executable.
    pub cheap_otto_bin: bool,
    /// `pixel-agents` is on the probe's `PATH`.
    pub pixel_agents_bin: bool,
    /// The user has a GUI session, which LaunchAgents need.
    pub gui_session: bool,
    /// The automatic-login user, or `None` when automatic login is off. [R5.3]
    pub autologin_user: Option<String>,
}

/// Everything the status probe reported, as parsed by [`parse_status_output`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardStatusReport {
    /// One entry per dashboard line, in set order; a dashboard with no line has no entry.
    pub dashboards: Vec<DashboardRemoteStatus>,
    /// The `host` line, or `None` when the probe printed none.
    pub host: Option<DashboardHostFacts>,
}

/// `key=value` tokens of one probe line. Tokens without `=` are skipped.
fn status_tokens<'a>(
    tokens: impl Iterator<Item = &'a str>,
) -> impl Iterator<Item = (&'a str, &'a str)> {
    tokens.filter_map(|token| token.split_once('='))
}

/// `1` is true; anything else, including a missing key, is false.
fn status_flag(value: &str) -> bool {
    value == "1"
}

/// `-` and empty values are `None`. Control characters are dropped and the value is capped, so a
/// strange `launchctl` or `defaults` answer cannot garble the operator's terminal.
fn status_text(value: &str) -> Option<String> {
    let clean: String = value
        .chars()
        .filter(|ch| !ch.is_control())
        .take(STATUS_VALUE_MAX_CHARS)
        .collect();
    (!clean.is_empty() && clean != "-").then_some(clean)
}

/// Parses what [`render_status_script`] printed. [R5.1, R5.2]
///
/// Never panics. Lines that are neither `dashboard=<id> …` nor `host …`, unknown keys, unknown
/// ids and ids outside `set` are ignored. A missing or unparsable flag is false; `pid=-`, an
/// unparsable pid, `last_exit=-` and `autologin_user=-` are `None`. A `host` line without any
/// known key is ignored. When an id or the `host` line appears twice, the last one wins.
#[must_use = "the parsed report should be explained or printed"]
pub fn parse_status_output(raw: &str, set: &DashboardSet) -> DashboardStatusReport {
    let mut seen: Vec<DashboardRemoteStatus> = Vec::new();
    let mut host = None;
    for line in raw.lines() {
        let mut tokens = line.split_whitespace();
        let Some(first) = tokens.next() else {
            continue;
        };
        if first == "host" {
            let mut facts = DashboardHostFacts::default();
            let mut any_known_key = false;
            for (key, value) in status_tokens(tokens) {
                match key {
                    "cheap_otto_bin" => facts.cheap_otto_bin = status_flag(value),
                    "pixel_agents_bin" => facts.pixel_agents_bin = status_flag(value),
                    "gui_session" => facts.gui_session = status_flag(value),
                    "autologin_user" => facts.autologin_user = status_text(value),
                    _ => continue,
                }
                any_known_key = true;
            }
            // A bare `host` (a stray word, not the probe's line) would otherwise read as "no GUI
            // session" and blame the wrong cause.
            if any_known_key {
                host = Some(facts);
            }
            continue;
        }
        let Some(slug) = first.strip_prefix("dashboard=") else {
            continue;
        };
        let Some(id) = DashboardId::parse(slug)
            .ok()
            .filter(|id| set.get(*id).is_some())
        else {
            continue;
        };
        let mut status = DashboardRemoteStatus {
            id,
            loaded: false,
            pid: None,
            listening: false,
            last_exit: None,
        };
        for (key, value) in status_tokens(tokens) {
            match key {
                "loaded" => status.loaded = status_flag(value),
                "pid" => status.pid = value.parse::<u32>().ok().filter(|pid| *pid > 0),
                "listening" => status.listening = status_flag(value),
                "last_exit" => status.last_exit = status_text(value),
                _ => {}
            }
        }
        seen.retain(|earlier| earlier.id != id);
        seen.push(status);
    }
    let dashboards = set
        .services()
        .iter()
        .filter_map(|service| seen.iter().find(|status| status.id == service.id))
        .cloned()
        .collect();
    DashboardStatusReport { dashboards, host }
}

/// Why a dashboard is unavailable, and where its log is. [R5.1]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardDiagnosis {
    /// The first cause that applies, with what to do about it.
    pub reason: String,
    /// The dashboard's log on the Mac, always given.
    pub log: String,
}

/// The dashboard's log on the Mac, as the operator would type it.
#[must_use = "returns the log path to show the operator"]
pub fn dashboard_log_path(id: DashboardId) -> String {
    match id {
        DashboardId::MissionControl | DashboardId::PixelOffice => {
            format!("~/.cheap-otto/logs/dashboards/{}.log", id.slug())
        }
        DashboardId::Reviewer => "~/.cheap-otto/logs/code-reviewer/ui-server.log".to_string(),
    }
}

/// `None` when the dashboard is loaded and listening. Otherwise the first cause that applies, in
/// the order of the table in design 3.4; the host rows are skipped when `host` is `None`. [R5.1]
///
/// A port that answers while the LaunchAgent is not loaded is held by another process (what a
/// port-held install leaves behind), so it is explained rather than shown as healthy.
#[must_use = "the diagnosis should be shown next to the dashboard"]
pub fn explain_unavailable(
    service: &DashboardService,
    status: &DashboardRemoteStatus,
    host: Option<&DashboardHostFacts>,
) -> Option<DashboardDiagnosis> {
    if status.loaded && status.listening {
        return None;
    }
    let host_reason = host.and_then(|host| {
        if !host.gui_session {
            Some("No GUI session for the Mac user; log in to the Mac desktop or enable automatic login".to_string())
        } else if service.id == DashboardId::MissionControl && !host.cheap_otto_bin {
            Some("`cheap-otto` is not installed on the Mac; run `code-reviewer m1 sync`".to_string())
        } else if service.id == DashboardId::PixelOffice && !host.pixel_agents_bin {
            Some("`pixel-agents` is not installed on the Mac, so the service was skipped".to_string())
        } else {
            None
        }
    });
    let install = match service.manager {
        ServiceManager::CheapOtto => "`cheap-otto dashboards install --execute`",
        ServiceManager::CodeReviewerProvisioner => "`code-reviewer m1 sync`",
    };
    let port = service.port;
    let reason = host_reason.unwrap_or_else(|| match (status.loaded, status.listening) {
        (false, true) => format!(
            "The LaunchAgent is not loaded, but port {port} answers, so another process holds it (a port-held install leaves this); find it with `lsof -nP -iTCP:{port} -sTCP:LISTEN` on the Mac, then run {install}"
        ),
        (false, false) => format!(
            "The LaunchAgent is not loaded; run {install}. A failed or port-held install leaves this too"
        ),
        (true, _) => match &status.last_exit {
            Some(last_exit) => format!(
                "The LaunchAgent is loaded but port {port} is not listening; launchd's last exit status is {last_exit}"
            ),
            None => format!(
                "The LaunchAgent is loaded but port {port} is not listening; launchd reports no last exit status"
            ),
        },
    });
    Some(DashboardDiagnosis {
        reason,
        log: dashboard_log_path(service.id),
    })
}

/// One printable row per dashboard in set order: loaded, pid, listening on the Mac, tunnelled to
/// the workstation, and the reason and log when it is unavailable. [R5.1]
///
/// `tunnelled` holds the CLI's local port checks; a dashboard missing from it shows `-`. A
/// dashboard the probe printed no line for is explained as such, with its log.
#[must_use = "the status rows should be printed"]
pub fn render_status_rows(
    report: &DashboardStatusReport,
    set: &DashboardSet,
    tunnelled: &[(DashboardId, bool)],
) -> Vec<String> {
    let yes_no = |value: bool| if value { "yes" } else { "no" };
    set.services()
        .iter()
        .map(|service| {
            let name = format!("{} ({})", service.display_name, service.port);
            let tunnel = tunnelled
                .iter()
                .find(|(id, _)| *id == service.id)
                .map_or("-", |(_, up)| yes_no(*up));
            let status = report.dashboards.iter().find(|s| s.id == service.id);
            let Some(status) = status else {
                return format!(
                    "{name}: no status from the Mac, tunnelled={tunnel}; reason: the probe printed no line for it; log: {}",
                    dashboard_log_path(service.id)
                );
            };
            let pid = status
                .pid
                .map_or_else(|| "-".to_string(), |pid| pid.to_string());
            let row = format!(
                "{name}: loaded={} pid={pid} listening={} tunnelled={tunnel}",
                yes_no(status.loaded),
                yes_no(status.listening)
            );
            match explain_unavailable(service, status, report.host.as_ref()) {
                Some(why) => format!("{row}; reason: {}; log: {}", why.reason, why.log),
                None => row,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_set_lists_three_dashboards_with_default_ports() -> Result<(), DashboardError> {
        let set = DashboardSet::standard();
        let ids: Vec<DashboardId> = set.services().iter().map(|service| service.id).collect();
        assert_eq!(ids, DashboardId::all());
        assert_eq!(set.ports(), [7420, 7421, 7430]);
        assert_eq!(
            set.get(DashboardId::PixelOffice)
                .map(|s| s.display_name.as_str()),
            Some("Pixel Office")
        );
        // `standard` skips validation, so prove the defaults would pass it.
        assert_eq!(
            DashboardSet::with_ports(MISSION_CONTROL_PORT, PIXEL_OFFICE_PORT, REVIEWER_PORT)?,
            set
        );
        Ok(())
    }

    #[test]
    fn rejects_zero_and_duplicate_ports() {
        assert_eq!(
            DashboardSet::with_ports(0, 7421, 7430),
            Err(DashboardError::InvalidPort(0))
        );
        assert_eq!(
            DashboardSet::with_ports(7420, 7421, 0),
            Err(DashboardError::InvalidPort(0))
        );
        assert_eq!(
            DashboardSet::with_ports(7420, 7420, 7430),
            Err(DashboardError::DuplicatePort(7420))
        );
        assert_eq!(
            DashboardSet::with_ports(7420, 7421, 7420),
            Err(DashboardError::DuplicatePort(7420))
        );
        assert!(DashboardSet::with_ports(8420, 8421, 8430).is_ok());
    }

    #[test]
    fn managed_labels_use_reserved_dashboards_namespace() {
        let set = DashboardSet::standard();
        let managed: Vec<&str> = set
            .managed()
            .map(|service| service.launchd_label.as_str())
            .collect();
        assert_eq!(
            managed,
            [
                "com.cheapotto.dashboards.mission-control",
                "com.cheapotto.dashboards.pixel-office",
            ]
        );
        let reviewer = set.get(DashboardId::Reviewer);
        assert_eq!(
            reviewer.map(|s| (s.launchd_label.as_str(), s.manager)),
            Some((
                REVIEWER_LAUNCHD_LABEL,
                ServiceManager::CodeReviewerProvisioner
            ))
        );
    }

    #[test]
    fn parses_dashboard_ids_and_rejects_unknown() {
        for &id in DashboardId::all() {
            assert_eq!(DashboardId::parse(id.slug()), Ok(id));
        }
        assert_eq!(DashboardId::parse(" reviewer\n"), Ok(DashboardId::Reviewer));
        for bad in ["", "nope", "Reviewer", "mission_control", "pixel"] {
            assert_eq!(
                DashboardId::parse(bad),
                Err(DashboardError::UnknownDashboard(bad.to_string()))
            );
        }
        let message = DashboardError::UnknownDashboard("nope".to_string()).to_string();
        assert!(message.contains("mission-control, pixel-office or reviewer"));
    }

    #[test]
    fn set_serializes_with_kebab_case_ids() -> Result<(), serde_json::Error> {
        let set = DashboardSet::standard();
        let json = serde_json::to_value(&set)?;
        assert_eq!(json["services"][0]["id"], "mission-control");
        assert_eq!(json["services"][1]["id"], "pixel-office");
        assert_eq!(json["services"][2]["manager"], "code-reviewer-provisioner");
        assert_eq!(json["services"][0]["manager"], "cheap-otto");
        let back: DashboardSet = serde_json::from_value(json)?;
        assert_eq!(back, set);
        Ok(())
    }

    #[test]
    fn deserializing_validates_ports_and_services_like_with_ports() -> Result<(), serde_json::Error>
    {
        let good = serde_json::to_value(DashboardSet::standard())?;
        let parse = |value: serde_json::Value| serde_json::from_value::<DashboardSet>(value);

        let mut zero = good.clone();
        zero["services"][1]["port"] = serde_json::json!(0);
        let err = parse(zero).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err.contains("invalid dashboard port 0"), "{err}");

        let mut duplicate = good.clone();
        duplicate["services"][2]["port"] = serde_json::json!(7420);
        let err = parse(duplicate)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(err.contains("dashboard port 7420 is used twice"), "{err}");

        let mut missing = good.clone();
        if let Some(services) = missing["services"].as_array_mut() {
            services.pop();
        }
        let err = parse(missing)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(err.contains("once each, in that order"), "{err}");

        let mut reordered = good.clone();
        if let Some(services) = reordered["services"].as_array_mut() {
            services.swap(0, 1);
        }
        assert!(parse(reordered).is_err());

        assert_eq!(parse(good)?, DashboardSet::standard());
        Ok(())
    }

    fn template(set: &DashboardSet, id: DashboardId) -> String {
        set.get(id)
            .and_then(|service| render_service_plist_template(service, set))
            .unwrap_or_default()
    }

    #[test]
    fn mission_control_plist_keeps_alive_and_binds_loopback() {
        let plist = template(&DashboardSet::standard(), DashboardId::MissionControl);
        assert!(plist.contains("<string>com.cheapotto.dashboards.mission-control</string>"));
        assert!(plist.contains("<key>KeepAlive</key>\n  <true/>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(plist.contains("<string>127.0.0.1</string>"));
        assert!(plist.contains(
            "<string>$HOME_XML/.cheap-otto/logs/dashboards/mission-control.log</string>"
        ));
        assert!(plist.contains("<string>serve</string>\n    <string>--host</string>"));
        assert!(plist.contains("<key>ThrottleInterval</key>\n  <integer>10</integer>"));
        assert!(!plist.contains("0.0.0.0"));
        assert!(!plist.contains('\r'));
    }

    #[test]
    fn pixel_office_plist_uses_resolved_binary_placeholder() {
        let plist = template(&DashboardSet::standard(), DashboardId::PixelOffice);
        assert!(plist.contains("<string>com.cheapotto.dashboards.pixel-office</string>"));
        assert!(plist.contains("<array>\n    <string>$PIXEL_BIN_XML</string>"));
        assert!(plist.contains("<string>--no-terminal</string>"));
        assert!(plist.contains("<string>--port</string>\n    <string>7421</string>"));
        assert!(plist.contains("<string>--host</string>\n    <string>127.0.0.1</string>"));
        assert!(plist.contains("logs/dashboards/pixel-office.log</string>"));
        assert!(!plist.contains("0.0.0.0"));
    }

    #[test]
    fn plist_templates_use_escaped_placeholders_not_raw_home() {
        let set = DashboardSet::standard();
        for id in [DashboardId::MissionControl, DashboardId::PixelOffice] {
            let plist = template(&set, id);
            assert!(plist.contains("$HOME_XML"), "{id:?}");
            let rest = plist.replace("$HOME_XML", "").replace("$PIXEL_BIN_XML", "");
            assert!(!rest.contains("$HOME"), "{id:?}: {rest}");
            assert!(!rest.contains("$PIXEL_BIN"), "{id:?}: {rest}");
            assert!(!rest.contains('$'), "{id:?}: {rest}");
        }
    }

    #[test]
    fn reviewer_has_no_template_because_provisioner_manages_it() {
        let set = DashboardSet::standard();
        assert_eq!(
            set.get(DashboardId::Reviewer)
                .and_then(|service| render_service_plist_template(service, &set)),
            None
        );
        assert_eq!(
            set.managed()
                .filter_map(|service| render_service_plist_template(service, &set))
                .count(),
            2
        );
    }

    #[test]
    fn plist_ports_follow_the_set() -> Result<(), DashboardError> {
        let set = DashboardSet::with_ports(8420, 8421, 8430)?;
        let mission_control = template(&set, DashboardId::MissionControl);
        assert!(mission_control.contains("<string>--port</string>\n    <string>8420</string>"));
        assert!(
            mission_control.contains("<string>--pixel-port</string>\n    <string>8421</string>")
        );
        assert!(!mission_control.contains("7420"));
        assert!(!mission_control.contains("7421"));
        let pixel_office = template(&set, DashboardId::PixelOffice);
        assert!(pixel_office.contains("<string>8421</string>"));
        assert!(!pixel_office.contains("7421"));
        Ok(())
    }

    #[test]
    fn plist_label_comes_from_the_id_not_from_the_service_text() -> Result<(), DashboardError> {
        let set = DashboardSet::standard();
        let mut service = set
            .get(DashboardId::MissionControl)
            .cloned()
            .ok_or(DashboardError::UnexpectedServices)?;
        service.launchd_label = "x'; rm -rf ~ #$(id)</string><string>-evil".to_string();
        service.port = 9999;
        let plist = render_service_plist_template(&service, &set).unwrap_or_default();
        assert!(plist.contains("<string>com.cheapotto.dashboards.mission-control</string>"));
        assert!(plist.contains("<string>7420</string>"));
        assert!(!plist.contains("rm -rf"));
        assert!(!plist.contains("$(id)"));
        assert!(!plist.contains("9999"));
        Ok(())
    }

    #[test]
    fn deserializing_rejects_services_that_differ_from_the_registry()
    -> Result<(), Box<dyn std::error::Error>> {
        let good = serde_json::to_value(DashboardSet::standard())?;
        let parse = |value: serde_json::Value| serde_json::from_value::<DashboardSet>(value);
        let message = DashboardError::UnexpectedServices.to_string();

        let mut manager = good.clone();
        manager["services"][2]["manager"] = serde_json::json!("cheap-otto");
        let err = parse(manager)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(err.contains(&message), "{err}");

        let mut label = good.clone();
        label["services"][0]["launchd_label"] = serde_json::json!("com.cheapotto.other");
        let err = parse(label)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(err.contains(&message), "{err}");

        let mut name = good.clone();
        name["services"][1]["display_name"] = serde_json::json!("Renamed");
        let err = parse(name).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err.contains(&message), "{err}");

        let custom = DashboardSet::with_ports(8420, 8421, 8430)?;
        let round_trip = parse(serde_json::to_value(&custom)?)?;
        assert_eq!(round_trip, custom);
        Ok(())
    }

    // --- Tunnel (T1.7) ---

    fn tunnel_peer() -> Result<PeerNode, crate::bridge::BridgeError> {
        PeerNode::new("otto-mac", "192.0.2.10", "admin", 2222)
    }

    fn covering_record(peer: &PeerNode) -> TunnelRecord {
        TunnelRecord {
            pid: 4242,
            ports: DashboardSet::standard().ports(),
            destination: peer.ssh_destination(),
            ssh_port: peer.ssh_port,
            started_epoch_secs: 1_700_000_000,
            process_started_at: Some("2026-11-16T21:00:00.0000000Z".to_string()),
        }
    }

    #[test]
    fn tunnel_spec_forwards_every_port_on_loopback_with_safe_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let peer = tunnel_peer()?;
        let spec = build_tunnel_spec(&peer, &DashboardSet::standard());
        assert_eq!(spec.program, "ssh");
        assert!(!spec.allow_failure);
        assert_eq!(
            spec.args,
            [
                "-N",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ServerAliveInterval=30",
                "-o",
                "ServerAliveCountMax=3",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                "-p",
                "2222",
                "-L",
                "127.0.0.1:7420:127.0.0.1:7420",
                "-L",
                "127.0.0.1:7421:127.0.0.1:7421",
                "-L",
                "127.0.0.1:7430:127.0.0.1:7430",
                "admin@192.0.2.10",
            ]
        );
        let forwards: Vec<&String> = spec
            .args
            .windows(2)
            .filter(|pair| pair[0] == "-L")
            .map(|pair| &pair[1])
            .collect();
        assert_eq!(forwards.len(), 3);
        for forward in forwards {
            // Loopback bind on the workstation, loopback target on the Mac.
            assert!(forward.starts_with("127.0.0.1:"), "{forward}");
            assert!(forward.contains(":127.0.0.1:"), "{forward}");
        }
        assert_eq!(
            spec.args.last().map(String::as_str),
            Some("admin@192.0.2.10")
        );

        // Custom ports are forwarded in set order.
        let custom = DashboardSet::with_ports(8420, 8421, 8430)?;
        let rendered = build_tunnel_spec(&peer, &custom).display_command();
        assert!(rendered.contains(
            "-L 127.0.0.1:8420:127.0.0.1:8420 -L 127.0.0.1:8421:127.0.0.1:8421 -L 127.0.0.1:8430:127.0.0.1:8430 admin@192.0.2.10"
        ));
        Ok(())
    }

    #[test]
    fn tunnel_destination_rejects_hostile_input_and_stays_one_argument()
    -> Result<(), crate::bridge::BridgeError> {
        // Hostile hosts and users never become a PeerNode, so they never reach the ssh argv,
        // where a leading `-` would be read as an ssh option.
        for host in [
            "-oProxyCommand=touch /tmp/x",
            "192.0.2.10; rm -rf ~",
            "$(reboot)",
            "x'; rm -rf ~ #",
        ] {
            assert!(
                PeerNode::new("otto-mac", host, "admin", 22).is_err(),
                "{host}"
            );
        }
        for user in ["-oProxyCommand=x", "admin;id", "$(id)", "a'b"] {
            assert!(
                PeerNode::new("otto-mac", "192.0.2.10", user, 22).is_err(),
                "{user}"
            );
        }
        // The destination is one discrete argument and there is no remote command for a shell.
        let spec = build_tunnel_spec(&tunnel_peer()?, &DashboardSet::standard());
        assert_eq!(
            spec.args
                .iter()
                .filter(|arg| arg.contains("192.0.2.10"))
                .count(),
            1
        );
        assert!(spec.args.contains(&"-N".to_string()));

        // A look-alike destination in a recycled pid's command line is not the recorded tunnel.
        let record = covering_record(&tunnel_peer()?);
        let forwards = "-L 127.0.0.1:7420:127.0.0.1:7420 -L 127.0.0.1:7421:127.0.0.1:7421 -L 127.0.0.1:7430:127.0.0.1:7430";
        for hostile in [
            format!("ssh -N {forwards} admin@192.0.2.10.example.net"),
            format!("ssh -N {forwards} 'admin@192.0.2.10;id'"),
            format!("ssh -N {forwards} evil@admin@192.0.2.10"),
            format!("sh -c 'ssh -N {forwards} admin@192.0.2.10'"),
        ] {
            assert!(
                !command_line_is_recorded_tunnel(&hostile, &record),
                "{hostile}"
            );
        }
        Ok(())
    }

    #[test]
    fn decides_already_up_start_and_conflict() {
        let all_up = [(7420, true), (7421, true), (7430, true)];
        assert_eq!(decide_tunnel(&all_up, true), TunnelDecision::AlreadyUp);

        let all_down = [(7420, false), (7421, false), (7430, false)];
        assert_eq!(decide_tunnel(&all_down, true), TunnelDecision::Start);
        assert_eq!(decide_tunnel(&all_down, false), TunnelDecision::Start);
        assert_eq!(decide_tunnel(&[], false), TunnelDecision::Start);
        assert_eq!(decide_tunnel(&[], true), TunnelDecision::Start);

        let mixed = [(7420, false), (7421, true), (7430, false)];
        let expected = TunnelDecision::Conflict {
            held: vec![7421],
            missing: vec![7420, 7430],
        };
        assert_eq!(decide_tunnel(&mixed, false), expected);
        // A covering record does not hide that some forwards are down.
        assert_eq!(decide_tunnel(&mixed, true), expected);
    }

    #[test]
    fn all_ports_answering_without_a_recorded_tunnel_is_a_conflict() {
        let all_up = [(7420, true), (7421, true), (7430, true)];
        assert_eq!(
            decide_tunnel(&all_up, false),
            TunnelDecision::Conflict {
                held: vec![7420, 7421, 7430],
                missing: vec![],
            }
        );
    }

    #[test]
    fn recorded_tunnel_must_be_alive_ssh_and_cover_every_port()
    -> Result<(), crate::bridge::BridgeError> {
        let peer = tunnel_peer()?;
        let set = DashboardSet::standard();
        let record = covering_record(&peer);

        assert!(!recorded_tunnel_covers(None, true, &set, &peer));
        // A dead pid, or one whose command line is not the recorded ssh tunnel.
        assert!(!recorded_tunnel_covers(Some(&record), false, &set, &peer));

        let mut missing_one = record.clone();
        missing_one.ports.retain(|&port| port != PIXEL_OFFICE_PORT);
        assert!(!recorded_tunnel_covers(
            Some(&missing_one),
            true,
            &set,
            &peer
        ));

        let mut empty = record.clone();
        empty.ports.clear();
        assert!(!recorded_tunnel_covers(Some(&empty), true, &set, &peer));

        assert!(recorded_tunnel_covers(Some(&record), true, &set, &peer));
        let mut extra = record;
        extra.ports.push(5173);
        assert!(recorded_tunnel_covers(Some(&extra), true, &set, &peer));
        Ok(())
    }

    #[test]
    fn recorded_tunnel_for_another_peer_or_ssh_port_does_not_cover()
    -> Result<(), crate::bridge::BridgeError> {
        let peer = tunnel_peer()?;
        let set = DashboardSet::standard();
        let record = covering_record(&peer);

        let mut other_host = record.clone();
        other_host.destination = "admin@198.51.100.7".to_string();
        assert!(!recorded_tunnel_covers(
            Some(&other_host),
            true,
            &set,
            &peer
        ));

        let mut other_user = record.clone();
        other_user.destination = "someone@192.0.2.10".to_string();
        assert!(!recorded_tunnel_covers(
            Some(&other_user),
            true,
            &set,
            &peer
        ));

        let mut other_port = record;
        other_port.ssh_port = 22;
        assert!(!recorded_tunnel_covers(
            Some(&other_port),
            true,
            &set,
            &peer
        ));
        Ok(())
    }

    #[test]
    fn tunnel_failure_message_names_unanswered_ports_for_exit_and_timeout() {
        let test_command = "ssh -p 2222 admin@192.0.2.10 true";
        let failures = [
            TunnelFailure::ExitedEarly {
                status: "exit code: 255".to_string(),
            },
            TunnelFailure::TimedOut { waited_secs: 5 },
        ];
        for failure in &failures {
            let message = describe_tunnel_failure(failure, &[7421, 7430], test_command);
            assert!(message.contains("7421"), "{message}");
            assert!(message.contains("7430"), "{message}");
            assert!(!message.contains("7420"), "{message}");
            assert!(message.ends_with(test_command), "{message}");
        }
        let exited = describe_tunnel_failure(&failures[0], &[7420], test_command);
        assert!(exited.contains("exited") && exited.contains("exit code: 255"));
        let timed_out = describe_tunnel_failure(&failures[1], &[7420], test_command);
        assert!(timed_out.contains("within 5 s"), "{timed_out}");
        let none = describe_tunnel_failure(&failures[1], &[], test_command);
        assert!(none.contains("127.0.0.1: none."), "{none}");
    }

    #[test]
    fn command_line_must_match_recorded_destination_and_forwards()
    -> Result<(), crate::bridge::BridgeError> {
        let peer = tunnel_peer()?;
        let record = covering_record(&peer);
        let own = build_tunnel_spec(&peer, &DashboardSet::standard()).display_command();
        assert!(command_line_is_recorded_tunnel(&own, &record), "{own}");
        // As `ps -o args=` on macOS and Win32 `CommandLine` (quoted program, spaces) report it.
        let args = own.trim_start_matches("ssh ");
        for line in [
            format!("/usr/bin/ssh {args}"),
            format!("\"C:\\Program Files\\OpenSSH\\ssh.exe\" {args}"),
            format!("\"ssh\" {args}\n"),
        ] {
            assert!(command_line_is_recorded_tunnel(&line, &record), "{line}");
        }

        let one_missing = own.replace("-L 127.0.0.1:7421:127.0.0.1:7421 ", "");
        let other_host = own.replace("admin@192.0.2.10", "admin@198.51.100.7");
        for line in [
            "ssh user@other-host".to_string(),
            "ssh admin@192.0.2.10".to_string(),
            one_missing,
            other_host,
            String::new(),
            "   ".to_string(),
            format!("python3 {args}"),
            format!("sshd {args}"),
            own.replace(" -N", ""),
            // Another SSH port than the recorded one.
            own.replace("-p 2222", "-p 22"),
            // A remote command after the destination.
            format!("{own} sleep 3600"),
            // The right tokens, in another order.
            own.replace("-N -o", "-o")
                .replace("admin@192.0.2.10", "-N admin@192.0.2.10"),
            // An extra forward the record does not list.
            own.replace(" admin@", " -L 127.0.0.1:5173:127.0.0.1:5173 admin@"),
        ] {
            assert!(!command_line_is_recorded_tunnel(&line, &record), "{line}");
        }

        // A record without ports never matches, so a bare session to the Mac is never signalled.
        let mut empty = record;
        empty.ports.clear();
        assert!(!command_line_is_recorded_tunnel(&own, &empty));
        Ok(())
    }

    #[test]
    fn tunnel_record_round_trips_as_json() -> Result<(), serde_json::Error> {
        let record = TunnelRecord {
            pid: 4242,
            ports: vec![7420, 7421, 7430],
            destination: "admin@192.0.2.10".to_string(),
            ssh_port: 2222,
            started_epoch_secs: 1_700_000_000,
            process_started_at: Some("Mon Nov 16 15:00:00 2026".to_string()),
        };
        let json = serde_json::to_string_pretty(&record)?;
        assert!(json.contains("\"ssh_port\": 2222"), "{json}");
        assert!(
            json.contains("\"started_epoch_secs\": 1700000000"),
            "{json}"
        );
        assert!(
            json.contains("\"process_started_at\": \"Mon Nov 16 15:00:00 2026\""),
            "{json}"
        );
        let back: TunnelRecord = serde_json::from_str(&json)?;
        assert_eq!(back, record);

        // Legacy records written before `process_started_at` existed still deserialize cleanly.
        let legacy_json = r#"{
  "pid": 4242,
  "ports": [7420, 7421, 7430],
  "destination": "admin@192.0.2.10",
  "ssh_port": 2222,
  "started_epoch_secs": 1700000000
}"#;
        let legacy: TunnelRecord = serde_json::from_str(legacy_json)?;
        assert_eq!(legacy.process_started_at, None);
        assert_eq!(legacy.pid, 4242);
        Ok(())
    }

    #[test]
    fn process_identity_checks_start_time_and_falls_back_for_legacy_records()
    -> Result<(), crate::bridge::BridgeError> {
        let peer = tunnel_peer()?;
        let set = DashboardSet::standard();
        let cmdline = build_tunnel_spec(&peer, &set).display_command();
        let record = covering_record(&peer);

        // Exact command-line and start-time match (surrounding whitespace tolerated).
        assert!(process_matches_recorded_tunnel(
            &cmdline,
            Some("  2026-11-16T21:00:00.0000000Z\n"),
            &record
        ));
        // Recycled PID with an identical command line but different or unreadable start time fails.
        assert!(!process_matches_recorded_tunnel(
            &cmdline,
            Some("2026-11-16T21:05:00.0000000Z"),
            &record
        ));
        assert!(!process_matches_recorded_tunnel(&cmdline, None, &record));
        assert!(!process_matches_recorded_tunnel(
            "ssh admin@other-host",
            record.process_started_at.as_deref(),
            &record
        ));

        // Corrupt empty `Some("")` fails closed rather than acting like `None`.
        let mut empty_stamp = record.clone();
        empty_stamp.process_started_at = Some("   ".to_string());
        assert!(!process_matches_recorded_tunnel(
            &cmdline,
            None,
            &empty_stamp
        ));
        assert!(!process_matches_recorded_tunnel(
            &cmdline,
            Some(""),
            &empty_stamp
        ));

        // Legacy `None` falls back to the command-line check.
        let mut legacy = record;
        legacy.process_started_at = None;
        assert!(process_matches_recorded_tunnel(&cmdline, None, &legacy));
        assert!(process_matches_recorded_tunnel(
            &cmdline,
            Some("Mon Nov 16 15:00:00 2026"),
            &legacy
        ));
        assert!(!process_matches_recorded_tunnel(
            "ssh admin@other-host",
            Some("Mon Nov 16 15:00:00 2026"),
            &legacy
        ));
        Ok(())
    }

    #[test]
    fn utc_style_start_stamp_matches_and_a_different_stamp_is_rejected()
    -> Result<(), crate::bridge::BridgeError> {
        let peer = tunnel_peer()?;
        let set = DashboardSet::standard();
        let cmdline = build_tunnel_spec(&peer, &set).display_command();
        let mut record = covering_record(&peer);
        record.process_started_at = Some("Mon Nov 16 21:00:00 2026".to_string());

        assert!(process_matches_recorded_tunnel(
            &cmdline,
            Some("Mon Nov 16 21:00:00 2026\n"),
            &record
        ));
        assert!(!process_matches_recorded_tunnel(
            &cmdline,
            Some("Mon Nov 16 15:00:00 2026"),
            &record
        ));
        Ok(())
    }

    // --- Install script and plan (T1.4) ---

    /// The text of the shell function `name`, from its definition to its closing brace.
    fn shell_function<'a>(script: &'a str, name: &str) -> &'a str {
        let start = script
            .find(&format!("{name}() {{\n"))
            .unwrap_or_else(|| panic!("{name} is not defined"));
        let len = script[start..]
            .find("\n}\n")
            .unwrap_or_else(|| panic!("{name} is not closed"));
        &script[start..start + len + 3]
    }

    /// The text after `marker`, which must be present.
    fn after<'a>(text: &'a str, marker: &str) -> &'a str {
        let start = text
            .find(marker)
            .unwrap_or_else(|| panic!("missing `{marker}`"));
        &text[start + marker.len()..]
    }

    fn port_held_line(script: &str) -> &str {
        script
            .lines()
            .find(|line| line.contains("cause=port-held"))
            .unwrap_or_default()
    }

    #[test]
    fn install_script_is_posix_and_guards_gui_session() {
        let script = render_install_script(&DashboardSet::standard());
        assert!(script.starts_with("set -eu\n"), "{script}");
        assert!(script.contains("error=no-gui-session"));
        assert!(script.contains("exit 3"));
        assert!(!script.contains("[["));
        // The guard runs before anything is installed.
        let guard = script.find("error=no-gui-session").unwrap_or(usize::MAX);
        let first_call = script.find("if install_service ").unwrap_or(0);
        assert!(guard < first_call);
    }

    #[test]
    fn install_script_skips_unchanged_services_and_reports_actions() {
        let script = render_install_script(&DashboardSet::standard());
        assert!(script.contains("cmp -s"));
        assert!(script.contains("action=unchanged"));
        assert!(script.contains("action=installed"));
        assert!(script.contains("reason=cheap-otto-not-installed"));
    }

    #[test]
    fn install_script_skips_pixel_office_when_binary_missing() {
        let script = render_install_script(&DashboardSet::standard());
        assert!(script.contains("reason=pixel-agents-not-installed"));
        // `PIXEL_BIN_XML` is only set, and only used, once pixel-agents was found (`set -u`).
        let pixel_block = after(&script, "# --- pixel-office ---");
        assert!(pixel_block.contains("PIXEL_BIN_XML=$(xml_escape \"$PIXEL_BIN\")"));
        assert!(!script[..script.len() - pixel_block.len()].contains("$PIXEL_BIN_XML"));
    }

    #[test]
    fn install_script_frees_ports_by_listener_not_by_name() {
        let script = render_install_script(&DashboardSet::standard());
        assert!(script.contains("lsof -nP -iTCP:"));
        assert!(!script.contains("pkill"));
    }

    #[test]
    fn install_script_never_binds_non_loopback() {
        let script = render_install_script(&DashboardSet::standard());
        assert!(!script.contains("0.0.0.0"));
        assert!(script.contains("<string>127.0.0.1</string>"));
    }

    #[test]
    fn install_script_waits_for_port_release_before_bootstrap() {
        let script = render_install_script(&DashboardSet::standard());
        assert!(script.contains("kill -9"));
        assert!(script.contains("cause=port-held"));
        let wait_loop = script.find("while [ -n \"$(lsof -nP -iTCP:");
        let bootstrap = script.find("launchctl bootstrap");
        assert!(wait_loop.is_some() && bootstrap.is_some());
        assert!(wait_loop < bootstrap, "{script}");
    }

    #[test]
    fn install_script_reports_port_held_with_diagnostics_and_skips_wait()
    -> Result<(), DashboardError> {
        let set = DashboardSet::with_ports(8420, 8421, 8430)?;
        let script = render_install_script(&set);
        let line = port_held_line(&script);
        for field in [
            "listening=0",
            "cause=port-held",
            "last_exit=",
            "log=$LOG_DIR/",
        ] {
            assert!(line.contains(field), "{line} lacks {field}");
        }
        let branch = after(&script, line);
        let end = branch.find("fi\n").unwrap_or(branch.len());
        assert!(branch[..end].trim_end().ends_with("return 1"), "{branch}");

        for (id, label, port) in [
            (
                "mission-control",
                "com.cheapotto.dashboards.mission-control",
                8420,
            ),
            (
                "pixel-office",
                "com.cheapotto.dashboards.pixel-office",
                8421,
            ),
        ] {
            let call = format!("  if install_service {id} {label} {port} <<PLIST\n");
            let body = after(&script, &call);
            let then = format!("\nPLIST\n  then\n    wait_port {id} {label} {port}\n  fi\n");
            assert!(body.contains(&then), "{id}: wait_port is not the if body");
        }
        assert_eq!(script.matches("if install_service ").count(), 2);
        assert_eq!(script.matches("    wait_port ").count(), 2);
        Ok(())
    }

    #[test]
    fn install_script_checks_each_step_because_errexit_is_off_in_conditions() {
        let script = render_install_script(&DashboardSet::standard());
        let function = shell_function(&script, "install_service");
        for (step, cause) in [
            ("if ! cat > \"$tmp\"", "cause=plist-write-failed"),
            ("if ! mv \"$tmp\" \"$plist\"", "cause=plist-write-failed"),
            ("if ! launchctl bootstrap", "cause=bootstrap-failed"),
        ] {
            let branch = after(function, step);
            let end = branch.find("\n  fi\n").unwrap_or(branch.len());
            let branch = &branch[..end];
            for expected in [
                cause,
                "launchd_last_exit",
                "last_exit=${last_exit:--}",
                "FAILED=1",
                "return 1",
            ] {
                assert!(branch.contains(expected), "{step} lacks {expected}");
            }
        }
        let bootstrap_check = function
            .find("if ! launchctl bootstrap")
            .unwrap_or(usize::MAX);
        let installed = function.find("action=installed").unwrap_or(0);
        assert!(bootstrap_check < installed);
        assert_eq!(script.matches("action=installed").count(), 1);
    }

    #[test]
    fn install_script_reports_and_exits_four_when_the_directories_cannot_be_created() {
        let script = render_install_script(&DashboardSet::standard());
        let check = "if ! mkdir -p \"$AGENTS_DIR\" \"$LOG_DIR\"; then echo \"error=dashboard-directories-not-created path=$AGENTS_DIR log=$LOG_DIR\"; exit 4; fi\n";
        assert!(script.contains(check), "{script}");
        assert!(!script.contains("\nmkdir -p \"$AGENTS_DIR\""));
        let helper = script.find("launchd_last_exit() {").unwrap_or(usize::MAX);
        let install = script.find("install_service() {").unwrap_or(0);
        assert!(helper < install);
    }

    #[test]
    fn install_script_accepts_only_the_launchagent_listener_as_the_dashboard() {
        let script = render_install_script(&DashboardSet::standard());
        let function = shell_function(&script, "wait_port");
        for expected in [
            "launchctl print",
            "lsof -nP -iTCP:",
            "ps -o ppid=",
            "cause=foreign-listener",
            "cause=not-listening-after-10s",
            "listening=1",
            "FAILED=1",
        ] {
            assert!(function.contains(expected), "wait_port lacks {expected}");
        }
        for line in function
            .lines()
            .filter(|line| line.contains("cause=foreign-listener"))
        {
            assert!(line.contains("last_exit=") && line.contains("log=$LOG_DIR/"));
        }
        let probe = function.find("nc -z").unwrap_or(usize::MAX);
        let success = function.find("listening=1").unwrap_or(0);
        assert!(probe < success);
    }

    #[test]
    fn install_script_exits_four_when_a_service_never_listens() {
        let script = render_install_script(&DashboardSet::standard());
        assert!(shell_function(&script, "wait_port").contains("FAILED=1"));
        assert!(script.ends_with("if [ \"$FAILED\" -ne 0 ]; then exit 4; fi\n"));
    }

    #[test]
    fn install_script_reports_cause_log_and_last_exit_when_a_service_never_listens() {
        let script = render_install_script(&DashboardSet::standard());
        let line = script
            .lines()
            .find(|line| line.contains("cause=not-listening-after-10s"))
            .unwrap_or_default();
        for field in ["listening=0", "last_exit=", "log=$LOG_DIR/"] {
            assert!(line.contains(field), "{line} lacks {field}");
        }
        assert!(
            script.contains(
                "wait_port mission-control com.cheapotto.dashboards.mission-control 7420\n"
            )
        );
        assert!(
            script.contains("wait_port pixel-office com.cheapotto.dashboards.pixel-office 7421\n")
        );
    }

    #[test]
    fn install_script_escapes_xml_values_for_plists() {
        let script = render_install_script(&DashboardSet::standard());
        assert!(script.contains("sed -e 's/&/\\&amp;/g'"));
        assert!(script.contains("HOME_XML=$(xml_escape \"$HOME\")"));
        // The heredocs are unquoted so `$HOME_XML` expands, and the templates use only escaped values.
        assert!(!script.contains("<<'PLIST'"));
        let plists: Vec<&str> = script.split("<<PLIST\n").skip(1).collect();
        assert_eq!(plists.len(), 2);
        for plist in plists {
            let text = plist.split("\nPLIST\n").next().unwrap_or_default();
            assert!(text.contains("$HOME_XML"));
            assert!(!text.contains("$HOME/") && !text.contains("$PIXEL_BIN<"));
        }
    }

    #[test]
    fn install_plan_wraps_script_for_remote_sh() -> Result<(), crate::bridge::BridgeError> {
        let peer = tunnel_peer()?;
        let set = DashboardSet::standard();
        let plan = build_install_plan(&peer, &set);
        assert_eq!(plan.spec.program, "ssh");
        assert_eq!(
            plan.spec.args[..6],
            [
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                "-p",
                "2222"
            ]
        );
        assert_eq!(plan.spec.args[6], "admin@192.0.2.10");
        let batch = plan.spec.args.iter().position(|arg| arg == "BatchMode=yes");
        let destination = plan
            .spec
            .args
            .iter()
            .position(|arg| arg == "admin@192.0.2.10");
        assert!(batch < destination);
        let remote = plan
            .spec
            .args
            .last()
            .map(String::as_str)
            .unwrap_or_default();
        assert_eq!(remote, "/bin/sh -s");
        assert!(plan.spec.args.iter().all(|arg| !arg.contains('\n')));
        assert_eq!(plan.script, render_install_script(&set));
        assert_eq!(plan.spec.stdin.as_deref(), Some(plan.script.as_str()));
        assert!(!plan.script.contains('\r'));
        assert_eq!(
            plan.display_steps,
            [
                "Ensure LaunchAgent com.cheapotto.dashboards.mission-control -> 127.0.0.1:7420",
                "Ensure LaunchAgent com.cheapotto.dashboards.pixel-office -> 127.0.0.1:7421",
            ]
        );
        assert_eq!(plan.set, set);
        Ok(())
    }

    // --- Status (T1.6) ---

    /// Parses probe text in which `;` stands for a line break, so a case fits on one line.
    fn probe(text: &str) -> DashboardStatusReport {
        parse_status_output(&text.replace(';', "\n"), &DashboardSet::standard())
    }

    fn remote(
        id: DashboardId,
        up: [bool; 2],
        pid: Option<u32>,
        exit: Option<&str>,
    ) -> DashboardRemoteStatus {
        let [loaded, listening] = up;
        let last_exit = exit.map(str::to_string);
        DashboardRemoteStatus {
            id,
            loaded,
            pid,
            listening,
            last_exit,
        }
    }

    fn host(cheap_otto_bin: bool, pixel_agents_bin: bool, gui_session: bool) -> DashboardHostFacts {
        let autologin_user = Some("admin".to_string());
        DashboardHostFacts {
            cheap_otto_bin,
            pixel_agents_bin,
            gui_session,
            autologin_user,
        }
    }

    #[test]
    fn status_script_is_read_only() -> Result<(), crate::bridge::BridgeError> {
        let set = DashboardSet::standard();
        let script = render_status_script(&set);
        for forbidden in ["bootstrap", "bootout", "kickstart", "kill ", "rm ", "mv "] {
            assert!(!script.contains(forbidden), "{forbidden}");
        }
        assert!(script.contains("status_line reviewer com.cheapotto.code-reviewer.ui 7430\n"));
        assert!(script.contains("last_exit=${last_exit:--}") && script.contains("autoLoginUser"));
        let spec = build_status_spec(&tunnel_peer()?, &set);
        let head = format!("{} {}", spec.program, spec.args[..7].join(" "));
        assert_eq!(
            head,
            "ssh -o BatchMode=yes -o ConnectTimeout=3 -p 2222 admin@192.0.2.10"
        );
        assert_eq!(spec.args.len(), 8);
        assert_eq!(spec.args[7], "/bin/sh -s");
        assert!(spec.args.iter().all(|arg| !arg.contains('\n')));
        assert_eq!(spec.stdin.as_deref(), Some(script.as_str()));
        Ok(())
    }

    #[test]
    fn parses_status_lines_for_all_dashboards() {
        let raw = "dashboard=mission-control loaded=1 pid=812 listening=1 last_exit=-
dashboard=pixel-office loaded=0 pid=- listening=0 last_exit=-
dashboard=reviewer loaded=1 pid=- listening=0 last_exit=78:_EX_CONFIG
host cheap_otto_bin=1 pixel_agents_bin=0 gui_session=1 autologin_user=admin
";
        let report = parse_status_output(raw, &DashboardSet::standard());
        let exit = Some("78:_EX_CONFIG");
        let expected = [
            remote(DashboardId::MissionControl, [true, true], Some(812), None),
            remote(DashboardId::PixelOffice, [false, false], None, None),
            remote(DashboardId::Reviewer, [true, false], None, exit),
        ];
        assert_eq!(report.dashboards, expected);
        assert_eq!(report.host, Some(host(true, false, true)));
    }

    #[test]
    fn ignores_unknown_lines_keys_and_ids() {
        let raw = "Last login: Mon Nov  9 on ttys000
dashboard=grafana loaded=1 pid=1 listening=1
dashboard=Reviewer loaded=1
reviewer loaded=1
  dashboard=pixel-office   loaded=1 color=blue pid=77 listening=1 extra
host cheap_otto_bin=1 shoe_size=9 gui_session=1 autologin_user=-
";
        let report = parse_status_output(raw, &DashboardSet::standard());
        let pixel = remote(DashboardId::PixelOffice, [true, true], Some(77), None);
        assert_eq!(report.dashboards, [pixel]);
        let mut expected_host = host(true, false, true);
        expected_host.autologin_user = None;
        assert_eq!(report.host, Some(expected_host));
    }

    #[test]
    fn missing_host_line_yields_none() {
        let report = probe("dashboard=mission-control loaded=1 pid=812 listening=1 last_exit=-");
        assert_eq!((report.host, report.dashboards.len()), (None, 1));
    }

    #[test]
    fn handles_empty_and_garbage_output_without_panicking() {
        let set = DashboardSet::standard();
        for raw in "|\n\n|dashboard=|=|host| \t \r\n|dashboard=\u{1F600}".split('|') {
            let report = parse_status_output(raw, &set);
            assert_eq!((report.dashboards.len(), report.host), (0, None), "{raw:?}");
        }
        let report = probe("dashboard=reviewer pid=abc loaded=maybe");
        let nothing = remote(DashboardId::Reviewer, [false, false], None, None);
        assert_eq!(report.dashboards, [nothing]);
        // Overflowing and zero pids, control characters and a very long value stay harmless.
        let long = "x".repeat(500);
        let raw = format!(
            "dashboard=reviewer pid=99999999999 last_exit={long}\ndashboard=pixel-office pid=0 last_exit=1\u{1b}[2J\nhost autologin_user=\u{7}"
        );
        let report = parse_status_output(&raw, &set);
        let capped = Some(&long[..STATUS_VALUE_MAX_CHARS]);
        let expected = [
            remote(DashboardId::PixelOffice, [false, false], None, Some("1[2J")),
            remote(DashboardId::Reviewer, [false, false], None, capped),
        ];
        assert_eq!(report.dashboards, expected);
        assert_eq!(report.host.map(|facts| facts.autologin_user), Some(None));
    }

    #[test]
    fn explains_every_unavailable_dashboard_with_reason_and_log() {
        let ok = "host cheap_otto_bin=1 pixel_agents_bin=1 gui_session=1";
        let not_loaded = "The LaunchAgent is not loaded; run `cheap-otto dashboards install --execute`. A failed or port-held install leaves this too";
        let cases = [
            (
                "dashboard=mission-control loaded=0;host gui_session=0",
                "No GUI session for the Mac user; log in to the Mac desktop or enable automatic login",
            ),
            (
                "dashboard=mission-control loaded=0;host gui_session=1",
                "`cheap-otto` is not installed on the Mac; run `code-reviewer m1 sync`",
            ),
            (
                "dashboard=pixel-office loaded=0;host gui_session=1 cheap_otto_bin=1",
                "`pixel-agents` is not installed on the Mac, so the service was skipped",
            ),
            ("dashboard=pixel-office loaded=0;{ok}", not_loaded),
            (
                "dashboard=reviewer loaded=0;{ok}",
                "The LaunchAgent is not loaded; run `code-reviewer m1 sync`. A failed or port-held install leaves this too",
            ),
            (
                "dashboard=mission-control loaded=0 listening=1;{ok}",
                "The LaunchAgent is not loaded, but port 7420 answers, so another process holds it (a port-held install leaves this); find it with `lsof -nP -iTCP:7420 -sTCP:LISTEN` on the Mac, then run `cheap-otto dashboards install --execute`",
            ),
            (
                "dashboard=reviewer loaded=1 listening=0 last_exit=78:_EX_CONFIG;{ok}",
                "The LaunchAgent is loaded but port 7430 is not listening; launchd's last exit status is 78:_EX_CONFIG",
            ),
            (
                "dashboard=mission-control loaded=1 listening=0 last_exit=-",
                "The LaunchAgent is loaded but port 7420 is not listening; launchd reports no last exit status",
            ),
            // Without a host line, rows 1 to 3 are skipped and the LaunchAgent row explains it.
            ("dashboard=mission-control loaded=0", not_loaded),
            ("dashboard=pixel-office loaded=0", not_loaded),
        ];
        let set = DashboardSet::standard();
        for (text, reason) in cases {
            let report = probe(&text.replace("{ok}", ok));
            let status = &report.dashboards[0];
            let log = match status.id {
                DashboardId::Reviewer => {
                    "~/.cheap-otto/logs/code-reviewer/ui-server.log".to_string()
                }
                DashboardId::MissionControl | DashboardId::PixelOffice => {
                    format!("~/.cheap-otto/logs/dashboards/{}.log", status.id.slug())
                }
            };
            let service = set.get(status.id);
            let got = service
                .and_then(|service| explain_unavailable(service, status, report.host.as_ref()));
            assert_eq!(
                got.map(|d| (d.reason, d.log)),
                Some((reason.to_string(), log)),
                "{text}"
            );
        }
    }

    #[test]
    fn listening_dashboard_needs_no_explanation() {
        let healthy = host(true, true, true);
        for service in DashboardSet::standard().services() {
            let up = remote(service.id, [true, true], Some(812), None);
            assert_eq!(explain_unavailable(service, &up, Some(&healthy)), None);
            assert_eq!(explain_unavailable(service, &up, None), None);
        }
    }

    #[test]
    fn status_rows_explain_port_held_skipped_and_unloaded_dashboards() {
        let set = DashboardSet::standard();
        // What the probe prints after an install found 7420 held by a foreign process (the
        // LaunchAgent was booted out and never bootstrapped) and skipped the Pixel Office because
        // pixel-agents is missing, while the Reviewer runs normally.
        let raw = "dashboard=mission-control loaded=0 pid=- listening=1 last_exit=-
dashboard=pixel-office loaded=0 pid=- listening=0 last_exit=-
dashboard=reviewer loaded=1 pid=903 listening=1 last_exit=-
host cheap_otto_bin=1 pixel_agents_bin=0 gui_session=1 autologin_user=-
";
        let tunnelled = [
            (DashboardId::MissionControl, true),
            (DashboardId::PixelOffice, false),
        ];
        let rows = render_status_rows(&parse_status_output(raw, &set), &set, &tunnelled);
        let expected = [
            "Mission Control (7420): loaded=no pid=- listening=yes tunnelled=yes; reason: The LaunchAgent is not loaded, but port 7420 answers, so another process holds it (a port-held install leaves this); find it with `lsof -nP -iTCP:7420 -sTCP:LISTEN` on the Mac, then run `cheap-otto dashboards install --execute`; log: ~/.cheap-otto/logs/dashboards/mission-control.log",
            "Pixel Office (7421): loaded=no pid=- listening=no tunnelled=no; reason: `pixel-agents` is not installed on the Mac, so the service was skipped; log: ~/.cheap-otto/logs/dashboards/pixel-office.log",
            "Reviewer (7430): loaded=yes pid=903 listening=yes tunnelled=-",
        ];
        assert_eq!(rows, expected);
        // A dashboard the probe never mentioned is still explained.
        let rows = render_status_rows(&probe(""), &set, &[]);
        let missing = "Reviewer (7430): no status from the Mac, tunnelled=-; reason: the probe printed no line for it; log: ~/.cheap-otto/logs/code-reviewer/ui-server.log";
        assert_eq!(rows.get(2).map(String::as_str), Some(missing));
    }

    // --- Uninstall and restart (T1.5) ---

    #[test]
    fn uninstall_script_removes_only_managed_agents() -> Result<(), crate::bridge::BridgeError> {
        let set = DashboardSet::standard();
        let script = render_uninstall_script(&set);
        for id in ["mission-control", "pixel-office"] {
            let label = format!("com.cheapotto.dashboards.{id}");
            assert!(script.contains(&format!("launchctl bootout \"gui/$UID_NUM/{label}\"")));
            assert!(script.contains(&format!("rm -f \"$AGENTS_DIR/{label}.plist\"")));
            assert!(script.contains(&format!("dashboard={id} action=removed")));
        }
        assert!(!script.contains(REVIEWER_LAUNCHD_LABEL));
        let plan = build_uninstall_plan(&tunnel_peer()?, &set);
        assert_eq!(plan.spec.program, "ssh");
        assert_eq!(
            plan.spec.args.last().map(String::as_str),
            Some("/bin/sh -s")
        );
        assert!(plan.spec.args.iter().all(|arg| !arg.contains('\n')));
        assert_eq!(plan.spec.stdin.as_deref(), Some(script.as_str()));
        assert_eq!(plan.display_steps.len(), 2);
        Ok(())
    }

    #[test]
    fn remote_sh_stdin_spec_normalizes_crlf_input() -> Result<(), crate::bridge::BridgeError> {
        let spec = remote_sh_stdin_spec(
            "Run CRLF script",
            &tunnel_peer()?,
            "set -eu\r\necho ready\r\n",
        );
        assert_eq!(spec.args.last().map(String::as_str), Some("/bin/sh -s"));
        assert_eq!(spec.stdin.as_deref(), Some("set -eu\necho ready\n"));
        assert!(spec.args.iter().all(|arg| !arg.contains('\n')));
        Ok(())
    }

    #[test]
    fn restart_spec_kickstarts_requested_label() -> Result<(), crate::bridge::BridgeError> {
        let peer = tunnel_peer()?;
        let set = DashboardSet::standard();
        for service in set.services() {
            let spec = build_restart_spec(&peer, &set, service.id);
            let remote = spec.as_ref().and_then(|spec| spec.args.last());
            let wanted = format!("kickstart -k \"gui/$(id -u)/{}\"", service.launchd_label);
            assert!(
                remote.is_some_and(|remote| remote.contains(&wanted)),
                "{remote:?}"
            );
        }
        let reviewer = build_restart_spec(&peer, &set, DashboardId::Reviewer);
        assert!(reviewer.is_some_and(|spec| {
            spec.program == "ssh"
                && spec
                    .args
                    .iter()
                    .any(|arg| arg.contains("com.cheapotto.code-reviewer.ui"))
        }));
        Ok(())
    }
}
