# Regenerates every icon size from the one source, app/icons/logo.svg, with
# the Tauri CLI (it rasterizes SVG itself; same CLI version as package.ps1).
# Commits keep only what Windows/macOS packaging and the web favicon use.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $env:TEMP 'photosee-icons'
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
Push-Location (Join-Path $root 'app')
try { npx --yes @tauri-apps/cli@2.12.4 icon icons/logo.svg -o $out } finally { Pop-Location }
foreach ($f in 'icon.ico', 'icon.png', 'icon.icns', '32x32.png', '128x128.png', '128x128@2x.png') {
  Copy-Item (Join-Path $out $f) (Join-Path $root "app\icons\$f") -Force
}
Copy-Item (Join-Path $root 'app\icons\logo.svg') (Join-Path $root 'web\public\favicon.svg') -Force
Write-Host 'icons: app/icons + web/public/favicon.svg updated'
