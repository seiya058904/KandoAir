# SPDX-FileCopyrightText: KandoAir contributors
# SPDX-License-Identifier: MIT

<#
.SYNOPSIS
  Builds KandoAir and assembles the portable folder dist\KandoAir.

.DESCRIPTION
  1. Builds the web frontend (menu and settings pages) into dist\web.
  2. Builds the native host in release mode.
  3. Copies KandoAir.exe and the frontend into dist\KandoAir:

       dist\KandoAir\KandoAir.exe
       dist\KandoAir\resources\web\...

.PARAMETER SkipWeb
  Reuse an existing dist\web instead of rebuilding the frontend.
#>
param([switch]$SkipWeb)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $SkipWeb) {
  Write-Host '==> Building the web frontend'
  npm run build:web
  if ($LASTEXITCODE -ne 0) { throw 'Web build failed.' }
}

Write-Host '==> Building the native host'
cargo build --release --manifest-path host/Cargo.toml
if ($LASTEXITCODE -ne 0) { throw 'Host build failed.' }

$out = Join-Path $root 'dist\KandoAir'
$web = Join-Path $root 'dist\web'
$targetWeb = Join-Path $out 'resources\web'
New-Item -ItemType Directory -Force $targetWeb | Out-Null

# Mirror dist\web into the package: copy all files, then remove files which are no longer
# part of the build one by one (no recursive deletion).
$sourceFiles = @{}
Get-ChildItem $web -Recurse -File | ForEach-Object {
  $relative = $_.FullName.Substring($web.Length + 1)
  $sourceFiles[$relative] = $true
  $target = Join-Path $targetWeb $relative
  New-Item -ItemType Directory -Force (Split-Path -Parent $target) | Out-Null
  Copy-Item $_.FullName $target -Force
}
Get-ChildItem $targetWeb -Recurse -File | ForEach-Object {
  $relative = $_.FullName.Substring($targetWeb.Length + 1)
  if (-not $sourceFiles.ContainsKey($relative)) {
    Remove-Item -LiteralPath $_.FullName
  }
}

Copy-Item (Join-Path $root 'host\target\release\KandoAir.exe') $out -Force
Copy-Item (Join-Path $root 'LICENSE.md') $out -Force
New-Item -ItemType Directory -Force (Join-Path $out 'LICENSES') | Out-Null
Get-ChildItem (Join-Path $root 'LICENSES') -File | ForEach-Object {
  Copy-Item $_.FullName (Join-Path $out 'LICENSES') -Force
}

$size = (Get-ChildItem $out -Recurse -File | Measure-Object Length -Sum).Sum / 1MB
Write-Host ('==> Done: {0} ({1:N1} MB)' -f $out, $size)
