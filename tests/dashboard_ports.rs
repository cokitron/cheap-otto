//! The `code-reviewer` crate is a binary the root crate cannot depend on, so it repeats the
//! dashboard ports as constants. This test reads its source and fails if they drift from the
//! registry in `src/dashboards.rs`. [R2.5]
#![forbid(unsafe_code)]

use cheap_otto::dashboards::{MISSION_CONTROL_PORT, PIXEL_OFFICE_PORT, REVIEWER_PORT};

const REVIEWER_SERVER_SOURCE: &str = include_str!("../crates/code-reviewer/src/server.rs");

#[test]
fn reviewer_ports_match_the_registry() {
    for declaration in [
        format!("pub const DEFAULT_UI_PORT: u16 = {REVIEWER_PORT};"),
        format!("pub const DEFAULT_PIXEL_PORT: u16 = {PIXEL_OFFICE_PORT};"),
        format!("pub const DEFAULT_MISSION_CONTROL_PORT: u16 = {MISSION_CONTROL_PORT};"),
    ] {
        assert!(
            REVIEWER_SERVER_SOURCE.contains(&declaration),
            "crates/code-reviewer/src/server.rs must declare `{declaration}`"
        );
    }
}
