# cheap-otto 🦀💻

**Turn a spare MacBook (even with a broken screen) into a Cross-OS Dev Bridge & Parallel Agent Worktree Farm for your main Windows workstation—in one 100% reversible command.**

Instead of tying up your Windows terminal, branch, and CPU waiting for coding agents or heavy test suites, `cheap-otto` pairs your **Windows PC** and **headless MacBook** over LAN or Tailscale SSH so they work as **one machine**:
1. **Cross-OS Dev Bridge & Interactive Terminal (`pair`, `sync`, `exec`, `shell`):** Sync your active Windows repo to the MacBook in seconds (protecting uncommitted manual M1 edits by default), run native macOS/ARM64 builds from Windows, forward dev ports (`3000`, `5173`, `8080`) to Windows `localhost`, or log straight into a persistent `manual` `tmux` terminal on the M1 (`cheap-otto shell`).
2. **Parallel Agent Worktree Farm & Live Visibility (`delegate`, `attach`, `logs`, `jobs`, `pull`):** Offload coding tasks (`claude`, `agy`, `kiro-cli`) to **isolated Git worktrees** (`otto/<task-id>`) with `remain-on-exit` `tmux` panes and persistent logs (`~/.cheap-otto/logs/`), attach live (`cheap-otto attach --id <id>`) or tail output (`cheap-otto logs --id <id> -f`), and fetch verified branches back to Windows.
3. **Recurring CI & Scheduled Agents (`schedule`, `runs`):** Set up recurring native macOS `launchd` jobs (`--every 15m --on-change`, `--daily 02:00`, `--weekly MON@03:00`) for commit-polled CI pipelines or overnight autonomous agent runs with Draft PRs—gated automatically by AC-power and passive M1 thermal checks, with structured JSONL history (`cheap-otto runs`).
4. **100% Reversible Clamshell & Battery Guardrails (`init`, `doctor`, `arbiter`, `restore`):** Disables AC lid-close sleep (`disablesleep 1`), caps battery charge at **78%** so staying plugged in 24/7 never swells the battery, keeps 5-minute battery fallback sleep if unplugged, and arbitrates unified RAM/thermals on fanless M1/M2/M3 MacBook Airs.

---

## Core Workflow: Windows PC $\leftrightarrow$ Headless MacBook

```text
[Windows Workstation]                              [Headless M1 MacBook (Lid Closed, 78% Battery Cap)]
  D:\projects\my-app                                 ~/otto-workspaces/my-app
  ├── (You code on `main`)                           ├── .otto-worktrees/fix-auth-race  (tmux: otto-fix-auth-race)
  │                                                  │     └── Claude Code + `cargo test` -> branch `otto/fix-auth-race`
  ├── $ cheap-otto delegate --task "Fix auth..." --->├── .otto-worktrees/add-api-tests  (tmux: otto-add-api-tests)
  │                                                  │     └── Antigravity + `cargo test` -> branch `otto/add-api-tests`
  └── $ cheap-otto pull --id fix-auth-race <---------└── git fetch ssh://otto-mac/... refs/heads/otto/fix-auth-race
```

---

## Quickstart

### 1. Prepare the Spare MacBook (Reversible Clamshell Mode)
```bash
# Probes live macOS sysctl/pmset (or pass --remote over SSH), saves .cheap-otto/state-backup.json,
# and applies reversible pmset rules when --apply is passed
cargo run -- init --battery-cap 78 --apply

# Run the 8-point appliance health check (auto-probes local macOS or paired SSH MacBook)
cargo run -- doctor
```

### 2. Pair Your Windows Workstation with the MacBook (`src/bridge.rs`)
```bash
# Pair over LAN IP (e.g. 192.168.1.42) or Tailscale IP (e.g. 100.101.102.103)
# Pass --apply to merge into ~/.ssh/config & verify SSH reachability, or --tunnel to forward ports live
cargo run -- pair --host 192.168.1.42 --user admin --alias otto-mac --ports 3000,5173,8080 --apply

# Sync your active Windows Git repo (project & branch auto-detected!) to the MacBook
cargo run -- sync --execute

# Run a native macOS / ARM64 build or test suite from your Windows terminal
cargo run -- exec --cmd "cargo test --all-targets" --execute
```

### 3. Delegate Parallel Coding Tasks & Pull Branches Back (`src/worktree.rs`)
```bash
# Delegate a task to an isolated Git worktree + background tmux session on the MacBook
cargo run -- delegate \
  --id fix-auth-race \
  --agent claude \
  --verify "cargo test --all-targets" \
  --task "Fix the token refresh race condition and add unit tests" \
  --execute

# Check active agent tmux sessions and Git worktrees on the MacBook
cargo run -- jobs --execute

# Inspect auto-detected repo, config path, and multi-project workspace registry
cargo run -- projects

# Fetch (and optionally --merge / --cleanup) the finished `otto/fix-auth-race` branch into Windows
cargo run -- pull --id fix-auth-race --execute --merge
```

### 4. Interactive M1 Terminal & Live Job Visibility (`src/terminal.rs`)
```bash
# Drop into a persistent `manual` tmux session on the M1 (isolated from automated `otto-*` jobs)
cargo run -- shell --execute

# Watch a running or finished agent/CI tmux pane live on the M1 (`--read-only` for safe observation)
cargo run -- attach --id fix-auth-race --execute

# Tail persistent logs for any delegated or scheduled job
cargo run -- logs --id fix-auth-race --follow --execute
```

### 5. Recurring CI Pipelines & Scheduled Agent Runs (`src/schedule.rs`)
```bash
# Run native ARM64 CI every 15m whenever `origin/main` (or `otto-sync`) has new commits
cargo run -- schedule add \
  --id main-ci \
  --every 15m \
  --on-change \
  --ci "cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test" \
  --execute

# Schedule a nightly autonomous agent audit at 02:00 AM that opens a Draft PR when tests pass
cargo run -- schedule add \
  --id nightly-audit \
  --daily 02:00 \
  --agent claude \
  --task "Audit dependencies and add missing edge-case unit tests" \
  --draft-pr \
  --execute

# Inspect configured schedules and pass/fail JSONL run history
cargo run -- schedule list --execute
cargo run -- runs --execute
```

### 6. Restore Factory macOS Sleep Settings (& Unload Schedules) Anytime
```bash
cargo run -- restore --unload-schedules --remote --apply
```
