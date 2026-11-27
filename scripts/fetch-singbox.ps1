<#
.SYNOPSIS
  Скачивает закреплённую версию sing-box и кладёт её как Tauri sidecar
  в src-tauri/binaries/sing-box-<target-triple>.exe (Windows x64).

  Бинарник не хранится в git (см. .gitignore). Запусти этот скрипт после клонирования:
    pwsh scripts/fetch-singbox.ps1

  macOS/Linux-варианты появятся вместе с поддержкой этих ОС (Phase 10).
#>
param(
  [string]$Version = "1.14.4",
  [string]$Triple  = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
$root    = Split-Path -Parent $PSScriptRoot
$binDir  = Join-Path $root "src-tauri\binaries"
$dest    = Join-Path $binDir "sing-box-$Triple.exe"
$cronetDest = Join-Path $binDir "libcronet.dll"

# Патченный sidecar с CONNECT-UDP (RFC 9298) поверх HTTP/3. В апстримном
# релизе SagerNet этого нет, поэтому берём сборку из форка TblP/sing-box
# (ветка unigate/udp-masque). Когда выйдет sing-box 1.15 с MASQUE в апстриме,
# эти ассеты и штамп можно убрать, вернувшись на релизы SagerNet.
# Обновляя sidecar: собери новый zip, залей в релиз, поменяй Tag/Asset/Sha256.
$distTag      = "singbox-1.14.4-masque.2"
$distAsset    = "sing-box-$Version-masque.2-windows-amd64.zip"
$distSha256   = "AA5E0B2C57E10A5989C0551C5036549BA0FCDA032E1B8BDF40A2607128675DBB"
$distUrl      = "https://github.com/TblP/sing-box/releases/download/$distTag/$distAsset"
$distStamp    = Join-Path $binDir ".singbox-windows-dist"

$downloadSingBox = $true
if ((Test-Path $dest) -and (Test-Path $cronetDest) -and (Test-Path $distStamp)) {
  $installed = (Get-Content -LiteralPath $distStamp -Raw).Trim()
  $existing = & $dest version 2>$null | Select-Object -First 1
  Write-Host "sing-box уже на месте: $existing (dist: $installed)"
  if ($existing -eq "sing-box version $Version" -and $installed -eq $distTag) {
    $downloadSingBox = $false
  } else {
    Write-Host "Дистрибутив отличается от $distTag — перекачиваю."
  }
}

if ($downloadSingBox) {
  $tmp   = Join-Path $env:TEMP $distAsset
  $ext   = Join-Path $env:TEMP "singbox_ext_$distTag"
  $actualSha256 = $null

  Write-Host "Скачиваю $distUrl"
  Invoke-WebRequest -Uri $distUrl -OutFile $tmp -Headers @{ "User-Agent" = "UniGate" }

  $actualSha256 = (Get-FileHash -LiteralPath $tmp -Algorithm SHA256).Hash
  if ($actualSha256 -ne $distSha256) {
    Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
    throw "sing-box zip SHA256 mismatch: expected $distSha256, got $actualSha256"
  }

  if (Test-Path $ext) { Remove-Item $ext -Recurse -Force }
  Expand-Archive -Path $tmp -DestinationPath $ext -Force

  $exe = Get-ChildItem -Path $ext -Recurse -Filter "sing-box.exe" | Select-Object -First 1
  if (-not $exe) { throw "sing-box.exe не найден в архиве" }
  $cronet = Join-Path $exe.DirectoryName "libcronet.dll"
  if (-not (Test-Path -LiteralPath $cronet)) { throw "libcronet.dll не найдена в архиве sing-box" }

  New-Item -ItemType Directory -Force -Path $binDir | Out-Null
  Copy-Item $exe.FullName $dest -Force
  Copy-Item -LiteralPath $cronet -Destination $cronetDest -Force
  Set-Content -LiteralPath $distStamp -Value $distTag -NoNewline
  Remove-Item $tmp, $ext -Recurse -Force

  Write-Host "Готово: $dest"
  & $dest version | Select-Object -First 1
}

& (Join-Path $PSScriptRoot "fetch-xray.ps1")

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
$awgVersion = "3.1.0"
$awgDest = Join-Path $binDir "amneziawg-x86_64-pc-windows-msvc.exe"
$awgStamp = Join-Path $binDir ".amneziawg-windows-version"
$installedAwgVersion = if (Test-Path $awgStamp) { (Get-Content -LiteralPath $awgStamp -Raw).Trim() } else { "" }
if ((Test-Path $awgDest) -and $installedAwgVersion -eq $awgVersion) {
  Write-Host "amneziawg.exe $awgVersion уже на месте"
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
  Set-Content -LiteralPath $awgStamp -Value $awgVersion -NoNewline
  Remove-Item $awgMsi, $awgExt -Recurse -Force
  Write-Host "Готово: $awgDest"
}

# --- geoip-ru.srs (RU-обход в split-tunneling, Phase 7b) ---
$geoipDest = Join-Path $binDir "geoip-ru.srs"
$geoipTmp = "$geoipDest.tmp"
Write-Host "Обновляю geoip-ru.srs (SagerNet rule-set)"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-ru.srs" -OutFile $geoipTmp -Headers @{ "User-Agent" = "UniGate" }
$geoipBytes = [System.IO.File]::ReadAllBytes($geoipTmp)
if ($geoipBytes.Length -lt 10000 -or
    $geoipBytes[0] -ne 0x53 -or $geoipBytes[1] -ne 0x52 -or
    $geoipBytes[2] -ne 0x53 -or $geoipBytes[3] -ne 0x01) {
  Remove-Item -LiteralPath $geoipTmp -Force -ErrorAction SilentlyContinue
  throw "Загруженный geoip-ru.srs не прошёл проверку"
}
Move-Item -LiteralPath $geoipTmp -Destination $geoipDest -Force
Write-Host "Готово: $geoipDest"

# --- awg-shim (userspace AmneziaWG -> SOCKS5; собирается из awg-shim/ на Go) ---
& (Join-Path $PSScriptRoot "build-awg-shim.ps1")
