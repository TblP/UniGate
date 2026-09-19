# UniGate

[Русский](README.md) · **English**

[![Release](https://img.shields.io/github/v/release/TblP/UniGate?style=flat-square&color=1f6feb)](https://github.com/TblP/UniGate/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/TblP/UniGate/total?style=flat-square&color=2ea043&logo=github&logoColor=white)](https://github.com/TblP/UniGate/releases)
[![Stars](https://img.shields.io/github/stars/TblP/UniGate?style=flat-square&color=e3b341)](https://github.com/TblP/UniGate/stargazers)
[![Build](https://img.shields.io/github/actions/workflow/status/TblP/UniGate/build-installers.yml?style=flat-square&label=build)](https://github.com/TblP/UniGate/actions/workflows/build-installers.yml)
[![License](https://img.shields.io/github/license/TblP/UniGate?style=flat-square&color=8957e5)](LICENSE)

![Windows](https://img.shields.io/badge/Windows-0078D4?style=flat-square&logo=windows&logoColor=white)
![macOS](https://img.shields.io/badge/macOS-000000?style=flat-square&logo=apple&logoColor=white)
![Android](https://img.shields.io/badge/Android_ARM64-3DDC84?style=flat-square&logo=android&logoColor=white)

![Tauri](https://img.shields.io/badge/Tauri_2-24C8DB?style=flat-square&logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white)
![React](https://img.shields.io/badge/React_19-61DAFB?style=flat-square&logo=react&logoColor=black)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=flat-square&logo=typescript&logoColor=white)
![Vite](https://img.shields.io/badge/Vite-646CFF?style=flat-square&logo=vite&logoColor=white)
![sing-box](https://img.shields.io/badge/sing--box-F5A623?style=flat-square)

**UniGate** is a modern open-source client for **self-hosted** VPN, proxy and tunnel connections, all behind a single interface.

Running your own servers usually means keeping a separate application for every connection type. UniGate replaces them with one control centre that works the same way regardless of protocol.

Under the hood there is a single network engine — [**sing-box**](https://sing-box.sagernet.org/) (plus a dedicated engine for AmneziaWG): UniGate stores the profiles, generates the config, launches the engine and reads its statistics. The client itself is native and lightweight — **Tauri 2** (Rust) + **React**, a ~15 MB binary with minimal resource usage.

> **Status:** UniGate is available for Windows, macOS and Android ARM64. TUN mode, RU/LAN bypass and per-app split tunneling work on all three platforms.

## Features

* 🚀 Modern interface with dark and light themes
* 🔌 Proxy mode (no administrator rights required) and full **TUN VPN** (all OS traffic)
* 🌐 Multiple connection profiles
* 🔗 Import from a share link, from JSON, or from a `vpn://` container (Amnezia)
* 🧭 **Split tunneling:**
  * by region (RU traffic goes direct, bypassing the VPN) and by local network (LAN)
  * by application — selected apps through the VPN, or selected apps direct (pick an `.exe` on Windows, an `.app` on macOS)
* 📤 Profile export: share link (`vless://` and others) or JSON (sing-box outbound)
* 📊 Live speed and traffic statistics
* 🖥️ System tray, minimise to tray, autostart, auto-connect
* 📱 Android: quick actions, a quick-settings tile and 1×1 / 2×2 widgets
* 🔒 No telemetry
* 🔓 Fully open source (MIT)

## Supported protocols

* Hysteria 2
* AmneziaWG 1.5 / 3.1 *(Windows/macOS/Android — userspace awg-shim + sing-box; desktop keeps a legacy fallback)*
* SOCKS5
* HTTP / HTTPS
* Shadowsocks
* VMess
* VLESS *(including Reality)*
* Trojan
* TUIC

Proxy protocols work both in local proxy mode and in TUN mode. They are added by hand (SOCKS/HTTP/Hysteria2) or by importing a link or a subscription (everything else).

## Stack

| Layer | Technology |
|-------|-----------|
| GUI shell | Tauri 2 (Rust) |
| Frontend | React + TypeScript + Vite (Zustand) |
| Network core | sing-box (sidecar on desktop, libbox on Android) |
| AmneziaWG | awg-shim + amneziawg-go/v3 (Windows, macOS, Android) |

## Prebuilt binaries

Installers for Windows (MSI + NSIS `setup.exe`), macOS (`.dmg`, Apple Silicon) and an APK for Android ARM64.

> The Windows and macOS installers are **not signed yet** — see [Code signing policy](#code-signing-policy). macOS notarization is not planned. The Android build is signed with the project's release key.
>
> **macOS:** after installing, clear the quarantine flag from the whole bundle — this also clears the bundled `sing-box` and AmneziaWG binaries:
> ```bash
> sudo xattr -dr com.apple.quarantine /Applications/UniGate.app
> ```
>
> **Windows:**
> - In the **SmartScreen** dialog choose "More info → Run anyway".
> - On Windows 11 the launch may also be blocked by **Smart App Control**, which rejects any program without a digital signature. Since UniGate has none, turn that feature off manually in Windows Security (*App & browser control* → *Smart App Control*).
>
> **Android:**
> - Allow your browser or file manager to **install unknown apps**.
> - **From 1.2.0** the build is a release build signed with the project's own key, so the system no longer flags it as unsafe.
> - The signing certificate changed along with it, so the APK will not install over a version older than 1.2.0: remove that one first, saving your profiles through Share beforehand.

## Building and running locally

You need **Node 22+** and **Rust** (Windows: stable-msvc + Microsoft C++ Build Tools + WebView2; macOS: Xcode Command Line Tools).

```bash
npm install                      # frontend dependencies

# core binaries are not kept in git — fetch them with a script:
pwsh scripts/fetch-singbox.ps1   # Windows: sing-box + wintun + amneziawg + geoip
bash scripts/fetch-singbox.sh    # macOS/Linux: sing-box + geoip
bash scripts/fetch-awg-macos.sh  # macOS: awg-shim + legacy AmneziaWG fallback (needs Go)

# Android ARM64 (Windows-hosted portable toolchain):
pwsh scripts/setup-android.ps1
pwsh scripts/fetch-android-assets.ps1
pwsh scripts/build-android.ps1   # dist/android/UniGate_<version>_android_arm64.apk

npm run tauri dev                # run in dev mode
npm run tauri build              # build (Windows → MSI + NSIS)
```

## Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

> **Status:** the SignPath Foundation application is under review. Until the certificate is issued, Windows release installers ship without a digital signature — see the warnings under "Prebuilt binaries".

**Project roles**

- Committers and reviewers: [TblP](https://github.com/TblP)
- Approvers: [TblP](https://github.com/TblP)

## Privacy

No telemetry, and no data leaves the device: [privacy policy](PRIVACY.md) · [data subject rights](PRIVACY_RIGHTS.md).

## License

[MIT](LICENSE) © TblP
