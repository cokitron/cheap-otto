use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MacOsConfigError {
    #[error("invalid battery charge ceiling {0}% (must be between 50% and 100%)")]
    InvalidBatteryCeiling(u8),
    #[error("failed to parse pmset output: {0}")]
    PmsetParseError(String),
}

/// Snapshot of original macOS power management (`pmset`) and battery settings
/// saved before `cheap-otto init` modifies the system so `cheap-otto restore` is 100% reversible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacOsStateBackup {
    pub created_at_epoch_secs: u64,
    pub ac_sleep_minutes: u32,
    pub ac_disablesleep: bool,
    pub ac_displaysleep_minutes: u32,
    pub ac_autorestart: bool,
    pub ac_womp: bool,
    pub tcpkeepalive: bool,
    pub battery_charge_ceiling_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliancePowerPlan {
    pub target_battery_ceiling_percent: u8,
    pub pmset_apply_commands: Vec<String>,
    pub pmset_restore_commands: Vec<String>,
    pub human_readable_diff: Vec<String>,
}

impl MacOsStateBackup {
    /// Parses the AC Power section of `pmset -g custom` output into a reversible backup snapshot.
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
        })
    }

    /// Builds an idempotent, reversible execution plan for headless clamshell operation.
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

        let pmset_restore_commands = vec![
            format!(
                "sudo pmset -c sleep {} displaysleep {} autorestart {} womp {} tcpkeepalive {}",
                self.ac_sleep_minutes,
                self.ac_displaysleep_minutes,
                u8::from(self.ac_autorestart),
                u8::from(self.ac_womp),
                u8::from(self.tcpkeepalive),
            ),
            format!(
                "sudo pmset -a disablesleep {}",
                u8::from(self.ac_disablesleep)
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
            human_readable_diff,
        })
    }
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
        let backup = MacOsStateBackup::from_pmset_custom(SAMPLE_PMSET, 1_730_000_000)?;
        assert_eq!(backup.ac_sleep_minutes, 1);
        assert!(!backup.ac_disablesleep);
        assert_eq!(backup.ac_displaysleep_minutes, 10);

        let plan = backup.build_clamshell_plan(78)?;
        assert_eq!(plan.target_battery_ceiling_percent, 78);
        assert_eq!(plan.pmset_apply_commands.len(), 3);
        assert_eq!(
            plan.pmset_restore_commands[0],
            "sudo pmset -c sleep 1 displaysleep 10 autorestart 0 womp 0 tcpkeepalive 1"
        );
        assert_eq!(
            plan.pmset_restore_commands[1],
            "sudo pmset -a disablesleep 0"
        );
        Ok(())
    }

    #[test]
    fn rejects_unsafe_battery_ceiling() -> Result<(), MacOsConfigError> {
        let backup = MacOsStateBackup::from_pmset_custom(SAMPLE_PMSET, 1_730_000_000)?;
        let err = backup.build_clamshell_plan(30).unwrap_err();
        assert_eq!(err, MacOsConfigError::InvalidBatteryCeiling(30));
        Ok(())
    }
}
