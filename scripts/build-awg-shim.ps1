# Собирает awg-shim (userspace AmneziaWG -> локальный SOCKS5) в
# src-tauri/binaries/awg-shim-x86_64-pc-windows-msvc.exe (Tauri externalBin).
#
# Нужен Go. Если его нет ни в PATH, ни в %LOCALAPPDATA%\unigate-tools\go —
# скрипт сам скачает портативный Go туда (без установки, без админа).

param(
  [string]$GoVersion = "1.26.4"
)

$ErrorActionPreference = "Stop"

# --- находим (или ставим) Go ---
$go = $null
$cmd = Get-Command go -ErrorAction SilentlyContinue
if ($cmd) {
  $go = $cmd.Source
} else {
  $portable = Join-Path $env:LOCALAPPDATA "unigate-tools\go\bin\go.exe"
  if (-not (Test-Path $portable)) {
    $tools = Join-Path $env:LOCALAPPDATA "unigate-tools"
    New-Item -ItemType Directory -Force $tools | Out-Null
    $zip = Join-Path $tools "go$GoVersion.zip"
    Write-Host "Go не найден — скачиваю портативный go$GoVersion в $tools"
    Invoke-WebRequest -Uri "https://go.dev/dl/go$GoVersion.windows-amd64.zip" -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $tools -Force
    Remove-Item $zip
  }
  $go = $portable
}

# --- сборка ---
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root "src-tauri\binaries\awg-shim-x86_64-pc-windows-msvc.exe"
New-Item -ItemType Directory -Force (Split-Path $out -Parent) | Out-Null

Push-Location (Join-Path $root "awg-shim")
try {
  $env:CGO_ENABLED = "0"
  $env:GOOS = "windows"
  $env:GOARCH = "amd64"
  & $go build -trimpath -ldflags "-s -w" -o $out .
  if ($LASTEXITCODE -ne 0) { Write-Error "go build failed ($LASTEXITCODE)" }
  Write-Host "Готово: $out"
} finally {
  Pop-Location
}
