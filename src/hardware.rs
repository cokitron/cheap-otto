//! Hardware capability profiling, `sysctl` / `vm_stat` telemetry parsing, and unified memory budgeting.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned when probing hardware capabilities or computing unified memory budgets.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HardwareError {
    /// Host has less than the minimum 4 GB RAM required for headless operation.
    #[error("insufficient RAM: detected {detected_gb} GB, minimum required is {minimum_gb} GB")]
    InsufficientRam { detected_gb: u32, minimum_gb: u32 },
    /// External `sysctl` or `vm_stat` output lacked required fields or was malformed.
    #[error("failed to parse hardware probe output: {0}")]
    ParseError(String),
}

/// CPU instruction set architecture of the target appliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuArch {
    /// Apple Silicon (`M1` / `M2` / `M3` / `M4`) with unified memory.
    AppleSilicon,
    /// Intel / AMD `x86_64`.
    X86_64,
    /// Non-Apple 64-bit ARM (`aarch64`).
    Arm64Generic,
}

/// Passive or active thermal pressure tier ordered by ascending severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ThermalLevel {
    /// Normal operating temperature; all workloads allowed.
    Nominal,
    /// Slightly elevated temperature; fans or passive dissipation keeping up.
    Fair,
    /// Thermal throttling active; heavy batch queues and local LLMs must yield.
    Serious,
    /// Emergency thermal ceiling; immediate workload shedding required.
    Critical,
}

impl ThermalLevel {
    /// Parses a thermal level string or numeric macOS thermal pressure code (`0..=3`).
    #[must_use = "parsed thermal level should be passed to health checks or the arbiter"]
    pub fn parse(raw: &str) -> Option<Self> {
        let cleaned = raw
            .lines()
            .find_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    None
                } else if let Some((_, rhs)) = trimmed.split_once(':') {
                    Some(rhs.trim())
                } else if let Some((_, rhs)) = trimmed.split_once('=') {
                    Some(rhs.trim())
                } else {
                    Some(trimmed)
                }
            })?
            .to_ascii_lowercase();

        match cleaned.as_str() {
            "0" | "nominal" | "normal" => Some(Self::Nominal),
            "1" | "fair" | "moderate" => Some(Self::Fair),
            "2" | "serious" | "heavy" => Some(Self::Serious),
            "3" | "critical" | "trapping" | "sleeping" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// Detected hardware characteristics of the spare laptop or mini PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// Hardware model identifier (e.g., `"MacBookAir10,1"`).
    pub model_name: String,
    /// Detected CPU architecture.
    pub arch: CpuArch,
    /// Total physical / unified memory in GiB.
    pub memory_gb: u32,
    /// Whether an internal lithium-ion battery is present.
    pub has_battery: bool,
    /// Optional battery maximum capacity health percentage (`0..=100`).
    pub battery_health_percent: Option<u8>,
    /// Whether the chassis relies strictly on passive fanless cooling (e.g., Apple Silicon MacBook Air).
    pub fanless: bool,
}

/// Deterministic unified-memory allocation plan across OS, containers, agents, CI, and local AI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryBudget {
    /// Total system memory in MiB.
    pub total_mb: u32,
    /// Memory reserved for macOS / kernel responsiveness in MiB.
    pub os_reserved_mb: u32,
    /// Memory reserved for lightweight container engine (`OrbStack`) in MiB.
    pub container_engine_mb: u32,
    /// Memory reserved for coding agents and headless browsers in MiB.
    pub agent_and_browser_mb: u32,
    /// Memory reserved for `cargo test` / CI worktree builds in MiB.
    pub ci_runner_mb: u32,
    /// Remaining ceiling available for optional local LLM weights in MiB.
    pub local_ai_max_mb: u32,
    /// Unallocated safety floor to prevent swap thrashing in MiB.
    pub safety_headroom_mb: u32,
    /// Whether the hardware safely supports keeping local LLM weights in unified memory.
    pub can_run_local_llm: bool,
    /// Idle timeout in seconds before cached LLM weights are evicted.
    pub llm_idle_unload_secs: u32,
}

impl HardwareProfile {
    /// Computes a deterministic unified-memory budget tailored to the detected hardware.
    ///
    /// # Errors
    /// Returns [`HardwareError::InsufficientRam`] if `self.memory_gb < 4`.
    #[must_use = "memory budget should be inspected or applied to capability packs"]
    pub fn compute_memory_budget(&self) -> Result<MemoryBudget, HardwareError> {
        const MIN_RAM_GB: u32 = 4;
        if self.memory_gb < MIN_RAM_GB {
            return Err(HardwareError::InsufficientRam {
                detected_gb: self.memory_gb,
                minimum_gb: MIN_RAM_GB,
            });
        }

        let total_mb = self.memory_gb.saturating_mul(1024);
        let os_reserved_mb: u32 = 2560;
        let container_engine_mb: u32 = 1024;
        let agent_and_browser_mb: u32 = 1536;
        let ci_runner_mb: u32 = if self.memory_gb >= 16 { 2560 } else { 1536 };
        let safety_headroom_mb: u32 = if self.memory_gb >= 16 { 2560 } else { 1024 };

        let committed_mb = os_reserved_mb
            .saturating_add(container_engine_mb)
            .saturating_add(agent_and_browser_mb)
            .saturating_add(ci_runner_mb)
            .saturating_add(safety_headroom_mb);

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
    ///
    /// # Errors
    /// Returns [`HardwareError::ParseError`] if `hw.memsize` is missing or unparseable.
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
        let memory_gb = u32::try_from(bytes / (1024 * 1024 * 1024)).unwrap_or(u32::MAX);
        let arch = if brand.contains("Apple M") {
            CpuArch::AppleSilicon
        } else if brand.to_ascii_lowercase().contains("arm")
            || brand.to_ascii_lowercase().contains("aarch64")
        {
            CpuArch::Arm64Generic
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

/// Parses macOS `vm_stat` output to compute reclaimable + free unified memory in MiB.
///
/// Sums `Pages free`, `Pages inactive`, and `Pages speculative` multiplied by the reported
/// VM page size (`16384` bytes on Apple Silicon, `4096` bytes fallback).
///
/// # Errors
/// Returns [`HardwareError::ParseError`] if `vm_stat` contains no recognizable page counters.
pub fn parse_macos_vm_stat_available_mb(vm_stat_output: &str) -> Result<u32, HardwareError> {
    let mut page_size_bytes: u64 = 16_384;
    let mut free_pages: u64 = 0;
    let mut inactive_pages: u64 = 0;
    let mut speculative_pages: u64 = 0;
    let mut found_counter = false;

    for line in vm_stat_output.lines() {
        let trimmed = line.trim();
        if let Some(after_prefix) = trimmed.split("page size of ").nth(1)
            && let Some(num_token) = after_prefix.split_whitespace().next()
            && let Ok(parsed_size) = num_token.parse::<u64>()
            && parsed_size > 0
        {
            page_size_bytes = parsed_size;
            continue;
        }

        if let Some((key, raw_val)) = trimmed.split_once(':') {
            let clean_val = raw_val.trim().trim_end_matches('.');
            match key.trim() {
                "Pages free" => {
                    if let Ok(v) = clean_val.parse::<u64>() {
                        free_pages = v;
                        found_counter = true;
                    }
                }
                "Pages inactive" => {
                    if let Ok(v) = clean_val.parse::<u64>() {
                        inactive_pages = v;
                        found_counter = true;
                    }
                }
                "Pages speculative" => {
                    if let Ok(v) = clean_val.parse::<u64>() {
                        speculative_pages = v;
                        found_counter = true;
                    }
                }
                _ => {}
            }
        }
    }

    if !found_counter {
        return Err(HardwareError::ParseError(
            "missing page counters in vm_stat output".to_string(),
        ));
    }

    let available_pages = free_pages
        .saturating_add(inactive_pages)
        .saturating_add(speculative_pages);
    let available_bytes = available_pages.saturating_mul(page_size_bytes);
    let available_mb = u32::try_from(available_bytes / (1024 * 1024)).unwrap_or(u32::MAX);
    Ok(available_mb)
}

/// Parses battery maximum capacity health percentage from `ioreg` or `system_profiler` output.
#[must_use = "parsed battery health percentage enriches HardwareProfile"]
pub fn parse_battery_health_percent(raw_output: &str) -> Option<u8> {
    for line in raw_output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("MaximumCapacityPercent")
            && let Some((_, rhs)) = trimmed.split_once('=')
        {
            let digits = rhs.trim().trim_matches(|c: char| !c.is_ascii_digit());
            if let Ok(pct) = digits.parse::<u16>() {
                return Some(u8::try_from(pct.min(100)).unwrap_or(100));
            }
        }
        if trimmed.starts_with("Maximum Capacity:")
            && let Some((_, rhs)) = trimmed.split_once(':')
        {
            let digits = rhs.trim().trim_end_matches('%').trim();
            if let Ok(pct) = digits.parse::<u16>() {
                return Some(u8::try_from(pct.min(100)).unwrap_or(100));
            }
        }
    }
    None
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

    #[test]
    fn parses_vm_stat_available_memory_and_thermal_levels() -> Result<(), HardwareError> {
        let sample_vm_stat = "\
Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                              128000.
Pages active:                            310000.
Pages inactive:                          256000.
Pages speculative:                        16000.
";
        // (128_000 + 256_000 + 16_000) * 16_384 = 400_000 * 16_384 = 6_553_600_000 bytes = 6250 MB
        let avail_mb = parse_macos_vm_stat_available_mb(sample_vm_stat)?;
        assert_eq!(avail_mb, 6250);

        assert_eq!(ThermalLevel::parse("0"), Some(ThermalLevel::Nominal));
        assert_eq!(
            ThermalLevel::parse("kern.thermalstate: 2"),
            Some(ThermalLevel::Serious)
        );
        assert_eq!(
            ThermalLevel::parse("Critical"),
            Some(ThermalLevel::Critical)
        );
        assert_eq!(ThermalLevel::parse("unknown"), None);
        assert_eq!(
            parse_battery_health_percent("\"MaximumCapacityPercent\" = 89\n"),
            Some(89)
        );
        Ok(())
    }
}
