# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A Rust 2024 Cargo workspace that turns a spare, lid-closed MacBook (`otto-mac`, usually a fanless M1) into a second machine for a Windows workstation, over plain SSH on LAN or Tailscale.

| Piece | Where | What it does |
| --- | --- | --- |
| `cheap-otto` | root package: `src/lib.rs` + `src/main.rs` | CLI for the dev bridge (`pair`, `sync`, `exec`, `shell`), the agent worktree farm (`delegate`, `inspect`, `ask`, `pull`), the `launchd` scheduler (`schedule`, `runs`), reversible power guardrails (`init`, `doctor`, `arbiter`, `restore`) and the Mission Control server (`serve`, `ui`, port 7420) |
| `code-reviewer` | `crates/code-reviewer`, binary only | Autonomous multi-lens PR reviewer and Code Tender that run on the Mac, with a dashboard on port 7430 |
| Site and brand | `docs/`, `.github/assets/`, `scripts/brand/` | Static GitHub Pages site (no build step), logo and hero images, and the scripts that generate them |
| Governance | `CONTRIBUTING.md`, `SECURITY.md`, `cubic.yaml`, `.ai/`, `.github/` | PR-only `main`, the Cubic AI review config and the context files it reads, CI, issue and PR templates |

## Safety: the appliance is real

Assume the machine you are running on is paired with a live MacBook and live GitHub repos. Real state lives in `~/.cheap-otto/`, `~/.ssh/config`, `~/.code-reviewer/` and the gitignored `crates/code-reviewer/config/repos.toml`.

- **`cheap-otto` is dry-run by default for OS and remote actions.** Those subcommands print their plan and stop unless given `--execute` (alias `--apply`; `init`, `pair` and `restore` name it `--apply`). Commands that only mutate local JSON, including `template add` and `template remove`, have no gate and write immediately. `pair --tunnel` opens a live tunnel. Pass side-effecting flags only when the user asked for that specific action.
- **`code-reviewer` is live by default.** `poll`, `tend-poll`, `review`, `tend`, `sync-docs`, `ui` and mutating `m1 …` subcommands act on GitHub or the Mac at once. Use `--dry-run` on the first four. Only `doctor`, `status`, `m1 status` and `watch list` are read-only.
- **`code-reviewer m1 sync` deploys.** It force-pushes the current `HEAD` to the Mac as `otto-sync`, reinstalls both binaries and restarts the `launchd` jobs. Never run it unasked or from a feature branch.
- **Dry runs still write state and may probe the Mac.** `init` and `pair` write directly to `--out-dir` (default `./.cheap-otto`) with no repo or home fallback. State-discovery commands such as `delegate` persist JSON into the state dir, which resolves `--state-dir`, then `./.cheap-otto`, then the repo root, then `~/.cheap-otto`. `doctor` and `init` read telemetry from the paired Mac over SSH. When you exercise the CLI, pass a temp `--out-dir` (`init`, `pair`) or `--state-dir` (the rest), and `--offline` on `init` and `doctor`.

## Commands

```bash
# The CI gate (.github/workflows/ci.yml runs it on Linux, macOS and Windows). Run all three before calling work done; together they take under a minute.
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --locked

# One module, one test, one integration test
cargo test -p cheap-otto --lib arbiter::tests
cargo test -p code-reviewer tender::tests::quota_exhausted_errors_are_detected
cargo test -p cheap-otto --test cli_integration schedule_lifecycle

# Run a binary. Plain `cargo run -- …` fails at the repo root because both packages are default members.
cargo run -p cheap-otto -- doctor --offline
cargo run -p code-reviewer -- --help
```

## Architecture

### `cheap-otto`: plans are data, execution happens at the edge

- **Functional core.** Every module in `src/` except `main.rs` and `server.rs` is pure, with one deliberate exception: `harness::generate_session_uuid` reads the clock and entropy. The core parses text captured from the OS (`pmset -g custom`, `sysctl`, `vm_stat`) and returns typed plans: structs holding a `Vec<CommandSpec>` (`src/command.rs`: a `program` plus discrete `args`). There is no filesystem, process or network I/O in the core, which is why the suite passes on Windows, Linux and macOS with no Mac attached.
- **Imperative shells.** `src/main.rs` is the synchronous one: clap parsing, loading and saving state JSON, printing plans, and `execute_command_spec` behind the `--execute` gate for CLI commands. `src/server.rs` is the async one for `serve` (axum, `127.0.0.1:7420` by default). It is a live privileged control plane: its REST handlers spawn local `tmux`, `/bin/sh`, `git` and `launchctl` commands with no `--execute` gate, and its terminal API can drive tmux sessions, so keep it reachable only through localhost or an SSH tunnel. Its REST API is shared by the embedded Mission Control page (`src/ui_assets.rs`) and by external agents; CLI `--json` output is the other machine interface.
- **Remote work is an SSH command string.** The Mac's shell interprets it, so every interpolated value is validated (`validate_safe_identifier`, `validate_model_identifier`, `validate_session_identifier`) or quoted (`posix_single_quote`). The local process is always spawned with discrete args.
- **Naming contract on the Mac.** Many modules depend on these, so change them everywhere or nowhere:
  - workspace `~/otto-workspaces/<project>`, worktree `.otto-worktrees/<task-id>` inside it
  - branch `otto/<task-id>`, tmux session `otto-<task-id>` (`manual` is reserved for the human)
  - log `~/.cheap-otto/logs/<project>/<task-id>.log`, run ledger `~/.cheap-otto/runs/<project>/history.jsonl`
  - LaunchAgent label `com.cheapotto.<project>.<id>`
- **Local state.** A `.cheap-otto/` dir of pretty-printed JSON: `peer.json`, `state-backup.json`, `projects.json`, `agents.json`, `templates.json`, `schedules.json`. Schemas grow additively (`#[serde(default)]` on new fields), because `restore` must load a backup written by an older build.

### `code-reviewer`: a synchronous appliance binary

- **No async runtime, no HTTP/TLS stack.** `server.rs` is a hand-rolled `std::net` server. GitHub access goes through `gh` subprocesses with a per-account `GH_TOKEN`; agents run as `claude` and `agy` subprocesses.
- **Review flow.** `poll` runs from `launchd` every two minutes with one batched GraphQL query, so idle ticks cost no AI tokens. `state.rs` picks the trigger (initial review, re-review after a push or reply, post-merge docs). Each PR gets a detached worker. `brains.rs` runs only the applicable brains `01`–`07`, in waves capped by `max_parallel_brains_per_pr`, in a read-only worktree pinned to the head SHA, then `08-adversarial-verifier` strikes unproven claims. `diff.rs` keeps only inline comments that land inside diff hunks, because one bad line makes GitHub reject the whole review. `github.rs` posts it in one call and the ledger goes to `~/.code-reviewer/state.json`.
- **Code Tender (`tender.rs`) is the write path.** It fixes review feedback in writable PR worktrees, runs the repo's check command, has the other provider verify the diff, then pushes or stages a patch.
- **Prompts are data.** `brains/<id>/PROMPT.md` with a `kb/` folder each, and `tender-agents/`.

## Invariants

1. Every macOS power change is reversible: snapshot to `state-backup.json` before generating a `pmset` change, and `restore` must reproduce the original exactly.
2. The battery charge ceiling stays in `50..=100` (default 78) and the battery sleep fallback (`pmset -b sleep 5`) is always kept.
3. Arbiter precedence: kill-switch first, then thermal `Serious` or worse, before any other rule.
4. No `unsafe` (`#![forbid(unsafe_code)]` at every crate root) and no `.unwrap()`, `.expect()` or `panic!` outside tests. Both crates are at zero today.
5. Servers default to `127.0.0.1`, but `cheap-otto serve --host` and `code-reviewer serve --bind` accept overrides; do not expose either server on a non-loopback address. Remote access goes through SSH tunnels.
6. Tests need no Mac, no network and no wall-clock time.

## Conventions

- **Commits and PR titles:** Conventional Commits, `<type>(<scope>): <summary>` or `<type>: <summary>` (the scope is optional), with type one of `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `ci`, `perf`, `build`, `revert`. CI rejects a PR whose title does not match.
- **Branches:** `main` is PR-only and squash-merged. Branch from `origin/main` with a prefix CI accepts: `feat/`, `fix/`, `chore/`, `docs/`, `refactor/`, `test/`, `ci/`, `perf/`, `otto/` or `claude/`. `dependabot/` is also accepted, for Dependabot's own branches.
- **A PR merges only when** the four CI jobs pass (`PR Governance & Secret Guard`, and `Rust Gate` on three OSes), the Cubic AI review check passes, every review thread is resolved, and the branch is up to date with `main`.
- **`main` deploys:** GitHub Pages serves `main:/docs`, and a `v*` tag builds release binaries.
- **Versions:** both `Cargo.toml` files carry the version and it must match the release tag.
- Rule 06 and `CONTRIBUTING.md` hold the full workflow; `SECURITY.md` says how to report a vulnerability.

## Specs

Larger work is planned in `specs/<feature>/` as requirements, then design, then tasks, and each task has a GitHub issue. `specs/README.md` is the protocol and the step-by-step for carrying out a task; follow it when asked to implement one. Current spec: `specs/m1-dashboards/` (every dashboard as a `launchd` service on the Mac, one SSH tunnel).

## Path-scoped rules

`.claude/rules/` holds detailed rules that load when you touch matching files:

| File | Loads for | Covers |
| --- | --- | --- |
| `01-rust-idioms-and-edition-2024.md` | Rust sources, manifests | Type-driven design, error enums, ownership, matching |
| `02-security-and-appliance-safety.md` | Rust sources, shell and PowerShell scripts | Remote shell quoting, defensive parsing, power and battery guardrails, token isolation |
| `03-architecture-and-testing.md` | `src/`, `tests/`, `crates/` | Core and shell split, test layout and naming, how to test without a Mac |
| `04-clippy-formatting-and-docs.md` | Rust sources, manifests | Zero-warning policy, rustdoc, the completion checklist |
| `05-crate-ecosystem-and-cli-patterns.md` | Manifests, `main.rs` files | Dependency policy, clap conventions, dry-run flags, state schema evolution |
| `06-git-workflow-and-governance.md` | Always | PR-only `main`, branch prefixes, Conventional Commits, squash merges, Cubic AI review and Code Tender coexistence, public sanitization |
| `07-code-reviewer.md` | `crates/code-reviewer/` | Review pipeline, brains and KBs, Code Tender limits, token handling, deploying to the Mac |
| `08-embedded-web-ui.md` | `ui_assets.rs`, `ui_html.rs`, both `server.rs` | Self-contained embedded pages, API parity, the request guard, previewing safely |
| `09-brand-and-pages-site.md` | `docs/`, `.github/assets/`, `scripts/brand/`, `README.md` | Brand tokens, logo rules, regenerating assets, Pages deploy |
| `10-ci-and-release.md` | `.github/`, manifests | CI jobs and required checks, release tags and targets, what never goes in a commit |
| `11-specs-protocol.md` | `specs/` | How to implement a spec task and how to edit a spec |
