<#
.SYNOPSIS
  Скачивает закреплённую версию sing-box и кладёт её как Tauri sidecar
  в src-tauri/binaries/sing-box-<target-triple>.exe (Windows x64).

  Бинарник не хранится в git (см. .gitignore). Запусти этот скрипт после клонирования:
    pwsh scripts/fetch-singbox.ps1

  macOS/Linux-варианты появятся вместе с поддержкой этих ОС (Phase 10).
#>
param(
  [string]$Version = "1.13.14",
  [string]$Triple  = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
$root    = Split-Path -Parent $PSScriptRoot
$binDir  = Join-Path $root "src-tauri\binaries"
$dest    = Join-Path $binDir "sing-box-$Triple.exe"

if (Test-Path $dest) {
  $existing = & $dest version 2>$null | Select-Object -First 1
  Write-Host "sing-box уже на месте: $existing"
  if ($existing -match [regex]::Escape($Version)) { return }
  Write-Host "Версия отличается от $Version — перекачиваю."
}

$asset = "sing-box-$Version-windows-amd64.zip"
$url   = "https://github.com/SagerNet/sing-box/releases/download/v$Version/$asset"
$tmp   = Join-Path $env:TEMP $asset
$ext   = Join-Path $env:TEMP "singbox_ext_$Version"

Write-Host "Скачиваю $url"
Invoke-WebRequest -Uri $url -OutFile $tmp -Headers @{ "User-Agent" = "UniGate" }

if (Test-Path $ext) { Remove-Item $ext -Recurse -Force }
Expand-Archive -Path $tmp -DestinationPath $ext -Force

$exe = Get-ChildItem -Path $ext -Recurse -Filter "sing-box.exe" | Select-Object -First 1
if (-not $exe) { throw "sing-box.exe не найден в архиве" }

New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Copy-Item $exe.FullName $dest -Force
Remove-Item $tmp, $ext -Recurse -Force

Write-Host "Готово: $dest"
& $dest version | Select-Object -First 1

# --- wintun.dll (нужен sing-box для TUN-режима на Windows) ---
$wintunVersion = "0.14.1"
$wintunDest = Join-Path $binDir "wintun.dll"
if (Test-Path $wintunDest) {
  Write-Host "wintun.dll уже на месте"
} else {
  $wtZip = Join-Path $env:TEMP "wintun.zip"
  $wtExt = Join-Path $env:TEMP "wintun_ext"
  Write-Host "Скачиваю wintun $wintunVersion"
  Invoke-WebRequest -Uri "https://www.wintun.net/builds/wintun-$wintunVersion.zip" -OutFile $wtZip -Headers @{ "User-Agent" = "UniGate" }
  if (Test-Path $wtExt) { Remove-Item $wtExt -Recurse -Force }
  Expand-Archive -Path $wtZip -DestinationPath $wtExt -Force
  $dll = Get-ChildItem -Path $wtExt -Recurse -Filter "wintun.dll" | Where-Object { $_.FullName -match "amd64" } | Select-Object -First 1
  if (-not $dll) { throw "wintun.dll (amd64) не найден в архиве" }
  Copy-Item $dll.FullName $wintunDest -Force
  Remove-Item $wtZip, $wtExt -Recurse -Force
  Write-Host "Готово: $wintunDest"
}

# --- amneziawg.exe + awg.exe (движок AmneziaWG, Phase 7c) ---
$awgVersion = "2.0.1"
$awgDest = Join-Path $binDir "amneziawg-x86_64-pc-windows-msvc.exe"
if (Test-Path $awgDest) {
  Write-Host "amneziawg.exe уже на месте"
} else {
  $awgMsi = Join-Path $env:TEMP "amneziawg.msi"
  $awgExt = Join-Path $env:TEMP "amneziawg_ext"
  Write-Host "Скачиваю amneziawg-windows-client $awgVersion (MSI)"
  Invoke-WebRequest -Uri "https://github.com/amnezia-vpn/amneziawg-windows-client/releases/download/$awgVersion/amneziawg-amd64-$awgVersion.msi" -OutFile $awgMsi -Headers @{ "User-Agent" = "UniGate" }
  if (Test-Path $awgExt) { Remove-Item $awgExt -Recurse -Force }
  New-Item -ItemType Directory -Force -Path $awgExt | Out-Null
  # административная распаковка MSI (без установки сервиса)
  Start-Process msiexec.exe -ArgumentList "/a `"$awgMsi`" /qn TARGETDIR=`"$awgExt`"" -Wait
  $awgExe = Get-ChildItem -Path $awgExt -Recurse -Filter "amneziawg.exe" | Select-Object -First 1
  $awgCli = Get-ChildItem -Path $awgExt -Recurse -Filter "awg.exe" | Select-Object -First 1
  if (-not $awgExe) { throw "amneziawg.exe не найден в MSI" }
  Copy-Item $awgExe.FullName $awgDest -Force
  if ($awgCli) { Copy-Item $awgCli.FullName (Join-Path $binDir "awg.exe") -Force }
  Remove-Item $awgMsi, $awgExt -Recurse -Force
  Write-Host "Готово: $awgDest"
}

# --- geoip-ru.srs (RU-обход в split-tunneling, Phase 7b) ---
$geoipDest = Join-Path $binDir "geoip-ru.srs"
if (Test-Path $geoipDest) {
  Write-Host "geoip-ru.srs уже на месте"
} else {
  Write-Host "Скачиваю geoip-ru.srs (SagerNet rule-set)"
  Invoke-WebRequest -Uri "https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-ru.srs" -OutFile $geoipDest -Headers @{ "User-Agent" = "UniGate" }
  Write-Host "Готово: $geoipDest"
}

# --- awg-shim (userspace AmneziaWG -> SOCKS5; собирается из awg-shim/ на Go) ---
& (Join-Path $PSScriptRoot "build-awg-shim.ps1")
