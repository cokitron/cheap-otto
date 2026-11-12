#!/usr/bin/env bash
set -euo pipefail

export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:$PATH"
WORKSPACE_DIR="${1:-$HOME/otto-workspaces/code-reviewer}"

echo "=== Step 1/4: Claude Code Login ==="
if claude auth status 2>/dev/null | grep -q '"loggedIn": true'; then
  echo "[SKIP] claude already logged in."
else
  claude auth login
fi

echo ""
echo "=== Step 2/4: Antigravity (agy) Login ==="
agy -p "Reply OK" --effort low

echo ""
echo "=== Step 3/4: Google Drive CLI Login (port 8085) ==="
if "$HOME/.gemini/antigravity/bin/gdrive" readonly ls root --max 1 >/dev/null 2>&1; then
  echo "[SKIP] gdrive already logged in."
else
  "$HOME/.gemini/antigravity/bin/gdrive" auth login --port 8085
fi

echo ""
echo "=== Step 4/4: Google Docs CLI Login (port 8085) ==="
"$HOME/.gemini/antigravity/bin/gdocs" auth login --port 8085 || true

echo ""
echo "=== Final Verification ==="
code-reviewer --workspace "$WORKSPACE_DIR" doctor
