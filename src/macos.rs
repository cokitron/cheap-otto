//! Reversible macOS power management (`pmset`) parsing, snapshot backup, and clamshell planning.

use crate::command::CommandSpec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when parsing `pmset` output or validating battery guardrails.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MacOsConfigError {
    /// Requested battery charge ceiling falls outside the safe `50%..=100%` range.
    #[error("invalid battery charge ceiling {0}% (must be between 50% and 100%)")]
    InvalidBatteryCeiling(u8),
    /// Output from `pmset -g custom` lacked an `AC Power:` section.
    #[error("failed to parse pmset output: {0}")]
    PmsetParseError(String),
}

/// Snapshot of original macOS power management (`pmset`) and battery settings
/// saved before `cheap-otto init` modifies the system so `cheap-otto restore` is 100% reversible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacOsStateBackup {
    /// Unix epoch timestamp when the original factory/user snapshot was captured.
    pub created_at_epoch_secs: u64,
    /// Original `sleep` minutes under `AC Power`.
    pub ac_sleep_minutes: u32,
    /// Original `disablesleep` flag (`true` if `1`).
    pub ac_disablesleep: bool,
    /// Original `displaysleep` minutes under `AC Power`.
    pub ac_displaysleep_minutes: u32,
    /// Original `autorestart` flag under `AC Power`.
    pub ac_autorestart: bool,
    /// Original wake-on-magic-packet (`womp`) flag under `AC Power`.
    pub ac_womp: bool,
    /// Original `tcpkeepalive` flag under `AC Power`.
    pub tcpkeepalive: bool,
    /// Original factory battery charge ceiling before appliance initialization (`100`).
    pub battery_charge_ceiling_percent: u8,
    /// Configured appliance battery ceiling (`50..=100%`, e.g., `Some(78)`) persisted by `init`.
    #[serde(default)]
    pub configured_ceiling_percent: Option<u8>,
}

/// Deterministic, reversible execution plan for headless clamshell operation and restoration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliancePowerPlan {
    /// Target battery charge ceiling percentage (`50..=100`).
    pub target_battery_ceiling_percent: u8,
    /// Human-readable `sudo pmset` commands to activate clamshell mode.
    pub pmset_apply_commands: Vec<String>,
    /// Human-readable `sudo pmset` commands to restore original laptop power behavior.
    pub pmset_restore_commands: Vec<String>,
    /// Shell-free process specifications for `--apply` execution in `init`.
    #[serde(default)]
    pub apply_specs: Vec<CommandSpec>,
    /// Shell-free process specifications for `--apply` execution in `restore`.
    #[serde(default)]
    pub restore_specs: Vec<CommandSpec>,
    /// Human-readable before/after summary lines.
    pub human_readable_diff: Vec<String>,
}

impl MacOsStateBackup {
    /// Parses the `AC Power:` section of `pmset -g custom` output into a reversible backup snapshot.
    ///
    /// # Errors
    /// Returns [`MacOsConfigError::PmsetParseError`] if `AC Power:` is missing from `pmset_output`.
    pub fn from_pmset_custom(
        pmset_output: &str,
        created_at_epoch_secs: u64,
    ) -> Result<Self, MacOsConfigError> {
        let mut in_ac_section = false;
        let mut ac_sleep_minutes = 1;
        let mut ac_disablesleep = false;
        let mut ac_displaysleep_minutes = 10;
        let mut ac_autorestart = false;
        let mut ac_womp = false;
        let mut tcpkeepalive = true;
        let mut found_any = false;

        for raw_line in pmset_output.lines() {
            let line = raw_line.trim();
            if line.starts_with("AC Power:") {
                in_ac_section = true;
                found_any = true;
                continue;
            }
            if line.ends_with("Power:") && !line.starts_with("AC Power:") {
                in_ac_section = false;
                continue;
            }
            if !in_ac_section || line.is_empty() {
                continue;
            }

            let mut parts = line.split_whitespace();
            if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                match key {
                    "SleepOn" | "sleep" => {
                        ac_sleep_minutes = val.parse::<u32>().unwrap_or(1);
                    }
                    "disablesleep" => {
                        ac_disablesleep = val == "1";
                    }
                    "displaysleep" => {
                        ac_displaysleep_minutes = val.parse::<u32>().unwrap_or(10);
                    }
                    "autorestart" => {
                        ac_autorestart = val == "1";
                    }
                    "womp" => {
                        ac_womp = val == "1";
                    }
                    "tcpkeepalive" => {
                        tcpkeepalive = val == "1";
                    }
                    _ => {}
                }
            }
        }

        if !found_any {
            return Err(MacOsConfigError::PmsetParseError(
                "missing 'AC Power:' section in pmset output".to_string(),
            ));
        }

        Ok(Self {
            created_at_epoch_secs,
            ac_sleep_minutes,
            ac_disablesleep,
            ac_displaysleep_minutes,
            ac_autorestart,
            ac_womp,
            tcpkeepalive,
            battery_charge_ceiling_percent: 100,
            configured_ceiling_percent: None,
        })
    }

    /// Records the target appliance battery charge ceiling (`50..=100%`) while preserving
    /// the original `battery_charge_ceiling_percent` for `cheap-otto restore`.
    ///
    /// # Errors
    /// Returns [`MacOsConfigError::InvalidBatteryCeiling`] if `ceiling` is outside `50..=100`.
    pub fn with_configured_ceiling(mut self, ceiling: u8) -> Result<Self, MacOsConfigError> {
        if !(50..=100).contains(&ceiling) {
            return Err(MacOsConfigError::InvalidBatteryCeiling(ceiling));
        }
        self.configured_ceiling_percent = Some(ceiling);
        Ok(self)
    }

    /// Builds an idempotent, reversible execution plan for headless clamshell operation.
    ///
    /// # Errors
    /// Returns [`MacOsConfigError::InvalidBatteryCeiling`] if `target_battery_ceiling_percent`
    /// is outside `50..=100`.
    #[must_use = "the generated power plan must be displayed or executed"]
    pub fn build_clamshell_plan(
        &self,
        target_battery_ceiling_percent: u8,
    ) -> Result<AppliancePowerPlan, MacOsConfigError> {
        if !(50..=100).contains(&target_battery_ceiling_percent) {
            return Err(MacOsConfigError::InvalidBatteryCeiling(
                target_battery_ceiling_percent,
            ));
        }

        let pmset_apply_commands = vec![
            // Never sleep on AC power, turn off panel after 1 min (ideal for flaky/cracked screens)
            "sudo pmset -c sleep 0 displaysleep 1 autorestart 1 womp 1 tcpkeepalive 1".to_string(),
            // Prevent lid-close sleep while running in headless appliance mode
            "sudo pmset -a disablesleep 1".to_string(),
            // Keep normal battery sleep intact (5 min) in case AC power is disconnected for long periods
            "sudo pmset -b sleep 5 displaysleep 1".to_string(),
        ];

        let apply_specs = vec![
            CommandSpec::new(
                "Disable AC idle sleep and dim panel after 1 min",
                "sudo",
                [
                    "pmset",
                    "-c",
                    "sleep",
                    "0",
                    "displaysleep",
                    "1",
                    "autorestart",
                    "1",
                    "womp",
                    "1",
                    "tcpkeepalive",
                    "1",
                ],
            ),
            CommandSpec::new(
                "Disable lid-close sleep for headless clamshell operation",
                "sudo",
                ["pmset", "-a", "disablesleep", "1"],
            ),
            CommandSpec::new(
                "Retain 5-minute battery sleep fallback if unplugged",
                "sudo",
                ["pmset", "-b", "sleep", "5", "displaysleep", "1"],
            ),
        ];

        let ac_sleep = self.ac_sleep_minutes.to_string();
        let ac_displaysleep = self.ac_displaysleep_minutes.to_string();
        let ac_autorestart = u8::from(self.ac_autorestart).to_string();
        let ac_womp = u8::from(self.ac_womp).to_string();
        let tcpkeepalive = u8::from(self.tcpkeepalive).to_string();
        let ac_disablesleep = u8::from(self.ac_disablesleep).to_string();

        let pmset_restore_commands = vec![
            format!(
                "sudo pmset -c sleep {ac_sleep} displaysleep {ac_displaysleep} autorestart {ac_autorestart} womp {ac_womp} tcpkeepalive {tcpkeepalive}"
            ),
            format!("sudo pmset -a disablesleep {ac_disablesleep}"),
        ];

        let restore_specs = vec![
            CommandSpec::new(
                "Restore original AC power sleep and wake settings",
                "sudo",
                [
                    "pmset",
                    "-c",
                    "sleep",
                    &ac_sleep,
                    "displaysleep",
                    &ac_displaysleep,
                    "autorestart",
                    &ac_autorestart,
                    "womp",
                    &ac_womp,
                    "tcpkeepalive",
                    &tcpkeepalive,
                ],
            ),
            CommandSpec::new(
                "Restore original clamshell lid-sleep setting",
                "sudo",
                ["pmset", "-a", "disablesleep", &ac_disablesleep],
            ),
        ];

        let human_readable_diff = vec![
            format!(
                "AC sleep: {} min -> 0 (never sleep on AC)",
                self.ac_sleep_minutes
            ),
            format!(
                "Lid-close sleep disabled: {} -> true (headless clamshell active)",
                self.ac_disablesleep
            ),
            format!(
                "Display panel sleep (AC): {} min -> 1 min (protects broken/flaky screen)",
                self.ac_displaysleep_minutes
            ),
            format!(
                "Auto-restart on power restore: {} -> true",
                self.ac_autorestart
            ),
            format!(
                "Battery charge ceiling: {}% -> {}% (prevents 24/7 battery swelling)",
                self.battery_charge_ceiling_percent, target_battery_ceiling_percent
            ),
        ];

        Ok(AppliancePowerPlan {
            target_battery_ceiling_percent,
            pmset_apply_commands,
            pmset_restore_commands,
            apply_specs,
            restore_specs,
            human_readable_diff,
        })
    }
}

/// Parses `pmset -g batt` output to determine whether the MacBook is drawing from AC wall power.
#[must_use = "parsed AC power state feeds SystemProbeInput"]
pub fn parse_pmset_batt_ac_power(pmset_batt_output: &str) -> Option<bool> {
    for line in pmset_batt_output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("'AC Power'") || trimmed.contains("\"AC Power\"") {
            return Some(true);
        }
        if trimmed.contains("'Battery Power'") || trimmed.contains("\"Battery Power\"") {
            return Some(false);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PMSET: &str = "\
Battery Power:
 sleep                5
 displaysleep         2
AC Power:
 sleep                1
 disablesleep         0
 displaysleep         10
 autorestart          0
 womp                 0
 tcpkeepalive         1
";

    #[test]
    fn parses_pmset_ac_section_and_generates_reversible_commands() -> Result<(), MacOsConfigError> {
        let backup = MacOsStateBackup::from_pmset_custom(SAMPLE_PMSET, 1_730_000_000)?
            .with_configured_ceiling(78)?;
        assert_eq!(backup.ac_sleep_minutes, 1);
        assert!(!backup.ac_disablesleep);
        assert_eq!(backup.ac_displaysleep_minutes, 10);
        assert_eq!(backup.configured_ceiling_percent, Some(78));
        assert_eq!(backup.battery_charge_ceiling_percent, 100);

        let plan = backup.build_clamshell_plan(78)?;
        assert_eq!(plan.target_battery_ceiling_percent, 78);
        assert_eq!(plan.pmset_apply_commands.len(), 3);
        assert_eq!(plan.apply_specs.len(), 3);
        assert_eq!(plan.restore_specs.len(), 2);
        assert_eq!(
            plan.pmset_restore_commands[0],
            "sudo pmset -c sleep 1 displaysleep 10 autorestart 0 womp 0 tcpkeepalive 1"
        );
        assert_eq!(
            plan.pmset_restore_commands[1],
            "sudo pmset -a disablesleep 0"
        );
        assert_eq!(
            plan.restore_specs[0].display_command(),
            plan.pmset_restore_commands[0]
        );
        Ok(())
    }

    #[test]
    fn parses_pmset_batt_power_source() {
        let ac = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=123)\t78%; charged; 0:00 remaining\n";
        let batt =
            "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=123)\t76%; discharging\n";
        assert_eq!(parse_pmset_batt_ac_power(ac), Some(true));
        assert_eq!(parse_pmset_batt_ac_power(batt), Some(false));
        assert_eq!(parse_pmset_batt_ac_power("empty"), None);
    }

    #[test]
    fn rejects_unsafe_battery_ceiling() -> Result<(), MacOsConfigError> {
        let backup = MacOsStateBackup::from_pmset_custom(SAMPLE_PMSET, 1_730_000_000)?;
        let err = backup.build_clamshell_plan(30).unwrap_err();
        assert_eq!(err, MacOsConfigError::InvalidBatteryCeiling(30));
        Ok(())
    }
}
