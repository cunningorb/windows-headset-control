#!/usr/bin/env pwsh
# Builds the release binaries and packages them into a setup executable.
$ErrorActionPreference = 'Stop'

function Find-Iscc {
    $onPath = Get-Command iscc -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    # Inno installs per-user when winget runs unelevated and per-machine when
    # chocolatey runs elevated in CI, so both are checked rather than assumed.
    $candidates = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    )
    foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
    return $null
}

$iscc = Find-Iscc
if (-not $iscc) {
    throw "Inno Setup is not installed. Install it with:`n  winget install --id JRSoftware.InnoSetup --exact"
}

# The single source of truth for the version is the workspace manifest.
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Building Headset Tray $version with $iscc"

cargo build --workspace --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

New-Item -ItemType Directory -Force dist | Out-Null
& $iscc "/DAppVersion=$version" installer\headset-tray.iss
if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed" }

Get-ChildItem dist\*-setup.exe | Select-Object Name, Length
