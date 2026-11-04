# Builds the Windows installer (NSIS, per-user) and the portable zip, then
# prints both sizes. Usage: powershell -File scripts/package.ps1
# Output: target\release\bundle\nsis\PhotoSee_<ver>_x64-setup.exe
#         target\release\bundle\PhotoSee-portable.zip
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# Pinned: latest 2.x CLI when this was written; the `tauri` crate is 2.12.5
# (CLI 2.12.5 does not exist on npm, the CLI and crate are versioned apart).
$cli = '@tauri-apps/cli@2.12.4'

function Step($name, [scriptblock]$cmd) {
  Write-Host "== $name" -ForegroundColor Cyan
  & $cmd
  if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $name" -ForegroundColor Red; exit 1 }
}

# The extension list lives in two places (Rust and the NSIS hook, which
# cannot read Rust). Fail loudly rather than ship an installer that registers
# the wrong set of types.
function Test-ExtensionSync {
  $rs = Get-Content (Join-Path $root 'core\src\format.rs') -Raw
  $body = [regex]::Match($rs, '(?s)pub fn from_extension.*?_ => return None').Value
  $fromRust = [regex]::Matches($body, '"([a-z0-9]+)"') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
  $nsh = Get-Content (Join-Path $root 'app\nsis\hooks.nsh') -Raw
  $fromNsh = [regex]::Matches($nsh, 'PS_ASSOC "([a-z0-9]+)"') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
  $diff = Compare-Object $fromRust $fromNsh
  if ($fromRust.Count -eq 0 -or $diff) {
    Write-Host "FAILED: app/nsis/hooks.nsh and core/src/format.rs from_extension disagree:" -ForegroundColor Red
    $diff | Format-Table | Out-String | Write-Host
    exit 1
  }
  Write-Host "extensions in sync: $($fromRust.Count)"
}

Write-Host '== extension list' -ForegroundColor Cyan
Test-ExtensionSync

Push-Location web
try {
  if (-not (Test-Path node_modules)) { Step 'npm ci' { npm ci } }
  Step 'web build' { npm run build }
} finally { Pop-Location }

# Runs cargo build --release (with custom-protocol) itself. Cwd must be app/
# so the CLI finds tauri.conf.json.
Push-Location app
try { Step 'tauri build (nsis)' { npx --yes $cli build --bundles nsis } } finally { Pop-Location }

$bundle = Join-Path $root 'target\release\bundle'
$setup = Get-ChildItem (Join-Path $bundle 'nsis') -Filter '*-setup.exe' |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) { Write-Host 'FAILED: no installer produced' -ForegroundColor Red; exit 1 }

# Portable = photosee.exe + web\ + the `portable` marker.
$stage = Join-Path $bundle 'portable'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$dir = Join-Path $stage 'PhotoSee'
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item (Join-Path $root 'target\release\photosee.exe') $dir
Copy-Item -Recurse (Join-Path $root 'web\dist') (Join-Path $dir 'web')
# The marker makes it "green": settings, thumbnails and the WebView2 profile
# go to PhotoSee\data\, never to %LOCALAPPDATA% (app/src/serve.rs).
Set-Content -Encoding ascii -Path (Join-Path $dir 'portable') -Value 'PhotoSee portable: data lives in .\data'
$zip = Join-Path $bundle 'PhotoSee-portable.zip'
if (Test-Path $zip) { Remove-Item -Force $zip }
# bsdtar (shipped with Windows) writes forward-slash entry names;
# Compress-Archive in Windows PowerShell 5.1 can write backslashes.
$bsdtar = Join-Path $env:SystemRoot 'System32\tar.exe'  # not Git's GNU tar on PATH
Step 'zip' { & $bsdtar -a -c -f $zip -C $stage PhotoSee }
Remove-Item -Recurse -Force $stage

Write-Host ('installer: {0} ({1:N2} MiB)' -f $setup.FullName, ($setup.Length / 1MB))
Write-Host ('zip:       {0} ({1:N2} MiB)' -f $zip, ((Get-Item $zip).Length / 1MB))
