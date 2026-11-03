# Thin wrapper: build tools/bench-smb in release mode, then run it.
#   powershell -File scripts/bench-smb.ps1 \\nas\photos --runs 3
# All arguments are passed through. The report goes to stdout (redirect it to a
# file to keep it); cargo's own chatter goes to stderr.
$ErrorActionPreference = 'Stop'
$crate = Join-Path $PSScriptRoot '..\tools\bench-smb'
$manifest = Join-Path $crate 'Cargo.toml'

cargo build --release --quiet --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& (Join-Path $crate 'target\release\bench-smb.exe') @args
exit $LASTEXITCODE
