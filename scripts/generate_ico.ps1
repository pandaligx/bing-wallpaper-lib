param(
    [string]$SourcePath = "ico/icon.ico",
    [string]$OutputPath = "ico/icon.ico",
    [int[]]$Sizes = @(16, 24, 32, 48, 64, 72, 96, 128, 256)
)

Add-Type -AssemblyName System.Drawing

# 从原始 ico 中加载一张最大的位图作为重采样源（原图为 256x256）。
# 用 MemoryStream 加载，避免 Image.FromFile 对源文件保持句柄占用（当 Source == Output 时会冲突）。
$sourceBytes = [System.IO.File]::ReadAllBytes((Resolve-Path $SourcePath))
$sourceStream = New-Object System.IO.MemoryStream (, $sourceBytes)
$sourceImage = [System.Drawing.Image]::FromStream($sourceStream)

$entries = @()
$imageDataList = @()

foreach ($size in $Sizes) {
    $bitmap = New-Object System.Drawing.Bitmap $size, $size
    $bitmap.SetResolution(96, 96)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.Clear([System.Drawing.Color]::Transparent)
    $graphics.DrawImage($sourceImage, 0, 0, $size, $size)
    $graphics.Dispose()

    $memoryStream = New-Object System.IO.MemoryStream
    $bitmap.Save($memoryStream, [System.Drawing.Imaging.ImageFormat]::Png)
    $pngBytes = $memoryStream.ToArray()
    $memoryStream.Dispose()
    $bitmap.Dispose()

    $imageDataList += , $pngBytes
    $entries += [PSCustomObject]@{
        Size  = $size
        Bytes = $pngBytes.Length
    }
}

$sourceImage.Dispose()
$sourceStream.Dispose()

# 组装 ICO 文件：ICONDIR(6字节) + N * ICONDIRENTRY(16字节) + 各尺寸 PNG 数据。
$headerSize = 6
$entrySize = 16
$dataOffset = $headerSize + ($entrySize * $Sizes.Count)

$outStream = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter $outStream

# ICONDIR
$writer.Write([UInt16]0)      # reserved
$writer.Write([UInt16]1)      # type = 1 (icon)
$writer.Write([UInt16]$Sizes.Count)

$currentOffset = $dataOffset
for ($i = 0; $i -lt $Sizes.Count; $i++) {
    $size = $Sizes[$i]
    $pngBytes = $imageDataList[$i]
    $byteSize = if ($size -ge 256) { 0 } else { $size }

    $writer.Write([Byte]$byteSize)    # width
    $writer.Write([Byte]$byteSize)    # height
    $writer.Write([Byte]0)            # color count
    $writer.Write([Byte]0)            # reserved
    $writer.Write([UInt16]1)          # planes
    $writer.Write([UInt16]32)         # bit count
    $writer.Write([UInt32]$pngBytes.Length)
    $writer.Write([UInt32]$currentOffset)

    $currentOffset += $pngBytes.Length
}

foreach ($pngBytes in $imageDataList) {
    $writer.Write($pngBytes)
}

$writer.Flush()
[System.IO.File]::WriteAllBytes((Join-Path (Get-Location) $OutputPath), $outStream.ToArray())
$writer.Dispose()
$outStream.Dispose()

Write-Output "Generated multi-resolution ICO:"
$entries | Format-Table -AutoSize
