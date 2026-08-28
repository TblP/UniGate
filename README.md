# UniGate

**UniGate** — современный open-source клиент для **SELF HOSTING** VPN, прокси и туннелей в едином удобном интерфейсе.

Проект объединяет разные сетевые технологии в одном приложении, избавляя от необходимости держать несколько разных клиентов. Единый центр управления подключениями, независимо от протокола.

Под капотом — один сетевой движок [**sing-box**](https://sing-box.sagernet.org/) (плюс отдельный движок для AmneziaWG): UniGate хранит профили, генерирует конфиг, запускает движок и читает статистику. Лёгкий нативный клиент на **Tauri 2** (Rust) + **React** — бинарник ~15 МБ, минимум ресурсов.

> **Статус:** UniGate доступен для Windows, macOS и Android ARM64. На всех трёх платформах работают TUN, RU/LAN-обход и раздельное туннелирование по приложениям.

## Возможности

* 🚀 Современный интерфейс с тёмной/светлой темой
* 🔌 Прокси-режим (без прав администратора) и полноценный **TUN-VPN** (весь трафик ОС)
* 🌐 Управление несколькими профилями
* 🔗 Импорт по ссылке, из JSON, из `vpn://` (Amnezia) 
* 🧭 **Раздельное туннелирование:**
  * обход по регионам (RU-трафик напрямую, мимо VPN) и локальной сети (LAN)
  * по приложениям — выбранные через VPN / выбранные напрямую (выбор `.exe` на Windows, `.app` на macOS)
* 📤 Экспорт профиля: share-ссылка (`vless://` и др.) или JSON (sing-box outbound) — на выбор
* 📊 Живая статистика скорости и трафика
* 🖥️ Системный трей, сворачивание в трей, автозапуск, автоподключение
* 📱 Android: быстрые действия, плитка в шторке и виджеты 1×1 / 2×2
* 🔒 Никакой телеметрии
* 🔓 Полностью открытый исходный код (MIT)

## Поддерживаемые протоколы

* Hysteria 2
* AmneziaWG 1.5 / 3.1 *(Windows/macOS/Android — userspace awg-shim + sing-box; desktop имеет legacy fallback)*
* SOCKS5
* HTTP / HTTPS
* Shadowsocks
* VMess
* VLESS *(в т.ч. Reality)*
* Trojan
* TUIC

Прокси-протоколы работают и в режиме локального прокси, и в TUN. Добавляются вручную (SOCKS/HTTP/Hysteria2) или импортом ссылки/подписки (остальные).

## Стек

| Слой | Технология |
|------|-----------|
| GUI-оболочка | Tauri 2 (Rust) |
| Фронтенд | React + TypeScript + Vite (Zustand) |
| Сетевое ядро | sing-box (sidecar на desktop, libbox на Android) |
| AmneziaWG | awg-shim + amneziawg-go/v3 (Windows, macOS, Android) |

## Готовые сборки

Инсталлеры под Windows (MSI + NSIS `setup.exe`), macOS (`.dmg`, Apple Silicon) и APK для Android ARM64.

> Инсталлеры **не подписаны** (подпись/нотаризация не планируются).
> 
> **macOS:** после установки снимите quarantine со всего bundle — это также разрешит вложенные `sing-box` и AmneziaWG:
> ```bash
> sudo xattr -dr com.apple.quarantine /Applications/UniGate.app
> ```
> 
> **Windows:** 
> - В окне **SmartScreen** выберите «Подробнее → Выполнить в любом случае».
> - В Windows 11 запуск также может блокировать новая функция **Интеллектуальное управление приложениями** (Smart App Control). Она срабатывает на любые программы без цифровой подписи. Поскольку у UniGate нет подписи, эту функцию придется отключить вручную в настройках «Безопасности Windows» (раздел *Управление приложениями и браузером* → *Интеллектуальное управление приложениями*).
>
> **Android:**
> - Разрешите браузеру или файловому менеджеру **установку неизвестных приложений**.
> - Play Protect и HyperOS могут предупредить, что APK получен не из магазина. У сборки нет доверенной publisher-подписи.

## Сборка и запуск (локально)

Нужны **Node 22+** и **Rust** (Windows: stable-msvc + Microsoft C++ Build Tools + WebView2; macOS: Xcode Command Line Tools).

```bash
npm install                      # зависимости фронта

# бинарники ядра в git не хранятся — тянутся скриптом:
pwsh scripts/fetch-singbox.ps1   # Windows: sing-box + wintun + amneziawg + geoip
bash scripts/fetch-singbox.sh    # macOS/Linux: sing-box + geoip
bash scripts/fetch-awg-macos.sh  # macOS: awg-shim + legacy AmneziaWG fallback (нужен Go)

# Android ARM64 (Windows-hosted portable toolchain):
pwsh scripts/setup-android.ps1
pwsh scripts/fetch-android-assets.ps1
pwsh scripts/build-android.ps1   # dist/android/UniGate_<version>_android_arm64.apk

npm run tauri dev                # запуск в dev-режиме
npm run tauri build              # сборка (Windows → MSI + NSIS)
