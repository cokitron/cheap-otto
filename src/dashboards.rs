//! Dashboard registry: the one place that names every dashboard served from the Mac.
//!
//! Mission Control, the Pixel Office and the Reviewer each run as a `launchd` service on the
//! paired MacBook and reach the workstation through one SSH tunnel. Install, uninstall, status,
//! the tunnel, `cheap-otto ui` and the hub API all read ids, ports and LaunchAgent labels from
//! [`DashboardSet`], so they can never disagree. Pure module: no I/O and no clock.

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
}
