$ErrorActionPreference = "Stop"
$version = "26.11.9"
$archiveHash = "1262522266337b824f07beb8e1375786b437683b3890105469088f8e17e0a69d"
$binDir = Join-Path (Split-Path -Parent $PSScriptRoot) "src-tauri\binaries"
$dest = Join-Path $binDir "xray-x86_64-pc-windows-msvc.exe"
if (Test-Path -LiteralPath $dest) {
  $existing = & $dest version 2>$null | Select-Object -First 1
  if ($existing -like "Xray $version *") { return }
}

$stage = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("unigate-xray-" + [Guid]::NewGuid()))
try {
  $zip = Join-Path $stage.FullName "xray.zip"
  Invoke-WebRequest -Uri "https://github.com/XTLS/Xray-core/releases/download/v$version/Xray-windows-64.zip" -OutFile $zip
  if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash -ne $archiveHash) {
    throw "Контрольная сумма архива Xray не совпала"
  }
  Expand-Archive -LiteralPath $zip -DestinationPath $stage.FullName
  $exe = Join-Path $stage.FullName "xray.exe"
  New-Item -ItemType Directory -Force -Path $binDir | Out-Null
  Copy-Item -LiteralPath $exe -Destination $dest -Force
} finally {
  # $stage — каталог с уникальным GUID, созданный выше этим же скриптом
  Remove-Item -LiteralPath $stage.FullName -Recurse -Force -ErrorAction SilentlyContinue
}
