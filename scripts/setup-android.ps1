<#
  Installs the portable Android build toolchain used by UniGate.

  Nothing is installed system-wide. Tools are stored in:
    %LOCALAPPDATA%\unigate-android-tools

  Downloads:
    - Eclipse Temurin JDK 17
    - Android command-line tools / SDK / NDK
    - Go (required to build sing-box libbox.aar)
#>

[CmdletBinding()]
param(
  [string]$ToolRoot = (Join-Path $env:LOCALAPPDATA "unigate-android-tools")
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$androidToolsVersion = "15859902"
$androidToolsSha256 = "90ae805d20434428bffcb699c290860f19bb5f66a67e6b330067e3de801fb04a"
$goVersion = "1.26.3"
$goSha256 = "20d2ceafb4ed41b96b879010927b28bc92a5be57a7c1801ce365a9ca51d3224a"

$downloadDir = Join-Path $ToolRoot "downloads"
$jdkDir = Join-Path $ToolRoot "jdk-17"
$sdkDir = Join-Path $ToolRoot "android-sdk"
$goDir = Join-Path $ToolRoot "go"

New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null

function Get-CheckedArchive {
  param(
    [Parameter(Mandatory = $true)][string]$Uri,
    [Parameter(Mandatory = $true)][string]$Destination,
    [string]$Sha256
  )

  if (-not (Test-Path -LiteralPath $Destination)) {
    Write-Host "Downloading $Uri"
    Invoke-WebRequest -Uri $Uri -OutFile $Destination
  }

  if ($Sha256) {
    $actual = (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Sha256) {
      throw "Checksum mismatch for $Destination (expected $Sha256, got $actual)"
    }
  }
}

if (-not (Test-Path -LiteralPath (Join-Path $jdkDir "bin\java.exe"))) {
  $jdkArchive = Join-Path $downloadDir "temurin-jdk17.zip"
  Get-CheckedArchive `
    -Uri "https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jdk/hotspot/normal/eclipse" `
    -Destination $jdkArchive

  $jdkExtract = Join-Path $ToolRoot "jdk-extract"
  if (Test-Path -LiteralPath $jdkExtract) {
    Remove-Item -LiteralPath $jdkExtract -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $jdkExtract | Out-Null
  Expand-Archive -LiteralPath $jdkArchive -DestinationPath $jdkExtract -Force
  $jdkSource = Get-ChildItem -LiteralPath $jdkExtract -Directory | Select-Object -First 1
  if (-not $jdkSource) {
    throw "JDK archive has no root directory"
  }
  Move-Item -LiteralPath $jdkSource.FullName -Destination $jdkDir
  Remove-Item -LiteralPath $jdkExtract -Recurse -Force
}

if (-not (Test-Path -LiteralPath (Join-Path $sdkDir "cmdline-tools\latest\bin\sdkmanager.bat"))) {
  $androidArchive = Join-Path $downloadDir "commandlinetools-win-$androidToolsVersion.zip"
  Get-CheckedArchive `
    -Uri "https://dl.google.com/android/repository/commandlinetools-win-${androidToolsVersion}_latest.zip" `
    -Destination $androidArchive `
    -Sha256 $androidToolsSha256

  $androidExtract = Join-Path $ToolRoot "android-extract"
  if (Test-Path -LiteralPath $androidExtract) {
    Remove-Item -LiteralPath $androidExtract -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $androidExtract | Out-Null
  Expand-Archive -LiteralPath $androidArchive -DestinationPath $androidExtract -Force
  $latestDir = Join-Path $sdkDir "cmdline-tools\latest"
  New-Item -ItemType Directory -Force -Path (Split-Path $latestDir) | Out-Null
  Move-Item -LiteralPath (Join-Path $androidExtract "cmdline-tools") -Destination $latestDir
  Remove-Item -LiteralPath $androidExtract -Recurse -Force
}

if (-not (Test-Path -LiteralPath (Join-Path $goDir "bin\go.exe"))) {
  $goArchive = Join-Path $downloadDir "go$goVersion.windows-amd64.zip"
  Get-CheckedArchive `
    -Uri "https://go.dev/dl/go$goVersion.windows-amd64.zip" `
    -Destination $goArchive `
    -Sha256 $goSha256

  $goExtract = Join-Path $ToolRoot "go-extract"
  if (Test-Path -LiteralPath $goExtract) {
    Remove-Item -LiteralPath $goExtract -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $goExtract | Out-Null
  Expand-Archive -LiteralPath $goArchive -DestinationPath $goExtract -Force
  Move-Item -LiteralPath (Join-Path $goExtract "go") -Destination $goDir
  Remove-Item -LiteralPath $goExtract -Recurse -Force
}

$env:JAVA_HOME = $jdkDir
$env:ANDROID_HOME = $sdkDir
$env:ANDROID_SDK_ROOT = $sdkDir
$env:Path = "$(Join-Path $jdkDir 'bin');$(Join-Path $sdkDir 'platform-tools');$(Join-Path $sdkDir 'cmdline-tools\latest\bin');$(Join-Path $goDir 'bin');$env:Path"

$sdkManager = Join-Path $sdkDir "cmdline-tools\latest\bin\sdkmanager.bat"
Write-Host "Accepting Android SDK licenses"
1..20 | ForEach-Object { "y" } | & $sdkManager --sdk_root=$sdkDir --licenses | Out-Host

Write-Host "Installing Android SDK packages"
& $sdkManager --sdk_root=$sdkDir `
  "platform-tools" `
  "platforms;android-36" `
  "build-tools;36.0.0" `
  "ndk;28.0.13004108"

$ndkDir = Join-Path $sdkDir "ndk\28.0.13004108"
[Environment]::SetEnvironmentVariable("JAVA_HOME", $jdkDir, "User")
[Environment]::SetEnvironmentVariable("ANDROID_HOME", $sdkDir, "User")
[Environment]::SetEnvironmentVariable("ANDROID_SDK_ROOT", $sdkDir, "User")
[Environment]::SetEnvironmentVariable("NDK_HOME", $ndkDir, "User")

rustup target add `
  aarch64-linux-android `
  armv7-linux-androideabi `
  i686-linux-android `
  x86_64-linux-android

Write-Host ""
Write-Host "Android toolchain is ready:"
& (Join-Path $jdkDir "bin\java.exe") -version
& (Join-Path $sdkDir "platform-tools\adb.exe") version
& (Join-Path $goDir "bin\go.exe") version
rustup target list --installed
