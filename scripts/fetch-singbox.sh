#!/usr/bin/env bash
# Скачивает sing-box (sidecar) + geoip-ru.srs для macOS/Linux и кладёт в
# src-tauri/binaries/ с именами под Tauri (target-triple). Бинарники в git не
# хранятся (см. .gitignore) — запусти после клонирования:
#   bash scripts/fetch-singbox.sh
#
# amneziawg/wintun — только Windows (см. fetch-singbox.ps1).
set -euo pipefail

SINGBOX_VERSION="1.13.14"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="$ROOT/src-tauri/binaries"
mkdir -p "$BIN_DIR"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) sb_os="darwin" ;;
  Linux)  sb_os="linux" ;;
  *) echo "Неподдерживаемая ОС: $os (для Windows используйте fetch-singbox.ps1)"; exit 1 ;;
esac

case "$arch" in
  arm64|aarch64) sb_arch="arm64"; cpu="aarch64" ;;
  x86_64|amd64)  sb_arch="amd64"; cpu="x86_64" ;;
  *) echo "Неподдерживаемая архитектура: $arch"; exit 1 ;;
esac

if [ "$sb_os" = "darwin" ]; then
  triple="${cpu}-apple-darwin"
else
  triple="${cpu}-unknown-linux-gnu"
fi

dest="$BIN_DIR/sing-box-${triple}"
if [ -f "$dest" ]; then
  echo "sing-box уже на месте: $("$dest" version | head -1)"
else
  asset="sing-box-${SINGBOX_VERSION}-${sb_os}-${sb_arch}.tar.gz"
  url="https://github.com/SagerNet/sing-box/releases/download/v${SINGBOX_VERSION}/${asset}"
  tmp="$(mktemp -d)"
  echo "Скачиваю $url"
  curl -sL "$url" -o "$tmp/sb.tar.gz"
  tar -xzf "$tmp/sb.tar.gz" -C "$tmp"
  exe="$(find "$tmp" -name sing-box -type f | head -1)"
  cp "$exe" "$dest"
  chmod +x "$dest"
  rm -rf "$tmp"
  echo "Готово: $dest"
  "$dest" version | head -1
fi

# geoip-ru.srs (RU-обход в split-tunneling)
geoip="$BIN_DIR/geoip-ru.srs"
if [ -f "$geoip" ]; then
  echo "geoip-ru.srs уже на месте"
else
  echo "Скачиваю geoip-ru.srs"
  curl -sL "https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-ru.srs" -o "$geoip"
  echo "Готово: $geoip"
fi
