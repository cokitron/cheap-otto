---
paths:
  - "**/Cargo.toml"
  - "Cargo.lock"
  - "src/main.rs"
  - "crates/code-reviewer/src/main.rs"
---

# Crate Ecosystem, CLI Design & Dependency Hygiene

Both binaries keep a small, audited dependency tree so they build fast on a fanless Mac, link cleanly on macOS, Linux and Windows, and carry little supply-chain risk.

---

## 1. Approved Dependencies

| Crate | Version | Used by | Purpose |
| :--- | :--- | :--- | :--- |
| `clap` | `4.5` (`derive`) | both | CLI parsing with `#[derive(Parser, Subcommand, Args)]` |
| `serde`, `serde_json` | `1.0` | both | State files, plans, API payloads, `stream-json` NDJSON |
| `thiserror` | `2.0` | both | Domain error enums |
| `axum`, `tokio` | `0.8`, `1.43` | `cheap-otto` only | The `serve` REST API and the `pixel-agents` event forwarder |
| `tower` | `0.5` (dev) | `cheap-otto` only | `oneshot` router tests |
| `chrono`, `toml` | `0.4`, `0.8` | `code-reviewer` only | Ledger timestamps, `config/repos.toml` |

### Adding a dependency
- **`std` first.** No crate for string handling, path joining or formatting the standard library already covers.
- **`code-reviewer` stays synchronous and std-only.** No `tokio`, `axum`, `reqwest`, `hyper` or TLS stack: its HTTP server is hand-rolled on `std::net`, and GitHub and Drive access goes through `gh`, `gdrive` and `gdocs` subprocesses. It is compiled on the Mac on every deploy, so build time is a feature.
- **`cheap-otto` uses the async runtime only for `serve`.** Other subcommands are synchronous; don't make them async to reuse a helper.
- **No HTTP client, `regex`, `openssl` or `anyhow`.** Network calls are subprocesses, parsing is string splitting, errors are `thiserror` enums.
- **No platform-only crates as unconditional dependencies** (`core-foundation`, `io-kit`, `windows`). Everything must compile on all three OSes.
- A new dependency needs a reason in the PR description. `Cargo.lock` is committed and `cargo install --locked` is used on the Mac, so commit lockfile changes with the manifest change.

---

## 2. Workspace Layout

- The root `Cargo.toml` is both the workspace and the `cheap-otto` package. `default-members` lists both packages, so:
  - bare `cargo build`, `cargo test` and `cargo clippy` at the root cover both;
  - bare `cargo run` fails with "could not determine which binary to run". Always pass `-p cheap-otto` or `-p code-reviewer`.
- Both packages share one version. Bump them together.

---

## 3. `clap` Conventions (`src/main.rs`)

- **Doc comments are the help text.** Every subcommand and flag has a `///` line giving purpose, units (`MB`, `GB`, `secs`, `%`) and default.
- **Dry-run by default.** A subcommand that touches the Mac, `~/.ssh/config` or macOS settings takes a boolean gate and only prints its plan without it:
  - `#[arg(long, visible_alias = "apply")] execute: bool` for bridge, farm and schedule commands;
  - `#[arg(long, visible_alias = "execute")] apply: bool` for `init`, `pair` and `restore`.
  A new side-effecting subcommand must follow this, and its handler prints the full plan before the `if execute { … }` block.
- **`--json`** on commands an agent might script (`delegate`, `inspect`, `ask`, `harnesses`, `template …`). JSON goes to stdout alone, with no banner text mixed in.
- **`--state-dir` / `--out-dir`** on anything that reads or writes `.cheap-otto/`, so tests and experiments can redirect state.
- **`--offline`** on commands that would otherwise probe hardware, so they can run deterministically anywhere.
- **Safe defaults for the canonical appliance:** 16 GB M1 Air, 78% charge ceiling, alias `otto-mac`, ports `3000,5173,8080`.
- **Thin handlers.** Each `run_<subcommand>` takes one `&XOptions<'_>` struct, calls planners from `cheap_otto::*`, does file I/O at the boundary and prints. No business rules in `main.rs`.
- **Identifiers are validated at the boundary.** Project names, task ids, aliases, model names and session ids go through the `validate_*` functions before a plan is built.

`code-reviewer` differs on one point: its commands act immediately and offer `--dry-run` instead. Keep `--dry-run` on anything that calls an LLM or writes to GitHub.

---

## 4. State Serialization & Compatibility

- **Pretty-printed JSON** (`serde_json::to_string_pretty`) for every state file, so an operator can read and audit it over SSH.
- **Additive schema evolution.** New fields on persisted structs (`MacOsStateBackup`, `PeerNode`, `AgentSessionRecord`, `ScheduleRegistry`, `TemplateRegistry`, the reviewer ledger and config) take `#[serde(default)]`. Never rename or remove a persisted field without a migration: `restore` has to read a `state-backup.json` written by an older build, and the Mac may be running an older binary against the same files.
- **Config changes come with the example.** A new key in the reviewer config goes into `crates/code-reviewer/config/repos.example.toml` too. The real `repos.toml` is gitignored.
- **Loading is forgiving, saving is strict.** A missing registry file loads as the default; a write error is returned to the user.
