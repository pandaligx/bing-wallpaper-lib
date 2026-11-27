[CmdletBinding()]
param([Parameter(Mandatory)][string]$RehearsalExe)
$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path $RehearsalExe).Path
$root = Split-Path $exe
if (!(Test-Path -LiteralPath (Join-Path $root 'source/desktop/windows/src/main.rs')) -or
    !([IO.File]::ReadAllText((Join-Path $root 'source/src/startup.rs')).Contains('cn.lgxng.wallpaper.rehearsal.'))) {
    throw 'Use the isolated executable created by rehearse-windows-update.mjs'
}
$profile = New-Item -ItemType Directory (Join-Path $root ('resident-'+[guid]::NewGuid().ToString('N')))
$data = New-Item -ItemType Directory (Join-Path $profile 'BingWallpaperLib')
Set-Content (Join-Path $data 'settings.json') '{"background_resident_enabled":true}' -Encoding utf8NoBOM
Set-Content (Join-Path $data 'update-region') 'international' -Encoding utf8NoBOM
$oldProfile = $env:BING_WALLPAPER_TEST_PROFILE
$first = $null
function Wait-State($predicate, $failure) {
    $deadline = (Get-Date).AddSeconds(25)
    do {
        $first.Refresh()
        if ($first.HasExited) { throw 'The background instance exited' }
        if (& $predicate) { return }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    throw $failure
}
function Wake-Instance {
    $second = Start-Process -FilePath $exe -PassThru -WindowStyle Normal
    try {
        if (!$second.WaitForExit(12000)) { throw 'A second instance remained running' }
        Wait-State { $first.MainWindowHandle -ne 0 } 'Second launch did not restore the first window'
    } finally {
        if (!$second.HasExited) { $second.Kill(); $second.WaitForExit() }
        $second.Dispose()
    }
}
try {
    $env:BING_WALLPAPER_TEST_PROFILE = $profile.FullName
    $first = Start-Process -FilePath $exe -ArgumentList '--background' -PassThru -WindowStyle Normal
    $ready = Join-Path $data 'nsh-installed-path'
    Wait-State { (Test-Path -LiteralPath $ready) -and $first.MainWindowHandle -eq 0 } 'Background start did not hide the window'
    Wake-Instance
    if (!$first.CloseMainWindow()) { throw 'The window did not accept its close request' }
    Wait-State { $first.MainWindowHandle -eq 0 } 'Closing the resident window did not hide it'
    Wake-Instance
    'PASS · background start, single-instance activation, close to tray and restore'
} finally {
    $env:BING_WALLPAPER_TEST_PROFILE = $oldProfile
    if ($first) {
        if (!$first.HasExited) { $first.Kill(); $first.WaitForExit() }
        $first.Dispose()
    }
}
