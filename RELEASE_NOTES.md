# UniGate 1.2.4 — защита локальных интерфейсов VPN

## Русский

- Clash API sing-box требует случайный токен для каждого VPN-подключения. Счётчики трафика в UniGate продолжают работать.
- Внутренний SOCKS5 AmneziaWG теперь требует пароль для TCP и UDP. Приложения вне туннеля больше не могут использовать этот прокси без учётных данных.
- На Android защита применяется также при запуске из плитки, виджета и при системном восстановлении старого подключения.

## English

- The sing-box Clash API now requires a random token per VPN session. Traffic counters remain available in UniGate.
- The internal AmneziaWG SOCKS5 proxy now requires a password for TCP and UDP. Apps outside the tunnel need credentials to use it.
- On Android, these protections also apply to connections started from the quick settings tile, the widget, or restored by the system.
