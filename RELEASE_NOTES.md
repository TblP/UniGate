# UniGate 1.2.5 — изоляция приложений VPN на Android

## Русский

- В режиме «Только выбранные» на Android 10 и новее sing-box проверяет владельца соединения по списку приложений. Трафик от остальных пакетов или с неизвестным владельцем не получает доступ к VPN-выходу.
- DNS-запросы из TUN без подтверждённого владельца используют прямой резолвер. Если Android не принял ни одного выбранного приложения, UniGate останавливает подключение.

## English

- In Selected apps only mode on Android 10 or later, sing-box checks the connection owner against the selected package list. Traffic from other packages or an unknown owner cannot use the VPN exit.
- TUN DNS queries without an identified owner use the direct resolver. UniGate stops the connection if Android does not accept any selected app.
