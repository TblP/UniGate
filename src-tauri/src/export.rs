//! Экспорт профиля в share-ссылку (обратное к [`crate::import`]).
//!
//! Протоколы со ссылками (hysteria2/vless/vmess/ss/trojan/tuic) → соответствующий
//! URL; socks/http → JSON sing-box outbound; AmneziaWG → `vpn://`.

use crate::models::{Outbound, TlsOpts, Transport};
use base64::{engine::general_purpose, Engine};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use std::io::Write;

// кодируем всё, кроме безопасных символов
const ESCAPE: &AsciiSet = &CONTROLS
    .add(b' ').add(b'"').add(b'#').add(b'<').add(b'>').add(b'?')
    .add(b'`').add(b'{').add(b'}').add(b'/').add(b':').add(b'@')
    .add(b'&').add(b'=').add(b'+').add(b'%');

fn enc(s: &str) -> String {
    utf8_percent_encode(s, ESCAPE).to_string()
}

fn query(pairs: &[(&str, String)]) -> String {
    let parts: Vec<String> = pairs
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{k}={}", enc(v)))
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

fn tls_query(tls: &TlsOpts, default_security: &str) -> Vec<(&'static str, String)> {
    let mut q: Vec<(&'static str, String)> = Vec::new();
    if !tls.enabled {
        q.push(("security", "none".into()));
        return q;
    }
    let security = if tls.reality_public_key.is_some() {
        "reality"
    } else {
        default_security
    };
    q.push(("security", security.into()));
    if let Some(s) = &tls.sni {
        q.push(("sni", s.clone()));
    }
    if let Some(fp) = &tls.fingerprint {
        q.push(("fp", fp.clone()));
    }
    if let Some(pbk) = &tls.reality_public_key {
        q.push(("pbk", pbk.clone()));
    }
    if let Some(sid) = &tls.reality_short_id {
        q.push(("sid", sid.clone()));
    }
    if !tls.alpn.is_empty() {
        q.push(("alpn", tls.alpn.join(",")));
    }
    if tls.insecure {
        q.push(("insecure", "1".into()));
    }
    q
}

fn transport_query(transport: &Option<Transport>) -> Vec<(&'static str, String)> {
    match transport {
        None => vec![("type", "tcp".into())],
        Some(Transport::Ws { path, host }) => {
            let mut q = vec![("type", "ws".into())];
            if let Some(p) = path {
                q.push(("path", p.clone()));
            }
            if let Some(h) = host {
                q.push(("host", h.clone()));
            }
            q
        }
        Some(Transport::Grpc { service_name }) => {
            let mut q = vec![("type", "grpc".into())];
            if let Some(s) = service_name {
                q.push(("serviceName", s.clone()));
            }
            q
        }
    }
}

/// Упаковывает AmneziaWG-конфиг в совместимый с нашим импортом контейнер
/// `vpn://base64url(qCompress(JSON))`. В `.conf` уже находятся endpoint,
/// ключи и все параметры обфускации, поэтому ссылка полностью обратима.
fn amnezia_link(name: &str, config: &str, server: &str, port: u16) -> String {
    let last_config = serde_json::json!({ "config": config }).to_string();
    let container = serde_json::json!({
        "defaultContainer": "amnezia-awg2",
        "description": name,
        "hostName": server,
        "dns1": "1.1.1.1",
        "dns2": "1.0.0.1",
        "containers": [{
            "container": "amnezia-awg2",
            "awg": {
                "last_config": last_config,
                "port": port.to_string(),
                "protocol_version": 2
            }
        }]
    })
    .to_string();

    let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(container.as_bytes())
        .expect("zlib write to Vec cannot fail");
    let compressed = encoder.finish().expect("zlib finish to Vec cannot fail");
    let mut packed = (container.len() as u32).to_be_bytes().to_vec();
    packed.extend_from_slice(&compressed);
    format!(
        "vpn://{}",
        general_purpose::URL_SAFE_NO_PAD.encode(packed)
    )
}

/// JSON-представление профиля — sing-box outbound-объект (для вставки в конфиг
/// или импорта как JSON). AmneziaWG отдаём его `.conf` — sing-box outbound'а у
/// него нет (движок отдельный).
pub fn to_json(outbound: &Outbound) -> String {
    match outbound {
        Outbound::AmneziaWg { config, .. } => config.clone(),
        _ => serde_json::to_string_pretty(&crate::config::outbound_to_singbox(outbound))
            .unwrap_or_default(),
    }
}

/// Строит share-ссылку/представление профиля.
pub fn to_share(name: &str, outbound: &Outbound) -> String {
    match outbound {
        Outbound::Shadowsocks { server, port, method, password } => {
            let userinfo = general_purpose::STANDARD_NO_PAD.encode(format!("{method}:{password}"));
            format!("ss://{userinfo}@{server}:{port}#{}", enc(name))
        }
        Outbound::Trojan { server, port, password, tls, transport } => {
            let mut q = tls_query(tls, "tls");
            q.extend(transport_query(transport));
            format!("trojan://{}@{server}:{port}{}#{}", enc(password), query(&q), enc(name))
        }
        Outbound::Vless { server, port, uuid, flow, tls, transport } => {
            let mut q: Vec<(&str, String)> = vec![("encryption", "none".into())];
            if let Some(f) = flow {
                q.push(("flow", f.clone()));
            }
            q.extend(tls_query(tls, "tls"));
            q.extend(transport_query(transport));
            format!("vless://{uuid}@{server}:{port}{}#{}", query(&q), enc(name))
        }
        Outbound::Tuic { server, port, uuid, password, congestion_control, tls } => {
            let mut q = tls_query(tls, "tls");
            if let Some(cc) = congestion_control {
                q.push(("congestion_control", cc.clone()));
            }
            format!("tuic://{uuid}:{}@{server}:{port}{}#{}", enc(password), query(&q), enc(name))
        }
        Outbound::Hysteria2 { server, port, password, sni, insecure, obfs_password, up_mbps, down_mbps } => {
            let mut q: Vec<(&str, String)> = Vec::new();
            if let Some(s) = sni {
                q.push(("sni", s.clone()));
            }
            if *insecure {
                q.push(("insecure", "1".into()));
            }
            if let Some(op) = obfs_password {
                q.push(("obfs", "salamander".into()));
                q.push(("obfs-password", op.clone()));
            }
            if let Some(up) = up_mbps {
                q.push(("up", up.to_string()));
            }
            if let Some(down) = down_mbps {
                q.push(("down", down.to_string()));
            }
            format!("hysteria2://{}@{server}:{port}{}#{}", enc(password), query(&q), enc(name))
        }
        Outbound::Vmess { server, port, uuid, alter_id, security, tls, transport } => {
            let (net, host, path) = match transport {
                None => ("tcp", String::new(), String::new()),
                Some(Transport::Ws { path, host }) => {
                    ("ws", host.clone().unwrap_or_default(), path.clone().unwrap_or_default())
                }
                Some(Transport::Grpc { service_name }) => {
                    ("grpc", String::new(), service_name.clone().unwrap_or_default())
                }
            };
            let json = serde_json::json!({
                "v": "2",
                "ps": name,
                "add": server,
                "port": port.to_string(),
                "id": uuid,
                "aid": alter_id.to_string(),
                "scy": security.clone().unwrap_or_else(|| "auto".into()),
                "net": net,
                "host": host,
                "path": path,
                "tls": if tls.enabled { "tls" } else { "" },
                "sni": tls.sni.clone().unwrap_or_default()
            });
            format!("vmess://{}", general_purpose::STANDARD_NO_PAD.encode(json.to_string()))
        }
        // socks/http без общепринятой share-ссылки → JSON sing-box outbound
        Outbound::Socks { .. } | Outbound::Http { .. } => {
            serde_json::to_string_pretty(&crate::config::outbound_to_singbox(outbound))
                .unwrap_or_default()
        }
        Outbound::AmneziaWg {
            config,
            server,
            port,
        } => amnezia_link(name, config, server, *port),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import;

    fn roundtrip(name: &str, ob: Outbound) {
        let link = to_share(name, &ob);
        let (parsed_name, parsed) = import::parse(&link).unwrap();
        assert_eq!(parsed_name, name, "link: {link}");
        assert_eq!(parsed, ob, "link: {link}");
    }

    #[test]
    fn roundtrip_hysteria2() {
        roundtrip(
            "HY2",
            Outbound::Hysteria2 {
                server: "ex.com".into(),
                port: 443,
                password: "pw".into(),
                sni: Some("sni.com".into()),
                insecure: true,
                obfs_password: Some("obf".into()),
                up_mbps: Some(100),
                down_mbps: Some(200),
            },
        );
    }

    #[test]
    fn roundtrip_vless_reality() {
        roundtrip(
            "VL",
            Outbound::Vless {
                server: "1.2.3.4".into(),
                port: 443,
                uuid: "11111111-1111-1111-1111-111111111111".into(),
                flow: Some("xtls-rprx-vision".into()),
                tls: TlsOpts {
                    enabled: true,
                    sni: Some("www.example.com".into()),
                    insecure: false,
                    alpn: vec![],
                    fingerprint: Some("chrome".into()),
                    reality_public_key: Some("PUBKEY".into()),
                    reality_short_id: Some("ab12".into()),
                },
                transport: Some(Transport::Ws {
                    path: Some("/p".into()),
                    host: Some("h.example".into()),
                }),
            },
        );
    }

    #[test]
    fn roundtrip_ss_trojan_tuic() {
        roundtrip(
            "SS",
            Outbound::Shadowsocks {
                server: "1.2.3.4".into(),
                port: 8388,
                method: "aes-256-gcm".into(),
                password: "pw".into(),
            },
        );
        roundtrip(
            "TR",
            Outbound::Trojan {
                server: "ex.com".into(),
                port: 443,
                password: "secret".into(),
                tls: TlsOpts { enabled: true, sni: Some("ex.com".into()), ..Default::default() },
                transport: None,
            },
        );
        roundtrip(
            "TU",
            Outbound::Tuic {
                server: "ex.com".into(),
                port: 443,
                uuid: "uid".into(),
                password: "pw".into(),
                congestion_control: Some("bbr".into()),
                tls: TlsOpts { enabled: true, sni: Some("ex.com".into()), ..Default::default() },
            },
        );
    }

    #[test]
    fn roundtrip_vmess_ws() {
        roundtrip(
            "VM",
            Outbound::Vmess {
                server: "1.2.3.4".into(),
                port: 443,
                uuid: "uid".into(),
                alter_id: 0,
                security: Some("auto".into()),
                tls: TlsOpts { enabled: true, sni: Some("sni.com".into()), ..Default::default() },
                transport: Some(Transport::Ws {
                    path: Some("/p".into()),
                    host: Some("host.com".into()),
                }),
            },
        );
    }

    #[test]
    fn roundtrip_amneziawg_vpn_link() {
        let outbound = Outbound::AmneziaWg {
            config: "[Interface]\nAddress = 10.8.1.7/32\nPrivateKey = KEY\nJc = 5\nI1 = <b 0x01>\nI2 =\n[Peer]\nPublicKey = PUB\nEndpoint = 1.2.3.4:34196\nAllowedIPs = 0.0.0.0/0, ::/0\n".into(),
            server: "1.2.3.4".into(),
            port: 34196,
        };

        let link = to_share("FR-AWG", &outbound);
        assert!(link.starts_with("vpn://"));
        roundtrip("FR-AWG", outbound);
    }
}
