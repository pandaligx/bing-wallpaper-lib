[CmdletBinding()]
param([Parameter(Mandatory)][string]$Engine,[Parameter(Mandatory)][string]$Aria,[string]$Month='2026-02')
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$enginePath=(Resolve-Path $Engine).Path
$ariaPath=(Resolve-Path $Aria).Path
if ($Month -notmatch '^\d{4}-\d{2}$'){throw 'Use YYYY-MM'}
$rows=@((Get-Content -Raw -Encoding utf8 (Join-Path $root 'assets/data/zxyongyo-bing-wallpaper.json')|ConvertFrom-Json).images|Where-Object {$_.date.StartsWith($Month)}|Sort-Object date)
if(!$rows.Count){throw 'No entries for this month'}
$work=New-Item -ItemType Directory -Path (Join-Path (Join-Path $root 'target/nsh-validation') ('month-'+[guid]::NewGuid().ToString('N')))
$urls=Join-Path $work.FullName 'urls.json'
[IO.File]::WriteAllText($urls,($rows.url_4k|ConvertTo-Json -Compress))
$baseline=$null
$report=@()
for ($round=1;$round -le 5;$round++){
  foreach ($mode in 'single','multi','aria2'){
    $dir=(New-Item -ItemType Directory (Join-Path $work.FullName "$round-$mode")).FullName
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    if ($mode -eq 'aria2'){
      $inputPath=Join-Path $dir 'aria2.txt'
      $lines=for($i=0;$i -lt $rows.Count;$i++){ $rows[$i].url_4k; "  out=day-$($i.ToString('00'))" }
      [IO.File]::WriteAllLines($inputPath,[string[]]$lines)
      $start.FileName=$ariaPath
      $argsList=@('--no-conf','--summary-interval=0','--console-log-level=warn','--download-result=hide','-j4','-x16','-s16','-k1M',"--dir=$dir","--input-file=$inputPath")
    }else{
      $start.FileName=$enginePath
      $argsList=@("@$urls",$dir,$(if($mode -eq 'single'){'1'}else{'16'}))
    }
    foreach($v in $argsList){$start.ArgumentList.Add($v)}
    $clock=[Diagnostics.Stopwatch]::StartNew()
    $process=[Diagnostics.Process]::Start($start)
    $peak=0L
    while(!$process.WaitForExit(50)){ $process.Refresh();$peak=[Math]::Max($peak,$process.PeakWorkingSet64) }
    $clock.Stop()
    if($process.ExitCode -ne 0){throw "$mode failed in round $round (exit $($process.ExitCode))"}
    $hashes=@(Get-ChildItem -LiteralPath $dir -Filter 'day-*' -File|Sort-Object Name|ForEach-Object {(Get-FileHash -LiteralPath $_.FullName).Hash})
    if($hashes.Count -ne $rows.Count){throw 'Incomplete month'}
    if($null -eq $baseline){$baseline=$hashes}elseif(($hashes -join ',') -ne ($baseline -join ',')){throw 'Month digest mismatch'}
    $bytes=(Get-ChildItem -LiteralPath $dir -Filter 'day-*' -File|Measure-Object Length -Sum).Sum
    $report+=[ordered]@{round=$round;mode=$mode;ms=$clock.Elapsed.TotalMilliseconds;peakWorkingSetBytes=$peak;bytes=$bytes;files=$rows.Count}
    $result=[ordered]@{month=$Month;dates=@($rows.date);sha256ByDate=$baseline;rounds=$report}
    [IO.File]::WriteAllText((Join-Path $work.FullName 'report.json'),($result|ConvertTo-Json -Depth 5))
    Write-Output "$mode $round $([math]::Round($clock.Elapsed.TotalSeconds,2)) s · $([math]::Round($peak/1MB,1)) MiB"
  }
}
Write-Output (Join-Path $work.FullName 'report.json')
