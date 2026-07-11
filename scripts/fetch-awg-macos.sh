#!/usr/bin/env bash
# Собирает движок AmneziaWG для macOS и кладёт в src-tauri/binaries/:
#   - amneziawg-go  — userspace-датапас (создаёт utun, крутит крипту)
#   - awg           — UAPI-конфигуратор (аналог wg)
#   - awg-quick     — bash-обёртка: поднимает utun+адреса+маршруты+DNS из .conf
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
# Фиксируем совместимые релизы: master у обоих проектов меняется и уже ломал
# формат AWG 1.5/2.0. v0.2.19 поддерживает S3/S4 и I1-I5 из vpn:// Amnezia.
GO_VERSION="v0.2.19"
TOOLS_VERSION="v1.0.20260223"

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

STAMP="$BIN_DIR/.awg-macos-version"
EXPECTED_VERSION="amneziawg-go=$GO_VERSION amneziawg-tools=$TOOLS_VERSION"
if [ ! -f "$STAMP" ] || [ "$(cat "$STAMP")" != "$EXPECTED_VERSION" ]; then
  echo "Версия движка изменилась — пересобираю macOS AmneziaWG"
  rm -f "$BIN_DIR/amneziawg-go" "$BIN_DIR/awg" "$BIN_DIR/awg-quick" "$STAMP"
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
  git -C "$tmp/awg-tools" apply --recount "$ROOT/scripts/awg-quick-bash3.patch"
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
echo "AmneziaWG-движок для macOS готов в $BIN_DIR"
