<#
  Builds an installable, signed ARM64 release APK without requiring Windows
  Developer Mode.

  Tauri normally creates a JNI symlink on Windows. This script lets Tauri
  compile Rust into an ASCII-only target directory, copies the library, strips
  debug symbols, and asks Gradle to package the APK.
#>

[CmdletBinding()]
param(
  [string]$ToolRoot = (Join-Path $env:LOCALAPPDATA "unigate-android-tools")
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

# Магазины не принимают debug-сборки, а Android не даёт обновить APK, подписанный
# другим сертификатом, поэтому ключ обязателен: без него сборка обязана упасть, а
# не выпустить несовместимый пакет.
$keystore = $env:UNIGATE_ANDROID_KEYSTORE
if ([string]::IsNullOrWhiteSpace($keystore)) {
  throw "UNIGATE_ANDROID_KEYSTORE is not set (path to the release .jks)"
}
$keystore = (Resolve-Path -LiteralPath $keystore).Path
if ([string]::IsNullOrWhiteSpace($env:UNIGATE_ANDROID_KEYSTORE_PASSWORD)) {
  throw "UNIGATE_ANDROID_KEYSTORE_PASSWORD is not set"
}
$keyAlias = $env:UNIGATE_ANDROID_KEY_ALIAS
if ([string]::IsNullOrWhiteSpace($keyAlias)) { $keyAlias = "unigate" }
$sdkDir = Join-Path $ToolRoot "android-sdk"
$ndkDir = Join-Path $sdkDir "ndk\28.0.13004108"
$targetDir = Join-Path $env:LOCALAPPDATA "unigate-android-target"
$aar = Join-Path $projectRoot "src-tauri\gen\android\app\libs\libbox.aar"
$androidIcons = Join-Path $projectRoot "src-tauri\icons\android"
$androidResources = Join-Path $projectRoot "src-tauri\gen\android\app\src\main\res"
$geoipSource = Join-Path $projectRoot "src-tauri\binaries\geoip-ru.srs"
$androidAssets = Join-Path $projectRoot "src-tauri\gen\android\app\src\main\assets"

$awgSourceDir = Join-Path $projectRoot "awg-shim"
$libboxNeedsBuild = -not (Test-Path -LiteralPath $aar)
if (-not $libboxNeedsBuild) {
  $libboxTime = (Get-Item -LiteralPath $aar).LastWriteTimeUtc
  $libboxInputs = @(
    Get-Item -LiteralPath (Join-Path $PSScriptRoot "build-libbox-android.ps1")
    Get-ChildItem -LiteralPath $awgSourceDir -Recurse -File |
      Where-Object { $_.Extension -eq ".go" -or $_.Name -in @("go.mod", "go.sum") }
  )
  $libboxNeedsBuild = $libboxInputs |
    Where-Object { $_.LastWriteTimeUtc -gt $libboxTime } |
    Select-Object -First 1
}
if ($libboxNeedsBuild) {
  & (Join-Path $PSScriptRoot "build-libbox-android.ps1") -ToolRoot $ToolRoot
}

# `tauri android init` starts with the generic Tauri launcher. Keep Android
# launcher/notification artwork synchronized with the desktop UniGate icon.
Get-ChildItem -LiteralPath $androidIcons -Directory | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination $androidResources -Recurse -Force
}
if (-not (Test-Path -LiteralPath $geoipSource)) {
  throw "geoip-ru.srs is missing. Run the platform fetch script first."
}
New-Item -ItemType Directory -Force -Path $androidAssets | Out-Null
Copy-Item -LiteralPath $geoipSource -Destination (Join-Path $androidAssets "geoip-ru.srs") -Force

$env:JAVA_HOME = Join-Path $ToolRoot "jdk-17"
$env:ANDROID_HOME = $sdkDir
$env:ANDROID_SDK_ROOT = $sdkDir
$env:NDK_HOME = $ndkDir
$env:CARGO_TARGET_DIR = $targetDir

# Свойства через ORG_GRADLE_PROJECT_* передаёт клиент Gradle, поэтому они доходят
# до сборки даже при уже запущенном демоне с другим окружением.
$env:ORG_GRADLE_PROJECT_unigateKeystore = $keystore
$env:ORG_GRADLE_PROJECT_unigateKeystorePassword = $env:UNIGATE_ANDROID_KEYSTORE_PASSWORD
$env:ORG_GRADLE_PROJECT_unigateKeyAlias = $keyAlias
if (-not [string]::IsNullOrWhiteSpace($env:UNIGATE_ANDROID_KEY_PASSWORD)) {
  $env:ORG_GRADLE_PROJECT_unigateKeyPassword = $env:UNIGATE_ANDROID_KEY_PASSWORD
}

$buildStarted = Get-Date
Push-Location $projectRoot
try {
  # A non-zero result is expected when Windows disallows Tauri's final symlink.
  & npm.cmd run tauri -- android build --target aarch64
  $tauriExit = $LASTEXITCODE

  $rustLibrary = Join-Path $targetDir "aarch64-linux-android\release\libunigate_lib.so"
  if (-not (Test-Path -LiteralPath $rustLibrary)) {
    throw "Rust Android library was not produced (Tauri exit code $tauriExit)"
  }
  if ((Get-Item -LiteralPath $rustLibrary).LastWriteTime -lt $buildStarted) {
    throw "Rust Android library is stale (Tauri exit code $tauriExit)"
  }

  $jniLibrary = Join-Path $projectRoot "src-tauri\gen\android\app\src\main\jniLibs\arm64-v8a\libunigate_lib.so"
  New-Item -ItemType Directory -Force -Path (Split-Path $jniLibrary) | Out-Null
  $jniIsCurrent = (Test-Path -LiteralPath $jniLibrary) -and
    ((Get-FileHash -Algorithm SHA256 -LiteralPath $rustLibrary).Hash -eq
      (Get-FileHash -Algorithm SHA256 -LiteralPath $jniLibrary).Hash)
  if ($jniIsCurrent) {
    Write-Host "JNI library already points to the Rust build; skipping copy."
  } else {
    Copy-Item -LiteralPath $rustLibrary -Destination $jniLibrary -Force
  }

  $strip = Join-Path $ndkDir "toolchains\llvm\prebuilt\windows-x86_64\bin\llvm-strip.exe"
  & $strip --strip-unneeded $jniLibrary
  if ($LASTEXITCODE -ne 0) { throw "Failed to strip Rust debug symbols" }

  Push-Location (Join-Path $projectRoot "src-tauri\gen\android")
  try {
    & .\gradlew.bat :app:assembleArm64Release -x :app:rustBuildArm64Release --rerun-tasks
    if ($LASTEXITCODE -ne 0) { throw "Gradle APK build failed" }
  } finally {
    Pop-Location
  }
} finally {
  Pop-Location
}

$apk = Join-Path $projectRoot "src-tauri\gen\android\app\build\outputs\apk\arm64\release\app-arm64-release.apk"
if (-not (Test-Path -LiteralPath $apk)) {
  # Без подключённого signingConfig Gradle кладёт рядом *-unsigned.apk.
  throw "Signed APK was not produced (check the signing configuration)"
}
$version = (Get-Content -LiteralPath (Join-Path $projectRoot "package.json") -Raw | ConvertFrom-Json).version
$releaseDir = Join-Path $projectRoot "dist\android"
$releaseApk = Join-Path $releaseDir "UniGate_${version}_android_arm64.apk"
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
Copy-Item -LiteralPath $apk -Destination $releaseApk -Force
Write-Host "APK ready: $releaseApk"
