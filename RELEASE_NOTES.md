# UniGate 1.1.3 — split-tunneling AmneziaWG на macOS

Этот релиз добавляет в macOS-версию тот же путь AmneziaWG через sing-box, который уже используется на Windows.

## Что нового

- AmneziaWG на macOS работает через userspace `awg-shim` и sing-box.
- Для AmneziaWG заработал обход RU-трафика, LAN и split-tunneling по приложениям.
- Добавлена живая статистика скорости и трафика AmneziaWG на macOS.
- Доменные endpoint AmneziaWG предварительно разрешаются в IP, чтобы исключить петлю трафика через TUN.
- Прежний `awg-quick` сохранён как запасной движок, если shim недоступен.

## Установка

Сборки проекта не имеют доверенной подписи издателя:

- **Android:** разрешите установку из неизвестного источника; система может показать предупреждение.
- **Windows:** SmartScreen — «Подробнее → Выполнить в любом случае».
- **macOS:** после установки выполните
  `sudo xattr -dr com.apple.quarantine /Applications/UniGate.app`

> Android APK предназначен для устройств ARM64.
