---
paths:
  - "Cargo.toml"
  - "src/**/*.rs"
---

# Crate Ecosystem, CLI Design & Dependency Hygiene

`cheap-otto` intentionally keeps a **minimal, audited dependency tree** so the appliance binary builds fast, links cleanly across macOS/Linux/Windows, and minimizes supply-chain risk.

---

## 1. Approved Core Dependencies (`Cargo.toml`)

| Crate | Version | Purpose & Idiomatic Usage |
| :--- | :--- | :--- |
| **`clap`** | `4.5` (`derive`) | Declarative CLI argument & subcommand parsing (`#[derive(Parser, Subcommand, Debug)]`). |
| **`serde`** | `1.0` (`derive`) | Deterministic serialization/deserialization of hardware profiles, `pmset` backups, agent sessions, and templates. |
| **`serde_json`** | `1.0` | Human-readable JSON persistence & `stream-json` NDJSON parsing. |
| **`thiserror`** | `2.0` | Zero-boilerplate `std::error::Error` and `Display` implementations on domain error enums. |
| **`axum` / `tokio` / `tower-http`** | `0.8` / `1.43` / `0.6` | Powers the `cheap-otto serve` HTTP/JSON Mission Control & REST API daemon (`:7420`) and `pixel-agents` hook forwarder. |

### Dependency Addition Policy
- **Prefer `std` first:** Do not add external crates for string manipulation, simple path joining, or basic formatting when the Rust standard library (`std::str`, `std::path`, `std::fs`, `std::fmt`) suffices.
- **No Heavy Async Runtimes Unless Required:** `cheap-otto` CLI subcommands (`init`, `doctor`, `arbiter`, `restore`) are fast, synchronous operations. Avoid pulling in `tokio`, `reqwest`, or `openssl` unless a future daemon mode explicitly requires async network I/O.
- **Cross-Platform Compilation:** Never add macOS-only C bindings (`core-foundation`, `io-kit`) as unconditional dependencies; the project must always compile and pass `cargo test` on Windows, Linux, and macOS.

---

## 2. `clap` CLI Design Guidelines (`src/main.rs`)

- **Doc Comments as Help Text:**
  - Every `Commands` variant and CLI flag must have a `///` doc comment explaining its purpose, units (e.g., `MB`, `GB`, `secs`, `%`), and default behavior. `clap` automatically renders these into `--help`.
- **Safe Defaults for Target Hardware:**
  - Default flags should target the canonical appliance profile (`16 GB` M1 MacBook Air, `78%` battery charge ceiling, `.cheap-otto` state directory) while allowing overrides via `--ram-gb`, `--battery-cap`, and `--out-dir`.
- **Thin Command Handlers:**
  - Keep `run_init`, `run_doctor`, `run_arbiter`, and `run_restore` in `src/main.rs` thin: parse inputs, invoke library functions from `cheap_otto::*`, perform file I/O at the boundary, and print structured console summaries.

---

## 3. State Serialization & Backward Compatibility (`serde`)

- **Pretty-Printed State Backups:**
  - Always write `.cheap-otto/state-backup.json` using `serde_json::to_string_pretty(&backup)` so human operators can inspect or manually audit their original `pmset` values over SSH.
- **Additive Schema Evolution:**
  - If adding fields to `MacOsStateBackup` in the future, annotate new fields with `#[serde(default)]` so existing `.cheap-otto/state-backup.json` files created by older versions of `cheap-otto init` can still be restored cleanly by `cheap-otto restore`.
