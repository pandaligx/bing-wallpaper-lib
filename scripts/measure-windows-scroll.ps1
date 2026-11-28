[CmdletBinding()]
param([Parameter(Mandatory)][string]$Exe, [Parameter(Mandatory)][string]$OutputDir,
      [ValidateSet('notches','precision')][string]$InputMode = 'notches',
      [ValidateRange(1,60)][int]$NotchesPerSecond = 24,
      [ValidateRange(30,1000)][int]$PrecisionEventsPerSecond = 150, [switch]$NoFrameMonitor, [string]$PresentMon)
$ErrorActionPreference = 'Stop'
$binary = (Resolve-Path -LiteralPath $Exe).Path
$out = (New-Item -ItemType Directory -Force $OutputDir).FullName
$sampler = Join-Path $out 'visible-frames.exe'
$source = Join-Path $PSScriptRoot 'windows-visible-frames.cpp'
if (!(Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $finder = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $vs = & $finder -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (!$vs) { throw 'The pixel sampler needs the Visual C++ desktop tools.' }
    Import-Module (Join-Path $vs 'Common7/Tools/Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
}
& cl.exe /nologo /EHsc /std:c++17 /O2 /MT "/Fo$out/visible-frames.obj" "/Fe$sampler" $source d3d11.lib dxgi.lib user32.lib
if ($LASTEXITCODE) { throw 'Cannot build the pixel sampler' }
$settingsPath = Join-Path $env:LOCALAPPDATA 'BingWallpaperLib/settings.json'
$settings = if (Test-Path $settingsPath) { Get-Content -Raw $settingsPath | ConvertFrom-Json }
$startup = [Microsoft.Win32.Registry]::GetValue('HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run','BingWallpaperLib',$null)
if ($startup -or $settings.startup_enabled -or $settings.auto_wallpaper_enabled -or $settings.periodic_task_enabled) {
    throw 'Run the portable benchmark with a test user that has no active wallpaper schedules.'
}
Add-Type @'
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class WallpaperScrollInput {
    [StructLayout(LayoutKind.Sequential)] struct Point { public int x,y; }
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref Point pt);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateWaitableTimerEx(IntPtr security, string name, uint flags, uint access);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool SetWaitableTimer(IntPtr timer, ref long due, int period, IntPtr callback, IntPtr arg, bool resume);
    [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr handle, uint timeout);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    public static void Run(IntPtr hwnd, bool precision, int rate) {
        Point pt = new Point {x=680,y=420};
        if (!ClientToScreen(hwnd,ref pt)) throw new Exception("Window missing");
        var location = new IntPtr((pt.y << 16) | (pt.x & 65535));
        // A default Windows Sleep(1) can bunch 150 nominal events into just 64 bursts/s.
        IntPtr timer = CreateWaitableTimerEx(IntPtr.Zero, null, 2, 0x00100002);
        if (timer == IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
        try {
            var clock = Stopwatch.StartNew();
            for (int i=0;i<12*rate;i++) {
                int delta = (i / (2*rate) % 2 == 0 ? -1 : 1) * (precision ? 12 : 120);
                if (!PostMessage(hwnd,0x020a,new IntPtr(delta << 16),location)) throw new Exception("Wheel delivery failed");
                long remaining = (long)((i+1)*10000000.0/rate) - clock.Elapsed.Ticks;
                if (remaining > 0) {
                    long due = -remaining;
                    if (!SetWaitableTimer(timer, ref due, 0, IntPtr.Zero, IntPtr.Zero, false) ||
                        WaitForSingleObject(timer, 1000) != 0) throw new Exception("Wheel pacing timer failed");
                }
            }
        } finally { CloseHandle(timer); }
    }
}
'@
$log = Join-Path $out 'frames.log'
$launch = @{ FilePath = $binary; WindowStyle = 'Normal'; RedirectStandardError = $log; PassThru = $true }
if (!$NoFrameMonitor) { $launch.ArgumentList = '--fps' }
$window = Start-Process @launch
try {
    $limit = [DateTime]::UtcNow.AddSeconds(45)
    do {
        Start-Sleep -Milliseconds 250
        $window.Refresh()
        if ($window.HasExited) { throw 'Test window exited. Close existing wallpaper windows before measuring.' }
    } until (($window.MainWindowHandle -ne 0 -and ($NoFrameMonitor -or (Get-Content $log -ErrorAction SilentlyContinue | Select-String '^fps '))) -or [DateTime]::UtcNow -ge $limit)
    if ($window.MainWindowHandle -eq 0) { throw 'Native window was unavailable' }
    Start-Sleep -Seconds 4
    $samplerInfo = [Diagnostics.ProcessStartInfo]::new($sampler, $window.MainWindowHandle.ToInt64().ToString())
    $samplerInfo.UseShellExecute = $false
    $samplerInfo.CreateNoWindow = $true
    $samplerInfo.RedirectStandardOutput = $true
    $observer = [Diagnostics.Process]::Start($samplerInfo)
    if ($observer.StandardOutput.ReadLine() -ne 'ready') { throw 'Cannot observe the benchmark window' }
    $initial = @(Get-Content $log).Count
    $inputRate = if ($InputMode -eq 'precision') { $PrecisionEventsPerSecond } else { $NotchesPerSecond }
    if ($PresentMon) {
        $capture = Start-Process -FilePath (Resolve-Path $PresentMon) -ArgumentList @("--process_id", $window.Id, "--timed", 13, "--terminate_after_timed", "--output_file", ('"'+(Join-Path $out "presents.csv")+'"')) -WindowStyle Hidden -RedirectStandardError (Join-Path $out "presentmon.log") -RedirectStandardOutput (Join-Path $out "presentmon.stdout") -PassThru
    }
    [WallpaperScrollInput]::Run($window.MainWindowHandle, $InputMode -eq 'precision', $inputRate)
    $samples = @($observer.StandardOutput.ReadToEnd().Trim() -split '\s+')
    $visible = @($samples | Select-Object -First 12 | ForEach-Object { [int]$_ })
    $intervals = @($samples | Select-Object -Skip 12 | ForEach-Object { [double]::Parse($_, [CultureInfo]::InvariantCulture) } | Sort-Object)
    $observer.WaitForExit()
    if ($capture) { $capture.WaitForExit(20000) | Out-Null; if ($capture.ExitCode -ne 0) { throw "PresentMon capture failed" } }
    if ($observer.ExitCode -ne 0 -or $visible.Count -ne 12) { throw 'Incomplete visible-frame measurement' }
    $frames = @(Get-Content $log | Select-Object -Skip ($initial + 1) | Where-Object { $_ -match '^fps ' })
    if (!$NoFrameMonitor -and $frames.Count -lt 8) { throw "Too few native frame samples. See $log" }
    $rates = @($frames | ForEach-Object { if ($_ -match '^fps (\d+)/(\d+)') { [int]$Matches[1] } } | Sort-Object)
    $result = [pscustomobject]@{ exe = $binary; inputMode = $InputMode; frameMonitor = !$NoFrameMonitor; displayHz = $null; medianFps = $null; sampleFps = $rates; samples = $frames }
    if (!$NoFrameMonitor) {
        $result.displayHz = [int]($frames[0].Split(' ')[1].Split('/')[1])
        $result.medianFps = $rates[[int][Math]::Floor($rates.Count/2)]
    }
    if ($intervals.Count) {
        foreach ($p in 50,95,99) { $result | Add-Member NoteProperty ("visibleFrameTimeP"+$p+"Ms") $intervals[[int][Math]::Ceiling($p*$intervals.Count/100)-1] }
        $result | Add-Member NoteProperty visibleFrameIntervals $intervals.Count
    }
    $result | Add-Member NoteProperty visibleFps $visible
    $result | Add-Member NoteProperty inputEventsPerSecond $inputRate
    $result | Add-Member NoteProperty medianVisibleFps (($visible[1..10] | Sort-Object)[4..5] | Measure-Object -Average).Average
    $result | ConvertTo-Json | Set-Content (Join-Path $out 'scroll.json') -Encoding utf8
    $result | Select-Object displayHz,medianFps,medianVisibleFps,visibleFps
} finally {
    if ($observer) { if (!$observer.HasExited) { $observer.Kill(); $observer.WaitForExit() }; $observer.Dispose() }
    if (!$window.HasExited) { $window.Kill(); $window.WaitForExit() }
    $window.Dispose()
}
