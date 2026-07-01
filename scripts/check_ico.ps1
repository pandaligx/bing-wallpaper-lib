param(
    [string]$Path = "ico/icon.ico"
)

$bytes = [System.IO.File]::ReadAllBytes($Path)
$count = [BitConverter]::ToUInt16($bytes, 4)
Write-Output ("Image count: " + $count)
for ($i = 0; $i -lt $count; $i++) {
    $offset = 6 + ($i * 16)
    $w = $bytes[$offset]
    $h = $bytes[$offset + 1]
    if ($w -eq 0) { $w = 256 }
    if ($h -eq 0) { $h = 256 }
    $bpp = [BitConverter]::ToUInt16($bytes, $offset + 6)
    Write-Output ("Size: " + $w + "x" + $h + " bpp=" + $bpp)
}
