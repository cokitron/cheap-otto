#!/bin/sh
# Idempotent M1 (`otto-mac`) environment, skill, symlink, package, and launchd provisioner.
# Invoked automatically by `code-reviewer m1 sync` or directly on `otto-mac`.
set -eu

export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:/usr/bin:/bin:/usr/sbin:/sbin"
REPO_ROOT="${REPO_ROOT:-$HOME/otto-workspaces/code-reviewer}"

echo "[m1-provision] Ensuring directory layout..."
mkdir -p \
  "$HOME/.local/bin" \
  "$HOME/.code-reviewer/tokens" \
  "$HOME/.cheap-otto/bin" \
  "$HOME/.cheap-otto/logs/code-reviewer" \
  "$HOME/.cheap-otto/runs/code-reviewer" \
  "$HOME/.cheap-otto/scheduled-worktrees/code-reviewer" \
  "$HOME/.claude/skills" \
  "$HOME/.claude/rules" \
  "$HOME/.gemini/antigravity/skills"
chmod 700 "$HOME/.code-reviewer" "$HOME/.code-reviewer/tokens"

# 1. Symlink Antigravity Google Workspace plugin CLIs (`gdrive`, `gdocs`, `gsheets`, `gslides`) into ~/.local/bin
for tool in gdrive gdocs gsheets gslides; do
  if [ -x "$HOME/.gemini/antigravity/bin/$tool" ]; then
    ln -sf "$HOME/.gemini/antigravity/bin/$tool" "$HOME/.local/bin/$tool"
    echo "[m1-provision] Linked ~/.local/bin/$tool -> ~/.gemini/antigravity/bin/$tool"
  fi
done

# 2. Sync repository skills & rules globally into ~/.claude and ~/.gemini/antigravity on M1
if [ -d "$REPO_ROOT/.claude/skills" ]; then
  for skill_dir in "$REPO_ROOT"/.claude/skills/*; do
    [ -d "$skill_dir" ] || continue
    skill_name=$(basename "$skill_dir")
    mkdir -p "$HOME/.claude/skills/$skill_name"
    cp -R "$skill_dir"/. "$HOME/.claude/skills/$skill_name/"
  done
  echo "[m1-provision] Synced Claude global skills -> ~/.claude/skills/"
fi

if [ -f "$REPO_ROOT/.claude/rules/review-safety.md" ]; then
  cp "$REPO_ROOT/.claude/rules/review-safety.md" "$HOME/.claude/rules/review-safety.md"
fi

if [ -d "$REPO_ROOT/.agents/skills" ]; then
  for skill_dir in "$REPO_ROOT"/.agents/skills/*; do
    [ -d "$skill_dir" ] || continue
    skill_name=$(basename "$skill_dir")
    mkdir -p "$HOME/.gemini/antigravity/skills/$skill_name"
    cp -R "$skill_dir"/. "$HOME/.gemini/antigravity/skills/$skill_name/"
  done
  echo "[m1-provision] Synced Antigravity global skills -> ~/.gemini/antigravity/skills/"
fi

# 3. Install optional Homebrew, Cargo, or npm packages if requested via env vars
if [ -n "${BREW_PKGS:-}" ] && command -v brew >/dev/null 2>&1; then
  for pkg in $(printf '%s' "$BREW_PKGS" | tr ',' ' '); do
    [ -n "$pkg" ] || continue
    if brew list "$pkg" >/dev/null 2>&1; then
      echo "[m1-provision] brew package '$pkg' already installed"
    else
      echo "[m1-provision] Installing brew package '$pkg'..."
      HOMEBREW_NO_AUTO_UPDATE=1 brew install "$pkg"
    fi
  done
fi

if [ -n "${CARGO_PKGS:-}" ] && command -v cargo >/dev/null 2>&1; then
  for crate in $(printf '%s' "$CARGO_PKGS" | tr ',' ' '); do
    [ -n "$crate" ] || continue
    echo "[m1-provision] Installing cargo crate '$crate'..."
    cargo install "$crate" --locked
  done
fi

if [ -n "${NPM_PKGS:-}" ] && command -v npm >/dev/null 2>&1; then
  for npkg in $(printf '%s' "$NPM_PKGS" | tr ',' ' '); do
    [ -n "$npkg" ] || continue
    echo "[m1-provision] Installing global npm package '$npkg' (--ignore-scripts)..."
    npm install -g --ignore-scripts "$npkg"
  done
fi

# 4. Write hardened launchd poller wrapper (overlap-safe + clean worktree reset)
cat > "$HOME/.cheap-otto/bin/otto-job-code-reviewer-pr-poller.sh" <<'WRAPPER_EOF'
#!/bin/sh
set -u
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:/usr/bin:/bin:/usr/sbin:/sbin"
START_TS=$(date +%s)
mkdir -p "$HOME/.cheap-otto/logs/code-reviewer" "$HOME/.cheap-otto/runs/code-reviewer" "$HOME/.cheap-otto/scheduled-worktrees/code-reviewer"

# 0. Overlap Guard (never interrupt an in-flight multi-lens review even on manual kickstart)
if [ -f "$HOME/.code-reviewer/poll.lock" ]; then
  LOCK_PID=$(head -n 1 "$HOME/.code-reviewer/poll.lock" 2>/dev/null | tr -d '[:space:]')
  if [ -n "$LOCK_PID" ] && kill -0 "$LOCK_PID" 2>/dev/null; then
    exit 0
  fi
fi

# 1. Hardware & Fanless Thermal Guard (allow Battery Power >= 20% so battery-limiter / normal unplugged runs aren't starved)
BATT_INFO=$(pmset -g batt 2>/dev/null || true)
if ! printf '%s' "$BATT_INFO" | grep -q "AC Power"; then
  BATT_PCT=$(printf '%s' "$BATT_INFO" | grep -Eo '[0-9]+%' | head -n 1 | tr -d '%')
  if [ -n "$BATT_PCT" ] && [ "$BATT_PCT" -lt 20 ] 2>/dev/null; then
    printf '{"job_id":"pr-poller","project":"code-reviewer","kind":"ci","status":"skipped_hardware_guard","commit_sha":"none","started_epoch_secs":%s,"duration_secs":0,"log_file":"~/.cheap-otto/logs/code-reviewer/sched-pr-poller.log"}\n' "$START_TS" >> "$HOME/.cheap-otto/runs/code-reviewer/history.jsonl"
    exit 0
  fi
fi
THERM=$(sysctl -n kern.thermalstate 2>/dev/null || echo 0)
if [ "$THERM" -ge 2 ] 2>/dev/null; then
  printf '{"job_id":"pr-poller","project":"code-reviewer","kind":"ci","status":"skipped_hardware_guard","commit_sha":"none","started_epoch_secs":%s,"duration_secs":0,"log_file":"~/.cheap-otto/logs/code-reviewer/sched-pr-poller.log"}\n' "$START_TS" >> "$HOME/.cheap-otto/runs/code-reviewer/history.jsonl"
  exit 0
fi

# 2. Resolve Base Git Commit (origin/main -> otto-sync -> HEAD)
if [ ! -d "$HOME/otto-workspaces/code-reviewer/.git" ]; then
  echo "Base repository $HOME/otto-workspaces/code-reviewer not initialized yet." >> "$HOME/.cheap-otto/logs/code-reviewer/sched-pr-poller.log"
  exit 1
fi
if git -C "$HOME/otto-workspaces/code-reviewer" remote get-url origin >/dev/null 2>&1; then
  git -C "$HOME/otto-workspaces/code-reviewer" fetch origin "main" --quiet 2>/dev/null || true
fi
TARGET_REF="HEAD"
if git -C "$HOME/otto-workspaces/code-reviewer" rev-parse --verify "refs/remotes/origin/main" >/dev/null 2>&1; then
  TARGET_REF="refs/remotes/origin/main"
elif git -C "$HOME/otto-workspaces/code-reviewer" rev-parse --verify "refs/heads/otto-sync" >/dev/null 2>&1; then
  TARGET_REF="refs/heads/otto-sync"
fi
TARGET_SHA=$(git -C "$HOME/otto-workspaces/code-reviewer" rev-parse --short=12 "$TARGET_REF" 2>/dev/null || echo "unknown")

# 3. Prepare Isolated Scheduled Worktree Cleanly (reset existing worktree without branch-in-use error)
WT_DIR="$HOME/.cheap-otto/scheduled-worktrees/code-reviewer/pr-poller"
if [ -e "$WT_DIR/.git" ]; then
  git -C "$WT_DIR" reset --hard "$TARGET_REF" --quiet 2>/dev/null || true
else
  git -C "$HOME/otto-workspaces/code-reviewer" worktree add --force -B "otto/sched-pr-poller" "$WT_DIR" "$TARGET_REF" >> "$HOME/.cheap-otto/logs/code-reviewer/sched-pr-poller.log" 2>&1
fi

# 4. Pure Rust GraphQL Poll (0 AI tokens on idle polls; detached async workers survive via AbandonProcessGroup + process_group(0))
cd "$WT_DIR"
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:/usr/bin:/bin:/usr/sbin:/sbin"
"$HOME/.cargo/bin/code-reviewer" --workspace "$HOME/otto-workspaces/code-reviewer" poll >> "$HOME/.cheap-otto/logs/code-reviewer/sched-pr-poller.log" 2>&1
RC=$?
printf '\n[OTTO_STATUS exit=%s]\n' "$RC" >> "$HOME/.cheap-otto/logs/code-reviewer/sched-pr-poller.log"
END_TS=$(date +%s)
DUR=$((END_TS - START_TS))
if [ "$RC" -eq 0 ] 2>/dev/null; then STATUS="pass"; else STATUS="fail"; fi
printf '{"job_id":"pr-poller","project":"code-reviewer","kind":"ci","status":"%s","commit_sha":"%s","started_epoch_secs":%s,"duration_secs":%s,"log_file":"~/.cheap-otto/logs/code-reviewer/sched-pr-poller.log"}\n' "$STATUS" "$TARGET_SHA" "$START_TS" "$DUR" >> "$HOME/.cheap-otto/runs/code-reviewer/history.jsonl"
exit "$RC"
WRAPPER_EOF
chmod +x "$HOME/.cheap-otto/bin/otto-job-code-reviewer-pr-poller.sh"
echo "[m1-provision] Updated hardened launchd wrapper ~/.cheap-otto/bin/otto-job-code-reviewer-pr-poller.sh"

# 5. Ensure launchd LaunchAgent plist has AbandonProcessGroup=true so spawned async PR sessions survive poll exit
PLIST_PATH="$HOME/Library/LaunchAgents/com.cheapotto.code-reviewer.pr-poller.plist"
mkdir -p "$HOME/Library/LaunchAgents"
NEEDS_RELOAD=0
if ! grep -q "AbandonProcessGroup" "$PLIST_PATH" 2>/dev/null; then
  NEEDS_RELOAD=1
fi
cat > "$PLIST_PATH" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.cheapotto.code-reviewer.pr-poller</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/sh</string>
    <string>$HOME/.cheap-otto/bin/otto-job-code-reviewer-pr-poller.sh</string>
  </array>
  <key>RunAtLoad</key>
  <false/>
  <key>StartInterval</key>
  <integer>120</integer>
  <key>AbandonProcessGroup</key>
  <true/>
  <key>ProcessType</key>
  <string>Background</string>
  <key>Nice</key>
  <integer>5</integer>
  <key>StandardOutPath</key>
  <string>$HOME/.cheap-otto/logs/code-reviewer/launchd-pr-poller.out.log</string>
  <key>StandardErrorPath</key>
  <string>$HOME/.cheap-otto/logs/code-reviewer/launchd-pr-poller.err.log</string>
</dict>
</plist>
PLIST_EOF

UID_NUM=$(id -u)
if [ "$NEEDS_RELOAD" -eq 1 ]; then
  launchctl bootout "gui/$UID_NUM/com.cheapotto.code-reviewer.pr-poller" 2>/dev/null || true
  launchctl bootstrap "gui/$UID_NUM" "$PLIST_PATH" 2>/dev/null || true
  echo "[m1-provision] Reloaded launchd job gui/$UID_NUM/com.cheapotto.code-reviewer.pr-poller with AbandonProcessGroup=true"
elif ! launchctl print "gui/$UID_NUM/com.cheapotto.code-reviewer.pr-poller" >/dev/null 2>&1; then
  launchctl bootstrap "gui/$UID_NUM" "$PLIST_PATH" 2>/dev/null || true
  echo "[m1-provision] Bootstrapped launchd job gui/$UID_NUM/com.cheapotto.code-reviewer.pr-poller"
else
  echo "[m1-provision] launchd job gui/$UID_NUM/com.cheapotto.code-reviewer.pr-poller is active"
fi

# 6. Provision & hot-restart persistent Web Dashboard GUI service (`127.0.0.1:7430`) inside `gui/<uid>`
UI_PLIST_PATH="$HOME/Library/LaunchAgents/com.cheapotto.code-reviewer.ui.plist"
cat > "$UI_PLIST_PATH" <<UI_PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.cheapotto.code-reviewer.ui</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
  </dict>
  <key>ProgramArguments</key>
  <array>
    <string>$HOME/.cargo/bin/code-reviewer</string>
    <string>--workspace</string>
    <string>$REPO_ROOT</string>
    <string>serve</string>
    <string>--bind</string>
    <string>127.0.0.1:7430</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>StandardOutPath</key>
  <string>$HOME/.cheap-otto/logs/code-reviewer/ui-server.log</string>
  <key>StandardErrorPath</key>
  <string>$HOME/.cheap-otto/logs/code-reviewer/ui-server.log</string>
</dict>
</plist>
UI_PLIST_EOF

if ! launchctl print "gui/$UID_NUM/com.cheapotto.code-reviewer.ui" >/dev/null 2>&1; then
  launchctl bootstrap "gui/$UID_NUM" "$UI_PLIST_PATH" 2>/dev/null || true
fi
launchctl kickstart -k "gui/$UID_NUM/com.cheapotto.code-reviewer.ui" 2>/dev/null || true
sleep 1
if ! nc -z 127.0.0.1 7430 2>/dev/null; then
  pkill -f "code-reviewer.*serve --bind 127.0.0.1:7430" 2>/dev/null || true
  nohup "$HOME/.cargo/bin/code-reviewer" --workspace "$REPO_ROOT" serve --bind 127.0.0.1:7430 >> "$HOME/.cheap-otto/logs/code-reviewer/ui-server.log" 2>&1 &
  sleep 1
fi
echo "[m1-provision] Web Dashboard GUI active on 127.0.0.1:7430"

# 7. Write hardened launchd poller wrapper & LaunchAgent for Code Tender (`tend-poll`, 0 AI tokens when idle)
cat > "$HOME/.cheap-otto/bin/otto-job-code-reviewer-code-tender.sh" <<'TENDER_WRAPPER_EOF'
#!/bin/sh
set -u
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:/usr/bin:/bin:/usr/sbin:/sbin"
START_TS=$(date +%s)
mkdir -p "$HOME/.cheap-otto/logs/code-reviewer" "$HOME/.cheap-otto/runs/code-reviewer"

if [ -f "$HOME/.code-reviewer/tender-poll.lock" ]; then
  LOCK_PID=$(head -n 1 "$HOME/.code-reviewer/tender-poll.lock" 2>/dev/null | tr -d '[:space:]')
  if [ -n "$LOCK_PID" ] && kill -0 "$LOCK_PID" 2>/dev/null; then
    exit 0
  fi
fi

BATT_INFO=$(pmset -g batt 2>/dev/null || true)
if ! printf '%s' "$BATT_INFO" | grep -q "AC Power"; then
  BATT_PCT=$(printf '%s' "$BATT_INFO" | grep -Eo '[0-9]+%' | head -n 1 | tr -d '%')
  if [ -n "$BATT_PCT" ] && [ "$BATT_PCT" -lt 20 ] 2>/dev/null; then
    printf '{"job_id":"code-tender","status":"skipped_hardware_guard","started_epoch_secs":%s,"duration_secs":0}\n' "$START_TS" >> "$HOME/.cheap-otto/runs/code-reviewer/tender-history.jsonl"
    exit 0
  fi
fi
THERM=$(sysctl -n kern.thermalstate 2>/dev/null || echo 0)
if [ "$THERM" -ge 2 ] 2>/dev/null; then
  printf '{"job_id":"code-tender","status":"skipped_hardware_guard","started_epoch_secs":%s,"duration_secs":0}\n' "$START_TS" >> "$HOME/.cheap-otto/runs/code-reviewer/tender-history.jsonl"
  exit 0
fi

WT_DIR="$HOME/.cheap-otto/scheduled-worktrees/code-reviewer/pr-poller"
if [ -d "$WT_DIR" ]; then
  cd "$WT_DIR"
fi
"$HOME/.cargo/bin/code-reviewer" --workspace "$HOME/otto-workspaces/code-reviewer" tend-poll >> "$HOME/.cheap-otto/logs/code-reviewer/sched-code-tender.log" 2>&1
RC=$?
END_TS=$(date +%s)
DUR=$((END_TS - START_TS))
if [ "$RC" -eq 0 ] 2>/dev/null; then STATUS="pass"; else STATUS="fail"; fi
printf '{"job_id":"code-tender","status":"%s","started_epoch_secs":%s,"duration_secs":%s}\n' "$STATUS" "$START_TS" "$DUR" >> "$HOME/.cheap-otto/runs/code-reviewer/tender-history.jsonl"
exit "$RC"
TENDER_WRAPPER_EOF
chmod +x "$HOME/.cheap-otto/bin/otto-job-code-reviewer-code-tender.sh"

TENDER_PLIST_PATH="$HOME/Library/LaunchAgents/com.cheapotto.code-reviewer.code-tender.plist"
TENDER_RELOAD=0
if ! grep -q "AbandonProcessGroup" "$TENDER_PLIST_PATH" 2>/dev/null; then
  TENDER_RELOAD=1
fi
cat > "$TENDER_PLIST_PATH" <<TENDER_PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.cheapotto.code-reviewer.code-tender</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/sh</string>
    <string>$HOME/.cheap-otto/bin/otto-job-code-reviewer-code-tender.sh</string>
  </array>
  <key>RunAtLoad</key>
  <false/>
  <key>StartInterval</key>
  <integer>120</integer>
  <key>AbandonProcessGroup</key>
  <true/>
  <key>ProcessType</key>
  <string>Background</string>
  <key>Nice</key>
  <integer>5</integer>
  <key>StandardOutPath</key>
  <string>$HOME/.cheap-otto/logs/code-reviewer/launchd-code-tender.out.log</string>
  <key>StandardErrorPath</key>
  <string>$HOME/.cheap-otto/logs/code-reviewer/launchd-code-tender.err.log</string>
</dict>
</plist>
TENDER_PLIST_EOF

if [ "$TENDER_RELOAD" -eq 1 ]; then
  launchctl bootout "gui/$UID_NUM/com.cheapotto.code-reviewer.code-tender" 2>/dev/null || true
  launchctl bootstrap "gui/$UID_NUM" "$TENDER_PLIST_PATH" 2>/dev/null || true
elif ! launchctl print "gui/$UID_NUM/com.cheapotto.code-reviewer.code-tender" >/dev/null 2>&1; then
  launchctl bootstrap "gui/$UID_NUM" "$TENDER_PLIST_PATH" 2>/dev/null || true
fi
echo "[m1-provision] Code Tender LaunchAgent active (com.cheapotto.code-reviewer.code-tender, 120s)"

echo "[m1-provision] Provisioning complete."
