# CLAUDE.md — `cheap-otto` Project Guidelines & Rust Engineering Standards

## 1. Project Overview

**`cheap-otto`** turns any spare MacBook (especially fanless Apple Silicon M1/M2/M3 MacBook Airs, including units with broken screens) or old laptop into a **24/7 headless AI agent host, residential browser worker, local AI gateway, and self-hosted CI runner** in a single, 100% reversible command.

### Core Value Propositions & Invariants
1. **100% Reversible OS Changes (`src/macos.rs`):** Every host power/sleep modification (`pmset`) must first snapshot original settings to `.cheap-otto/state-backup.json` and generate deterministic, idempotent restore commands (`cheap-otto restore`).
2. **Battery Swell Protection (`src/macos.rs`, `src/doctor.rs`):** Laptops plugged into AC power 24/7 must enforce a safe battery charge ceiling (`50..=100%`, default `78%`) and retain battery sleep (`5 min`) if AC power is lost.
3. **Unified Memory & Passive Thermal Arbitration (`src/hardware.rs`, `src/arbiter.rs`):** Fanless laptops have strict thermal and unified RAM envelopes. The arbiter deterministically evicts cached local LLM weights on idle timeout (`300s` fanless / `600s` active cooling), low memory, CI job startup, or `Serious`/`Critical` thermal pressure, and pauses background queues when thermals spike or the mobile kill-switch is engaged.
4. **Private-by-Default Networking (`src/packs.rs`, `src/doctor.rs`):** All generated Docker/OrbStack service ports (`n8n`, Playwright residential browser, Ollama local AI gateway, Uptime Kuma) must bind exclusively to `127.0.0.1` and rely on Tailscale private mesh for remote access.
5. **Cross-Platform Functional Core (`src/*.rs`):** All parsing (`pmset`, `sysctl`), memory budgeting, thermal/memory arbitration, health evaluation, and Compose rendering are pure, deterministic Rust functions that compile and pass `cargo test` on **macOS, Linux, and Windows**.

---

## 2. Repository Structure & Module Map

```text
cheapotto/
├── Cargo.toml                        # Rust 2024 edition crate manifest (cheap-otto)
├── CLAUDE.md                         # Primary instructions for Claude Code & AI agents
├── AGENTS.md                         # Cross-agent rules entry point (Antigravity / Codex / Cursor)
├── .claude/
│   └── rules/
│       ├── 01-rust-idioms-and-edition-2024.md
│       ├── 02-security-and-appliance-safety.md
│       ├── 03-architecture-and-testing.md
│       ├── 04-clippy-formatting-and-docs.md
│       └── 05-crate-ecosystem-and-cli-patterns.md
├── scripts/
│   └── bootstrap-macos.sh            # Idempotent macOS SSH bootstrapper
└── src/
    ├── lib.rs                        # Library crate root exporting public modules
    ├── main.rs                       # CLI binary entrypoint (clap derive) & I/O orchestration
    ├── hardware.rs                   # HardwareProfile, CpuArch, ThermalLevel, MemoryBudget, sysctl parser
    ├── macos.rs                      # MacOsStateBackup, AppliancePowerPlan, pmset parser & command builder
    ├── arbiter.rs                    # ArbiterPolicy, ArbiterSnapshot, ArbiterAction (RAM/thermal state machine)
    ├── doctor.rs                     # SystemProbeInput, DoctorReport, 8-point appliance invariant checker
    ├── packs.rs                      # CapabilityPack enum & deterministic Docker Compose stack generator
    ├── network.rs                    # ApplianceNode, ServiceEndpoints, RemoteWorkload (LAN/Tailscale bridge)
    ├── dev_worker.rs                 # Pillar 1: PrSandboxPlan (agent-fix/triage) & CanarySpec E2E monitor
    ├── chief_of_staff.rs             # Pillar 2: 15m Inbox/Calendar triage & Telegram [Approve Send] cards
    ├── ai_gateway.rs                 # Pillar 3: Apple Silicon LLM/Whisper/RAG stack & 99%-to-1% scraper filter
    └── remote_dev.rs                 # Pillar 4: SSH port-forward config, tmux, and VS Code Remote Tunnel kit
```

---

## 3. Essential Commands

Always verify changes using the local Cargo toolchain before completing any task:

```bash
# Compile check (fast feedback)
cargo check --all-targets

# Run the full unit and integration test suite
cargo test

# Lint with Clippy (zero warnings allowed)
cargo clippy --all-targets --all-features -- -D warnings

# Verify formatting
cargo fmt --all -- --check

# Run CLI subcommands locally
cargo run -- init --ram-gb 16 --battery-cap 78
cargo run -- doctor
cargo run -- arbiter --available-mb 6144 --ci-running --llm-loaded --llm-idle-secs 15
cargo run -- restore
```

---

## 4. Non-Negotiable Rust & Security Rules Summary

Detailed rules live in `.claude/rules/` and are loaded automatically:
- **[01-rust-idioms-and-edition-2024.md](.claude/rules/01-rust-idioms-and-edition-2024.md):** Rust 2024 edition idioms, ownership, type-driven design, `thiserror` v2 error handling, `#[must_use]` annotations, and zero `.unwrap()`/`.expect()` in library code.
- **[02-security-and-appliance-safety.md](.claude/rules/02-security-and-appliance-safety.md):** `#![forbid(unsafe_code)]`, saturating arithmetic (`saturating_sub`, `clamp`), defensive parser design for OS command output, shell/command injection prevention, localhost-only bindings (`127.0.0.1`), and pinned container image versions.
- **[03-architecture-and-testing.md](.claude/rules/03-architecture-and-testing.md):** Functional Core / Imperative Shell separation, cross-platform testability, and `Result`-returning unit tests covering happy paths, boundary conditions, and error variants.
- **[04-clippy-formatting-and-docs.md](.claude/rules/04-clippy-formatting-and-docs.md):** Clippy pedantic discipline, rustdoc standards, and pre-flight verification workflow.
- **[05-crate-ecosystem-and-cli-patterns.md](.claude/rules/05-crate-ecosystem-and-cli-patterns.md):** Approved crate stack (`clap`, `serde`, `serde_json`, `thiserror`), CLI design conventions, and additive schema evolution.
