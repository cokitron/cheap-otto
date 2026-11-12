param(
    [string]$HostAddr = "otto-mac",
    [int]$Port = 7430,
    [switch]$NoOpen
)

$ErrorActionPreference = "Stop"
$url = "http://127.0.0.1:$Port"

# 1. Check if tunnel is already up locally
$alive = $false
try {
    $null = Invoke-RestMethod -Uri "$url/api/health" -TimeoutSec 1
    $alive = $true
} catch {
    $alive = $false
}

if (-not $alive) {
    Write-Host "[ui] Ensuring UI server is active on $HostAddr and opening persistent SSH tunnel on 127.0.0.1:$Port..."
    $RepoRoot = Split-Path -Parent $PSScriptRoot
    Push-Location $RepoRoot
    try {
        & cargo run --quiet -- m1 exec --cmd "UID_NUM=`$(id -u); launchctl kickstart `"gui/`$UID_NUM/com.cheapotto.code-reviewer.ui`" 2>/dev/null || true" | Out-Null
    } finally {
        Pop-Location
    }

    Start-Process -FilePath "ssh" -WindowStyle Hidden -ArgumentList @(
        "-N",
        "-o", "ExitOnForwardFailure=yes",
        "-o", "ServerAliveInterval=30",
        "-L", "127.0.0.1:${Port}:127.0.0.1:${Port}",
        $HostAddr
    ) | Out-Null

    for ($i = 0; $i -lt 20; $i++) {
        Start-Sleep -Milliseconds 300
        try {
            $null = Invoke-RestMethod -Uri "$url/api/health" -TimeoutSec 1
            $alive = $true
            break
        } catch {}
    }
}

if (-not $alive) {
    throw "Could not reach UI tunnel at $url"
}

Write-Host "[ui] Dashboard live at: $url"
if (-not $NoOpen) {
    Start-Process $url
}
