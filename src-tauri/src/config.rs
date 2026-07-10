//! Генератор конфигурации sing-box из профиля UniGate.
//!
//! Phase 3 (proxy mode): локальный `mixed`-inbound на 127.0.0.1:<port>
//! + один outbound, описанный профилем. Трафик: приложения → локальный
//! sing-box → сервер из профиля → интернет.
//!
//! Формат сверяется со схемой sing-box (зафиксированная версия — см. CLAUDE.md).

use crate::models::{AppMode, Mode, Outbound, Profile, Routing, TlsOpts, Transport, TunStack};
use serde_json::{json, Value};

/// Порт локального Clash API (статистика трафика). Phase 5.
pub const CLASH_API_PORT: u16 = 9090;

/// Полный конфиг sing-box по режиму. `geoip_ru` — путь к geoip-ru.srs (для RU-обхода).
/// `bootstrap_ips` — заранее разрезолвленные IP домена сервера (macOS TUN,
/// см. комментарий про DNS-бутстрап в `generate_tun`); пустой список = нет.
/// `tun_ipv6` — перехватывать ли IPv6 (только если у системы РЕАЛЬНО есть
/// IPv6-маршрут, см. комментарий у tun-адреса в `generate_tun`).
pub fn generate(
    profile: &Profile,
    mode: Mode,
    local_port: u16,
    routing: &Routing,
    geoip_ru: Option<&str>,
    bootstrap_ips: &[std::net::IpAddr],
    tun_ipv6: bool,
    stack: TunStack,
) -> Value {
    match mode {
        Mode::Proxy => generate_proxy(profile, local_port),
        Mode::Tun => generate_tun(profile, routing, geoip_ru, bootstrap_ips, tun_ipv6, stack),
    }
}

/// Домен сервера outbound, если это именно домен (IP-литерал → None).
/// Для AmneziaWG не применимо (свой движок, не sing-box).
pub fn server_domain(outbound: &Outbound) -> Option<&str> {
    let server = match outbound {
        Outbound::Socks { server, .. }
        | Outbound::Http { server, .. }
        | Outbound::Hysteria2 { server, .. }
        | Outbound::Shadowsocks { server, .. }
        | Outbound::Trojan { server, .. }
        | Outbound::Vless { server, .. }
        | Outbound::Vmess { server, .. }
        | Outbound::Tuic { server, .. } => server,
        Outbound::AmneziaWg { .. } => return None,
    };
    if server.parse::<std::net::IpAddr>().is_ok() {
        None
    } else {
        Some(server)
    }
}

fn clash_api() -> Value {
    json!({ "clash_api": { "external_controller": format!("127.0.0.1:{CLASH_API_PORT}") } })
}

/// Proxy mode: локальный `mixed`-inbound + системный прокси (без админа).
fn generate_proxy(profile: &Profile, local_port: u16) -> Value {
    json!({
        "log": { "level": "warn", "timestamp": true },
        "inbounds": [{
            "type": "mixed",
            "tag": "in",
            "listen": "127.0.0.1",
            "listen_port": local_port
        }],
        "outbounds": [ outbound_to_json(&profile.outbound) ],
        "experimental": clash_api()
    })
}

/// TUN mode: виртуальный адаптер, весь трафик ОС идёт через профиль (нужен админ).
/// Схема sing-box 1.13 (новый DNS, default_domain_resolver, sniff+hijack-dns).
/// Правила split-tunneling строятся из `routing`.
fn generate_tun(
    profile: &Profile,
    routing: &Routing,
    geoip_ru: Option<&str>,
    bootstrap_ips: &[std::net::IpAddr],
    tun_ipv6: bool,
    stack: TunStack,
) -> Value {
    #[cfg(target_os = "windows")]
    let _ = (bootstrap_ips, tun_ipv6); // Windows: проверенный конфиг, эти механизмы не используем
    let use_ru = routing.bypass_ru && geoip_ru.is_some();

    let mut rules: Vec<Value> = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
    ];
    // обходы (напрямую, мимо туннеля) — до правил по приложениям
    if routing.bypass_lan {
        rules.push(json!({ "ip_is_private": true, "outbound": "direct" }));
    }
    if use_ru {
        rules.push(json!({ "rule_set": ["geoip-ru"], "outbound": "direct" }));
        rules.push(json!({ "domain_suffix": [".ru", ".рф", ".su"], "outbound": "direct" }));
    }
    // split по приложениям (у каждого режима свой список)
    let final_out = match routing.app_mode {
        AppMode::Only if !routing.only_apps.is_empty() => {
            rules.extend(per_app_rules(&routing.only_apps, "proxy"));
            "direct" // остальные — напрямую
        }
        AppMode::Except if !routing.except_apps.is_empty() => {
            rules.extend(per_app_rules(&routing.except_apps, "direct"));
            "proxy" // остальные — через туннель
        }
        _ => "proxy",
    };

    // DNS-серверы. `remote` — для проксируемого трафика (через туннель).
    // `local` — резолвер, ходящий мимо tun (бутстрап домена сервера + RU-домены).
    //
    // `remote` = DoH (`type:"https"`, 8.8.8.8, detour:"proxy") на ВСЕХ платформах.
    // Раньше на Windows тут был UDP:53 — но это самый хрупкий путь: при заторе
    // канала/стека UDP-пакеты молча теряются и резолв висит по 20-40 с (в логе —
    // «dns: exchange failed ... context deadline exceeded»), из-за чего под
    // нагрузкой «перестаёт открываться вообще всё». DoH — обычный TCP/TLS,
    // переживает потери куда лучше (ретраи, переиспользование соединения).
    //
    // `local`:
    // - Windows: `{type:"local"}` — системный резолвер, ходит мимо tun по
    //   underlying-сети. Работает стабильно.
    // - macOS/прочие: явный публичный upstream (77.88.8.8, Яндекс.DNS) БЕЗ detour.
    //   `{type:"local"}` там даёт дедлок при активном СТОРОННЕМ VPN (системный
    //   резолвер = его локальный стаб, молчит, когда наш tun перехватил трафик).
    //   DNS-серверы без detour ходят через default-dialer, привязанный
    //   `auto_detect_interface` к физическому интерфейсу и минующий tun.
    //   `detour:"direct"` тут НЕЛЬЗЯ — sing-box падает «detour to an empty
    //   direct outbound makes no sense».
    // - `bootstrap` (только не-Windows) = hosts-сервер с заранее разрезолвленными
    //   IP домена сервера (резолвим в Rust ДО запуска ядра). Подъём туннеля не
    //   зависит от доступности 77.88.8.8 (сети с фильтрацией стороннего DNS).
    #[cfg(target_os = "windows")]
    let local_dns = json!({ "type": "local", "tag": "local" });
    #[cfg(not(target_os = "windows"))]
    let local_dns = json!({ "type": "udp", "tag": "local", "server": "77.88.8.8" });
    let remote_dns = json!({ "type": "https", "tag": "remote", "server": "8.8.8.8", "detour": "proxy" });

    #[cfg_attr(target_os = "windows", allow(unused_mut))]
    let mut servers = vec![remote_dns, local_dns];
    #[cfg_attr(target_os = "windows", allow(unused_mut))]
    let mut domain_resolver = "local";
    #[cfg(not(target_os = "windows"))]
    if !bootstrap_ips.is_empty() {
        if let Some(host) = server_domain(&profile.outbound) {
            let ips: Vec<String> = bootstrap_ips.iter().map(|ip| ip.to_string()).collect();
            servers.push(json!({
                "type": "hosts",
                "tag": "bootstrap",
                "predefined": { host: ips }
            }));
            domain_resolver = "bootstrap";
        }
    }

    let mut route = json!({
        "auto_detect_interface": true,
        "default_domain_resolver": { "server": domain_resolver },
        "final": final_out,
        "rules": rules
    });
    if use_ru {
        route["rule_set"] = json!([{
            "type": "local",
            "tag": "geoip-ru",
            "format": "binary",
            "path": geoip_ru.unwrap()
        }]);
    }

    let mut dns = json!({
        "servers": servers,
        "final": "remote",
        "strategy": "ipv4_only"
    });
    // RU-домены резолвим напрямую (через `local`, мимо туннеля): адрес совпадает
    // с тем, куда пойдёт direct-трафик (route rule_set geoip-ru срабатывает), а
    // главное — резолв НЕ зависит от туннеля. Иначе .ru-домен (например,
    // gw-pvp.escapefromtarkov.ru) при заторе резолвится через забитый туннель и
    // висит по 20-40 с — игра не находит свой шлюз, хотя маршрут для неё direct.
    // Раньше было только на macOS; на Windows .ru молча резолвился через туннель.
    if use_ru {
        dns["rules"] = json!([
            { "domain_suffix": [".ru", ".рф", ".su"], "server": "local" }
        ]);
    }

    // Адрес tun. Windows — как было (проверенный конфиг). macOS/прочие — плюс
    // IPv6, но ТОЛЬКО если у системы реально есть IPv6-маршрут (`tun_ipv6`):
    // - есть IPv6: без перехвата auto_route забирает только IPv4-маршрут, а
    //   IPv6-трафик (браузеры со своим DoH получают AAAA мимо нашего hijack-dns)
    //   уходит в обход туннеля → заблокированные сайты «иногда не открываются»;
    // - нет IPv6 (IPv4-only сеть): перехватывать нельзя — фейковый IPv6-маршрут
    //   заставляет приложения выбирать IPv6, а direct-обход (bypass_ru) не может
    //   его никуда доставить → мгновенный разрыв (ERR_CONNECTION_CLOSED на
    //   ya.ru при включённом RU-обходе). Утечь IPv6 тут и так не может.
    #[cfg(target_os = "windows")]
    let tun_address = json!(["172.18.0.1/30"]);
    #[cfg(not(target_os = "windows"))]
    let tun_address = if tun_ipv6 {
        json!(["172.18.0.1/30", "fdfe:dcba:9876::1/126"])
    } else {
        json!(["172.18.0.1/30"])
    };

    json!({
        "log": { "level": "warn", "timestamp": true },
        "dns": dns,
        "inbounds": [{
            "type": "tun",
            "tag": "tun-in",
            "address": tun_address,
            "auto_route": true,
            "strict_route": true,
            "stack": stack.as_singbox()
        }],
        "outbounds": [
            outbound_to_json(&profile.outbound),
            { "type": "direct", "tag": "direct" }
        ],
        "route": route,
        "experimental": clash_api()
    })
}

/// Route-правила split по приложениям. Windows: точное `process_name` («Foo.exe»).
/// macOS: сеть у бандла часто живёт НЕ в процессе с именем бандла — у
/// Chromium/Electron это хелперы («Google Chrome Helper»), а исполняемый файл
/// может называться иначе (Visual Studio Code.app → `Electron`), поэтому одного
/// `process_name` мало. Добавляем правило по пути исполняемого файла:
/// `process_path_regex` на `/<Name>.app/` покрывает ВСЕ процессы внутри бандла
/// независимо от каталога установки. Правила в списке объединяются по ИЛИ.
fn per_app_rules(apps: &[String], outbound: &str) -> Vec<Value> {
    let rules = vec![json!({ "process_name": apps, "outbound": outbound })];
    #[cfg(target_os = "macos")]
    let rules = {
        let mut rules = rules;
        let patterns: Vec<String> = apps.iter().map(|a| app_bundle_regex(a)).collect();
        rules.push(json!({ "process_path_regex": patterns, "outbound": outbound }));
        rules
    };
    rules
}

/// Regex для пути внутри бандла `<name>.app` (метасимволы имени экранируем).
#[cfg(any(target_os = "macos", test))]
fn app_bundle_regex(name: &str) -> String {
    let mut escaped = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(
            c,
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\'
        ) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    format!("/{escaped}\\.app/")
}

/// Публичный маппинг Outbound → sing-box outbound-объект (для экспорта в JSON).
pub fn outbound_to_singbox(outbound: &Outbound) -> Value {
    outbound_to_json(outbound)
}

/// Маппинг доменного `Outbound` в outbound-объект sing-box.
fn outbound_to_json(outbound: &Outbound) -> Value {
    match outbound {
        Outbound::Socks {
            server,
            port,
            username,
            password,
        } => {
            let mut v = json!({
                "type": "socks",
                "tag": "proxy",
                "server": server,
                "server_port": port,
                "version": "5"
            });
            // sing-box: для socks аутентификация задаётся парой username/password
            if let (Some(u), Some(p)) = (username, password) {
                v["username"] = json!(u);
                v["password"] = json!(p);
            }
            v
        }
        Outbound::Http {
            server,
            port,
            username,
            password,
            tls,
        } => {
            let mut v = json!({
                "type": "http",
                "tag": "proxy",
                "server": server,
                "server_port": port
            });
            if let Some(u) = username {
                v["username"] = json!(u);
            }
            if let Some(p) = password {
                v["password"] = json!(p);
            }
            if *tls {
                v["tls"] = json!({ "enabled": true });
            }
            v
        }
        Outbound::Hysteria2 {
            server,
            port,
            password,
            sni,
            insecure,
            obfs_password,
            up_mbps,
            down_mbps,
        } => {
            let mut tls = json!({ "enabled": true });
            if let Some(s) = sni {
                tls["server_name"] = json!(s);
            }
            if *insecure {
                tls["insecure"] = json!(true);
            }
            let mut v = json!({
                "type": "hysteria2",
                "tag": "proxy",
                "server": server,
                "server_port": port,
                "password": password,
                "tls": tls
            });
            if let Some(op) = obfs_password {
                v["obfs"] = json!({ "type": "salamander", "password": op });
            }
            if let Some(up) = up_mbps {
                v["up_mbps"] = json!(up);
            }
            if let Some(down) = down_mbps {
                v["down_mbps"] = json!(down);
            }
            v
        }
        Outbound::Shadowsocks {
            server,
            port,
            method,
            password,
        } => json!({
            "type": "shadowsocks",
            "tag": "proxy",
            "server": server,
            "server_port": port,
            "method": method,
            "password": password
        }),
        Outbound::Trojan {
            server,
            port,
            password,
            tls,
            transport,
        } => {
            let mut v = json!({
                "type": "trojan",
                "tag": "proxy",
                "server": server,
                "server_port": port,
                "password": password
            });
            apply_tls(&mut v, tls);
            apply_transport(&mut v, transport);
            v
        }
        Outbound::Vless {
            server,
            port,
            uuid,
            flow,
            tls,
            transport,
        } => {
            let mut v = json!({
                "type": "vless",
                "tag": "proxy",
                "server": server,
                "server_port": port,
                "uuid": uuid
            });
            if let Some(f) = flow {
                v["flow"] = json!(f);
            }
            apply_tls(&mut v, tls);
            apply_transport(&mut v, transport);
            v
        }
        Outbound::Vmess {
            server,
            port,
            uuid,
            alter_id,
            security,
            tls,
            transport,
        } => {
            let mut v = json!({
                "type": "vmess",
                "tag": "proxy",
                "server": server,
                "server_port": port,
                "uuid": uuid,
                "alter_id": alter_id
            });
            if let Some(s) = security {
                v["security"] = json!(s);
            }
            apply_tls(&mut v, tls);
            apply_transport(&mut v, transport);
            v
        }
        Outbound::Tuic {
            server,
            port,
            uuid,
            password,
            congestion_control,
            tls,
        } => {
            let mut v = json!({
                "type": "tuic",
                "tag": "proxy",
                "server": server,
                "server_port": port,
                "uuid": uuid,
                "password": password
            });
            if let Some(c) = congestion_control {
                v["congestion_control"] = json!(c);
            }
            // TUIC всегда поверх TLS
            let mut tls = tls.clone();
            tls.enabled = true;
            apply_tls(&mut v, &tls);
            v
        }
        // AmneziaWG идёт через собственный движок (amneziawg.exe), не через sing-box —
        // сюда поток не доходит (менеджер подключения ветвится раньше).
        Outbound::AmneziaWg { .. } => json!({}),
    }
}

/// Добавляет в outbound объект `tls`, если TLS включён.
fn apply_tls(v: &mut Value, tls: &TlsOpts) {
    if !tls.enabled {
        return;
    }
    let mut t = json!({ "enabled": true });
    if let Some(s) = &tls.sni {
        t["server_name"] = json!(s);
    }
    if tls.insecure {
        t["insecure"] = json!(true);
    }
    if !tls.alpn.is_empty() {
        t["alpn"] = json!(tls.alpn);
    }
    if let Some(fp) = &tls.fingerprint {
        t["utls"] = json!({ "enabled": true, "fingerprint": fp });
    }
    if let Some(pbk) = &tls.reality_public_key {
        let mut r = json!({ "enabled": true, "public_key": pbk });
        if let Some(sid) = &tls.reality_short_id {
            r["short_id"] = json!(sid);
        }
        t["reality"] = r;
    }
    v["tls"] = t;
}

/// Добавляет в outbound объект `transport`, если он задан (ws/grpc).
fn apply_transport(v: &mut Value, transport: &Option<Transport>) {
    let Some(transport) = transport else { return };
    v["transport"] = match transport {
        Transport::Ws { path, host } => {
            let mut t = json!({ "type": "ws" });
            if let Some(p) = path {
                t["path"] = json!(p);
            }
            if let Some(h) = host {
                t["headers"] = json!({ "Host": h });
            }
            t
        }
        Transport::Grpc { service_name } => {
            let mut t = json!({ "type": "grpc" });
            if let Some(s) = service_name {
                t["service_name"] = json!(s);
            }
            t
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn socks_profile() -> Profile {
        Profile {
            id: "x".into(),
            name: "t".into(),
            outbound: Outbound::Socks {
                server: "1.2.3.4".into(),
                port: 1080,
                username: None,
                password: None,
            },
            subscription_id: None,
        }
    }

    #[test]
    fn tun_routing_ru_lan_apps_only() {
        let routing = Routing {
            bypass_lan: true,
            bypass_ru: true,
            app_mode: AppMode::Only,
            only_apps: vec!["telegram.exe".into()],
            except_apps: vec![],
        };
        let cfg = generate(&socks_profile(), Mode::Tun, 2080, &routing, Some("C:/x/geoip-ru.srs"), &[], false, TunStack::Gvisor);
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("ip_is_private"));
        assert!(s.contains("geoip-ru"));
        assert!(s.contains("telegram.exe"));
        assert_eq!(cfg["route"]["final"], "direct"); // Only → остальные напрямую
        assert!(cfg["route"]["rule_set"].is_array());
        // RU-домены резолвятся через прямой DNS
        assert_eq!(cfg["dns"]["rules"][0]["server"], "local");
    }

    #[test]
    fn tun_stack_selectable() {
        // по умолчанию gvisor; system прокидывается в inbound
        let cfg = generate(&socks_profile(), Mode::Tun, 2080, &Routing::default(), None, &[], false, TunStack::Gvisor);
        assert_eq!(cfg["inbounds"][0]["stack"], "gvisor");
        let cfg = generate(&socks_profile(), Mode::Tun, 2080, &Routing::default(), None, &[], false, TunStack::System);
        assert_eq!(cfg["inbounds"][0]["stack"], "system");
        let cfg = generate(&socks_profile(), Mode::Tun, 2080, &Routing::default(), None, &[], false, TunStack::Mixed);
        assert_eq!(cfg["inbounds"][0]["stack"], "mixed");
    }

    #[test]
    fn tun_routing_off_defaults_to_proxy() {
        let cfg = generate(&socks_profile(), Mode::Tun, 2080, &Routing::default(), None, &[], false, TunStack::Gvisor);
        assert_eq!(cfg["route"]["final"], "proxy");
        assert!(cfg["route"].get("rule_set").is_none());
        assert!(cfg["dns"].get("rules").is_none());
    }

    #[test]
    fn app_bundle_regex_escapes_meta() {
        assert_eq!(app_bundle_regex("Google Chrome"), "/Google Chrome\\.app/");
        assert_eq!(app_bundle_regex("Foo.Bar (1)+"), "/Foo\\.Bar \\(1\\)\\+\\.app/");
    }

    #[test]
    fn tun_dns_bootstrap_goes_direct() {
        let cfg = generate(&socks_profile(), Mode::Tun, 2080, &Routing::default(), None, &[], false, TunStack::Gvisor);
        let servers = cfg["dns"]["servers"].as_array().unwrap();
        // `remote` — через туннель
        assert_eq!(servers[0]["detour"], "proxy");
        // `local` — явный публичный upstream БЕЗ detour (default-dialer мимо tun);
        // НЕ системный резолвер (дедлок при стороннем VPN) и НЕ detour:direct (fatal)
        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(servers[1]["type"], "udp");
            assert!(servers[1]["server"].is_string());
        }
        assert!(servers[1].get("detour").is_none());
        assert_eq!(cfg["route"]["default_domain_resolver"]["server"], "local");
    }

    /// Профиль с доменом сервера (не IP) — для проверки предрезолва.
    fn domain_profile() -> Profile {
        Profile {
            id: "x".into(),
            name: "t".into(),
            outbound: Outbound::Socks {
                server: "vpn.example.com".into(),
                port: 1080,
                username: None,
                password: None,
            },
            subscription_id: None,
        }
    }

    #[test]
    fn server_domain_skips_ip_literals() {
        assert_eq!(server_domain(&domain_profile().outbound), Some("vpn.example.com"));
        assert_eq!(server_domain(&socks_profile().outbound), None); // 1.2.3.4
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn tun_preresolved_bootstrap_uses_hosts_server() {
        let ips = vec!["5.6.7.8".parse::<std::net::IpAddr>().unwrap()];
        let cfg = generate(&domain_profile(), Mode::Tun, 2080, &Routing::default(), None, &ips, true, TunStack::Gvisor);
        let servers = cfg["dns"]["servers"].as_array().unwrap();
        // remote — DoH через туннель (не UDP: тот хрупок поверх TCP-прокси)
        assert_eq!(servers[0]["type"], "https");
        // hosts-сервер с предрезолвленным IP; бутстрап через него
        let hosts = servers.iter().find(|s| s["type"] == "hosts").unwrap();
        assert_eq!(hosts["predefined"]["vpn.example.com"][0], "5.6.7.8");
        assert_eq!(cfg["route"]["default_domain_resolver"]["server"], "bootstrap");
        // tun перехватывает и IPv6 (иначе обход туннеля в IPv6-сетях)
        let addr = cfg["inbounds"][0]["address"].as_array().unwrap();
        assert_eq!(addr.len(), 2);

        // IP-литерал сервера или пустой предрезолв → обычный фолбэк `local`
        let cfg = generate(&socks_profile(), Mode::Tun, 2080, &Routing::default(), None, &ips, true, TunStack::Gvisor);
        assert_eq!(cfg["route"]["default_domain_resolver"]["server"], "local");

        // IPv4-only сеть (tun_ipv6=false): IPv6 НЕ перехватываем — фейковый
        // маршрут ломает direct-обход (ERR_CONNECTION_CLOSED на RU-сайтах)
        let cfg = generate(&domain_profile(), Mode::Tun, 2080, &Routing::default(), None, &ips, false, TunStack::Gvisor);
        let addr = cfg["inbounds"][0]["address"].as_array().unwrap();
        assert_eq!(addr.len(), 1);
    }
}
