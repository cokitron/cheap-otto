---
paths:
  - "**/*.rs"
  - "**/*.sh"
  - "**/*.ps1"
  - "**/Cargo.toml"
---

# Security & Appliance Safety Rules

`cheap-otto` changes power management with `sudo pmset`, holds a lithium-ion battery on AC around the clock, runs a fanless laptop near its thermal limit, builds shell commands that execute on another machine, and hands GitHub tokens to subprocesses. Security and hardware safety are engineering requirements here, not polish.

---

## 1. Memory & Type Safety

- **No `unsafe` Rust.** `#![forbid(unsafe_code)]` sits at every crate root (`src/lib.rs`, `src/main.rs`, `crates/code-reviewer/src/main.rs`, `tests/cli_integration.rs`). A new crate root or test binary gets it too.
- **Saturating and checked arithmetic.** Hardware math (RAM budgets, disk percentages, idle timers, battery caps) must never panic on overflow or underflow. Use `saturating_sub`, `saturating_add`, `checked_mul` or `.clamp(min, max)`.

---

## 2. Defensive Parsing of External Output

- **Treat `pmset`, `sysctl`, `vm_stat`, `tmux`, `gh` JSON and `stream-json` agent output as untrusted input.**
  - Parsers (`MacOsStateBackup::from_pmset_custom`, `HardwareProfile::from_macos_sysctl`, the GraphQL poll parser) must never panic on empty strings, malformed lines, odd whitespace, non-ASCII text, missing sections or integer overflow.
  - Validate required sections and return a typed error: `MacOsConfigError::PmsetParseError` when `AC Power:` is missing, `HardwareError::ParseError` when `hw.memsize` is absent or invalid.
  - Parse with `lines()`, `trim()`, `split_whitespace()` and `split_once(':')`. Neither crate depends on `regex`; don't add it for parsing.

---

## 3. Command Generation & Shell Injection

There are two layers, and they have different rules.

- **Local process: never a shell string.** Plans carry `CommandSpec { program, args }` and `src/main.rs` spawns them with `Command::new(program).args(args)`. Do not build `sh -c` or `cmd /C` strings for local execution in the root crate.
- **Remote command: a POSIX string the Mac's shell will interpret.** The last argument to `ssh` is shell text, so every interpolated value must be one of:
  - a typed number (`u8`, `u16`, `u32`, `u8::from(bool)`);
  - an identifier that passed a validator: `validate_safe_identifier` (project names, task ids, aliases, users, tmux sessions), `validate_model_identifier` or `validate_session_identifier`. All three allow only a small ASCII set and reject a leading `-`, so a value can't be read as a flag;
  - free text wrapped by `posix_single_quote` (prompts, verify commands, questions).
  New interpolation points need one of these three, plus a test with a hostile input such as `x'; rm -rf ~ #`.
- **`launchd` plists** are XML: pass every interpolated string through `xml_escape` (`src/schedule.rs`).
- **Multi-line remote scripts go over stdin.** `code-reviewer` uses `run_ssh_script` (`ssh host sh -s`), which also normalises CRLF. Prefer it to nesting quotes, especially from PowerShell.
- **The one deliberate `sh -c`:** Code Tender runs the operator-configured `local_check_cmd` and install command through `/bin/sh -c` (`cmd /C` on Windows) inside the PR worktree. That text comes from `config/repos.toml`, never from PR content. Keep it that way: nothing read from a PR, a review comment or an agent's reply may reach that string.

---

## 4. Tokens & Agent Subprocesses (`code-reviewer`)

- GitHub access is `gh` with a per-account `GH_TOKEN` read from `~/.code-reviewer/tokens/<user>.token` (mode 600). It never mutates the global `gh` login.
- Every LLM agent and check-command subprocess is spawned with `.env_remove("GH_TOKEN")` and `.env_remove("GITHUB_TOKEN")`. A new spawn site for `claude`, `agy` or a repo check must do the same.
- Code Tender refuses to commit `.env`, `.env.*` (except `.env.example`), keys and submodule changes; it checks `git status --porcelain` before every commit. Extend that guard rather than working around it.
- Bot-authored comments carry `BOT_COMMENT_MARKER` or `BOT_TENDER_MARKER`. Loop prevention depends on them, so every comment or review body the tool posts must carry one.
- Never log or print a token, and never write one outside `tokens_dir`.

---

## 5. Hardware, Battery & Thermal Guardrails

- **Reversible power changes.** Never generate or run a `pmset` change without first persisting `MacOsStateBackup` to `state-backup.json`. `restore` rebuilds the original values from that file.
- **Battery sleep fallback stays on.** Keep `sudo pmset -b sleep 5 displaysleep 1` even when AC sleep is off and `disablesleep 1` is set, so an unplugged laptop still sleeps.
- **Charge ceiling validation.** Reject anything outside `50..=100` with `MacOsConfigError::InvalidBatteryCeiling`. `doctor` passes at 80% or below and warns above it.
- **Thermal and kill-switch precedence.** In `ArbiterPolicy::evaluate`, `kill_switch_engaged` and then `ThermalLevel::Serious | Critical` return before any lower-priority rule is evaluated.
- **Scheduled jobs are gated.** Generated `launchd` runner scripts check power (`pmset -g batt`) and thermal state (`kern.thermalstate`) before starting work. Don't add a scheduled path that skips the gate.

---

## 6. Network Exposure

- `cheap-otto serve` defaults to `127.0.0.1:7420` and `code-reviewer serve` to `127.0.0.1:7430`. Port forwards are written `127.0.0.1:<port>:127.0.0.1:<port>`.
- Never default a listener to `0.0.0.0`. Reaching a server from another machine means an SSH tunnel over LAN or Tailscale.
- Neither server authenticates requests, and Mission Control currently answers any origin (`CorsLayer::permissive()`). The loopback binding is the only barrier, and a web page open in the operator's browser can still reach loopback. Treat every endpoint that executes input (for example `POST /api/terminal/{session}`, which types into a tmux session) as reachable by a hostile page: validate identifiers, and don't add new ones without an origin check. Tightening CORS is welcome; widening the bind needs auth first.

---

## 7. Scripts

- **Bash** (`scripts/bootstrap-macos.sh`): start with `#!/usr/bin/env bash` and `set -euo pipefail`.
- **POSIX sh** (`crates/code-reviewer/scripts/*.sh`): these run on the Mac under `/bin/sh` with `set -eu`. No bashisms (`[[ ]]`, arrays, `pipefail`).
- Quote every expansion (`"${STATE_DIR}"`), stay idempotent, and preserve an existing backup instead of overwriting it on a re-run.
- `.gitattributes` forces LF on `*.sh`. Don't commit CRLF shell scripts; macOS `sh` fails on `\r`.
- **PowerShell** (`*.ps1`): set `$ErrorActionPreference = "Stop"`, pass arguments as an array (`& cargo @argsList`), and check `$LASTEXITCODE`. Don't assemble an `ssh` command string in PowerShell: it strips quotes. Call `code-reviewer m1 …` instead.
