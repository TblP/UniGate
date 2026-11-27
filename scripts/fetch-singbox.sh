#!/usr/bin/env bash
# Скачивает sing-box (sidecar) + geoip-ru.srs для macOS/Linux и кладёт в
# src-tauri/binaries/ с именами под Tauri (target-triple). Бинарники в git не
# хранятся (см. .gitignore) — запусти после клонирования:
#   bash scripts/fetch-singbox.sh
#
# amneziawg/wintun — только Windows (см. fetch-singbox.ps1).
#
# macOS/Windows берут патченный sing-box из форка TblP/sing-box
# (ветка unigate/udp-masque): CONNECT-UDP (RFC 9298) поверх HTTP/3.
# В апстриме 1.14.4 этого нет. Когда выйдет sing-box 1.15 с MASQUE,
# верни URL на SagerNet и удали штампы/константы ниже.
set -euo pipefail

SINGBOX_VERSION="1.14.4"
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

# Патченный дистрибутив форка. macOS arm64 собирается в CI с cgo
# (libcronet.a линкуется статически), поэтому libcronet.dylib не нужен.
dist_tag=""
dist_sha256=""
case "${sb_os}/${sb_arch}" in
  darwin/arm64)
    dist_tag="singbox-1.14.4-masque.2"
    dist_sha256="e3ed854af9d110ab8dcc21adf0b29d99685fcbe4a8e028fcb15cb28462f956db"
    ;;
esac
stamp="$BIN_DIR/.singbox-${sb_os}-dist"

need_download=1
if [ -n "$dist_tag" ]; then
  if [ -f "$dest" ] && [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$dist_tag" ]; then
    echo "sing-box уже на месте: $("$dest" version 2>/dev/null | head -1 || true) (dist: $dist_tag)"
    need_download=0
  fi
elif [ -f "$dest" ]; then
  existing="$("$dest" version 2>/dev/null | head -1 || true)"
  echo "sing-box уже на месте: $existing"
  case "$existing" in
    *"$SINGBOX_VERSION"*) need_download=0 ;;
    *) echo "Версия отличается от $SINGBOX_VERSION — перекачиваю." ;;
  esac
fi

if [ "$need_download" = 1 ]; then
  if [ -n "$dist_tag" ]; then
    asset="sing-box-${SINGBOX_VERSION}-masque.2-${sb_os}-${sb_arch}.tar.gz"
    url="https://github.com/TblP/sing-box/releases/download/${dist_tag}/${asset}"
  else
    asset="sing-box-${SINGBOX_VERSION}-${sb_os}-${sb_arch}.tar.gz"
    url="https://github.com/SagerNet/sing-box/releases/download/v${SINGBOX_VERSION}/${asset}"
  fi
  tmp="$(mktemp -d)"
  echo "Скачиваю $url"
  curl -fsSL "$url" -o "$tmp/sb.tar.gz"
  if [ -n "$dist_sha256" ]; then
    actual="$(shasum -a 256 "$tmp/sb.tar.gz" | awk '{print $1}' | tr 'A-F' 'a-f')"
    if [ "$actual" != "$dist_sha256" ]; then
      rm -rf "$tmp"
      echo "sing-box SHA256 не совпал: ожидался $dist_sha256, получен $actual" >&2
      exit 1
    fi
  fi
  tar -xzf "$tmp/sb.tar.gz" -C "$tmp"
  exe="$(find "$tmp" -name sing-box -type f | head -1)"
  # Снимаем старый бинарник, а не пишем поверх: перезапись работающего
  # sing-box вернёт ETXTBSY («Text file busy»), а unlink разрешён всегда.
  rm -f "$dest"
  cp "$exe" "$dest"
  chmod +x "$dest"
  rm -rf "$tmp"
  if [ -n "$dist_tag" ]; then
    echo "$dist_tag" > "$stamp"
  fi
  echo "Готово: $dest"
  "$dest" version | head -1
fi

if [ "$sb_os" = "darwin" ]; then
  bash "$ROOT/scripts/fetch-xray-macos.sh"
fi

# geoip-ru.srs (RU-обход в split-tunneling)
geoip="$BIN_DIR/geoip-ru.srs"
geoip_tmp="$geoip.tmp"
echo "Обновляю geoip-ru.srs"
curl -fsSL "https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-ru.srs" -o "$geoip_tmp"
if [ "$(wc -c < "$geoip_tmp")" -lt 10000 ] || [ "$(head -c 4 "$geoip_tmp" | od -An -tx1 | tr -d ' \n')" != "53525301" ]; then
  rm -f "$geoip_tmp"
  echo "Загруженный geoip-ru.srs не прошёл проверку" >&2
  exit 1
fi
mv -f "$geoip_tmp" "$geoip"
echo "Готово: $geoip"
