# AGENTS.md — UniGate

Рабочий контекст проекта для Codex. Держим этот файл в актуальном состоянии: при смене решений — правим здесь.

## Что это

**UniGate** — кроссплатформенный (Windows → macOS → Linux) open-source клиент для VPN/прокси/туннелей с единым интерфейсом. Подробности и цели — в [README.md](README.md).

Главный архитектурный принцип: **протоколы мы не реализуем сами**. Все перечисленные в README протоколы (Hysteria 2, SOCKS5, HTTP(S), Shadowsocks, VMess, VLESS, Trojan, TUIC, AmneziaWG) умеет **sing-box** — один Go-бинарник. UniGate выступает оболочкой: хранит профили, генерирует для sing-box JSON-конфиг, запускает его дочерним процессом и читает статистику. Это проверенный подход (nekoray, Hiddify, sing-box GUI).

## Стек

| Слой | Технология | Причина |
|------|-----------|---------|
| GUI-оболочка | **Tauri 2** (Rust backend) | Минимальное потребление ресурсов (нативный webview ОС, бинарник ~10–15 МБ) — прямо по целям проекта |
| Фронтенд | **React + TypeScript + Vite** | Богатая экосистема UI, быстрый dev-цикл |
| Бэкенд/IPC | **Rust** (внутри Tauri) | Управление процессами, конфигами, правами, хранилище |
| Сетевое ядро | **sing-box** (бинарник-sidecar) | Покрывает все протоколы из README одним движком |
| Стейт фронта | TBD на Phase 1 (Zustand — кандидат) | Лёгкий, без бойлерплейта |
| Хранилище | JSON-файлы → при росте SQLite | Профили, настройки |

### Установлено в окружении
Node 22, npm 10, Python 3.10, .NET 9, cmake, git. **Rust — нужно доустановить** (`rustup`). Go не нужен (sing-box берём готовым бинарником, не собираем).

## Структура репозитория (целевая)

```
UniGate/
├── src/                  # фронтенд (React + TS)
│   ├── components/        # UI-компоненты
│   ├── features/          # профили, подключение, статистика, настройки
│   ├── store/             # стейт-менеджмент
│   └── lib/               # вызовы Tauri IPC, типы
├── src-tauri/            # бэкенд (Rust)
│   ├── src/
│   │   ├── core/          # менеджер процесса sing-box
│   │   ├── config/        # генератор sing-box JSON из профиля
│   │   ├── profiles/      # CRUD + хранилище
│   │   ├── stats/         # чтение Clash API статистики
│   │   └── commands.rs    # #[tauri::command] — мост во фронт
│   ├── binaries/          # sing-box-<target-triple>(.exe) — sidecar
│   └── tauri.conf.json
└── AGENTS.md
```

## Архитектура подключения

```
React UI  --IPC-->  Rust (Tauri)  --spawn/JSON-->  sing-box  --сеть-->  интернет
   ^                     |                              |
   |   статистика  <-- Clash API (localhost HTTP) <-----+
```

Два режима работы (важно для плана — растут по сложности прав):
- **Proxy mode** — sing-box поднимает локальный SOCKS/HTTP inbound, ставим системный прокси. **Прав администратора не нужно.** Делаем первым.
- **TUN mode** — полноценный VPN с маршрутизацией и split-tunneling. **Нужны admin (Win) / root-хелпер (macOS).** Сложный, делаем позже.

## План разработки (от простого к сложному)

> Принцип: каждая фаза заканчивается чем-то работающим и проверяемым. Не уходим в следующую, пока текущая не запускается.

### Phase 0 — Bootstrap ⚙️ ✅ ГОТОВО
- [x] Установить Rust (`rustup`) + проверить `cargo` (1.96, stable-msvc)
- [x] Скаффолд Tauri 2 + React + TS + Vite, запуск окна (`npm run tauri dev`)
- [x] Положить бинарник sing-box в `src-tauri/binaries/`, настроить как sidecar (`externalBin` + scope в capabilities)
- [x] Из Rust вызвать `sing-box version` (команда `singbox_version`) и показать версию в UI
- **Готово:** окно открывается, UI показывает версию sing-box v1.13.14.

### Phase 1 — Базовая архитектура 🏗️ ✅ ГОТОВО
- [x] Доменные модели: `Profile`, `Outbound`, `ConnectionState`, `Settings` ([models.rs](src-tauri/src/models.rs)) + зеркало в TS ([types.ts](src/lib/types.ts))
- [x] Стейт-менеджмент на фронте (Zustand, [useAppStore.ts](src/store/useAppStore.ts)) + типобезопасный IPC-слой ([ipc.ts](src/lib/ipc.ts))
- [x] Хранилище настроек на диске (атомарная запись JSON в app config dir, [settings.rs](src-tauri/src/settings.rs))
- [x] Каркас IPC-команд ([commands.rs](src-tauri/src/commands.rs)): `singbox_version`, `get_settings`, `save_settings`
- **Готово:** мост фронт↔Rust типобезопасен; настройки сохраняются на диск (`%APPDATA%/com.unigate.app/settings.json`) и переживают перезапуск.
- **Примечание:** тема пока только хранится, не применяется к UI — применение перенесено в Phase 9.

### Phase 2 — Управление профилями 👤 ✅ ГОТОВО
- [x] CRUD профилей в UI (создать/редактировать/удалить/дублировать) — [ProfilesPanel.tsx](src/features/profiles/ProfilesPanel.tsx), [ProfileForm.tsx](src/features/profiles/ProfileForm.tsx)
- [x] Персист профилей на диск (`profiles.json`, общий атомарный слой [storage.rs](src-tauri/src/storage.rs))
- [x] Список профилей, выбор/снятие активного (хранится в `settings.activeProfileId`)
- **Готово:** профили создаются/редактируются/удаляются/дублируются, активный выбирается и снимается; всё переживает перезапуск. Команды: `list/create/update/delete/duplicate_profile`.

### Phase 3 — Первое подключение (proxy mode) 🚀 ✅ ГОТОВО
- [x] Генератор sing-box config (SOCKS5/HTTP, валиден по схеме) — [config.rs](src-tauri/src/config.rs)
- [x] Менеджер процесса: spawn / kill sing-box, managed state, события, watch падений — [connection.rs](src-tauri/src/connection.rs)
- [x] Локальный `mixed`-inbound (127.0.0.1:2080) + системный прокси Windows (winreg + WinInet refresh, без админа) — [sysproxy.rs](src-tauri/src/sysproxy.rs)
- [x] Кнопка Connect/Disconnect + индикатор состояния — [ConnectionPanel.tsx](src/features/connection/ConnectionPanel.tsx)
- [x] Cleanup при выходе (kill + снять прокси) через `RunEvent::Exit`
- **Готово:** проверено реально — трафик идёт через профиль (выходной IP = адрес прокси). Команды: `connect/disconnect/get_connection_state/local_proxy_addr`, событие `connection-state`.
- **Заметки на будущее:** локальный порт пока фиксированный (2080) — сделать настраиваемым/динамическим; статистику скорости добавит Phase 5 (Clash API).

### Phase 4 — Hysteria 2 + импорт ⚡ ✅ ГОТОВО
- [x] Outbound::Hysteria2 в моделях + генератор конфига (tls/sni/insecure/obfs/up-down) — валиден по схеме
- [x] Реальное подключение к Hysteria 2 серверу — проверено (egress через туннель)
- [x] Импорт профиля из ссылки `hysteria2://` (формат Happ) и JSON sing-box — [import.rs](src-tauri/src/import.rs), команда `import_profile`, [ImportDialog.tsx](src/features/profiles/ImportDialog.tsx)
- [x] Hysteria2 в форме профиля; необязательные поля (SNI/obfs/insecure/скорости) свёрнуты в «Дополнительно»
- [x] **Доп. устойчивость:** реконсиляция системного прокси на старте (`reconcile_startup`) — снимает «зависший» прокси после нештатного выхода
- **Готово:** Hysteria 2 подключается; профили создаются и руками, и импортом ссылки/JSON.
- **На будущее (Phase 6):** ссылки остальных протоколов (vless/vmess/ss/trojan/tuic) + подписки.

### Phase 5 — Статистика и мониторинг 📊 ✅ ГОТОВО
- [x] Clash API sing-box на localhost (`experimental.clash_api`, порт 9090) — [config.rs](src-tauri/src/config.rs)
- [x] Опрос `/connections` раз в секунду, расчёт скорости из дельты, события `traffic` (reqwest) — [stats.rs](src-tauri/src/stats.rs)
- [x] Реалтайм ↓/↑ скорость + суммарный трафик за сессию в ConnectionPanel; сброс при отключении
- **Готово:** в UI бежит живая статистика — проверено реально.
- **Заметки:** Clash API без секрета (только localhost); порт 9090 фиксированный — при конфликте сделать настраиваемым.

### Phase 6 — Импорт / экспорт / подписки 💾 ✅ ГОТОВО
- [x] Парсинг share-ссылок: `hysteria2://`, `vless://`, `vmess://`, `ss://`, `trojan://`, `tuic://` + `vpn://` Amnezia (Phase 4 + 8 + 7c, [import.rs](src-tauri/src/import.rs))
- [x] **Экспорт** профиля в share-ссылку (socks/http → JSON, AmneziaWG → `vpn://base64url(qCompress(JSON))`) — [export.rs](src-tauri/src/export.rs), команда `export_profile`, кнопка «Поделиться»; для AWG интерфейс показывает только ссылку, сырой `.conf` не выводит; round-trip юнит-тесты
- [x] **Подписки** (subscription URL) — [subscriptions.rs](src-tauri/src/subscriptions.rs): reqwest+rustls, скачивание, base64/plain-парсинг списка ссылок → профили (с `subscriptionId`); команды `list/add/update/delete_subscription`, [SubscriptionsPanel.tsx](src/features/subscriptions/SubscriptionsPanel.tsx). Парсер покрыт тестами.
- **Готово:** профиль создаётся вставкой подписки (список серверов, обновление); профили экспортируются в ссылку.

### Phase 7 — TUN mode + split tunneling + AmneziaWG 🔀 (самая сложная) ✅ ГОТОВО
Требует прав администратора и реально перенаправляет весь трафик ОС. Разбита на под-этапы:

**7a — TUN-режим (sing-box):** ✅ ГОТОВО
- [x] wintun.dll рядом с sidecar (в git не коммитим, как и sing-box)
- [x] `Settings.mode` (proxy/tun) + ветка генератора TUN-конфига ([config.rs](src-tauri/src/config.rs) `generate_tun`)
- [x] Повышение прав: `is_elevated` + relaunch elevated (UAC) ([elevation.rs](src-tauri/src/elevation.rs)); в TUN системный прокси НЕ ставим
- [x] UI: переключатель режима + предупреждение про админа ([App.tsx](src/App.tsx))
- **Готово:** TUN проверен реально (Hysteria 2 + SOCKS5 под админом) — весь трафик через туннель. Лог ядра пишется в `sing-box.log` (level warn).

**7b — Split-tunneling:** ✅ ГОТОВО
- [x] `Settings.routing` (bypass_lan/bypass_ru/app_mode/apps) + генерация route-правил в [config.rs](src-tauri/src/config.rs) `generate_tun` (только TUN)
- [x] Обход LAN (`ip_is_private`) и RU (`geoip-ru.srs` rule-set + домены `.ru/.рф/.su`) → direct. При `bypass_lan` RFC1918-сети также идут в `tun.route_exclude_address`, чтобы нативные маршруты других VPN (OpenVPN/TAP/Wintun) не захватывались sing-box.
- [x] Служебная IPv4-сеть TUN — `198.18.0.1/30` (benchmark range), а не прежняя `172.18.0.1/30`: корпоративный маршрут OpenVPN `172.16.0.0/12` иначе пересекался с TUN UniGate и при одновременном подключении убивал интернет.
- [x] Опциональная совместимость DNS с OpenVPN Connect на Windows: отдельная настройка `routing.vpn_compatibility` (UI «Совместимость с другим VPN», по умолчанию выключена и требует `bypass_lan`). Только при её включении выключаем `strict_route`, после создания TUN очищаем его служебный DNS и ставим метрику 5000. Windows выбирает DNS TAP/DCO OpenVPN, а обычный трафик остаётся в более специфичных auto-route маршрутах UniGate. Компромисс явно показан в UI: системный DNS идёт вне UniGate.
- [x] По приложениям (`process_name`): режимы Only (только выбранные через VPN) / Except (выбранные напрямую)
- [x] DNS следует split по приложениям: в Only выбранные используют remote DNS через VPN, остальные — local; в Except выбранные используют local, остальные — remote. Кэши DNS-серверов разделены, чтобы remote-ответ не попадал в direct-приложение. Исключение: при включённой Windows-настройке совместимости с другим VPN системный DNS осознанно передаётся ОС/OpenVPN.
- [x] UI [RoutingPanel.tsx](src/features/routing/RoutingPanel.tsx): тогглы + список приложений; **выбор .exe** нативным диалогом (tauri-plugin-dialog) → берём имя процесса (точный регистр)
- [x] Проверено реально: RU-сайты напрямую (реальный IP на 2ip.ru), остальное через VPN; per-app работает
- **Заметки:** geosite-ru официального нет → RU по geoip-ru (IP) + `.ru/.рф/.su` (домены). `geoip-ru.srs` в `binaries/` (в git не коммитим, тянет fetch-скрипт). Split — только sing-box в TUN, не AmneziaWG. macOS: выбор `.app` / имя процесса — на Phase 10.

**7c — Amnezia `vpn://` импорт + AmneziaWG:**
- [x] Декодер контейнера `vpn://` Amnezia (base64url → qCompress[4 байта BE длина]+zlib → JSON) — [import.rs](src-tauri/src/import.rs) `from_amnezia`, крейт `flate2`
- [x] Маппинг контейнера `amnezia-xray` (встроенный Xray-конфиг) → наш Outbound (vless/vmess/trojan/ss, вкл. Reality/ws/grpc) — `xray_to_outbound`; юнит-тест `amnezia_vpn_xray_vless`
- [x] **Находка:** реальный `vpn://` пользователя — это `amnezia-xray` с **VLESS+Reality**, а НЕ AmneziaWG → работает через sing-box прямо сейчас (proxy+TUN). Импорт уже доступен через существующий диалог «Импорт».
- [x] Контейнеры `amnezia-awg`/`wireguard` → движок **amneziawg.exe** (amneziawg-windows-client 2.0.1). Импорт awg-контейнера: берём `awg.last_config.config` (готовый `.conf`), подставляем DNS (`dns1`/`dns2`), добавляем `MTU` из `last_config.mtu`. Модель `Outbound::AmneziaWg{config,server,port}`; ветка движка в [connection.rs](src-tauri/src/connection.rs) (`/installtunnelservice`/`/uninstalltunnelservice`). Проверено реально — подключается, IP меняется.
- **Готово:** `vpn://` импортируется и подключается — xray (VLESS) через sing-box ✅, AmneziaWG через amneziawg.exe ✅.

**AmneziaWG через awg-shim — split-tunneling и статистика (июль 2026; шим проверен standalone реально, связка в приложении ⏳ ждёт проверки):**
- **Архитектура:** свой Go-шим [awg-shim/](awg-shim/) — userspace AmneziaWG (`amneziawg-go` + gVisor netstack, БЕЗ TUN-адаптера и прав) торчит локальным **SOCKS5 с TCP CONNECT и UDP ASSOCIATE** на 127.0.0.1:2081. sing-box остаётся мозгом маршрутизации: для него AWG-профиль превращается в socks-outbound ([connection.rs](src-tauri/src/connection.rs) `gen_profile`) → **весь split (LAN/RU/per-app), статистика Clash API и proxy-режим (без админа!) работают для AmneziaWG** как для остальных протоколов.
- **Анти-петля:** трафик самого шима обязан идти мимо TUN — в route.rules вставляются (после sniff/hijack-dns) правила `process_name=<имя шима>` → direct и `ip_cidr=<IP сервера>/32` → direct.
- **Lifecycle:** шим запускается ДО ядра (endpoint резолвится системным резолвером, пока наш TUN не активен), READY по stdout; stdin-пайп = «умер UniGate → шим выходит сам». Убивается в disconnect/cleanup/при падении ядра (`kill_shim`).
- **Fallback:** если бинаря шима нет рядом — старый путь (`amneziawg.exe /installtunnelservice` / awg-quick), полный туннель без split; UI показывает «статистика недоступна» только для legacy (команда `awg_shim_available`).
- **Сборка:** [scripts/build-awg-shim.ps1](scripts/build-awg-shim.ps1) (вызывается из fetch-singbox.ps1; сам скачивает портативный Go в `%LOCALAPPDATA%\unigate-tools\go`, если Go нет). CI: setup-go до fetch-шага. Бинарь — externalBin `binaries/awg-shim` (в git не коммитится, как все бинарники).
- **⚠️ Зависимости Go (обе граблины проверены):** поддержка AWG 1.5 (`s3/s4`, `i1-i5`) есть только в `amneziawg-go@master` (= тег **v0.2.19**); тег v1.0.4 — СТАРАЯ ветка, отвергает `s3` («invalid UAPI device key»). Свежий gvisor убрал `PacketBuffer.IsNil`, а снапшот из go.mod v1.0.4 сломан («found packages stack and bridge») — с v0.2.19 + gvisor 20260701 собирается без replace.
- **Проверено standalone на реальном AWG 1.5 конфиге (Jc..I5):** READY, TCP через туннель (egress = IP AWG-сервера), UDP ASSOCIATE (DNS-ответ 8.8.8.8 через туннель). Первый CONNECT после холодного старта может отвалиться (рукопожатие) — повтор проходит.
- **macOS:** пока legacy awg-quick; после обкатки на Windows мигрировать на шим (netstack не требует root вообще — уйдут osascript-пляски для AWG).

**AmneziaWG-движок (legacy, полный туннель) — важные находки (проверено реально):**
- **wintun.dll обязана лежать РЯДОМ с `amneziawg.exe`**, который прописывается в сервис тоннеля — иначе сервис падает с `exit code 3` (не может создать Wintun-адаптер). Поэтому запускаем копию из `src-tauri/binaries/` (там wintun.dll), а НЕ из `target/debug/` (куда Tauri кладёт sidecar без wintun.dll) — см. `awg_exe()` в connection.rs.
- Вызывать `amneziawg.exe` через **`std::process::Command`** (как консоль), а НЕ через Tauri-sidecar `.output()` — GUI-exe с перенаправлением stdout/`CREATE_NO_WINDOW` не стартует сервис.
- `/tunnelservice` вручную не запускается (нужен SCM) — тоннель поднимает только сервис.
- У AmneziaWG **нет Clash API** → статистики скорости/трафика нет (в UI помечено «статистика недоступна»); при желании — опрос `awg.exe show`.
- **Production-сборка (решено):** `wintun.dll` и `geoip-ru.srs` бандлятся через `bundle.resources` с target `""` → кладутся **прямо рядом с exe** (проверено извлечением MSI: `Program Files\UniGate\` содержит `unigate.exe`+`sing-box.exe`+`amneziawg.exe`+`wintun.dll`+`geoip-ru.srs`). Резолверы `awg_exe`/`binaries_file` находят их через `current_exe().parent()`. Сборка: `npm run tauri build` → MSI + NSIS в `src-tauri/target/release/bundle/`.

**Структура AmneziaWG-контейнера `amnezia-awg2` (декодировано из реального `vpn://`, protocol_version 2 = AmneziaWG 1.5):**
- Поле `awg.config` — **готовый `.conf`** в формате AmneziaWG (то, что ест `amneziawg-go`/awg-tools):
  ```
  [Interface] Address=10.8.1.7/32, PrivateKey, DNS=$PRIMARY_DNS,$SECONDARY_DNS,
              Jc, Jmin, Jmax, S1, S2, S3, S4, H1..H4, I1 (hex-blob), I2..I5
  [Peer]      PublicKey, PresharedKey, AllowedIPs=0.0.0.0/0,::/0,
              Endpoint=<server-ip>:34196, PersistentKeepalive=25
  ```
  Плюс `mtu` (1376), `dns1`/`dns2` на верхнем уровне (подставить в `$PRIMARY_DNS`/`$SECONDARY_DNS`).
- Параметры обфускации: Jc=5, Jmin=10, Jmax=50, S1=95, S2=88, S3=19, S4=17, H1–H4 (вида "39764355-903914978"), I1 = `<b 0x...>` (фейковый DNS-ответ для маскировки).
- **План движка (7c-2):** встроить движок AmneziaWG. Кандидаты: `amneziawg-windows` (форк wireguard-windows — сервис `/installtunnelservice <conf>`, делает TUN+роуты+DNS сам) ИЛИ `amneziawg-go` + ручной UAPI/роутинг. Нужны: TUN+admin (есть из 7a), bundling бинаря, lifecycle через сервис/sidecar. Импорт awg-контейнера → берём `awg.config`, подставляем DNS → профиль AmneziaWG.

### Phase 8 — Расширение протоколов 🔓 ✅ ГОТОВО (AmneziaWG → перенесён в Phase 7c)
- [x] Прокси-протоколы: Shadowsocks, Trojan, VLESS (вкл. Reality), VMess, TUIC — модели, генератор конфига (общие TlsOpts/Transport), парсеры ссылок `ss://`/`trojan://`/`vless://`/`vmess://`/`tuic://`
- [x] Проверено: конфиги валидны (`sing-box check`), парсеры покрыты юнит-тестами (`cargo test`, 7 шт.)
- [x] Эти протоколы добавляются **импортом** (ссылка/JSON/подписка); ручная форма — для SOCKS/HTTP/Hysteria2, у остальных «Изменить» скрыт
- **AmneziaWG вынесен в Phase 7c** — sing-box его НЕ поддерживает (см. находки ниже), нужен отдельный TUN-движок.

### Phase 9 — Полировка 🎨 ✅ ГОТОВО (кроме отложенного)
- [x] **Редизайн**: дизайн-система (CSS-переменные), боковая навигация с вкладками, тёмная/светлая тема (реально применяется, `system` следит за ОС) — [App.tsx](src/App.tsx)/[App.css](src/App.css)
- [x] **System tray** + меню (Показать/Скрыть/Выход) + клик по иконке; **сворачивание в трей** при закрытии окна (`minimizeToTray`) — VPN живёт в фоне — [lib.rs](src-tauri/src/lib.rs)
- [x] **Автозапуск** при старте Windows (tauri-plugin-autostart) + **автоподключение** к активному профилю
- [x] **Автозапуск стартует в трее** (июль 2026, ⏳ ждёт проверки на Windows): автозапуск регистрируется с аргументом `--autostart`; окно создаётся скрытым (`visible:false` в [tauri.conf.json](src-tauri/tauri.conf.json)) и показывается в setup, только если запуск НЕ автостартовый ([lib.rs](src-tauri/src/lib.rs) `launched_via_autostart`). Связка с adminLaunch: задача Планировщика запускает exe без аргументов, поэтому редиректящий процесс оставляет файл-маркер `autostart-redirect.flag` (одноразовый, TTL 120 с) — elevated-инстанс читает его и остаётся в трее ([elevation.rs](src-tauri/src/elevation.rs) `admin_launch_startup`). Старые записи автозапуска без аргумента самолечатся на старте (`refresh_autostart` — enable() поверх включённого). Бонус: исчез флеш окна у редиректящего процесса.
- [x] **Production-сборка** (MSI + NSIS), бандлинг wintun/geoip рядом с exe — проверено на установленном приложении (все режимы)
- [x] Скролл длинных списков (профили/подписки/приложения)
- [ ] **Отложено (по решению пользователя):** локализация RU/EN (муторно, юзер RU); автообновление приложения (Tauri updater — нужна инфра релизов/подпись); автообновление ядра sing-box.

### Phase 10 — macOS, затем Linux 🍎🐧 🚧 В РАБОТЕ (на реальном Mac, Apple Silicon)
**Workflow (проверено):** на Mac → поставить Rust (`rustup`)/Node/Xcode CLI → `npm install` → `bash scripts/fetch-singbox.sh` (тянет sing-box+geoip под macOS) → `npm run tauri dev`. Кросс-компиляции с Windows нет — собираем на реальном Mac. Окружение подтверждено: Rust 1.96.1 `aarch64-apple-darwin`, sing-box 1.13.14 darwin-arm64.

**Задел уже сделан (с Windows, чтобы mac-сборка не падала):**
- [x] `amneziawg`/`wintun` вынесены в [tauri.windows.conf.json](src-tauri/tauri.windows.conf.json) (Windows-only externalBin/resources); база — только `sing-box`+`geoip-ru.srs` (кроссплатформенно). На маке базовый конфиг собирается (`cargo check`/`build` ок).
- [x] Платформенные модули под `#[cfg(...)]`: [sysproxy.rs](src-tauri/src/sysproxy.rs), [elevation.rs](src-tauri/src/elevation.rs).
- [x] [scripts/fetch-singbox.sh](scripts/fetch-singbox.sh) — sing-box+geoip для macOS/Linux.

**Модель прав на macOS (важно):** на современных macOS **и proxy, и TUN требуют root** — `networksetup -set*proxy` возвращает exit 14 «Command requires admin privileges», а TUN и подавно (utun+маршруты). Принцип «proxy без прав» на новых macOS недостижим. Рабочая модель — **как на Windows: запускать приложение с правами администратора** (в dev — `sudo npm run tauri dev`). Тогда `is_elevated()`=true → и `networksetup` (proxy), и прямой запуск sing-box (TUN) проходят. **Проверено пользователем: proxy-режим работает (egress через профиль).**

**⚠️ macOS TCC — критичная находка (подтверждено логом):** папки `~/Desktop`/`~/Documents`/`~/Downloads` закрыты TCC. Процесс sing-box (даже от root, запущенный приложением — другая цепочка ответственности TCC) **не может читать оттуда data-файлы**: `open .../binaries/geoip-ru.srs: operation not permitted` → ядро падает `FATAL initialize router` → TUN не поднимается. Проект лежал на Рабочем столе → RU-обход (`bypass_ru`, грузит `geoip-ru.srs`) ронял TUN. **Фикс:** `staged_geoip()` копирует geoip в app data dir (`~/Library/Application Support/com.unigate.app/` — под TCC НЕ попадает) и подставляет этот путь в конфиг ([connection.rs](src-tauri/src/connection.rs)). `running-config.json`/`sing-box.log` изначально там же — читаются/пишутся нормально.

**Сделано на маке (компилируется; ✅ ГОТОВО помечаем после проверки пользователем):**
- [x] **Системный прокси** через `networksetup` — [sysproxy.rs](src-tauri/src/sysproxy.rs): web/secure/socks на 127.0.0.1:2080 на все ВКЛючённые физические сервисы (en0/en9/…), отключённые (`*`)/VPN-псевдосервисы (пустой `Device`) пропускаем; снятие — на всех, `is_enabled_for` для реконсиляции. **Требует root** (запускать приложение от админа). ✅ проверено пользователем.
- [x] **`elevation::is_elevated`** для Unix (`geteuid()==0`, крейт `libc`).
- [x] **TUN-режим** — [connection.rs](src-tauri/src/connection.rs): если приложение УЖЕ root (`is_elevated`) — sing-box запускаем обычным дочерним sidecar-процессом (наследует root, есть отслеживание падений), как на Windows. Если НЕ root — фолбэк `connect_tun_macos`: sing-box от root через `osascript ... with administrator privileges` + lifecycle через сигнальный файл `tun-run.lock` (root-watcher снимает туннель при удалении файла, без повторного пароля). ⏳ ждёт ретеста после фикса geoip/TCC.
- [x] **Кроссплатформенный UI**: выбор `.app` вместо `.exe` в split ([RoutingPanel.tsx](src/features/routing/RoutingPanel.tsx), из бандла `Foo.app` берём имя процесса `Foo`); текст про TUN на маке ([SettingsPanel.tsx](src/features/settings/SettingsPanel.tsx)); определение ОС — [platform.ts](src/lib/platform.ts).

**⚠️ TUN DNS-дедлок при бутстрапе (подтверждено логом, исправлено):** после фикса TCC TUN поднимался, но валился на резолве домена сервера: `open connection ... hysteria2[proxy]: lookup *.ru: context deadline exceeded` + `resolve error: lookup ... on 127.0.0.53:53: no such host`. Причина: DNS-сервер `local` был типа `{type:local}` (системный резолвер). При активном СТОРОННЕМ VPN (Happ/AmneziaWG) системный резолвер = локальный стаб `127.0.0.53`, который не отвечает, когда наш TUN перехватил трафик → домен сервера не резолвится → туннель не встаёт → всё виснет. Первая попытка фикса — `8.8.8.8` через `detour:"direct"` — тоже падала: sing-box отвергает detour на «пустой» direct-outbound (`detour to an empty direct outbound makes no sense`). **Рабочий фикс** ([config.rs](src-tauri/src/config.rs) `generate_tun`): `local` DNS = `{type:"udp", server:"77.88.8.8"}` **БЕЗ detour** — DNS-серверы без detour ходят через default-dialer, который при `auto_detect_interface` привязан к физическому интерфейсу и минует tun (штатный бутстрап-механизм); `default_domain_resolver` резолвит домен сервера через него ДО подъёма туннеля. Upstream — Яндекс.DNS `77.88.8.8` (доступен из RU напрямую). Плюс DNS-rule: `.ru/.рф/.su` → `local` при `bypass_ru` (IP совпадает с direct-маршрутом, geoip-ru срабатывает корректно). **Windows НЕ трогаем:** там `local` остаётся `{type:local}` и DNS-правила для RU нет (проверенный рабочий конфиг) — новый бутстрап под `#[cfg(not(target_os="windows"))]`, Windows-ветка байт-в-байт прежняя. **При тесте TUN на маке выключать сторонние VPN** — они держат свой DNS-стаб и дерутся за маршрут по умолчанию.

**Диагностика падений ядра (добавлено):** `sing-box.log` обнуляется перед каждым запуском; при падении ядра хвост лога (последние 5 строк) попадает в сообщение об ошибке в UI ([connection.rs](src-tauri/src/connection.rs) `log_tail`). macOS non-root TUN-путь (`connect_tun_macos`) больше не рапортует «Connected» по факту получения PID: через 1.5 с проверяем, что root-процесс жив (`kill -0`, EPERM = жив), иначе — Error с хвостом лога; дальше фоновый watcher поллит PID раз в 2 с и переводит в Error при неожиданной смерти (у root-процесса нет CommandChild-событий).

**⚠️ Split по приложениям на macOS (найдено при ретесте):** одного `process_name` мало — у Chromium/Electron-приложений сеть живёт в хелперах («Google Chrome Helper»), а исполняемый файл бандла может называться иначе, чем сам бандл (Visual Studio Code.app → `Electron`), поэтому правило по имени бандла не совпадало. **Фикс** ([config.rs](src-tauri/src/config.rs) `per_app_rules`): на macOS к `process_name` добавляется правило `process_path_regex` на `/<Name>.app/` — путь покрывает все процессы внутри бандла независимо от каталога установки (правила объединяются по ИЛИ). Windows-ветка не изменилась (точное `process_name` c `.exe`). **Известное ограничение:** Safari и часть системных приложений ходят в сеть через внебандловые хелперы (`com.apple.WebKit.Networking` в /System/Library) — их split по приложению не поймает, это ограничение подхода как такового.

**⚠️ «Иногда не открываются сайты» на macOS — исправлено (июль 2026, ✅ подтверждено пользователем: TUN + RU-обход работают; ветка «IPv6-сеть» (раздача с iPhone) пока не проверялась):** три независимые причины, все чинятся только в macOS/прочих ветках (Windows-ветка генератора байт-в-байт прежняя):
1. **DNS через туннель был UDP:53** (`remote` 8.8.8.8 udp, detour proxy) — самый хрупкий путь: у TCP-протоколов (VLESS/Trojan) это UDP-over-TCP, часто теряется/таймаутит → «домен не резолвится». Теперь `remote` = **DoH** (`type:"https"`, 8.8.8.8, detour proxy) — обычный TLS через туннель, работает поверх любого протокола.
2. **IPv6 уходил мимо туннеля**: tun имел только IPv4-адрес → auto_route перехватывал только IPv4-маршрут. Браузеры со своим DoH (secure DNS) получают AAAA мимо нашего hijack-dns и ходят по IPv6 напрямую → в IPv6-сетях (раздача с iPhone и т.п.) заблокированные сайты «иногда не открываются». Теперь tun-адрес + `fdfe:dcba:9876::1/126`, **но только при реальном IPv6 у системы** (см. ниже).
   **⚠️ Найдено при ручном тесте:** безусловный перехват IPv6 на **IPv4-only сети** ломает RU-обход — фейковый IPv6-маршрут заставляет приложения выбирать IPv6 (ya.ru имеет AAAA), правило bypass_ru шлёт его в `direct`, а доставить IPv6 некуда → мгновенный разрыв, в браузере `ERR_CONNECTION_CLOSED` именно на RU-сайтах (через туннель IPv6 работал — у VPN-сервера IPv6 есть, поэтому «без RU-обхода всё работает»). **Фикс:** `has_ipv6_route()` в [connection.rs](src-tauri/src/connection.rs) (`route -n get -inet6 default`, смотрим stdout на `interface:` — код возврата всегда 0) → флаг `tun_ipv6` в генератор: IPv6-адрес tun добавляется только при живом IPv6-маршруте. На IPv4-only сети утечка IPv6 и так невозможна.
3. **Бутстрап зависел от захардкоженного 77.88.8.8**: сети с фильтрацией стороннего DNS (отели, мобильные операторы) роняли резолв домена сервера. Теперь домен сервера **предрезолвится в Rust системным резолвером ДО запуска ядра** (`resolve_server_ips` в [connection.rs](src-tauri/src/connection.rs), таймаут 4 с) и попадает в конфиг hosts-DNS-сервером (`type:"hosts"`, тег `bootstrap`) → `default_domain_resolver`. Дедлока со сторонним VPN нет (в момент предрезолва наш tun ещё не поднят). Фолбэк при неудаче предрезолва — прежняя схема 77.88.8.8. Схемы провалидированы `sing-box check` (1.13.14 darwin-arm64).

**Запуск без прав администратора на macOS (сделано, июль 2026):** `sudo npm run tauri dev` больше НЕ нужен:
- **Проверено на живом маке (macOS 26):** `networksetup -set*proxy` у админ-пользователя работает **без root** (exit 0) — «exit 14» из ранних заметок был особенностью другой машины/версии. На случай exit 14 в [sysproxy.rs](src-tauri/src/sysproxy.rs) добавлен фолбэк: весь батч networksetup-команд одним `osascript ... with administrator privileges` (один запрос пароля).
- Proxy-режим теперь ставит и **bypass-домены** (localhost/127.0.0.1/*.local/169.254/16/10/172.16/192.168 — аналог ProxyOverride на Windows), иначе локальные адреса шли через прокси.
- **TUN** — как и было: пароль запрашивается при подключении (osascript), не при запуске приложения.
- **AmneziaWG на macOS больше не требует root-приложения**: `awg-quick up` от root через osascript + сигнальный файл `awg-run.lock` (root-watcher снимает тоннель при его удалении — disconnect/выход без второго пароля), зеркально TUN-механизму (`spawn_awg_quick_root` в [connection.rs](src-tauri/src/connection.rs)). Жёсткое требование `is_elevated` осталось только на Windows.
- **Самолечение App Support после sudo-запусков**: если каталог/файлы принадлежат root (наследие `sudo npm run tauri dev`), обычный запуск не мог сохранять настройки и создавать сигнальные файлы — пользователь был вынужден снова запускать от root. `heal_config_dir_ownership` ([connection.rs](src-tauri/src/connection.rs)) на старте обнаруживает это и чинит владельца одним `chown -R` через osascript.
- Общий helper `elevation::osascript_admin` ([elevation.rs](src-tauri/src/elevation.rs)) — единая точка «выполнить shell от root с нативным диалогом пароля».

**Запуск от администратора без UAC на Windows (реализовано, ⏳ НЕ тестировалось — писалось на маке):** настройка `Settings.admin_launch` + переключатель в UI («Запускаться от администратора без запроса UAC», только Windows). Механика — модуль `admin_task` в [elevation.rs](src-tauri/src/elevation.rs): задача Планировщика `UniGate` с `RunLevel=HighestAvailable`, создаётся из elevated-процесса через `schtasks /Create /XML` (XML в UTF-16LE с BOM; без триггеров; `ExecutionTimeLimit=PT0S` — иначе планировщик убьёт процесс через 72 ч; `MultipleInstancesPolicy=Parallel` — иначе `/Run` откажет). Логика на старте (`admin_launch_startup`, вызывается первым в setup [lib.rs](src-tauri/src/lib.rs)): включено+elevated → пересоздать задачу под текущий exe (самолечение); включено+не elevated+задача есть → `schtasks /Run` и exit(0) — элевация без UAC (работает и для автозапуска: Run-key стартует обычный процесс, тот мгновенно редиректит); выключено+elevated → удалить задачу. Включение из UI: не elevated → один UAC-перезапуск (`relaunch_elevated`), задачу создаст elevated-инстанс на старте. **Проверить на Windows:** создание задачи, редирект, автозапуск, выключение. На **macOS решено НЕ делать** нулевой пароль для TUN: без подписи Apple безопасного способа нет (SMAppService-хелперы только для подписанных приложений; root-демон/NOPASSWD-sudoers, исполняющие пользовательский конфиг = локальная эскалация привилегий). Текущий UX: пароль один раз при подключении.

**⚠️ Установленный .app (из .dmg): «proxy падает, RU-обход пропал» (июль 2026, исправлено — в dev не воспроизводилось):** две независимые причины, найдены по `running-config.json`/логам живого установленного приложения:
1. **RU-обход молча отключался в бандле**: `bundle.resources` на macOS кладёт `geoip-ru.srs` в `Contents/Resources/`, а `binaries_file()` искал только рядом с exe (`Contents/MacOS/`) и в `exe_dir/resources` (виндовая раскладка). Файл не находился → `use_ru=false` → в конфиге НИ geoip-правила, НИ `.ru`-domain_suffix, ни DNS-rule (при `bypassRu:true` в настройках!). **Фикс**: кандидат `exe_dir/../Resources` в `binaries_file` ([connection.rs](src-tauri/src/connection.rs)) + `staged_geoip` при отсутствии источника использует ранее staged-копию из App Support вместо тихого отключения. На Windows раскладка прежняя (рядом с exe), не воспроизводилось.
2. **Proxy-режим падал «bind: address already in use» (Clash API 9090)**: отключение macOS TUN асинхронное — GUI удаляет сигнальный файл, root-watcher поллил его раз в 1 с и только потом убивал ядро. Мгновенный reconnect (TUN→disconnect→proxy connect) натыкался на живой старый sing-box, держащий 9090 → новое ядро FATAL (воспроизведено вручную на живом залипшем root-процессе; также залипание случается при обновлении .app — старое root-ядро переживает установку новой версии). **Фиксы**: watcher поллит раз в 0.3 с; `disconnect()` после удаления сигнального файла ЖДЁТ реальной смерти root-PID (до 5 с, poll 100 мс); пре-флайт в `connect()` — перед запуском ядра ждём освобождения 9090 (и 2080 в proxy) до 3 с (`wait_port_free`, bind-проверка), иначе понятная ошибка вместо «код 1»; при падении сайдкара без файлового лога в ошибку UI попадает хвост stderr ядра (FATAL занятого порта печатается до настройки file-лога).

**Осталось на маке:**
- [x] **Ретест TUN** после фиксов TCC+DNS: весь трафик, RU-обход, LAN — работает (подтверждено пользователем).
- [x] **Split по приложениям** после фикса `process_path_regex` — работает (подтверждено пользователем).
- [~] **AmneziaWG под macOS** (реализовано и упаковывается, ждёт теста на маке): движок = `amneziawg-go v0.2.19` (userspace-датапас, utun) + `amneziawg-tools v1.0.20260223` (`awg` + `awg-quick`). `connect_amneziawg` в [connection.rs](src-tauri/src/connection.rs) ветвится по ОС: Windows → `amneziawg.exe /installtunnelservice`; macOS → `run_awg_quick("up"/"down", conf)` (bash awg-quick от root, PATH+`WG_QUICK_USERSPACE_IMPLEMENTATION=amneziawg-go`). Тот же `.conf`, что и на Windows. Требует root (как TUN). [scripts/fetch-awg-macos.sh](scripts/fetch-awg-macos.sh) собирает зафиксированные релизы; CI вызывает его и [tauri.macos.conf.json](src-tauri/tauri.macos.conf.json) кладёт все три файла в `Contents/Resources`. `awg_engine_dir()` ищет этот каталог. **Статистики нет** (нет Clash API). После установки неподписанного `.app` один рекурсивный `xattr -dr com.apple.quarantine /Applications/UniGate.app` охватывает и вложенный движок.
- **Bash на macOS:** системный `/bin/bash` = 3.2, а upstream `awg-quick v1.0.20260223` требует Bash 4 из-за ассоциативных массивов и `$BASHPID`. Во время fetch применяется [awg-quick-bash3.patch](scripts/awg-quick-bash3.patch): DNS-map заменён параллельными индексными массивами, PID берётся через дочерний `sh`, а вызовы `wg` перенаправляются в упакованный `awg`. Патч совместим и с Bash 4/5; Homebrew пользователю не нужен.
- **Пустые AWG 2.0 `I1–I5` на macOS:** Unix-утилита `awg setconf` отвергает явно пустые строки (`Line unrecognized: I2=`), хотя отсутствие параметра имеет то же нулевое значение. Перед записью runtime-конфига `normalize_awg_config_for_macos` удаляет только пустые `I1–I5`; заполненные concealment-пакеты и сохранённый/экспортируемый профиль не меняются. `fetch-awg-macos.sh` включает hash локального bash-патча в stamp и проверяет подмену `wg -> awg`, поэтому старый движок с теми же upstream-тегами больше не переиспользуется.
- **Upstream runtime-path bug на macOS:** тот же `darwin.bash` создавал name/socket в `/var/run/amneziawg`, но проверял timestamp name-файла и удалял интерфейс через устаревший `/var/run/wireguard`. Поэтому свежесозданный `utun` считался невалидным и `up` завершался с кодом 1. Наш patch приводит все три места к `/var/run/amneziawg`. Конфиг перед запуском получает mode `0600`, чтобы `awg-quick` не предупреждал о world-accessible ключе.
- [x] **Сборка `.dmg`** под macOS (CI, [build-installers.yml](.github/workflows/build-installers.yml), Apple Silicon).
- **Подпись/нотаризация — решено НЕ делать** (запарно, Apple Developer $99/год не нужен). `.dmg` неподписанный → Gatekeeper: «ПКМ → Открыть» при первом запуске. Аналогично Windows-инсталлеры без подписи (SmartScreen).
- [ ] **Linux** (TUN обычно проще — CAP_NET_ADMIN/pkexec).
- **Готово когда:** UniGate ставится и работает на macOS (затем Linux).

## Договорённости

- **Язык общения с пользователем:** русский. Код/идентификаторы/коммиты — английский.
- **Безопасность:** никакой телеметрии (принцип проекта). Чувствительные данные профилей (пароли, ключи) хранить аккуратно, не логировать.
- **sing-box как источник истины по протоколам:** перед реализацией генератора конфига сверяться с актуальной схемой sing-box (формат меняется между мажорными версиями — зафиксировать версию).
- **Права:** не требовать admin там, где можно обойтись (proxy mode по умолчанию; TUN — по явному включению).
- **Лицензия:** MIT.

## Команды

```bash
npm install                      # зависимости фронта
pwsh scripts/fetch-singbox.ps1   # Windows: sing-box + wintun + amneziawg + geoip + сборка awg-shim (бинарники в git не хранятся)
bash scripts/fetch-singbox.sh    # macOS/Linux: sing-box + geoip
npm run tauri dev                # запуск в dev-режиме
npm run tauri build              # production-сборка (Windows → MSI + NSIS в src-tauri/target/release/bundle/)
```

## Архитектурные находки и решения

- **sing-box НЕ поддерживает AmneziaWG.** Поля `jc/jmin/jmax/s1/s2/h1..h4` и объект `amnezia` отвергаются (`unknown field`). Обычный WireGuard есть. ⇒ AmneziaWG делаем через отдельный движок **`amneziawg-go`** (Go, MIT — совместимо) в TUN-режиме (Phase 7c). Контейнер `vpn://` Amnezia — это qCompress+base64(JSON), декодировать отдельно.
- **serde + enum-варианты:** на тег-enum'ах (`Outbound`, `Transport`) обязателен `rename_all_fields = "camelCase"`, иначе поля внутри вариантов сериализуются snake_case и рассинхрон с TS-типами. Уже добавлено.
- **TUN-конфиг sing-box 1.13 (рабочая схема, проверена `sing-box check`):** новый формат DNS-серверов (`{type,tag,server}`, не legacy `address`); в `route` обязателен `default_domain_resolver` (иначе deprecation-fatal); `tun` inbound — `address`/`auto_route`/`strict_route`/`stack:"system"` (`strict_route:false` только на Windows при одновременно включённых `bypass_lan` + `vpn_compatibility`); в `route.rules` — `{action:"sniff"}` и `{protocol:"dns",action:"hijack-dns"}`; нужен `direct` outbound. В TUN-режиме системный прокси НЕ ставим.
- **wintun.dll** обязателен для TUN на Windows — кладём рядом с sidecar sing-box, в git не коммитим (как и бинарники).
- **TUN на Windows — рабочие настройки (проверено):** `stack: "gvisor"` по умолчанию (не `system` — тот плохо ловит трафик рядом с Docker/Hyper-V/VirtualBox); DNS — `strategy: "ipv4_only"` + сервер с `detour: "proxy"`. **Конфликт с другими активными TUN-клиентами** (Hiddify и т.п.): их нужно отключать — иначе наш TUN не получает трафик.
- **Стек TUN — настраиваемый (`Settings.tun_stack`, июль 2026, ⏳ ждёт проверки под нагрузкой):** `TunStack` gvisor/system/mixed ([models.rs](src-tauri/src/models.rs)), прокидывается в `inbounds[].stack` ([config.rs](src-tauri/src/config.rs) `generate_tun`), переключатель в UI (только Windows+TUN, [SettingsPanel.tsx](src/features/settings/SettingsPanel.tsx)). Диагноз проблемы «под связкой игра+дискорд+браузер через время отваливается всё»: в TUN **весь** трафик (даже direct-маршрутизируемый — игра) проходит через userspace-стек gvisor; на высоком packet-per-second он захлёбывается (CPU/буферы) → DNS через туннель таймаутит на 20-40 с → «перестаёт открываться вообще всё». `system` = стек ядра, на порядок дешевле, но у пользователя есть VirtualBox → нужен ручной тест (может не поймать трафик). По умолчанию gvisor — поведение остальных не меняется.
- **DNS под нагрузкой (июль 2026):** на Windows `remote` DNS переведён с UDP:53 на **DoH** (`type:"https"`, 8.8.8.8, detour:proxy) — переживает потери пакетов при заторе (UDP молча таймаутил, роняя резолв всех проксируемых приложений). RU-домены (`.ru/.рф/.су`) теперь резолвятся через `local` (мимо туннеля) и на Windows тоже — иначе `.ru`-домен игры (Тарков, `gw-pvp.escapefromtarkov.ru`) резолвился через забитый туннель и висел, хотя маршрут для него direct. macOS так делал и раньше; обе схемы провалидированы `sing-box check`.
- **Зависший системный прокси:** при нештатном выходе остаётся `ProxyEnable=1` на мёртвый порт → на старте `reconcile_startup` снимает его.

## Зафиксированные версии

- **sing-box: v1.13.14** (Windows amd64). Скрипт загрузки: [scripts/fetch-singbox.ps1](scripts/fetch-singbox.ps1). Бинарник лежит в `src-tauri/binaries/sing-box-<target-triple>.exe`, **в git не коммитится** (gitignore) — тянется скриптом. При апгрейде версии сверять схему конфига.
- **awg-shim** (свой, [awg-shim/](awg-shim/)): Go 1.26 + `amneziawg-go v0.2.19` (master; НЕ v1.0.4 — см. находки 7c) + gvisor go-ветка. Собирается [scripts/build-awg-shim.ps1](scripts/build-awg-shim.ps1) в `binaries/awg-shim-x86_64-pc-windows-msvc.exe`. Портативный Go для сборки — `%LOCALAPPDATA%\unigate-tools\go`.
- **wintun: v0.14.1** (amd64) — `src-tauri/binaries/wintun.dll`, в git не коммитится.
- **AmneziaWG-движок: amneziawg-windows-client v2.0.1** — `amneziawg.exe` (9 МБ, форк wireguard.exe) + `awg.exe`, извлекаются из MSI (`msiexec /a`), кладутся в `src-tauri/binaries/`, в git не коммитятся. Модель как у WireGuard: `amneziawg.exe /installtunnelservice <.conf>` поднимает тоннель Windows-сервисом (TUN+IP+роуты+DNS+обфускация — всё внутри), `/uninstalltunnelservice <name>` снимает. Нужен админ (есть из 7a).
- Rust: stable-msvc 1.96. Node 22.
- Зависимости Rust: tauri 2, tauri-plugin-shell, serde/serde_json, uuid, url, percent-encoding, base64, reqwest (http-only, default-features=false), tokio (time); Windows: winreg, windows-sys (WinInet).

## Открытые вопросы / TODO решить по ходу

- Стейт-менеджер фронта (кандидат — Zustand) — финализировать на Phase 1.
- UI-кит/стиль (Tailwind + headless? или готовая библиотека) — решить в начале Phase 2.
- Кроссплатформенный fetch-скрипт ядра (macOS/Linux) — на Phase 10.
