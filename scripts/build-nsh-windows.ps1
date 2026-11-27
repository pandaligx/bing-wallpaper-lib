[CmdletBinding()]
param([string]$NshPath = 'nsh', [switch]$SkipCheck)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $versionOutput = & $NshPath --version
    if ($LASTEXITCODE -ne 0 -or $versionOutput -notmatch '^nsh 0\.7\.2 \(0c29b5e') {
        throw "Expected NSH 0.7.2 (0c29b5e), found $versionOutput"
    }
    if (!$SkipCheck) {
        & $NshPath check desktop
        if ($LASTEXITCODE -ne 0) { throw 'TSX type check failed' }
    }
    & $NshPath build desktop --release --out desktop/.nsh/out
    if ($LASTEXITCODE -ne 0) { throw 'NSH bundle build failed' }
    $manifest = Get-Content -Raw desktop/nsh.json | ConvertFrom-Json
    $targetDir = Join-Path $root 'target/nsh-cargo'
    & cargo build --locked --manifest-path desktop/windows/Cargo.toml --release --target-dir $targetDir --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Rust host build failed' }
    $binary = Join-Path $targetDir 'x86_64-pc-windows-msvc/release/bing-wallpaper-desktop.exe'
    & (Join-Path $PSScriptRoot 'check_windows_imports.ps1') -ExePath $binary -MinimumWindows1809
    $out = New-Item -ItemType Directory -Force (Join-Path $root 'target/nsh-release')
    $artifact = Join-Path $out "BingWallpaperLib-v$($manifest.version)-windows-x64.exe"
    Copy-Item -LiteralPath $binary -Destination $artifact
    Get-FileHash $artifact -Algorithm SHA256
} finally {
    Pop-Location
}
