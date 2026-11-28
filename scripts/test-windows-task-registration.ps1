[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$id = [guid]::NewGuid().ToString('N')
$run = New-Item -ItemType Directory -Path (Join-Path $root "target/nsh-validation/task-registration-$id")
$source = Join-Path $run 'source'
$manifestDir = Join-Path $source 'desktop/ext/wallpaper-native'
$profile = Join-Path $run 'profile'
$periodic = "BingWallpaperLib-Test-$id-Periodic"
$daily = "BingWallpaperLib-Test-$id-Daily"

# Build a private copy of the real registrar with unique Task Scheduler names.
foreach ($path in 'src','assets/data','desktop/icon.png','desktop/ext/wallpaper-native/src','desktop/ext/wallpaper-native/Cargo.toml','desktop/ext/wallpaper-native/Cargo.lock') {
    $to = Join-Path $source $path
    New-Item -ItemType Directory -Force (Split-Path $to) | Out-Null
    Copy-Item -LiteralPath (Join-Path $root $path) -Destination $to -Recurse
}
foreach ($change in @(
    @{ File='src/task_scheduler.rs'; Old='BingWallpaperLibPeriodicWallpaper'; New=$periodic },
    @{ File='desktop/ext/wallpaper-native/src/registration.rs'; Old='BingWallpaperLibDailyWallpaper'; New=$daily }
)) {
    $file = Join-Path $source $change.File
    $text = [IO.File]::ReadAllText($file)
    if (-not $text.Contains($change.Old)) { throw "Task name missing from $file" }
    [IO.File]::WriteAllText($file, $text.Replace($change.Old, $change.New))
}
Add-Content -LiteralPath (Join-Path $manifestDir 'src/lib.rs') -Encoding utf8 -Value @'
pub fn exercise_scheduler(input: &str) -> anyhow::Result<()> {
    if input == "repair" {
        return registration::repair_portable_tasks();
    }
    registration::configure(input)?;
    Ok(())
}
'@
Set-Content -LiteralPath (Join-Path $manifestDir 'src/bin/task-registration-check.rs') -Encoding utf8NoBOM -Value @'
fn main() -> anyhow::Result<()> {
    let input = std::env::args().nth(1).expect("test input");
    match input.as_str() {
        "--daily-wallpaper" | "--scheduled-wallpaper" => {
            let exe = std::env::current_exe()?;
            std::fs::write(exe.with_extension(format!("{}.ran", &input[2..])), &input)?;
            Ok(())
        }
        _ => wallpaper_native::exercise_scheduler(&input),
    }
}
'@
$target = Join-Path $root 'target/nsh-cargo'
& cargo build --locked --manifest-path (Join-Path $manifestDir 'Cargo.toml') --target-dir $target --bin task-registration-check
if ($LASTEXITCODE -ne 0) { throw 'Task test build failed' }
$old = Join-Path (New-Item -ItemType Directory (Join-Path $run 'before')) 'BingWallpaperLib.exe'
$new = Join-Path (New-Item -ItemType Directory (Join-Path $run 'after')) 'BingWallpaperLib.exe'
Copy-Item -LiteralPath (Join-Path $target 'debug/task-registration-check.exe') -Destination $old
Copy-Item -LiteralPath $old -Destination $new
function Invoke-Registrar($program, $settings) {
    & $program $settings
    if ($LASTEXITCODE -ne 0) { throw "Task registration failed at $program" }
}
function Check-Actions($program) {
    foreach ($item in @(@{ Name=$daily; Arg='--daily-wallpaper' },@{ Name=$periodic; Arg='--scheduled-wallpaper' })) {
        $task = Get-ScheduledTask -TaskName $item.Name
        if ($task.Actions.Execute -ne $program -or $task.Actions.Arguments -ne $item.Arg) {
            throw "Unexpected action for $($item.Name)"
        }
    }
}
$priorProfile = $env:BING_WALLPAPER_TEST_PROFILE
try {
    $env:BING_WALLPAPER_TEST_PROFILE = $profile
    $time = (Get-Date).AddHours(2)
    Invoke-Registrar $old (@{auto_wallpaper_enabled=$true; auto_wallpaper_hour=$time.Hour; auto_wallpaper_minute=$time.Minute} | ConvertTo-Json -Compress)
    Invoke-Registrar $old '{"periodic_task_enabled":true,"periodic_interval_minutes":1439}'
    Check-Actions $old
    $dailyTask = Get-ScheduledTask -TaskName $daily
    $repetition = (Get-ScheduledTask -TaskName $periodic).Triggers.Repetition.Interval
    if ([Xml.XmlConvert]::ToTimeSpan($repetition).TotalMinutes -ne 1439) { throw 'Incorrect repetition' }
    if (([datetime]$dailyTask.Triggers.StartBoundary).ToString('HH:mm') -ne $time.ToString('HH:mm') -or -not $dailyTask.Settings.StartWhenAvailable) { throw 'Incorrect daily trigger' }

    Invoke-Registrar $new 'repair'
    Check-Actions $new
    Start-ScheduledTask -TaskName $daily
    Start-ScheduledTask -TaskName $periodic
    $deadline = (Get-Date).AddSeconds(40)
    foreach ($mode in 'daily-wallpaper','scheduled-wallpaper') {
        $receipt = [IO.Path]::ChangeExtension($new, "$mode.ran")
        while (!(Test-Path -LiteralPath $receipt) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
        if ([IO.File]::ReadAllText($receipt) -ne "--$mode") { throw "Wrong action receipt: $mode" }
    }
    $report = [ordered]@{ dailyTime=$time.ToString('HH:mm'); intervalMinutes=1439; catchUp=$true; relocated=$true; fired=@('--daily-wallpaper','--scheduled-wallpaper') }
    $report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $run 'result.json') -Encoding utf8NoBOM
    Write-Output "PASS · Windows Task Scheduler, daily and periodic triggers, executable relocation and both headless arguments · $run"
} finally {
    $env:BING_WALLPAPER_TEST_PROFILE = $priorProfile
    foreach ($name in @($daily,$periodic)) {
        if (Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue) { Unregister-ScheduledTask -TaskName $name -Confirm:$false }
    }
}
