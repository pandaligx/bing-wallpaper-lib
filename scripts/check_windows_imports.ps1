param(
    [string]$ExePath = (Join-Path $PSScriptRoot '..\target\x86_64-pc-windows-msvc\release\bing-wallpaper-lib.exe')
)

$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $ExePath).Path
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installation) { throw 'Visual Studio C++ Build Tools not found.' }
$toolset = Get-ChildItem -LiteralPath (Join-Path $installation 'VC\Tools\MSVC') -Directory |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
$dumpbin = Join-Path $toolset.FullName 'bin\Hostx64\x64\dumpbin.exe'
$imports = & $dumpbin /nologo /imports $exe
if ($LASTEXITCODE -ne 0) { throw 'dumpbin could not read the executable imports.' }
$dlls = @($imports | Where-Object { $_ -match '^\s+\S+\.dll\s*$' } | ForEach-Object { $_.Trim() } | Sort-Object -Unique)
if (-not $dlls.Count) { throw 'No DLL imports found; cannot verify executable.' }
$forbidden = @($dlls | Where-Object { $_ -match '^(icu.*|vcruntime.*|msvcp.*)\.dll$' })
if ($forbidden.Count) { throw "Unexpected runtime DLL dependencies: $($forbidden -join ', ')" }
# These functions are resolved with GetProcAddress and must never be static imports.
$newDpiImports = @($imports | Where-Object { $_ -match '\s(GetDpiForWindow|GetSystemMetricsForDpi)\s*$' })
if ($newDpiImports.Count) { throw "Windows 10 1607 DPI functions are still imported directly: $newDpiImports" }
Write-Output "PASS: no ICU/VC runtime DLLs or static Windows 10 1607 DPI helpers in $exe"
Write-Output ($dlls -join ', ')
