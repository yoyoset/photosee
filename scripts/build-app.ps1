# Builds the desktop app: web UI, then the release exe, then web/dist next to
# the exe (the app looks for `web\` beside itself; see app/src/serve.rs).
# No installer yet (Wave 3). Usage: powershell -File scripts/build-app.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Step($name, [scriptblock]$cmd) {
  Write-Host "== $name" -ForegroundColor Cyan
  & $cmd
  if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $name" -ForegroundColor Red; exit 1 }
}

Push-Location web
try {
  if (-not (Test-Path node_modules)) { Step 'npm ci' { npm ci } }
  Step 'web build' { npm run build }
} finally { Pop-Location }

Step 'cargo build --release' { cargo build -p photosee-app --release }

$exe = Join-Path $root 'target\release\photosee.exe'
$web = Join-Path (Split-Path $exe) 'web'
# Replace, never merge: a stale hashed asset from an older build must not stay.
if (Test-Path $web) { Remove-Item -Recurse -Force $web }
Copy-Item -Recurse (Join-Path $root 'web\dist') $web

$mib = [math]::Round((Get-Item $exe).Length / 1MB, 2)
Write-Host "exe: $exe ($mib MiB)"
Write-Host "web: $web"
