#!/usr/bin/env bash
# Собирает движок AmneziaWG для macOS и кладёт в src-tauri/binaries/:
#   - amneziawg-go  — userspace-датапас (создаёт utun, крутит крипту)
#   - awg           — UAPI-конфигуратор (аналог wg)
#   - awg-quick     — legacy bash-обёртка: поднимает utun+маршруты
#   - awg-shim      — основной движок: userspace AWG -> SOCKS5 для sing-box
#
# Prebuilt-бинарников у amneziawg-go/amneziawg-tools нет — собираем из исходников.
# Нужны: Go, make, git, Xcode Command Line Tools (clang). Только macOS.
#
#   bash scripts/fetch-awg-macos.sh
#
# Бинарники в git не хранятся (см. .gitignore). Windows-движок (amneziawg.exe) —
# в scripts/fetch-singbox.ps1.
set -euo pipefail

GO_REPO="https://github.com/amnezia-vpn/amneziawg-go"
TOOLS_REPO="https://github.com/amnezia-vpn/amneziawg-tools"
# Фиксируем совместимые релизы с поддержкой AmneziaWG 3.1. Ветка master у
# обоих проектов меняется, поэтому воспроизводимая сборка использует теги.
GO_VERSION="v3.1.20260814"
TOOLS_VERSION="v3.1.20260812"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "Этот скрипт только для macOS (Windows-движок — в fetch-singbox.ps1)." >&2
  exit 1
fi

for tool in go make git; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Не найден '$tool'. Поставь его (Go: https://go.dev/dl/, make/clang: xcode-select --install)." >&2
    exit 1
  fi
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="$ROOT/src-tauri/binaries"
mkdir -p "$BIN_DIR"

case "$(uname -m)" in
  arm64) TARGET_TRIPLE="aarch64-apple-darwin"; GO_ARCH="arm64" ;;
  x86_64) TARGET_TRIPLE="x86_64-apple-darwin"; GO_ARCH="amd64" ;;
  *) echo "Неподдерживаемая архитектура macOS: $(uname -m)" >&2; exit 1 ;;
esac

STAMP="$BIN_DIR/.awg-macos-version"
PATCH_HASH="$(git hash-object "$ROOT/scripts/awg-quick-bash3.patch")"
EXPECTED_VERSION="amneziawg-go=$GO_VERSION amneziawg-tools=$TOOLS_VERSION patch=$PATCH_HASH"
if [ ! -f "$STAMP" ] || [ "$(cat "$STAMP")" != "$EXPECTED_VERSION" ]; then
  echo "Версия движка изменилась — пересобираю macOS AmneziaWG"
  rm -f "$BIN_DIR/amneziawg-go" "$BIN_DIR/awg" "$BIN_DIR/awg-quick" "$STAMP"
fi

# Старые сборки могли иметь тот же upstream-тег и stamp, но ещё не содержать
# нашу подмену wg -> awg. Не переиспользуем такой awg-quick.
if [ -f "$BIN_DIR/awg-quick" ] && ! grep -Fq 'awg "$@"' "$BIN_DIR/awg-quick"; then
  echo "Патч awg-quick изменился — пересобираю macOS AmneziaWG"
  rm -f "$BIN_DIR/awg" "$BIN_DIR/awg-quick" "$STAMP"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# --- amneziawg-go ---
if [ -f "$BIN_DIR/amneziawg-go" ]; then
  echo "amneziawg-go уже на месте"
else
  echo "Собираю amneziawg-go…"
  git clone --depth 1 --branch "$GO_VERSION" "$GO_REPO" "$tmp/awg-go"
  ( cd "$tmp/awg-go" && make )
  awg_go="$(find "$tmp/awg-go" -maxdepth 1 -type f -name amneziawg-go -perm -111 | head -1)"
  [ -n "$awg_go" ] || { echo "amneziawg-go не собрался" >&2; exit 1; }
  cp "$awg_go" "$BIN_DIR/amneziawg-go"
  chmod +x "$BIN_DIR/amneziawg-go"
  echo "Готово: $BIN_DIR/amneziawg-go"
fi

# --- awg + awg-quick (amneziawg-tools) ---
if [ -f "$BIN_DIR/awg" ] && [ -f "$BIN_DIR/awg-quick" ]; then
  echo "awg + awg-quick уже на месте"
else
  echo "Собираю amneziawg-tools (awg)…"
  git clone --depth 1 --branch "$TOOLS_VERSION" "$TOOLS_REPO" "$tmp/awg-tools"
  # Upstream 3.1 slightly reformatted the DNS block; the semantic context is
  # unchanged, so ignore whitespace while applying our Bash 3/runtime fixes.
  git -C "$tmp/awg-tools" apply --recount --ignore-space-change --ignore-whitespace \
    "$ROOT/scripts/awg-quick-bash3.patch"
  # Цель сборки называется `wg`, а `make install` штатно переименовывает её в
  # `awg` и устанавливает правильный darwin-вариант как `awg-quick`.
  install_root="$tmp/awg-install"
  (
    cd "$tmp/awg-tools/src"
    make
    make install DESTDIR="$install_root" PREFIX=/usr WITH_WGQUICK=yes
  )
  awg_bin="$install_root/usr/bin/awg"
  awgq="$install_root/usr/bin/awg-quick"
  [ -x "$awg_bin" ] || { echo "awg не установлен после сборки" >&2; exit 1; }
  [ -x "$awgq" ] || { echo "awg-quick не установлен после сборки" >&2; exit 1; }
  cp "$awg_bin" "$BIN_DIR/awg"
  cp "$awgq" "$BIN_DIR/awg-quick"
  chmod +x "$BIN_DIR/awg" "$BIN_DIR/awg-quick"
  echo "Готово: $BIN_DIR/awg + $BIN_DIR/awg-quick"
fi

printf '%s\n' "$EXPECTED_VERSION" > "$STAMP"

# --- awg-shim (AmneziaWG -> local SOCKS5 -> sing-box) ---
# CGO не нужен: шим использует userspace netstack. Имя с target triple
# нужно Tauri externalBin; внутри .app файл будет называться awg-shim.
SHIM_DEST="$BIN_DIR/awg-shim-$TARGET_TRIPLE"
SHIM_REBUILD=false
if [ ! -x "$SHIM_DEST" ]; then
  SHIM_REBUILD=true
else
  for source in "$ROOT/awg-shim"/*.go "$ROOT/awg-shim/cmd/awg-shim"/*.go "$ROOT/awg-shim/go.mod" "$ROOT/awg-shim/go.sum"; do
    if [ "$source" -nt "$SHIM_DEST" ]; then
      SHIM_REBUILD=true
      break
    fi
  done
fi
if [ "$SHIM_REBUILD" = false ]; then
  echo "awg-shim уже на месте"
else
  echo "Собираю awg-shim…"
  (
    cd "$ROOT/awg-shim"
    CGO_ENABLED=0 GOOS=darwin GOARCH="$GO_ARCH" \
      go build -trimpath -ldflags="-s -w" -o "$SHIM_DEST" ./cmd/awg-shim
  )
  chmod +x "$SHIM_DEST"
  echo "Готово: $SHIM_DEST"
fi

echo "AmneziaWG-движок и shim для macOS готовы в $BIN_DIR"
