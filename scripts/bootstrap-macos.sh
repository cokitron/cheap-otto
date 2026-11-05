#!/usr/bin/env bash
set -euo pipefail

# cheap-otto: Idempotent macOS Clamshell Appliance Bootstrapper
# Usage: ./scripts/bootstrap-macos.sh

STATE_DIR="${HOME}/.cheap-otto"
mkdir -p "${STATE_DIR}"

echo "==> [1/5] Probing Apple Silicon / macOS hardware..."
SYSCTL_OUT="$(sysctl hw.memsize machdep.cpu.brand_string hw.model)"
echo "${SYSCTL_OUT}"

echo "==> [2/5] Backing up current pmset power configuration to ${STATE_DIR}/pmset-backup.txt..."
if [[ ! -f "${STATE_DIR}/pmset-backup.txt" ]]; then
  pmset -g custom > "${STATE_DIR}/pmset-backup.txt"
  echo "Saved original pmset snapshot."
else
  echo "Existing pmset backup found; preserving original snapshot."
fi

echo "==> [3/5] Applying reversible Clamshell Appliance power rules (AC no-sleep, 1-min panel off)..."
sudo pmset -c sleep 0 displaysleep 1 autorestart 1 womp 1 tcpkeepalive 1
sudo pmset -a disablesleep 1
sudo pmset -b sleep 5 displaysleep 1

echo "==> [4/5] Checking Tailscale & OrbStack..."
if ! command -v tailscale >/dev/null 2>&1; then
  echo "  [info] Installing Tailscale via Homebrew..."
  brew install --cask tailscale
fi

if ! command -v orb >/dev/null 2>&1; then
  echo "  [info] Installing OrbStack (lightweight container engine for Apple Silicon)..."
  brew install --cask orbstack
fi

echo "==> [5/5] Building and running cheap-otto init..."
if command -v cargo >/dev/null 2>&1; then
  cargo run --release -- init --out-dir "${STATE_DIR}"
  cargo run --release -- doctor
else
  echo "  [info] Rust toolchain not installed yet on this Mac. Install via: curl https://sh.rustup.rs -sSf | sh"
fi

echo "==> Done! You can now close the MacBook lid on AC power."
