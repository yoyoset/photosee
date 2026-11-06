# Home-screen icons for the browser build (web/public), from app/icons/icon.png
# (512 px, made by make-icons.ps1). System.Drawing ships with Windows
# PowerShell, so this needs nothing installed.
# - icon-192.png / icon-512.png: the manifest's icons (Android), margin kept.
# - apple-touch-icon.png (180): iOS rounds the corners itself and shows
#   transparency as black, so the tile is cropped to its edge and laid on an
#   opaque blue taken from the logo's gradient.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
$src = [System.Drawing.Image]::FromFile((Join-Path $root 'app\icons\icon.png'))
function Save-Icon([int]$size, [string]$name, [System.Drawing.Rectangle]$from, $bg) {
  $bmp = New-Object System.Drawing.Bitmap $size, $size
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  if ($bg) { $g.Clear($bg) }
  $g.DrawImage($src, (New-Object System.Drawing.Rectangle 0, 0, $size, $size), $from, [System.Drawing.GraphicsUnit]::Pixel)
  $g.Dispose()
  $bmp.Save((Join-Path $root "web\public\$name"), [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}
$whole = New-Object System.Drawing.Rectangle 0, 0, $src.Width, $src.Height
# logo.svg: the tile spans 64..960 of 1024, i.e. 6.25% margin on each side.
$m = [int]($src.Width * 64 / 1024)
$tile = New-Object System.Drawing.Rectangle $m, $m, ($src.Width - 2 * $m), ($src.Height - 2 * $m)
Save-Icon 192 'icon-192.png' $whole $null
Save-Icon 512 'icon-512.png' $whole $null
Save-Icon 180 'apple-touch-icon.png' $tile ([System.Drawing.Color]::FromArgb(0x27, 0x74, 0xc2))
$src.Dispose()
Write-Host 'web icons: web/public/icon-192.png, icon-512.png, apple-touch-icon.png'
