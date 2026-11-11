# CLAUDE.md — `cheap-otto` Project Guidelines & Rust Engineering Standards

## 1. Project Overview

**`cheap-otto`** turns a spare MacBook (especially fanless Apple Silicon M1/M2/M3 MacBook Airs, including units with broken screens) into a **Cross-OS Dev Bridge & Parallel Agent Worktree Farm** paired with your primary Windows workstation over LAN or Tailscale SSH.

### Core Value Propositions & Invariants
1. **Cross-OS Dev Bridge (`src/bridge.rs`):** Pairs Windows and macOS over standard SSH (`PeerNode`), syncs repositories via Git+SSH while excluding OS-specific build artifacts (`target/`, `node_modules/`), runs native macOS/ARM64 commands from Windows (`build_remote_exec`), and forwards localhost dev ports (`build_port_tunnel`).
2. **Parallel Agent Worktree Farm (`src/worktree.rs`):** Offloads coding tasks (`ClaudeCode`, `Antigravity`, `KiroCli`, `CustomShell`) to isolated Git worktrees (`~/otto-workspaces/<project>/.otto-worktrees/<task-id>`) on dedicated branches (`otto/<task-id>`) inside detached `tmux` sessions, and fetches completed branches straight back into the Windows Git repository (`build_pull_plan`).
3. **100% Reversible OS Changes (`src/macos.rs`):** Every host power/sleep modification (`pmset`) must first snapshot original settings to `.cheap-otto/state-backup.json` and generate deterministic, idempotent restore commands (`cheap-otto restore`).
4. **Battery Swell Protection (`src/macos.rs`, `src/doctor.rs`):** Laptops plugged into AC power 24/7 must enforce a safe battery charge ceiling (`50..=100%`, default `78%`) and retain battery sleep (`5 min`) if AC power is lost.
5. **Unified Memory & Passive Thermal Arbitration (`src/hardware.rs`, `src/arbiter.rs`):** Fanless laptops have strict thermal and unified RAM envelopes. The arbiter deterministically evicts cached local LLM weights on idle timeout, low memory, build/worktree startup, or `Serious`/`Critical` thermal pressure.
6. **Cross-Platform Functional Core (`src/*.rs`):** All parsing, SSH/Git command generation, worktree planning, memory budgeting, and health evaluation are pure, deterministic Rust functions that compile and pass `cargo test` on **Windows, Linux, and macOS**.

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
    ├── command.rs                    # CommandSpec: shell-free OS process specifications (program + args)
    ├── workspace.rs                  # Multi-project Git discovery, config hierarchy & WorkspaceRegistry
    ├── bridge.rs                     # Option 5: PeerNode, dirty-guarded WorkspaceSyncPlan, RemoteExec, PortTunnelPlan
    ├── terminal.rs                   # Interactive M1 shell (`manual` tmux / zsh), live `attach`, and `logs`
    ├── worktree.rs                   # Option 1: AgentRunner, DelegatedWorktreePlan (remain-on-exit + tee logs), WorktreePullPlan
    ├── schedule.rs                   # Reversible macOS launchd CI & autonomous agent scheduler + JSONL run ledger
    ├── hardware.rs                   # HardwareProfile, CpuArch, ThermalLevel, MemoryBudget, sysctl/vm_stat parsers
    ├── macos.rs                      # MacOsStateBackup, AppliancePowerPlan, pmset parser & command builder
    ├── arbiter.rs                    # ArbiterPolicy, ArbiterSnapshot, ArbiterAction (RAM/thermal state machine)
    └── doctor.rs                     # SystemProbeInput, RawTelemetrySnapshot, DoctorReport, 8-point checker
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
cargo run -- pair --host 192.168.1.42 --user admin --alias otto-mac
cargo run -- sync --project cheap-otto
cargo run -- exec --project cheap-otto --cmd "cargo test --all-targets"
cargo run -- delegate --project cheap-otto --id fix-auth-race --agent claude --task "Fix auth token race"
cargo run -- pull --project cheap-otto --id fix-auth-race
cargo run -- doctor
cargo run -- restore
```

---

## 4. Non-Negotiable Rust & Security Rules Summary

Detailed rules live in `.claude/rules/` and are loaded automatically:
- **[01-rust-idioms-and-edition-2024.md](.claude/rules/01-rust-idioms-and-edition-2024.md):** Rust 2024 edition idioms, ownership, type-driven design, `thiserror` v2 error handling, `#[must_use]` annotations, and zero `.unwrap()`/`.expect()` in library code.
- **[02-security-and-appliance-safety.md](.claude/rules/02-security-and-appliance-safety.md):** `#![forbid(unsafe_code)]`, saturating arithmetic (`saturating_sub`, `clamp`), defensive parser design for OS command output, shell/command injection prevention (`validate_safe_identifier`), and battery/thermal safety guardrails.
- **[03-architecture-and-testing.md](.claude/rules/03-architecture-and-testing.md):** Functional Core / Imperative Shell separation, cross-platform testability, and `Result`-returning unit tests covering happy paths, boundary conditions, and error variants.
- **[04-clippy-formatting-and-docs.md](.claude/rules/04-clippy-formatting-and-docs.md):** Clippy pedantic discipline, rustdoc standards, and pre-flight verification workflow.
- **[05-crate-ecosystem-and-cli-patterns.md](.claude/rules/05-crate-ecosystem-and-cli-patterns.md):** Approved crate stack (`clap`, `serde`, `serde_json`, `thiserror`), CLI design conventions, and additive schema evolution.
