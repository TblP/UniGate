# UniGate 1.2.3 — подписки, VLESS XHTTP, QR и NaïveProxy

## Что нового

- **Подписки в окне «+ VPN».** Сервер и подписку теперь добавляют из одного окна: вставьте ссылку подписки, и UniGate загрузит все её серверы. После обновления видно, сколько строк пропущено.
- **VLESS + XHTTP.** Профили VLESS с транспортом XHTTP импортируются из ссылок `vless://`, конфигов Xray и контейнеров Amnezia `vpn://`. Работают в режиме прокси и TUN на Windows и macOS, а также на Android.
- **QR-коды.** Любой профиль можно показать QR-кодом, чтобы перенести его на телефон. На Android ссылку сервера или подписки можно отсканировать камерой.
- **NaïveProxy.** Поддерживаются ссылки `naive+https://` и конфиги sing-box, а также экспорт профиля обратно в ссылку.
- Кнопки добавления переименованы: «+ VPN» импортирует сервер или подписку, «+ Прокси» создаёт SOCKS/HTTP вручную.
- Новые подписки принимаются только по HTTPS. Ранее добавленные подписки по `http://` продолжают обновляться.
- Ядро sing-box обновлено до 1.14.4.

## What's new

- **Subscriptions in the "+ VPN" window.** Add a server or a subscription from one window: paste a subscription URL and UniGate loads all of its servers. After an update, the app shows how many lines were skipped.
- **VLESS + XHTTP.** VLESS profiles with the XHTTP transport can be imported from `vless://` links, Xray configs, and Amnezia `vpn://` containers. They work in proxy and TUN modes on Windows and macOS, and on Android.
- **QR codes.** Any profile can be shown as a QR code to move it to a phone. On Android, a server or subscription link can be scanned with the camera.
- **NaïveProxy.** `naive+https://` links and sing-box configs are supported, and profiles can be exported back to a link.
- The add buttons were renamed: "+ VPN" imports a server or subscription, "+ Proxy" creates a SOCKS/HTTP profile manually.
- New subscriptions must use HTTPS. Subscriptions added earlier over `http://` keep updating.
- The sing-box core is updated to 1.14.4.

## Установка / Installation

Сборки для Windows, macOS и Android публикуются ниже. Android APK предназначен для устройств ARM64.

Windows, macOS, and Android downloads are listed below. The Android APK is for ARM64 devices.
