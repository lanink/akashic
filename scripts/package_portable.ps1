# Akashic portable package (Windows)
# Usage: powershell -ExecutionPolicy Bypass -File scripts/package_portable.ps1
# Output: dist/akashic-0.1.0-win-x86_64.zip
# Requires: Rust toolchain, NASM (imageflow c-codecs), git submodule initialized.

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Version = "0.1.0"
$Exe = Join-Path $Root "target\release\akashic.exe"
$OutDir = Join-Path $Root "dist"
$Stage = Join-Path $OutDir "akashic-$Version-win-x86_64"
$Zip = Join-Path $OutDir "akashic-$Version-win-x86_64.zip"

Write-Host "==> cargo build --release"
cargo build --release --manifest-path (Join-Path $Root "Cargo.toml")
if (-not (Test-Path $Exe)) { throw "release binary not found: $Exe" }

Write-Host "==> staging $Stage"
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item $Exe (Join-Path $Stage "akashic.exe")
foreach ($f in @("readme.md", "CHANGELOG.md")) {
    $src = Join-Path $Root $f
    if (Test-Path $src) { Copy-Item $src $Stage }
}

$License = @'
Akashic 0.1.0 - personal, local-only photo organizer.

Bundled third-party code:
- imageflow (imageflow_core / imageflow_types), AGPLv3, fixed commit 7cbe26be.
  Source: https://github.com/imazen/imageflow
  This build is for personal local use only. Redistribution requires
  complying with AGPLv3 or obtaining a commercial license exception.

All other dependencies are crates.io Rust crates with their own licenses
(see Cargo.lock / cargo tree). No photo or metadata is uploaded anywhere;
the app works fully offline.

Data directory: %LOCALAPPDATA%\akashic (index.sqlite3, cache, trash, log).
'@
Set-Content -Path (Join-Path $Stage "LICENSE-NOTICE.txt") -Value $License -Encoding UTF8

Write-Host "==> compressing $Zip"
if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path $Stage -DestinationPath $Zip -CompressionLevel Optimal
$SizeMb = (Get-Item $Zip).Length / 1MB
Write-Host ("==> done: {0} ({1:N1} MB)" -f $Zip, $SizeMb)
