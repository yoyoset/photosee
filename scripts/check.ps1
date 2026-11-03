# Full local gate. Everything must pass before a package is handed in and
# before anything merges to main. Usage:
#   powershell -File scripts/check.ps1              # whole repo
#   powershell -File scripts/check.ps1 -Rust        # Rust only (core/server workers)
#   powershell -File scripts/check.ps1 -Web         # web only
param([switch]$Rust, [switch]$Web)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$all = -not ($Rust -or $Web)

function Step($name, [scriptblock]$cmd) {
  Write-Host "== $name" -ForegroundColor Cyan
  & $cmd
  if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $name" -ForegroundColor Red; exit 1 }
}

if ($all -or $Rust) {
  Step 'cargo fmt' { cargo fmt --all -- --check }
  Step 'cargo clippy' { cargo clippy --workspace --all-targets -- -D warnings }
  Step 'cargo test' { cargo test --workspace }
}
if ($all -or $Web) {
  Push-Location web
  try {
    Step 'tsc' { npx tsc --noEmit }
    Step 'eslint' { npx eslint . }
    Step 'vitest' { npx vitest run }
    Step 'vite build' { npx vite build --logLevel warn }
  } finally { Pop-Location }
}
Step 'gates' { node scripts/gates.mjs }
Write-Host 'check: all green' -ForegroundColor Green
