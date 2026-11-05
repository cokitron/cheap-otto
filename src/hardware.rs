use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HardwareError {
    #[error("insufficient RAM: detected {detected_gb} GB, minimum required is {minimum_gb} GB")]
    InsufficientRam { detected_gb: u32, minimum_gb: u32 },
    #[error("failed to parse hardware probe output: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuArch {
    AppleSilicon,
    X86_64,
    Arm64Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ThermalLevel {
    Nominal,
    Fair,
    Serious,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub model_name: String,
    pub arch: CpuArch,
    pub memory_gb: u32,
    pub has_battery: bool,
    pub battery_health_percent: Option<u8>,
    pub fanless: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryBudget {
    pub total_mb: u32,
    pub os_reserved_mb: u32,
    pub container_engine_mb: u32,
    pub agent_and_browser_mb: u32,
    pub ci_runner_mb: u32,
    pub local_ai_max_mb: u32,
    pub safety_headroom_mb: u32,
    pub can_run_local_llm: bool,
    pub llm_idle_unload_secs: u32,
}

impl HardwareProfile {
    /// Computes a deterministic unified-memory budget tailored to the detected hardware.
    #[must_use = "memory budget should be inspected or applied to capability packs"]
    pub fn compute_memory_budget(&self) -> Result<MemoryBudget, HardwareError> {
        const MIN_RAM_GB: u32 = 4;
        if self.memory_gb < MIN_RAM_GB {
            return Err(HardwareError::InsufficientRam {
                detected_gb: self.memory_gb,
                minimum_gb: MIN_RAM_GB,
            });
        }

        let total_mb = self.memory_gb * 1024;
        let os_reserved_mb = 2560;
        let container_engine_mb = 1024;
        let agent_and_browser_mb = 1536;
        let ci_runner_mb = if self.memory_gb >= 16 { 2560 } else { 1536 };
        let safety_headroom_mb = if self.memory_gb >= 16 { 2560 } else { 1024 };

        let committed_mb = os_reserved_mb
            + container_engine_mb
            + agent_and_browser_mb
            + ci_runner_mb
            + safety_headroom_mb;

        let local_ai_max_mb = total_mb.saturating_sub(committed_mb);
        let can_run_local_llm =
            self.arch == CpuArch::AppleSilicon && self.memory_gb >= 16 && local_ai_max_mb >= 4096;

        let llm_idle_unload_secs = if self.fanless { 300 } else { 600 };

        Ok(MemoryBudget {
            total_mb,
            os_reserved_mb,
            container_engine_mb,
            agent_and_browser_mb,
            ci_runner_mb,
            local_ai_max_mb,
            safety_headroom_mb,
            can_run_local_llm,
            llm_idle_unload_secs,
        })
    }

    /// Parses macOS `sysctl` output lines (`hw.memsize`, `machdep.cpu.brand_string`, `hw.model`).
    pub fn from_macos_sysctl(sysctl_output: &str) -> Result<Self, HardwareError> {
        let mut mem_bytes: Option<u64> = None;
        let mut brand = String::new();
        let mut model = String::new();

        for line in sysctl_output.lines() {
            if let Some((key, raw_val)) = line.split_once(':') {
                let key = key.trim();
                let val = raw_val.trim();
                match key {
                    "hw.memsize" => {
                        mem_bytes = val.parse::<u64>().ok();
                    }
                    "machdep.cpu.brand_string" => {
                        brand = val.to_string();
                    }
                    "hw.model" => {
                        model = val.to_string();
                    }
                    _ => {}
                }
            }
        }

        let bytes = mem_bytes.ok_or_else(|| {
            HardwareError::ParseError("missing valid hw.memsize in sysctl output".to_string())
        })?;
        let memory_gb = (bytes / (1024 * 1024 * 1024)) as u32;
        let arch = if brand.contains("Apple M") {
            CpuArch::AppleSilicon
        } else {
            CpuArch::X86_64
        };
        let fanless = model.starts_with("MacBookAir") && arch == CpuArch::AppleSilicon;
        let has_battery = model.contains("MacBook");

        Ok(Self {
            model_name: if model.is_empty() { brand } else { model },
            arch,
            memory_gb,
            has_battery,
            battery_health_percent: None,
            fanless,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_m1_air_16gb_budget_with_local_llm_enabled() -> Result<(), HardwareError> {
        let profile = HardwareProfile {
            model_name: "MacBookAir10,1".to_string(),
            arch: CpuArch::AppleSilicon,
            memory_gb: 16,
            has_battery: true,
            battery_health_percent: Some(88),
            fanless: true,
        };

        let budget = profile.compute_memory_budget()?;
        assert_eq!(budget.total_mb, 16384);
        assert!(budget.can_run_local_llm);
        assert_eq!(budget.local_ai_max_mb, 6144);
        assert_eq!(budget.llm_idle_unload_secs, 300);
        Ok(())
    }

    #[test]
    fn disables_local_llm_on_8gb_x86_machine() -> Result<(), HardwareError> {
        let profile = HardwareProfile {
            model_name: "Intel N95 Mini PC".to_string(),
            arch: CpuArch::X86_64,
            memory_gb: 8,
            has_battery: false,
            battery_health_percent: None,
            fanless: false,
        };

        let budget = profile.compute_memory_budget()?;
        assert!(!budget.can_run_local_llm);
        assert_eq!(budget.total_mb, 8192);
        Ok(())
    }

    #[test]
    fn rejects_insufficient_ram() {
        let profile = HardwareProfile {
            model_name: "TinyBox".to_string(),
            arch: CpuArch::X86_64,
            memory_gb: 2,
            has_battery: false,
            battery_health_percent: None,
            fanless: true,
        };

        let err = profile.compute_memory_budget().unwrap_err();
        assert_eq!(
            err,
            HardwareError::InsufficientRam {
                detected_gb: 2,
                minimum_gb: 4
            }
        );
    }

    #[test]
    fn parses_macos_sysctl_output_for_m1_air() -> Result<(), HardwareError> {
        let sample = "\
hw.memsize: 17179869184
machdep.cpu.brand_string: Apple M1
hw.model: MacBookAir10,1
";
        let profile = HardwareProfile::from_macos_sysctl(sample)?;
        assert_eq!(profile.arch, CpuArch::AppleSilicon);
        assert_eq!(profile.memory_gb, 16);
        assert!(profile.fanless);
        assert!(profile.has_battery);
        assert_eq!(profile.model_name, "MacBookAir10,1");
        Ok(())
    }
}
