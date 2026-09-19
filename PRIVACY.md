# Политика конфиденциальности UniGate

**Дата вступления в силу: 19 сентября 2026 г.**

UniGate — клиент для подключения к VPN-, прокси- и туннельным серверам, которые указывает сам пользователь. Приложение распространяется бесплатно, с открытым исходным кодом под лицензией MIT: <https://github.com/TblP/UniGate>

## Коротко

UniGate **не собирает, не хранит и не передаёт разработчику никаких персональных данных**. В приложении нет аналитики, рекламы, трекеров и сторонних SDK. У проекта нет серверов, которые принимали бы данные пользователей.

## Какие данные обрабатываются

Все данные хранятся **исключительно на устройстве пользователя** и никуда не отправляются:

- **Профили подключений** — адреса серверов, порты, а также учётные данные, которые пользователь ввёл или импортировал: пароли, ключи, UUID, параметры шифрования.
- **Адреса подписок**, если пользователь их добавил.
- **Настройки приложения** — режим работы, правила раздельного туннелирования, список выбранных приложений, тема оформления.

Разработчик не имеет доступа к этим данным. Они удаляются вместе с приложением или при очистке его данных средствами системы.

## Сетевые соединения

UniGate устанавливает соединения только в трёх случаях:

1. **С серверами, которые указал пользователь.** Через них проходит сетевой трафик устройства, когда подключение активно. Эти серверы не принадлежат разработчику и не контролируются им; их работу регулирует политика того, кто этот сервер предоставил. Приложение не анализирует, не записывает и не передаёт содержимое трафика — оно только направляет его согласно настройкам пользователя.
2. **С адресом подписки**, если пользователь его добавил, — чтобы получить список серверов. Запрос уходит только на адрес, указанный пользователем.
3. **С api.github.com при запуске** — чтобы проверить, вышла ли новая версия приложения. Запрашивается только номер последней опубликованной версии. Приложение не передаёт при этом никаких идентификаторов; GitHub, как и любой веб-сервис, видит IP-адрес и стандартные заголовки запроса. Обработка этих данных регулируется политикой конфиденциальности GitHub: <https://docs.github.com/site-policy/privacy-policies/github-privacy-statement>

## Разрешения Android и зачем они нужны

| Разрешение | Назначение |
|---|---|
| `INTERNET` | сетевые соединения с сервером пользователя |
| `BIND_VPN_SERVICE` | создание VPN-туннеля средствами системы |
| `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_SPECIAL_USE` | поддержание активного подключения, пока приложение свёрнуто |
| `POST_NOTIFICATIONS` | уведомление о состоянии подключения |
| `ACCESS_NETWORK_STATE`, `CHANGE_NETWORK_STATE` | отслеживание смены сети и переподключение |
| `BIND_QUICK_SETTINGS_TILE` | плитка подключения в шторке уведомлений |

Разрешение VPN означает, что сетевой трафик устройства проходит через приложение и направляется на сервер, выбранный пользователем. UniGate не сохраняет и не передаёт содержимое этого трафика.

## Дети

Приложение не предназначено для детей и не собирает данные о них.

## Изменения

При изменении политики обновляется дата в начале документа. Актуальная версия всегда доступна по этому адресу.

## Контакты

По вопросам конфиденциальности используйте раздел Issues в репозитории проекта: <https://github.com/TblP/UniGate/issues>

Права субъекта персональных данных описаны отдельно: [PRIVACY_RIGHTS.md](PRIVACY_RIGHTS.md)

---

# UniGate Privacy Policy

**Effective date: 19 September 2026**

UniGate is a client for connecting to VPN, proxy and tunnel servers that the user supplies. It is free and open source under the MIT license: <https://github.com/TblP/UniGate>

## Summary

UniGate **does not collect, store or transmit any personal data to the developer**. There is no analytics, advertising, tracking or third-party SDK in the application. The project operates no servers that receive user data.

## What data is processed

All data is kept **on the user's device only** and is never sent anywhere:

- **Connection profiles** — server addresses, ports, and the credentials the user entered or imported: passwords, keys, UUIDs, encryption parameters.
- **Subscription URLs**, if the user added any.
- **Application settings** — operating mode, split tunneling rules, the list of selected applications, theme.

The developer has no access to this data. It is removed when the application is uninstalled or its data is cleared through the system.

## Network connections

UniGate connects to the network in three cases only:

1. **To servers specified by the user.** Device traffic passes through them while a connection is active. These servers belong to and are controlled by whoever provides them, not by the developer; their own policies apply. The application does not inspect, log or transmit traffic contents — it only routes traffic according to the user's settings.
2. **To a subscription URL**, if the user added one, in order to retrieve a server list. The request goes only to the address the user provided.
3. **To api.github.com on startup**, to check whether a newer version has been released. Only the latest published version number is requested. The application sends no identifiers; GitHub, like any web service, sees the IP address and standard request headers. That processing is governed by GitHub's privacy statement: <https://docs.github.com/site-policy/privacy-policies/github-privacy-statement>

## Android permissions

| Permission | Purpose |
|---|---|
| `INTERNET` | network connections to the user's server |
| `BIND_VPN_SERVICE` | creating the VPN tunnel through the system |
| `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_SPECIAL_USE` | keeping the connection alive while the app is in the background |
| `POST_NOTIFICATIONS` | connection status notification |
| `ACCESS_NETWORK_STATE`, `CHANGE_NETWORK_STATE` | detecting network changes and reconnecting |
| `BIND_QUICK_SETTINGS_TILE` | the connection tile in the notification shade |

The VPN permission means device traffic passes through the application and is routed to the server chosen by the user. UniGate neither stores nor transmits the contents of that traffic.

## Children

The application is not directed at children and collects no data about them.

## Changes

When this policy changes, the date at the top is updated. The current version is always available at this address.

## Contact

For privacy enquiries, please use the project's issue tracker: <https://github.com/TblP/UniGate/issues>

Data subject rights are described separately: [PRIVACY_RIGHTS.md](PRIVACY_RIGHTS.md)
