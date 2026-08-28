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
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$awgSourceDir = Join-Path $projectRoot "awg-shim"

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

New-Item -ItemType Directory -Force -Path (Split-Path $destination) | Out-Null
$workDir = Join-Path $env:TEMP "unigate-android-go-work-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $workDir | Out-Null
Push-Location $workDir
try {
  & $goExe work init $sourceDir $awgSourceDir
  if ($LASTEXITCODE -ne 0) { throw "Failed to create combined Go workspace" }
} finally {
  Pop-Location
}

$previousGoWork = [Environment]::GetEnvironmentVariable("GOWORK", "Process")
$env:GOWORK = Join-Path $workDir "go.work"
$tags = @(
  "with_gvisor", "with_quic", "with_wireguard", "with_utls",
  "with_naive_outbound", "with_clash_api", "badlinkname", "tfogo_checklinkname0",
  "with_tailscale", "ts_omit_logtail", "ts_omit_ssh", "ts_omit_drive",
  "ts_omit_taildrop", "ts_omit_webclient", "ts_omit_doctor", "ts_omit_capture",
  "ts_omit_kube", "ts_omit_aws", "ts_omit_synology", "ts_omit_bird"
) -join ","
$ldflags = "-X github.com/sagernet/sing-box/constant.Version=v$version -X internal/godebug.defaultGODEBUG=multipathtcp=0 -s -w -buildid= -checklinkname=0"

Push-Location $sourceDir
try {
  $bindArgs = @(
    "bind", "-v",
    "-o", $destination,
    "-target", "android/arm64",
    "-androidapi", "24",
    "-javapkg=io.nekohasekai",
    "-libname=box",
    "-trimpath",
    "-buildvcs=false",
    "-ldflags", $ldflags,
    "-tags", $tags,
    "./experimental/libbox",
    "unigate/awg-shim"
  )
  & (Join-Path $goBin "gomobile.exe") @bindArgs
  if ($LASTEXITCODE -ne 0) { throw "combined libbox + awg-shim build failed" }
} finally {
  Pop-Location
  if ([string]::IsNullOrEmpty($previousGoWork)) {
    Remove-Item Env:GOWORK -ErrorAction SilentlyContinue
  } else {
    $env:GOWORK = $previousGoWork
  }
  $resolvedWorkDir = [System.IO.Path]::GetFullPath($workDir)
  $resolvedTemp = [System.IO.Path]::GetFullPath($env:TEMP).TrimEnd('\') + '\'
  if ($resolvedWorkDir.StartsWith($resolvedTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
    Remove-Item -LiteralPath $resolvedWorkDir -Recurse -Force
  }
}

Write-Host "Combined libbox + awg-shim AAR ready: $destination"
