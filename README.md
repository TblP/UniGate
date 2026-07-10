# UniGate

**UniGate** — современный open-source клиент для VPN, прокси и туннелей в едином удобном интерфейсе.

Проект объединяет разные сетевые технологии в одном приложении, избавляя от необходимости держать несколько разных клиентов. Единый центр управления подключениями, независимо от протокола.

Под капотом — один сетевой движок [**sing-box**](https://sing-box.sagernet.org/) (плюс отдельный движок для AmneziaWG): UniGate хранит профили, генерирует конфиг, запускает движок и читает статистику. Лёгкий нативный клиент на **Tauri 2** (Rust) + **React** — бинарник ~15 МБ, минимум ресурсов.

> **Статус:** Windows и macOS готовы — proxy- и TUN-режимы работают (весь трафик, RU-обход, LAN, раздельное туннелирование по приложениям).

## Возможности

* 🚀 Современный интерфейс с тёмной/светлой темой
* 🔌 Прокси-режим (без прав администратора) и полноценный **TUN-VPN** (весь трафик ОС)
* 🌐 Управление несколькими профилями (создание/редактирование/дублирование)
* 🔗 Импорт по ссылке, из JSON, из `vpn://` (Amnezia) и по **подписке** (subscription URL)
* 🧭 **Раздельное туннелирование:**
  * обход по регионам (RU-трафик напрямую, мимо VPN) и локальной сети (LAN)
  * по приложениям — выбранные через VPN / выбранные напрямую (выбор `.exe` на Windows, `.app` на macOS)
* 📤 Экспорт профиля: share-ссылка (`vless://` и др.) или JSON (sing-box outbound) — на выбор
* 📊 Живая статистика скорости и трафика
* 🖥️ Системный трей, сворачивание в трей, автозапуск, автоподключение
* 🔒 Никакой телеметрии
* 🔓 Полностью открытый исходный код (MIT)

## Поддерживаемые протоколы

### VPN
* Hysteria 2
* AmneziaWG *(Windows — движок amneziawg; macOS — amneziawg-go + awg-quick, экспериментально)*

### Прокси
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
| Сетевое ядро | sing-box (sidecar) |
| AmneziaWG | amneziawg (Windows) |

## Готовые сборки

Инсталлеры под Windows (MSI + NSIS `setup.exe`) и macOS (`.dmg`, Apple Silicon).

> Инсталлеры **не подписаны** (подпись/нотаризация не планируются). На macOS после
> установки снимите quarantine со всего bundle — это также разрешит вложенные
> `sing-box` и AmneziaWG: `sudo xattr -dr com.apple.quarantine /Applications/UniGate.app`.
> На Windows выберите «Подробнее → Выполнить в любом случае» в SmartScreen.

## Сборка и запуск (локально)

Нужны **Node 22+** и **Rust** (Windows: stable-msvc + Microsoft C++ Build Tools + WebView2; macOS: Xcode Command Line Tools).

```bash
npm install                      # зависимости фронта

# бинарники ядра в git не хранятся — тянутся скриптом:
pwsh scripts/fetch-singbox.ps1   # Windows: sing-box + wintun + amneziawg + geoip
bash scripts/fetch-singbox.sh    # macOS/Linux: sing-box + geoip
bash scripts/fetch-awg-macos.sh  # macOS: движок AmneziaWG (собирает зафиксированные версии, нужен Go)

npm run tauri dev                # запуск в dev-режиме
npm run tauri build              # сборка (Windows → MSI + NSIS)
```

> TUN-режим и AmneziaWG требуют прав администратора. На macOS приложение покажет
> системный запрос пароля при подключении; движок AmneziaWG уже входит в `.app`.

## План развития

* [x] Базовая архитектура, управление профилями
* [x] Прокси-режим (SOCKS5/HTTP/Hysteria 2 и др.)
* [x] TUN-режим (полный VPN) + раздельное туннелирование
* [x] AmneziaWG (Windows)
* [x] Импорт ссылок/`vpn://`/подписок, экспорт
* [x] Статистика, трей, автозапуск, тема
* [x] Сборка под Windows (MSI/NSIS)
* [x] CI: автосборка инсталлеров Windows + macOS в GitHub Actions, релиз по тегу `v*`
* [x] macOS — proxy и TUN (весь трафик, RU-обход, LAN, split по приложениям), сборка `.dmg` (неподписанный и надо sudo xattr -dr com.apple.quarantine /Applications/UniGate.app)
* [ ] Локализация RU/EN, автообновление

## Windows версия: 

<img width="1183" height="899" alt="Снимок экрана 2026-07-05 190423" src="https://github.com/user-attachments/assets/1e13ef80-d3d2-4e45-953e-3ead811b33e8" />

<img width="1198" height="903" alt="Снимок экрана 2026-07-05 190417" src="https://github.com/user-attachments/assets/cd697862-0a39-4a2e-983e-4fecec69a796" />

<img width="1197" height="899" alt="Снимок экрана 2026-07-05 190430" src="https://github.com/user-attachments/assets/be32f658-83d0-4ed4-90a2-5a47daac9546" />

<img width="1191" height="894" alt="Снимок экрана 2026-07-05 190436" src="https://github.com/user-attachments/assets/d5ed7da9-918c-4c2b-986e-9032c29578fc" />





