---
paths:
  - "**/*.rs"
  - "scripts/**"
  - "Cargo.toml"
---

# Security & Appliance Safety Rules

Because `cheap-otto` configures power management (`sudo pmset`), manages lithium-ion battery charge ceilings on 24/7 AC power, arbitrates passive thermal envelopes on fanless laptops, and generates container stacks for AI agents, **security and hardware safety are first-class engineering requirements**.

---

## 1. Memory & Type Safety (`#![forbid(unsafe_code)]`)

- **No `unsafe` Rust:**
  - The crate must remain 100% safe Rust. Ensure `#![forbid(unsafe_code)]` is enforced at the crate root (`src/lib.rs` and `src/main.rs`).
  - Do not introduce raw pointers, `std::mem::transmute`, or unchecked indexing (`slice.get_unchecked`).
- **Checked & Saturating Arithmetic:**
  - Hardware resource math (RAM budgets, disk percentages, idle timers, battery caps) must never panic on integer underflow or overflow in debug or release builds.
  - Always use `saturating_sub`, `saturating_add`, `checked_mul`, or `.clamp(min, max)` when computing memory budgets (`total_mb.saturating_sub(committed_mb)`) or clamping percentages (`battery_charge_ceiling_percent.clamp(50, 100)`).

---

## 2. Defensive Parsing of External OS Outputs

- **Treat `pmset`, `sysctl`, and System Telemetry as Untrusted Input:**
  - Parsers (`MacOsStateBackup::from_pmset_custom`, `HardwareProfile::from_macos_sysctl`) must never panic on empty strings, malformed lines, unexpected whitespace, non-ASCII characters, missing sections, or integer overflow.
  - Always validate required sections (e.g., returning `MacOsConfigError::PmsetParseError` if `AC Power:` is missing, or `HardwareError::ParseError` if `hw.memsize` is absent/invalid).
  - Never use regular expressions with catastrophic backtracking; prefer zero-allocation string splitting (`lines()`, `trim()`, `split_whitespace()`, `split_once(':')`).

---

## 3. Command Generation & Shell Injection Prevention

- **No Unsanitized String Interpolation in Commands:**
  - Generated `pmset` commands (`pmset_apply_commands`, `pmset_restore_commands`) must only interpolate strongly-typed numeric values (`u32`, `u8::from(bool)`) — never raw `String` slices parsed from external input.
  - When executing child processes via `std::process::Command`, always pass arguments via `.arg()` / `.args()` rather than invoking `sh -c` or `cmd /C` with concatenated strings.
- **Shell Script Hardening (`scripts/bootstrap-macos.sh`):**
  - Every shell script must start with `#!/usr/bin/env bash` and `set -euo pipefail`.
  - Quote all variable expansions (`"${STATE_DIR}"`) and preserve existing state backups rather than overwriting them on re-runs.

---

## 4. Hardware, Battery & Thermal Guardrails

- **100% Reversible Power Mutations:**
  - Never generate or execute a `pmset` change without first persisting `MacOsStateBackup` to `state-backup.json`.
  - Always keep battery sleep active (`sudo pmset -b sleep 5 displaysleep 1`) even when AC sleep is disabled (`sudo pmset -c sleep 0`) and `disablesleep 1` is active, protecting the laptop if wall power is disconnected for extended periods.
- **Battery Swell Ceiling Validation:**
  - Reject any battery charge ceiling outside `50..=100%` with `MacOsConfigError::InvalidBatteryCeiling`.
  - `cheap-otto doctor` must warn whenever `battery_ceiling_percent > 80`.
- **Fanless Thermal & Kill-Switch Preemption:**
  - In `ArbiterPolicy::evaluate`, `kill_switch_engaged` and `ThermalLevel::Serious | ThermalLevel::Critical` must always take highest precedence, immediately pausing background queues and evicting loaded LLM weights before evaluating lower-priority rules.

---

## 5. Container & Network Security (`src/packs.rs`)

- **Localhost-Only Port Bindings:**
  - Every service port in generated Docker Compose manifests must bind strictly to `127.0.0.1` (e.g., `"127.0.0.1:5678:5678"`, `"127.0.0.1:3003:3003"`, `"127.0.0.1:11434:11434"`, `"127.0.0.1:3001:3001"`).
  - Never expose agent, browser, or AI gateway ports on `0.0.0.0` on the host network interface; remote access must traverse the authenticated Tailscale mesh (`100.x.y.z`).
- **Pinned Container Images & Resource Limits:**
  - Always pin explicit container image versions (e.g., `ollama/ollama:0.5.12`, `mcr.microsoft.com/playwright:v1.51.0-noble`, `docker.n8n.io/n8nio/n8n:1.82.0`, `louislam/uptime-kuma:1.23.16`) — never use `:latest`.
  - Enforce explicit memory limits (`deploy.resources.limits.memory`) derived from `MemoryBudget` and set `OLLAMA_KEEP_ALIVE` and `OLLAMA_MAX_LOADED_MODELS=1` to prevent unified memory exhaustion.
