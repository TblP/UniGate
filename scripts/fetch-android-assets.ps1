[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$binDir = Join-Path $projectRoot "src-tauri\binaries"
$geoip = Join-Path $binDir "geoip-ru.srs"

New-Item -ItemType Directory -Force -Path $binDir | Out-Null
if (-not (Test-Path -LiteralPath $geoip)) {
  Write-Host "Downloading geoip-ru.srs for Android split routing"
  Invoke-WebRequest `
    -Uri "https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-ru.srs" `
    -OutFile $geoip `
    -Headers @{ "User-Agent" = "UniGate" }
}

Write-Host "Android assets ready: $geoip"
