#!/usr/bin/env bash
set -euo pipefail

# Regenerates every brand asset: the logo SVGs, both README hero PNGs and the social preview PNG.
#
# Usage: scripts/brand/render.sh [out-root]
#   out-root defaults to the repository root. Pass a temp dir to inspect the output without
#   touching tracked files (the same docs/assets and .github/assets layout is created under it).
#
# Needs: node, and Chrome or Edge (set CHROME=/path/to/browser to override detection).
# Works from Git Bash on Windows, macOS and Linux. Fonts come from Google Fonts, so it needs network.

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../.." && pwd)"
OUT="${1:-${REPO}}"
mkdir -p "${OUT}/docs/assets" "${OUT}/.github/assets"
OUT="$(cd "${OUT}" && pwd)"

find_chrome() {
  if [[ -n "${CHROME:-}" ]]; then
    printf '%s\n' "${CHROME}"
    return 0
  fi
  local candidate
  for candidate in \
    "/c/Program Files/Google/Chrome/Application/chrome.exe" \
    "/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"; do
    if [[ -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  for candidate in google-chrome google-chrome-stable chromium chromium-browser microsoft-edge microsoft-edge-stable; do
    if command -v "${candidate}" >/dev/null 2>&1; then
      command -v "${candidate}"
      return 0
    fi
  done
  return 1
}

# Chrome on Windows wants C:/-style paths; everywhere else the POSIX path is fine.
native_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "$1"
  else
    printf '%s\n' "$1"
  fi
}

if ! BROWSER="$(find_chrome)"; then
  echo "error: Chrome or Edge not found. Set CHROME=/path/to/browser." >&2
  exit 1
fi

echo "==> [1/2] Logo SVGs -> ${OUT}/docs/assets"
node "${HERE}/gen-logos.js" "${OUT}/docs/assets" >/dev/null

TEMPLATE_PATH="$(native_path "${HERE}/hero.html")"
case "${TEMPLATE_PATH}" in
  /*) TEMPLATE_URL="file://${TEMPLATE_PATH}" ;;
  *) TEMPLATE_URL="file:///${TEMPLATE_PATH}" ;;
esac

ASSETS_PATH="$(native_path "${OUT}/docs/assets")"
case "${ASSETS_PATH}" in
  /*) ASSETS_URL="file://${ASSETS_PATH}/" ;;
  *) ASSETS_URL="file:///${ASSETS_PATH}/" ;;
esac

# shot <kind> <width> <height> <scale> <output path>
shot() {
  local kind="$1" width="$2" height="$3" scale="$4" target
  target="$(native_path "$5")"
  "${BROWSER}" --headless=new --disable-gpu --hide-scrollbars --allow-file-access-from-files \
    --virtual-time-budget=5000 --force-device-scale-factor="${scale}" \
    --window-size="${width},${height}" --screenshot="${target}" \
    "${TEMPLATE_URL}?k=${kind}&a=$(node -p 'encodeURIComponent(process.argv[1])' "${ASSETS_URL}")" >/dev/null 2>&1
  if [[ ! -s "$5" ]]; then
    echo "error: ${BROWSER} did not write $5" >&2
    exit 1
  fi
  echo "    wrote $5"
}

echo "==> [2/2] Hero and social PNGs (headless ${BROWSER##*/})"
shot night 1280 440 2 "${OUT}/.github/assets/readme-hero-dark.png"
shot paper 1280 440 2 "${OUT}/.github/assets/readme-hero-light.png"
shot social 1280 640 1 "${OUT}/docs/assets/social-preview.png"

echo "==> Done. Review the images, then commit them."
