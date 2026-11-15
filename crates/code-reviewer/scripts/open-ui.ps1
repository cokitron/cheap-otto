param(
    [string]$HostAddr = "otto-mac",
    [int]$Port = 7430,
    [switch]$NoOpen
)

$ErrorActionPreference = "Stop"

# A thin wrapper over `code-reviewer ui`, which owns the tunnel rules: reuse a tunnel that already
# answers, otherwise share the single `cheap-otto ui` tunnel, otherwise open a Reviewer-only fallback.
# This script never starts ssh or forwards Pixel Office itself.
$argsList = @("run", "--quiet", "--", "ui", "--host", $HostAddr, "--port", "$Port")
if ($NoOpen) {
    $argsList += "--no-open"
}

$CrateDir = Split-Path -Parent $PSScriptRoot
Push-Location $CrateDir
try {
    & cargo @argsList
    if ($LASTEXITCODE -ne 0) {
        throw "code-reviewer ui failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}
