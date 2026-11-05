# cheap-otto 🦀💻

**Turn a spare MacBook (even with a broken screen) into a Cross-OS Dev Bridge & Parallel Agent Worktree Farm for your main Windows workstation—in one 100% reversible command.**

Instead of tying up your Windows terminal, branch, and CPU waiting for coding agents or heavy test suites, `cheap-otto` pairs your **Windows PC** and **headless MacBook** over LAN or Tailscale SSH so they work as **one machine**:
1. **Cross-OS Dev Bridge (`pair`, `sync`, `exec`):** Sync your active Windows repo to the MacBook in seconds, run native macOS/ARM64 builds (`cargo test`, `npm test`, `swift build`) from your Windows terminal, and forward MacBook dev server ports (`3000`, `5173`, `8080`) straight to Windows `localhost`.
2. **Parallel Agent Worktree Farm (`delegate`, `jobs`, `pull`):** Offload coding tasks (`claude`, `agy`, `kiro-cli`) to **isolated Git worktrees** (`otto/<task-id>`) running in background `tmux` sessions on the MacBook while you keep coding on Windows—then fetch the verified branch straight back into your Windows Git repo (`cheap-otto pull --id <task-id>`).
3. **100% Reversible Clamshell & Battery Guardrails (`init`, `doctor`, `arbiter`, `restore`):** Disables AC lid-close sleep (`disablesleep 1`), caps battery charge at **78%** so staying plugged in 24/7 never swells the battery, keeps 5-minute battery fallback sleep if unplugged, and arbitrates unified RAM/thermals on fanless M1/M2/M3 MacBook Airs.

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
# Generates .cheap-otto/state-backup.json and prints the reversible pmset commands
cargo run -- init --ram-gb 16 --battery-cap 78

# Run the 8-point appliance health check
cargo run -- doctor
```

### 2. Pair Your Windows Workstation with the MacBook (`src/bridge.rs`)
```bash
# Pair over LAN IP (e.g. 192.168.1.42) or Tailscale IP (e.g. 100.101.102.103)
cargo run -- pair --host 192.168.1.42 --user admin --alias otto-mac --ports 3000,5173,8080

# Sync your active Windows repo to the MacBook (excludes target/, node_modules/, etc.)
cargo run -- sync --project cheap-otto

# Run a native macOS / ARM64 build or test suite from your Windows terminal
cargo run -- exec --project cheap-otto --cmd "cargo test --all-targets"
```

### 3. Delegate Parallel Coding Tasks & Pull Branches Back (`src/worktree.rs`)
```bash
# Delegate a task to an isolated Git worktree + background tmux session on the MacBook
cargo run -- delegate \
  --project cheap-otto \
  --id fix-auth-race \
  --agent claude \
  --verify "cargo test --all-targets" \
  --task "Fix the token refresh race condition and add unit tests"

# Check active agent tmux sessions and Git worktrees on the MacBook
cargo run -- jobs --project cheap-otto

# Fetch the finished `otto/fix-auth-race` branch back into your Windows Git repo
cargo run -- pull --project cheap-otto --id fix-auth-race
```

### 4. Restore Factory macOS Sleep Settings Anytime
```bash
cargo run -- restore
```
