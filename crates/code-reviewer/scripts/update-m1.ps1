<#
.SYNOPSIS
    Turnkey Windows -> M1 (`otto-mac`) updater & provisioner for `code-reviewer`.
.EXAMPLE
    .\scripts\update-m1.ps1
    .\scripts\update-m1.ps1 -SyncTokens -SyncDrive -Brew "jq,ripgrep"
#>
param(
    [string]$HostTarget = "otto-mac",
    [switch]$SyncTokens,
    [switch]$SyncDrive,
    [string]$Brew = "",
    [string]$Cargo = "",
    [string]$Npm = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $RepoRoot
try {
    $argsList = @(
        "run", "--quiet", "--",
        "--workspace", $RepoRoot,
        "m1", "--host", $HostTarget,
        "sync"
    )
    if ($SyncTokens) { $argsList += "--sync-tokens" }
    if ($SyncDrive)  { $argsList += "--sync-drive" }
    if ($Brew -ne "")  { $argsList += @("--brew", $Brew) }
    if ($Cargo -ne "") { $argsList += @("--cargo", $Cargo) }
    if ($Npm -ne "")   { $argsList += @("--npm", $Npm) }

    & cargo @argsList
    if ($LASTEXITCODE -ne 0) {
        throw "code-reviewer m1 sync failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}
