---
paths:
  - "src/**/*.rs"
  - "tests/**/*.rs"
---

# Architecture & Testing Standards

## 1. Functional Core, Imperative Shell

`cheap-otto` strictly separates pure domain decision-making from OS and filesystem side effects:

1. **Functional Core (`src/hardware.rs`, `src/macos.rs`, `src/arbiter.rs`, `src/doctor.rs`, `src/packs.rs`):**
   - Pure, deterministic functions and data structures.
   - Takes structured inputs or raw string snapshots (`&str`, `&HardwareProfile`, `&ArbiterSnapshot`, `&SystemProbeInput`) and returns typed outputs or domain errors (`Result<T, E>`).
   - **Never performs direct filesystem I/O, network requests, or `sudo` command execution inside these modules.**
   - Must compile and pass 100% of unit tests on **Windows, Linux, and macOS** without mocking OS syscalls or requiring Apple hardware.
2. **Imperative Shell (`src/main.rs` and `scripts/bootstrap-macos.sh`):**
   - Parses CLI arguments via `clap` derive macros.
   - Reads/writes files (`state-backup.json`, `docker-compose.yml`) and formats human-readable console output.

---

## 2. Unit & Integration Testing Requirements

Every domain module must include a `#[cfg(test)] mod tests` block (or corresponding integration test in `tests/`) adhering to these rules:

- **Test Naming Convention:**
  - Name tests using descriptive snake_case verbs that state the behavior and condition:
    - `computes_m1_air_16gb_budget_with_local_llm_enabled`
    - `disables_local_llm_on_8gb_x86_machine`
    - `rejects_insufficient_ram`
    - `parses_pmset_ac_section_and_generates_reversible_commands`
    - `evicts_local_llm_when_ci_build_starts`
    - `pauses_queues_and_evicts_llm_on_serious_thermal_pressure`
- **Test Coverage Triad for Every Feature:**
  1. **Happy Path:** Standard target hardware (e.g., 16GB M1 MacBook Air on AC power with nominal thermals).
  2. **Boundary / Degraded Conditions:** 8GB x86 hardware, fanless `Serious` thermal throttling, idle timeout thresholds (`300s`), CI build preemption, low available memory (`< 2048 MB`).
  3. **Invalid / Error Inputs:** Insufficient RAM (`< 4 GB`), unsafe battery ceiling (`< 50%` or `> 100%`), malformed/missing `pmset` or `sysctl` sections.
- **Deterministic Assertions:**
  - Assert full struct/enum equality (`assert_eq!`) rather than partial string checks whenever possible.
  - Never depend on wall-clock time (`SystemTime::now()`), random numbers, or network connectivity inside unit tests; pass timestamps (`created_at_epoch_secs: u64`) explicitly as parameters.
