[CmdletBinding()]
param([Parameter(Mandatory)][string]$Exe,[switch]$Offline)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$root=Join-Path (Split-Path $PSScriptRoot -Parent) 'target/nsh-validation'
$directory=(New-Item -ItemType Directory -Path (Join-Path $root ('portable-'+[guid]::NewGuid().ToString('N')))).FullName
$binary=Join-Path $directory 'BingWallpaperLib.exe'
Copy-Item -LiteralPath $Exe -Destination $binary
$rule=$null
$app=$null
try {
    if($Offline){
      $rule=New-NetFirewallRule -DisplayName ('BingWallpaperLib isolated test '+[guid]::NewGuid()) -Direction Outbound -Program $binary -Action Block -Profile Any
    }
    $app=Start-Process -FilePath $binary -WorkingDirectory $directory -PassThru -WindowStyle Normal
    $limit=[DateTime]::UtcNow.AddSeconds(50)
    do {
      Start-Sleep -Milliseconds 350
      $app.Refresh()
      if($app.HasExited){throw "Portable app stopped with $($app.ExitCode)"}
      if($app.MainWindowHandle -eq 0){continue}
      $window=[System.Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
      $texts=@($window.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object {$_.Current.Name})
      $dates=@($texts | Where-Object {$_ -match '\d{4}-\d{2}-\d{2}'})
      if($dates.Count -ge 2){break}
    } while ([DateTime]::UtcNow -lt $limit)
    if($dates.Count -lt 2){throw 'The portable window did not expose dated photos'}
    if(@(Get-ChildItem -LiteralPath $directory -File).Count -ne 1){throw 'Extra files next to portable executable'}
    [pscustomobject]@{OS=[Environment]::OSVersion.Version.ToString();Version=$app.MainModule.FileVersionInfo.ProductVersion;Offline=[bool]$Offline;Files=1;ExampleDates=@($dates|Select-Object -First 4);SHA256=(Get-FileHash $binary).Hash}
} finally {
    if($app -and !$app.HasExited){
      [void]$app.CloseMainWindow()
      if(!$app.WaitForExit(3500)){$app.Kill();$app.WaitForExit()}
    }
    if($rule){Remove-NetFirewallRule -Name $rule.Name}
}
