$ErrorActionPreference = "Stop"
cargo build --release --locked
$executable = Join-Path (Get-Location) "target/release/voxel-engine.exe"
if (-not (Test-Path -LiteralPath $executable)) {
    throw "Release executable was not produced: $executable"
}
Write-Host "Ready: $executable"
