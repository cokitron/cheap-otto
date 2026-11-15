//! The `code-reviewer` crate is a binary the root crate cannot depend on, so it repeats the
//! Pixel Office LaunchAgent label as a constant. This test reads its source and fails if the
//! label drifts from the registry in `src/dashboards.rs`.
#![forbid(unsafe_code)]

use cheap_otto::dashboards::{DashboardId, DashboardSet};

const REVIEWER_SERVER_SOURCE: &str = include_str!("../crates/code-reviewer/src/server.rs");

#[test]
fn reviewer_pixel_office_label_matches_the_registry() {
    let set = DashboardSet::standard();
    let label = set
        .get(DashboardId::PixelOffice)
        .map(|service| service.launchd_label.as_str());
    assert_eq!(label, Some("com.cheapotto.dashboards.pixel-office"));
    let declaration = format!(
        "pub const PIXEL_OFFICE_LAUNCHD_LABEL: &str = \"{}\";",
        label.unwrap_or_default()
    );
    assert!(
        REVIEWER_SERVER_SOURCE
            .lines()
            .any(|line| line.trim() == declaration),
        "crates/code-reviewer/src/server.rs must declare `{declaration}`"
    );
}
