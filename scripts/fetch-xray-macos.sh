#!/usr/bin/env bash
set -euo pipefail

version=26.11.9
root="$(cd "$(dirname "$0")/.." && pwd)"
case "$(uname -m)" in
  arm64)
    triple=aarch64-apple-darwin
    asset=Xray-macos-arm64-v8a.zip
    hash=233f0c66681cd0726f81a6b2b6ed6a5710869e312047b522e46ed53035bebd26
    ;;
  x86_64)
    triple=x86_64-apple-darwin
    asset=Xray-macos-64.zip
    hash=488963e676e06f5e48547f9dd38116aaca8de4234959b4272ae8f284e658dfd0
    ;;
  *) echo "Неподдерживаемая архитектура Xray для macOS" >&2; exit 1 ;;
esac

dest="$root/src-tauri/binaries/xray-$triple"
if [ -x "$dest" ]; then
  existing="$("$dest" version 2>/dev/null || true)"
  case "$existing" in "Xray $version "*) exit 0 ;; esac
fi
stage="$(mktemp -d)"
trap 'rm -f -- "$stage/xray.zip" "$stage/xray"; rmdir -- "$stage"' EXIT
curl -fsSL "https://github.com/XTLS/Xray-core/releases/download/v$version/$asset" -o "$stage/xray.zip"
printf '%s  %s\n' "$hash" "$stage/xray.zip" | shasum -a 256 -c -
unzip -p "$stage/xray.zip" xray > "$stage/xray"
chmod 755 "$stage/xray"
mkdir -p "$(dirname "$dest")"
mv -f "$stage/xray" "$dest"
