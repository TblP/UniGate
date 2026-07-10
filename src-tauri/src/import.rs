//! Импорт профиля из share-ссылки или JSON.
//!
//! Поддержаны ссылки: `hysteria2://`/`hy2://`, `vless://`, `trojan://`,
//! `tuic://`, `ss://`, `vmess://`, а также JSON sing-box (одиночный outbound
//! или полный конфиг с `outbounds`). Подписки — Phase 6.

use crate::models::{Outbound, TlsOpts, Transport};
use base64::{engine::general_purpose, Engine};
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

type Params = HashMap<String, String>;

/// Разбирает ввод (ссылка или JSON) в `(имя, outbound)`.
pub fn parse(input: &str) -> Result<(String, Outbound), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("пустой ввод".into());
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        from_json(trimmed)
    } else {
        from_link(trimmed)
    }
}

// --- ссылки ------------------------------------------------------------------

fn from_link(s: &str) -> Result<(String, Outbound), String> {
    // vpn:// — контейнер Amnezia (qCompress+base64), разбираем отдельно
    if s.starts_with("vpn://") {
        return from_amnezia(s);
    }
    // ss:// и vmess:// разбираем вручную (base64 ломает обычный URL-парсер)
    if let Some(rest) = s.strip_prefix("ss://") {
        return parse_shadowsocks(rest);
    }
    if let Some(rest) = s.strip_prefix("vmess://") {
        return parse_vmess(rest);
    }

    let url = Url::parse(s).map_err(|e| format!("ссылка не распознана: {e}"))?;
    match url.scheme() {
        "hysteria2" | "hy2" => parse_hysteria2(&url),
        "vless" => parse_vless(&url),
        "trojan" => parse_trojan(&url),
        "tuic" => parse_tuic(&url),
        other => Err(format!(
            "протокол ссылки «{other}» пока не поддерживается"
        )),
    }
}

fn parse_hysteria2(url: &Url) -> Result<(String, Outbound), String> {
    let server = host_of(url)?;
    let port = url.port().unwrap_or(443);
    let password = decode(url.username());
    let p = params_of(url);

    Ok((
        name_of(url, || format!("Hysteria2 {server}")),
        Outbound::Hysteria2 {
            server,
            port,
            password,
            sni: p.get("sni").or_else(|| p.get("peer")).cloned().filter(non_empty),
            insecure: is_true(p.get("insecure").or_else(|| p.get("allowInsecure"))),
            obfs_password: p
                .get("obfs-password")
                .or_else(|| p.get("obfs_password"))
                .cloned()
                .filter(non_empty),
            up_mbps: p.get("up").or_else(|| p.get("upmbps")).and_then(|v| parse_mbps(v)),
            down_mbps: p.get("down").or_else(|| p.get("downmbps")).and_then(|v| parse_mbps(v)),
        },
    ))
}

fn parse_vless(url: &Url) -> Result<(String, Outbound), String> {
    let server = host_of(url)?;
    let port = url.port().unwrap_or(443);
    let uuid = decode(url.username());
    let p = params_of(url);

    Ok((
        name_of(url, || format!("VLESS {server}")),
        Outbound::Vless {
            server,
            port,
            uuid,
            flow: p.get("flow").cloned().filter(non_empty),
            tls: build_tls(&p, true),
            transport: build_transport(&p),
        },
    ))
}

fn parse_trojan(url: &Url) -> Result<(String, Outbound), String> {
    let server = host_of(url)?;
    let port = url.port().unwrap_or(443);
    let password = decode(url.username());
    let p = params_of(url);

    Ok((
        name_of(url, || format!("Trojan {server}")),
        Outbound::Trojan {
            server,
            port,
            password,
            tls: build_tls(&p, true),
            transport: build_transport(&p),
        },
    ))
}

fn parse_tuic(url: &Url) -> Result<(String, Outbound), String> {
    let server = host_of(url)?;
    let port = url.port().unwrap_or(443);
    let uuid = decode(url.username());
    let password = decode(url.password().unwrap_or(""));
    let p = params_of(url);

    Ok((
        name_of(url, || format!("TUIC {server}")),
        Outbound::Tuic {
            server,
            port,
            uuid,
            password,
            congestion_control: p
                .get("congestion_control")
                .cloned()
                .filter(non_empty),
            tls: build_tls(&p, true),
        },
    ))
}

fn parse_shadowsocks(rest: &str) -> Result<(String, Outbound), String> {
    let (body, name) = match rest.split_once('#') {
        Some((b, n)) => (b, decode(n)),
        None => (rest, String::new()),
    };
    let body = body.split('?').next().unwrap_or(body); // отбрасываем plugin-параметры

    let (method, password, server, port) = if let Some((userinfo, hostport)) = body.rsplit_once('@')
    {
        // SIP002: userinfo = base64(method:password) или открытым текстом
        let creds = b64_decode(userinfo).unwrap_or_else(|| userinfo.to_string());
        let (m, p) = creds
            .split_once(':')
            .ok_or("ss: не удалось разобрать method:password")?;
        let (h, port) = split_hostport(hostport)?;
        (m.to_string(), p.to_string(), h, port)
    } else {
        // legacy: base64(method:password@host:port)
        let decoded = b64_decode(body).ok_or("ss: не удалось декодировать base64")?;
        let (creds, hostport) = decoded
            .rsplit_once('@')
            .ok_or("ss: неверный формат после декодирования")?;
        let (m, p) = creds.split_once(':').ok_or("ss: нет method:password")?;
        let (h, port) = split_hostport(hostport)?;
        (m.to_string(), p.to_string(), h, port)
    };

    let name = if name.is_empty() {
        format!("Shadowsocks {server}")
    } else {
        name
    };
    Ok((
        name,
        Outbound::Shadowsocks {
            server,
            port,
            method,
            password,
        },
    ))
}

fn parse_vmess(rest: &str) -> Result<(String, Outbound), String> {
    let json = b64_decode(rest).ok_or("vmess: не удалось декодировать base64")?;
    let v: Value = serde_json::from_str(&json).map_err(|e| format!("vmess: JSON: {e}"))?;

    let server = v.get("add").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let port = v.get("port").and_then(flexible_u64).unwrap_or(0) as u16;
    let uuid = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let alter_id = v.get("aid").and_then(flexible_u64).unwrap_or(0) as u32;
    let security = v.get("scy").and_then(|x| x.as_str()).map(String::from).filter(non_empty);

    let tls_on = v.get("tls").and_then(|x| x.as_str()).map(|s| s == "tls").unwrap_or(false);
    let host = v.get("host").and_then(|x| x.as_str()).map(String::from).filter(non_empty);
    let mut tls = TlsOpts::default();
    if tls_on {
        tls.enabled = true;
        tls.sni = v
            .get("sni")
            .and_then(|x| x.as_str())
            .map(String::from)
            .filter(non_empty)
            .or_else(|| host.clone());
    }

    let transport = match v.get("net").and_then(|x| x.as_str()) {
        Some("ws") => Some(Transport::Ws {
            path: v.get("path").and_then(|x| x.as_str()).map(String::from).filter(non_empty),
            host,
        }),
        Some("grpc") => Some(Transport::Grpc {
            service_name: v.get("path").and_then(|x| x.as_str()).map(String::from).filter(non_empty),
        }),
        _ => None,
    };

    let name = v
        .get("ps")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("VMess {server}"));

    Ok((
        name,
        Outbound::Vmess {
            server,
            port,
            uuid,
            alter_id,
            security,
            tls,
            transport,
        },
    ))
}

// --- Amnezia vpn:// -----------------------------------------------------------

/// Декодирует контейнер Amnezia `vpn://` и мапит поддержанный контейнер в Outbound.
/// Формат: base64url(qCompress) = [4 байта BE длина][zlib(JSON)].
fn from_amnezia(s: &str) -> Result<(String, Outbound), String> {
    let blob = s.trim_start_matches("vpn://").trim();
    let bytes = b64_decode_bytes(blob).ok_or("vpn://: не удалось декодировать base64")?;
    if bytes.len() < 5 {
        return Err("vpn://: слишком короткие данные".into());
    }
    // пропускаем 4-байтовый префикс длины qCompress, распаковываем zlib
    let mut decoder = flate2::read::ZlibDecoder::new(&bytes[4..]);
    let mut json = String::new();
    std::io::Read::read_to_string(&mut decoder, &mut json)
        .map_err(|e| format!("vpn://: распаковка не удалась: {e}"))?;
    let cfg: Value = serde_json::from_str(&json).map_err(|e| format!("vpn://: JSON: {e}"))?;

    let default = cfg.get("defaultContainer").and_then(|v| v.as_str()).unwrap_or("");
    let containers = cfg
        .get("containers")
        .and_then(|v| v.as_array())
        .ok_or("vpn://: нет containers")?;
    let container = containers
        .iter()
        .find(|c| c.get("container").and_then(|v| v.as_str()) == Some(default))
        .or_else(|| containers.first())
        .ok_or("vpn://: пустой список containers")?;
    let ctype = container.get("container").and_then(|v| v.as_str()).unwrap_or("");

    let name = cfg
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| cfg.get("hostName").and_then(|v| v.as_str()))
        .unwrap_or("Amnezia")
        .to_string();

    if ctype.contains("xray") {
        let last = container
            .get("xray")
            .and_then(|x| x.get("last_config"))
            .and_then(|v| v.as_str())
            .ok_or("vpn://: нет xray.last_config")?;
        let xray: Value =
            serde_json::from_str(last).map_err(|e| format!("vpn://: xray config: {e}"))?;
        Ok((name, xray_to_outbound(&xray)?))
    } else if ctype.contains("awg") || ctype.contains("wireguard") {
        let awg = container
            .get("awg")
            .or_else(|| container.get("wireguard"))
            .ok_or("vpn://: контейнер без объекта awg/wireguard")?;
        // last_config (JSON-строка) содержит готовый .conf и mtu
        let last: Option<Value> = awg
            .get("last_config")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str(s).ok());
        // .conf: awg.config либо last_config.config
        let raw = awg
            .get("config")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                last.as_ref()?
                    .get("config")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .ok_or("vpn://: не найден .conf (awg.config / last_config.config)")?;
        // DNS из контейнера (конфиг рассчитан на них; вторичный обычно публичный фолбэк)
        let dns1 = cfg.get("dns1").and_then(|v| v.as_str()).unwrap_or("1.1.1.1");
        let dns2 = cfg.get("dns2").and_then(|v| v.as_str()).unwrap_or("1.0.0.1");
        let mut conf = raw
            .replace("$PRIMARY_DNS", dns1)
            .replace("$SECONDARY_DNS", dns2);
        // MTU как у клиента Amnezia (из awg.mtu или last_config.mtu) — если в .conf его нет
        if !conf.contains("MTU") {
            let mtu = awg
                .get("mtu")
                .and_then(flexible_u64)
                .or_else(|| last.as_ref().and_then(|l| l.get("mtu")).and_then(flexible_u64));
            if let Some(m) = mtu {
                conf = conf.replacen("[Interface]\n", &format!("[Interface]\nMTU = {m}\n"), 1);
            }
        }
        let (server, port) = awg_endpoint(&conf).unwrap_or_else(|| {
            let host = cfg.get("hostName").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let port = awg.get("port").and_then(flexible_u64).unwrap_or(0) as u16;
            (host, port)
        });
        Ok((name, Outbound::AmneziaWg { config: conf, server, port }))
    } else if ctype.contains("openvpn") {
        Err("OpenVPN-контейнер не поддерживается (sing-box без OpenVPN)".into())
    } else {
        Err(format!("контейнер Amnezia «{ctype}» пока не поддержан"))
    }
}

/// Берёт первый «боевой» outbound из Xray-конфига и мапит в наш Outbound.
fn xray_to_outbound(xray: &Value) -> Result<Outbound, String> {
    let outbounds = xray
        .get("outbounds")
        .and_then(|v| v.as_array())
        .ok_or("xray: нет outbounds")?;
    let ob = outbounds
        .iter()
        .find(|o| {
            matches!(
                o.get("protocol").and_then(|v| v.as_str()),
                Some("vless") | Some("vmess") | Some("trojan") | Some("shadowsocks")
            )
        })
        .ok_or("xray: нет поддерживаемого outbound (vless/vmess/trojan/ss)")?;

    let proto = ob.get("protocol").and_then(|v| v.as_str()).unwrap_or("");
    let settings = ob.get("settings");
    let stream = ob.get("streamSettings");

    match proto {
        "vless" => {
            let node = vnext_first(settings)?;
            let user = users_first(node)?;
            let (tls, transport) = xray_stream(stream);
            Ok(Outbound::Vless {
                server: xstr(node, "address"),
                port: xport(node),
                uuid: xstr(user, "id"),
                flow: xopt(user, "flow"),
                tls,
                transport,
            })
        }
        "vmess" => {
            let node = vnext_first(settings)?;
            let user = users_first(node)?;
            let (tls, transport) = xray_stream(stream);
            Ok(Outbound::Vmess {
                server: xstr(node, "address"),
                port: xport(node),
                uuid: xstr(user, "id"),
                alter_id: user.get("alterId").and_then(Value::as_u64).unwrap_or(0) as u32,
                security: xopt(user, "security"),
                tls,
                transport,
            })
        }
        "trojan" => {
            let srv = servers_first(settings)?;
            let (tls, transport) = xray_stream(stream);
            Ok(Outbound::Trojan {
                server: xstr(srv, "address"),
                port: xport(srv),
                password: xstr(srv, "password"),
                tls,
                transport,
            })
        }
        "shadowsocks" => {
            let srv = servers_first(settings)?;
            Ok(Outbound::Shadowsocks {
                server: xstr(srv, "address"),
                port: xport(srv),
                method: xstr(srv, "method"),
                password: xstr(srv, "password"),
            })
        }
        other => Err(format!("xray: протокол «{other}» пока не поддержан")),
    }
}

/// streamSettings Xray → (TlsOpts, Transport).
fn xray_stream(stream: Option<&Value>) -> (TlsOpts, Option<Transport>) {
    let Some(stream) = stream else {
        return (TlsOpts::default(), None);
    };
    let security = stream.get("security").and_then(|v| v.as_str()).unwrap_or("none");
    let mut tls = TlsOpts::default();
    match security {
        "reality" => {
            let r = stream.get("realitySettings");
            tls.enabled = true;
            tls.sni = r.and_then(|x| xopt(x, "serverName"));
            tls.fingerprint = r.and_then(|x| xopt(x, "fingerprint"));
            tls.reality_public_key = r.and_then(|x| xopt(x, "publicKey"));
            tls.reality_short_id = r.and_then(|x| xopt(x, "shortId"));
        }
        "tls" => {
            let t = stream.get("tlsSettings");
            tls.enabled = true;
            tls.sni = t.and_then(|x| xopt(x, "serverName"));
            tls.fingerprint = t.and_then(|x| xopt(x, "fingerprint"));
            tls.insecure = t
                .and_then(|x| x.get("allowInsecure"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }
        _ => {}
    }

    let network = stream.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");
    let transport = match network {
        "ws" => {
            let ws = stream.get("wsSettings");
            Some(Transport::Ws {
                path: ws.and_then(|x| xopt(x, "path")),
                host: ws
                    .and_then(|x| x.get("headers"))
                    .and_then(|h| h.get("Host"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .filter(|s| !s.is_empty()),
            })
        }
        "grpc" => Some(Transport::Grpc {
            service_name: stream.get("grpcSettings").and_then(|x| xopt(x, "serviceName")),
        }),
        _ => None,
    };
    (tls, transport)
}

fn vnext_first(settings: Option<&Value>) -> Result<&Value, String> {
    settings
        .and_then(|s| s.get("vnext"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or("xray: нет settings.vnext".into())
}

fn servers_first(settings: Option<&Value>) -> Result<&Value, String> {
    settings
        .and_then(|s| s.get("servers"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or("xray: нет settings.servers".into())
}

fn users_first(node: &Value) -> Result<&Value, String> {
    node.get("users")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or("xray: нет users".into())
}

fn xstr(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

fn xopt(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(String::from).filter(|s| !s.is_empty())
}

fn xport(v: &Value) -> u16 {
    v.get("port")
        .and_then(flexible_u64)
        .unwrap_or(443) as u16
}

/// Достаёт `Endpoint = host:port` из текста AmneziaWG `.conf`.
fn awg_endpoint(conf: &str) -> Option<(String, u16)> {
    for line in conf.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Endpoint") {
            let val = rest
                .trim_start_matches(|c: char| c == '=' || c.is_whitespace())
                .trim();
            let (host, port) = val.rsplit_once(':')?;
            return Some((host.to_string(), port.parse().ok()?));
        }
    }
    None
}

// --- JSON --------------------------------------------------------------------

fn from_json(s: &str) -> Result<(String, Outbound), String> {
    let value: Value = serde_json::from_str(s).map_err(|e| format!("JSON не распознан: {e}"))?;

    let ob = if let Some(arr) = value.get("outbounds").and_then(|v| v.as_array()) {
        arr.iter()
            .find(|o| {
                let t = o.get("type").and_then(|t| t.as_str()).unwrap_or("");
                !matches!(t, "direct" | "block" | "dns" | "selector" | "urltest")
            })
            .ok_or("в конфиге нет подходящего outbound")?
    } else {
        &value
    };

    let outbound = outbound_from_singbox(ob)?;
    let name = ob
        .get("tag")
        .and_then(|t| t.as_str())
        .map(|t| t.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Импортированный профиль".to_string());

    Ok((name, outbound))
}

fn outbound_from_singbox(o: &Value) -> Result<Outbound, String> {
    let kind = o
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("у outbound нет поля type")?;
    let server = req_str(o, "server");
    let port = o.get("server_port").and_then(Value::as_u64).unwrap_or(0) as u16;
    let opt = |name: &str| o.get(name).and_then(|v| v.as_str()).map(String::from).filter(non_empty);

    match kind {
        "hysteria2" => {
            let tls = o.get("tls");
            Ok(Outbound::Hysteria2 {
                server,
                port,
                password: req_str(o, "password"),
                sni: tls.and_then(|t| t.get("server_name")).and_then(|v| v.as_str()).map(String::from),
                insecure: tls.and_then(|t| t.get("insecure")).and_then(Value::as_bool).unwrap_or(false),
                obfs_password: o.get("obfs").and_then(|ob| ob.get("password")).and_then(|v| v.as_str()).map(String::from),
                up_mbps: o.get("up_mbps").and_then(Value::as_u64).map(|n| n as u32),
                down_mbps: o.get("down_mbps").and_then(Value::as_u64).map(|n| n as u32),
            })
        }
        "socks" => Ok(Outbound::Socks {
            server,
            port,
            username: opt("username"),
            password: opt("password"),
        }),
        "http" => Ok(Outbound::Http {
            server,
            port,
            username: opt("username"),
            password: opt("password"),
            tls: o.get("tls").and_then(|t| t.get("enabled")).and_then(Value::as_bool).unwrap_or(false),
        }),
        "shadowsocks" => Ok(Outbound::Shadowsocks {
            server,
            port,
            method: req_str(o, "method"),
            password: req_str(o, "password"),
        }),
        "trojan" => Ok(Outbound::Trojan {
            server,
            port,
            password: req_str(o, "password"),
            tls: tls_from_singbox(o.get("tls")),
            transport: transport_from_singbox(o.get("transport")),
        }),
        "vless" => Ok(Outbound::Vless {
            server,
            port,
            uuid: req_str(o, "uuid"),
            flow: opt("flow"),
            tls: tls_from_singbox(o.get("tls")),
            transport: transport_from_singbox(o.get("transport")),
        }),
        "vmess" => Ok(Outbound::Vmess {
            server,
            port,
            uuid: req_str(o, "uuid"),
            alter_id: o.get("alter_id").and_then(Value::as_u64).unwrap_or(0) as u32,
            security: opt("security"),
            tls: tls_from_singbox(o.get("tls")),
            transport: transport_from_singbox(o.get("transport")),
        }),
        "tuic" => Ok(Outbound::Tuic {
            server,
            port,
            uuid: req_str(o, "uuid"),
            password: req_str(o, "password"),
            congestion_control: opt("congestion_control"),
            tls: tls_from_singbox(o.get("tls")),
        }),
        other => Err(format!("тип outbound «{other}» пока не поддерживается")),
    }
}

fn tls_from_singbox(t: Option<&Value>) -> TlsOpts {
    let Some(t) = t else { return TlsOpts::default() };
    let mut tls = TlsOpts {
        enabled: t.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        sni: t.get("server_name").and_then(|v| v.as_str()).map(String::from),
        insecure: t.get("insecure").and_then(Value::as_bool).unwrap_or(false),
        ..Default::default()
    };
    if let Some(alpn) = t.get("alpn").and_then(|v| v.as_array()) {
        tls.alpn = alpn.iter().filter_map(|x| x.as_str().map(String::from)).collect();
    }
    tls.fingerprint = t.get("utls").and_then(|u| u.get("fingerprint")).and_then(|v| v.as_str()).map(String::from);
    if let Some(r) = t.get("reality") {
        tls.reality_public_key = r.get("public_key").and_then(|v| v.as_str()).map(String::from);
        tls.reality_short_id = r.get("short_id").and_then(|v| v.as_str()).map(String::from);
    }
    tls
}

fn transport_from_singbox(t: Option<&Value>) -> Option<Transport> {
    let t = t?;
    match t.get("type").and_then(|v| v.as_str())? {
        "ws" => Some(Transport::Ws {
            path: t.get("path").and_then(|v| v.as_str()).map(String::from),
            host: t.get("headers").and_then(|h| h.get("Host")).and_then(|v| v.as_str()).map(String::from),
        }),
        "grpc" => Some(Transport::Grpc {
            service_name: t.get("service_name").and_then(|v| v.as_str()).map(String::from),
        }),
        _ => None,
    }
}

// --- общие хелперы -----------------------------------------------------------

fn build_tls(p: &Params, default_enabled: bool) -> TlsOpts {
    let security = p.get("security").map(|s| s.to_lowercase());
    let mut enabled = match security.as_deref() {
        Some("tls") | Some("reality") | Some("xtls") => true,
        Some("none") | Some("") => false,
        _ => default_enabled,
    };
    let reality_public_key = p.get("pbk").cloned().filter(non_empty);
    if reality_public_key.is_some() {
        enabled = true;
    }
    if !enabled {
        return TlsOpts::default();
    }
    TlsOpts {
        enabled: true,
        sni: p.get("sni").or_else(|| p.get("peer")).cloned().filter(non_empty),
        insecure: is_true(p.get("insecure").or_else(|| p.get("allowInsecure"))),
        alpn: p
            .get("alpn")
            .map(|a| a.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default(),
        fingerprint: p.get("fp").cloned().filter(non_empty),
        reality_short_id: p.get("sid").cloned().filter(non_empty),
        reality_public_key,
    }
}

fn build_transport(p: &Params) -> Option<Transport> {
    match p.get("type").map(|s| s.to_lowercase()).as_deref() {
        Some("ws") => Some(Transport::Ws {
            path: p.get("path").cloned().filter(non_empty),
            host: p.get("host").cloned().filter(non_empty),
        }),
        Some("grpc") => Some(Transport::Grpc {
            service_name: p
                .get("serviceName")
                .or_else(|| p.get("servicename"))
                .cloned()
                .filter(non_empty),
        }),
        _ => None,
    }
}

fn params_of(url: &Url) -> Params {
    url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
}

fn host_of(url: &Url) -> Result<String, String> {
    Ok(url.host_str().ok_or("в ссылке нет адреса сервера")?.to_string())
}

fn name_of(url: &Url, fallback: impl FnOnce() -> String) -> String {
    url.fragment()
        .map(decode)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(fallback)
}

fn split_hostport(s: &str) -> Result<(String, u16), String> {
    let (host, port) = s.rsplit_once(':').ok_or("ожидался host:port")?;
    let port = port.parse::<u16>().map_err(|_| "неверный порт")?;
    Ok((host.to_string(), port))
}

fn decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().into_owned()
}

fn b64_decode(s: &str) -> Option<String> {
    let s = s.trim().trim_end_matches('=');
    let bytes = general_purpose::STANDARD_NO_PAD
        .decode(s)
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(s))
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// Декод base64 в сырые байты (для бинарных контейнеров вроде Amnezia).
fn b64_decode_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim().trim_end_matches('=');
    general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(s))
        .ok()
}

fn req_str(o: &Value, key: &str) -> String {
    o.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string()
}

fn flexible_u64(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

fn non_empty(s: &String) -> bool {
    !s.is_empty()
}

fn is_true(v: Option<&String>) -> bool {
    matches!(v.map(|s| s.as_str()), Some("1") | Some("true"))
}

/// Парсит ведущее число из строк вида "100", "100 mbps".
fn parse_mbps(s: &str) -> Option<u32> {
    let digits: String = s.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ss_sip002() {
        let creds = general_purpose::STANDARD_NO_PAD.encode("aes-256-gcm:pass");
        let (name, ob) = parse(&format!("ss://{creds}@1.2.3.4:8388#nodeA")).unwrap();
        assert_eq!(name, "nodeA");
        match ob {
            Outbound::Shadowsocks { server, port, method, password } => {
                assert_eq!((server.as_str(), port), ("1.2.3.4", 8388));
                assert_eq!((method.as_str(), password.as_str()), ("aes-256-gcm", "pass"));
            }
            _ => panic!("expected shadowsocks"),
        }
    }

    #[test]
    fn ss_legacy() {
        let blob = general_purpose::STANDARD_NO_PAD.encode("chacha20-ietf-poly1305:pw@1.2.3.4:8388");
        let (_, ob) = parse(&format!("ss://{blob}#x")).unwrap();
        match ob {
            Outbound::Shadowsocks { method, password, port, .. } => {
                assert_eq!(method, "chacha20-ietf-poly1305");
                assert_eq!(password, "pw");
                assert_eq!(port, 8388);
            }
            _ => panic!("expected shadowsocks"),
        }
    }

    #[test]
    fn vless_reality_ws() {
        let link = "vless://11111111-1111-1111-1111-111111111111@1.2.3.4:443\
            ?security=reality&sni=ex.com&pbk=KEY&sid=ab&type=ws&path=%2Fp&flow=xtls-rprx-vision#myvless";
        let (name, ob) = parse(link).unwrap();
        assert_eq!(name, "myvless");
        match ob {
            Outbound::Vless { server, port, uuid, flow, tls, transport } => {
                assert_eq!((server.as_str(), port), ("1.2.3.4", 443));
                assert_eq!(uuid, "11111111-1111-1111-1111-111111111111");
                assert_eq!(flow.as_deref(), Some("xtls-rprx-vision"));
                assert!(tls.enabled);
                assert_eq!(tls.sni.as_deref(), Some("ex.com"));
                assert_eq!(tls.reality_public_key.as_deref(), Some("KEY"));
                assert_eq!(tls.reality_short_id.as_deref(), Some("ab"));
                match transport {
                    Some(Transport::Ws { path, .. }) => assert_eq!(path.as_deref(), Some("/p")),
                    _ => panic!("expected ws"),
                }
            }
            _ => panic!("expected vless"),
        }
    }

    #[test]
    fn trojan_ws() {
        let (_, ob) = parse("trojan://secret@1.2.3.4:443?sni=ex.com&type=ws&path=%2Fp#t").unwrap();
        match ob {
            Outbound::Trojan { password, tls, transport, .. } => {
                assert_eq!(password, "secret");
                assert!(tls.enabled);
                assert_eq!(tls.sni.as_deref(), Some("ex.com"));
                assert!(matches!(transport, Some(Transport::Ws { .. })));
            }
            _ => panic!("expected trojan"),
        }
    }

    #[test]
    fn tuic_basic() {
        let (_, ob) = parse("tuic://uuidval:pw@1.2.3.4:443?congestion_control=bbr&sni=ex.com#x").unwrap();
        match ob {
            Outbound::Tuic { uuid, password, congestion_control, tls, .. } => {
                assert_eq!((uuid.as_str(), password.as_str()), ("uuidval", "pw"));
                assert_eq!(congestion_control.as_deref(), Some("bbr"));
                assert!(tls.enabled);
            }
            _ => panic!("expected tuic"),
        }
    }

    #[test]
    fn vmess_base64_json() {
        let json = r#"{"v":"2","ps":"myvmess","add":"1.2.3.4","port":"443","id":"uuidv","aid":"0","net":"ws","host":"ex.com","path":"/p","tls":"tls","sni":"ex.com","scy":"auto"}"#;
        let b64 = general_purpose::STANDARD_NO_PAD.encode(json);
        let (name, ob) = parse(&format!("vmess://{b64}")).unwrap();
        assert_eq!(name, "myvmess");
        match ob {
            Outbound::Vmess { server, port, uuid, security, tls, transport, .. } => {
                assert_eq!((server.as_str(), port), ("1.2.3.4", 443));
                assert_eq!(uuid, "uuidv");
                assert_eq!(security.as_deref(), Some("auto"));
                assert!(tls.enabled);
                assert_eq!(tls.sni.as_deref(), Some("ex.com"));
                assert!(matches!(transport, Some(Transport::Ws { .. })));
            }
            _ => panic!("expected vmess"),
        }
    }

    #[test]
    fn hysteria2_link_and_json() {
        let (_, ob) = parse("hysteria2://pw@1.2.3.4:443?sni=ex.com&insecure=1#h").unwrap();
        match ob {
            Outbound::Hysteria2 { password, sni, insecure, .. } => {
                assert_eq!(password, "pw");
                assert_eq!(sni.as_deref(), Some("ex.com"));
                assert!(insecure);
            }
            _ => panic!("expected hysteria2"),
        }

        let js = r#"{"type":"hysteria2","server":"1.2.3.4","server_port":443,"password":"pw","tls":{"server_name":"ex.com"}}"#;
        assert!(matches!(parse(js).unwrap().1, Outbound::Hysteria2 { .. }));
    }

    fn pack_amnezia(amnezia: &str) -> String {
        use std::io::Write;
        let mut enc =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(amnezia.as_bytes()).unwrap();
        let zlib = enc.finish().unwrap();
        let mut raw = (amnezia.len() as u32).to_be_bytes().to_vec();
        raw.extend_from_slice(&zlib);
        format!("vpn://{}", general_purpose::URL_SAFE_NO_PAD.encode(&raw))
    }

    #[test]
    fn amnezia_vpn_awg() {
        let conf = "[Interface]\nAddress = 10.8.1.7/32\nDNS = $PRIMARY_DNS, $SECONDARY_DNS\nPrivateKey = KEY\nJc = 5\n[Peer]\nPublicKey = PUB\nEndpoint = 1.2.3.4:34196\nAllowedIPs = 0.0.0.0/0, ::/0\n";
        // реальная структура: .conf и mtu внутри awg.last_config (JSON-строка)
        let last_config = serde_json::json!({ "config": conf, "mtu": "1376" }).to_string();
        let amnezia = serde_json::json!({
            "defaultContainer": "amnezia-awg2",
            "description": "FR-AWG",
            "hostName": "1.2.3.4",
            "dns1": "1.1.1.1",
            "dns2": "8.8.8.8",
            "containers": [ { "container": "amnezia-awg2", "awg": { "last_config": last_config, "port": "34196" } } ]
        })
        .to_string();

        let (name, ob) = parse(&pack_amnezia(&amnezia)).unwrap();
        assert_eq!(name, "FR-AWG");
        match ob {
            Outbound::AmneziaWg { config, server, port } => {
                assert_eq!((server.as_str(), port), ("1.2.3.4", 34196));
                assert!(config.contains("Jc = 5"));
                assert!(config.contains("DNS = 1.1.1.1, 8.8.8.8"));
                assert!(config.contains("MTU = 1376"));
                assert!(!config.contains("$PRIMARY_DNS"));
            }
            _ => panic!("expected amneziawg"),
        }
    }

    #[test]
    fn amnezia_vpn_xray_vless() {
        use std::io::Write;
        // встроенный Xray-конфиг (как в контейнере amnezia-xray)
        let xray_inner = r#"{"outbounds":[{"protocol":"vless","settings":{"vnext":[{"address":"1.2.3.4","port":443,"users":[{"id":"abc-uuid","flow":"xtls-rprx-vision"}]}]},"streamSettings":{"network":"tcp","security":"reality","realitySettings":{"fingerprint":"chrome","serverName":"www.example.com","publicKey":"PUBKEY","shortId":"ab12"}}}]}"#;
        let amnezia = serde_json::json!({
            "defaultContainer": "amnezia-xray",
            "description": "TestFR",
            "hostName": "1.2.3.4",
            "containers": [ { "container": "amnezia-xray", "xray": { "last_config": xray_inner } } ]
        })
        .to_string();

        // qCompress: [4 байта BE длина][zlib]
        let mut enc =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(amnezia.as_bytes()).unwrap();
        let zlib = enc.finish().unwrap();
        let mut raw = (amnezia.len() as u32).to_be_bytes().to_vec();
        raw.extend_from_slice(&zlib);
        let blob = general_purpose::URL_SAFE_NO_PAD.encode(&raw);

        let (name, ob) = parse(&format!("vpn://{blob}")).unwrap();
        assert_eq!(name, "TestFR");
        match ob {
            Outbound::Vless { server, port, uuid, flow, tls, .. } => {
                assert_eq!((server.as_str(), port), ("1.2.3.4", 443));
                assert_eq!(uuid, "abc-uuid");
                assert_eq!(flow.as_deref(), Some("xtls-rprx-vision"));
                assert!(tls.enabled);
                assert_eq!(tls.sni.as_deref(), Some("www.example.com"));
                assert_eq!(tls.reality_public_key.as_deref(), Some("PUBKEY"));
                assert_eq!(tls.reality_short_id.as_deref(), Some("ab12"));
                assert_eq!(tls.fingerprint.as_deref(), Some("chrome"));
            }
            _ => panic!("expected vless"),
        }
    }
}
