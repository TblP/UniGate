<#
  Builds the pinned sing-box Android libbox AAR for ARM64.

  Prerequisite:
    powershell -ExecutionPolicy Bypass -File scripts/setup-android.ps1
#>

[CmdletBinding()]
param(
  [string]$ToolRoot = (Join-Path $env:LOCALAPPDATA "unigate-android-tools")
)

$ErrorActionPreference = "Stop"
$version = "1.13.14"
$sourceDir = Join-Path $ToolRoot "sing-box-$version"
$goExe = Join-Path $ToolRoot "go\bin\go.exe"
$goPath = Join-Path $ToolRoot "gopath"
$goBin = Join-Path $goPath "bin"
$sdkDir = Join-Path $ToolRoot "android-sdk"
$ndkDir = Join-Path $sdkDir "ndk\28.0.13004108"
$destination = Join-Path $PSScriptRoot "..\src-tauri\gen\android\app\libs\libbox.aar"

if (-not (Test-Path -LiteralPath $goExe)) {
  throw "Go not found. Run scripts/setup-android.ps1 first."
}
if (-not (Test-Path -LiteralPath $ndkDir)) {
  throw "Android NDK not found. Run scripts/setup-android.ps1 first."
}

if (-not (Test-Path -LiteralPath (Join-Path $sourceDir ".git"))) {
  git clone --branch "v$version" --depth 1 https://github.com/SagerNet/sing-box.git $sourceDir
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to download sing-box v$version"
  }
}

New-Item -ItemType Directory -Force -Path $goBin | Out-Null
$env:GOPATH = $goPath
$env:GOBIN = $goBin
$env:JAVA_HOME = Join-Path $ToolRoot "jdk-17"
$env:ANDROID_HOME = $sdkDir
$env:ANDROID_SDK_ROOT = $sdkDir
$env:ANDROID_NDK_HOME = $ndkDir
$env:NDK_HOME = $ndkDir
$env:Path = "$(Join-Path $ToolRoot 'go\bin');$goBin;$(Join-Path $env:JAVA_HOME 'bin');$env:Path"

if (-not (Test-Path -LiteralPath (Join-Path $goBin "gomobile.exe"))) {
  & $goExe install github.com/sagernet/gomobile/cmd/gomobile@v0.1.12
  if ($LASTEXITCODE -ne 0) { throw "Failed to install gomobile" }
}
if (-not (Test-Path -LiteralPath (Join-Path $goBin "gobind.exe"))) {
  & $goExe install github.com/sagernet/gomobile/cmd/gobind@v0.1.12
  if ($LASTEXITCODE -ne 0) { throw "Failed to install gobind" }
}

Push-Location $sourceDir
try {
  & $goExe run ./cmd/internal/build_libbox -target android -platform android/arm64
  if ($LASTEXITCODE -ne 0) { throw "libbox build failed" }
} finally {
  Pop-Location
}

$aar = Join-Path $sourceDir "libbox.aar"
if (-not (Test-Path -LiteralPath $aar)) {
  throw "libbox.aar was not produced"
}
New-Item -ItemType Directory -Force -Path (Split-Path $destination) | Out-Null
Copy-Item -LiteralPath $aar -Destination $destination -Force
Write-Host "libbox.aar ready: $destination"
