---
name: update-m1
description: Push code, brains, KBs, watched repo configs, packages, files, GitHub tokens, or Google Drive review docs to the headless M1 (`otto-mac`) using `code-reviewer m1`.
---

# Updating & Provisioning `otto-mac` (M1)

Whenever you need to deploy code changes, update brain prompts/KBs, add watched repositories, install packages (`brew`, `cargo`, `npm`), push files, refresh GitHub tokens, or bridge Google Drive review docs onto `otto-mac`, **always use `code-reviewer m1` subcommands** instead of raw ad-hoc `ssh` strings (avoids Windows PowerShell quote-stripping and UTF-8 BOM bugs).

`m1 sync` deploys whatever is checked out: it installs both binaries (`cheap-otto`, then `code-reviewer`) on the Mac, rewrites the reviewer's `launchd` jobs, and installs the dashboard services (`com.cheapotto.dashboards.mission-control` on 7420, `com.cheapotto.dashboards.pixel-office` on 7421; the office is skipped when `pixel-agents` is missing). A failed dashboard install is a warning, not a failed deploy; check it from Windows with `cargo run -p cheap-otto -- dashboards status --execute`.

## Standard Commands

```bash
# 1. Full push + remote test/clippy + both binaries + skill/symlink/launchd + dashboard service provisioning
cargo run -p code-reviewer -- m1 sync

# 2. Full sync + refresh primary/secondary GitHub tokens + bridge pending Drive docs
cargo run -p code-reviewer -- m1 sync --sync-tokens --sync-drive

# 3. Install Homebrew, Cargo, or npm packages on M1 during sync
cargo run -p code-reviewer -- m1 sync --brew "jq,ripgrep" --cargo "cargo-nextest"

# 4. Live M1 dashboard (AC/thermal state, active brain PID, launchd status, PR ledger, log tail)
cargo run -p code-reviewer -- m1 status

# 5. Copy any arbitrary local file to M1 with exact permissions (8-bit Base64 safe)
cargo run -p code-reviewer -- m1 push-file --local ./crates/code-reviewer/config/repos.toml --remote ~/otto-workspaces/code-reviewer/config/repos.toml --mode 644

# 6. Run any remote shell command safely via SSH stdin (`sh -s`)
cargo run -p code-reviewer -- m1 exec --cmd "code-reviewer doctor"

# 7. Trigger an immediate poll tick on M1 (spawns parallel async PR sessions)
cargo run -p code-reviewer -- m1 kickstart

# 8. Open the Web Dashboard GUI (`http://127.0.0.1:7430`) through the shared `cheap-otto ui` tunnel
cargo run -p code-reviewer -- ui
```
