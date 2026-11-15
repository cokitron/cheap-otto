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
    /// A deserialized set does not list each dashboard exactly once, in registry order.
    #[error(
        "a dashboard set must list mission-control, pixel-office and reviewer once each, in that order"
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
    let ports = set.ports();
    let mut args = Vec::with_capacity(4 + TUNNEL_SSH_OPTIONS.len() * 2 + ports.len() * 2);
    args.push("-N".to_string());
    for option in TUNNEL_SSH_OPTIONS {
        args.push("-o".to_string());
        args.push(option.to_string());
    }
    args.push("-p".to_string());
    args.push(peer.ssh_port.to_string());
    for port in ports {
        args.push("-L".to_string());
        args.push(loopback_forward(port));
    }
    args.push(peer.ssh_destination());
    CommandSpec::new(
        "Forward every dashboard port from the Mac to 127.0.0.1 on this workstation",
        "ssh",
        args,
    )
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

/// True only when `command_line` (as `ps -o args=` or Win32 `CommandLine` reports it) is an `ssh`
/// tunnel (`-N`) whose arguments include the record's destination and
/// `127.0.0.1:<port>:127.0.0.1:<port>` for every recorded port, each as a whole argument.
///
/// Guards against a recycled pid: after a reboot the recorded pid can belong to someone's
/// interactive ssh session, which must never be signalled. A record with no ports never matches.
/// [R3.5]
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
    is_ssh
        && !record.ports.is_empty()
        && args.contains(&"-N")
        && args.contains(&record.destination.as_str())
        && record
            .ports
            .iter()
            .all(|&port| args.contains(&loopback_forward(port).as_str()))
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
        };
        let json = serde_json::to_string_pretty(&record)?;
        assert!(json.contains("\"ssh_port\": 2222"), "{json}");
        assert!(
            json.contains("\"started_epoch_secs\": 1700000000"),
            "{json}"
        );
        let back: TunnelRecord = serde_json::from_str(&json)?;
        assert_eq!(back, record);
        Ok(())
    }
}
