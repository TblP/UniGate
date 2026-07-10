//! Управление системным HTTP-прокси.
//!
//! Phase 3: Windows — через реестр (`Internet Settings`) + уведомление WinINet,
//! чтобы изменение подхватилось без перезапуска приложений. Прав администратора
//! не требует (пишем в HKCU).
//! Phase 10: macOS — через `networksetup` (web/secure/socks-прокси на активном
//! сетевом сервисе). Тоже без прав администратора. Прочие ОС (Linux) — заглушки.

#[cfg(windows)]
mod imp {
    use winreg::enums::*;
    use winreg::RegKey;

    const INTERNET_SETTINGS: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    fn open() -> Result<RegKey, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey_with_flags(INTERNET_SETTINGS, KEY_READ | KEY_WRITE)
            .map_err(|e| format!("не удалось открыть Internet Settings: {e}"))
    }

    /// Уведомляет WinINet, что настройки прокси изменились (немедленный эффект).
    fn notify_changed() {
        use windows_sys::Win32::Networking::WinInet::{
            InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
        };
        unsafe {
            InternetSetOptionW(std::ptr::null_mut(), INTERNET_OPTION_SETTINGS_CHANGED, std::ptr::null_mut(), 0);
            InternetSetOptionW(std::ptr::null_mut(), INTERNET_OPTION_REFRESH, std::ptr::null_mut(), 0);
        }
    }

    pub fn enable(port: u16) -> Result<(), String> {
        let key = open()?;
        key.set_value("ProxyServer", &format!("127.0.0.1:{port}"))
            .map_err(|e| format!("ProxyServer: {e}"))?;
        // локальные адреса — в обход прокси
        key.set_value("ProxyOverride", &"localhost;127.*;10.*;172.16.*;192.168.*;<local>")
            .map_err(|e| format!("ProxyOverride: {e}"))?;
        key.set_value("ProxyEnable", &1u32)
            .map_err(|e| format!("ProxyEnable: {e}"))?;
        notify_changed();
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let key = open()?;
        key.set_value("ProxyEnable", &0u32)
            .map_err(|e| format!("ProxyEnable: {e}"))?;
        notify_changed();
        Ok(())
    }

    /// true, если системный прокси включён и указывает именно на наш порт.
    pub fn is_enabled_for(port: u16) -> bool {
        let Ok(key) = open() else { return false };
        let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
        let server: String = key.get_value("ProxyServer").unwrap_or_default();
        enabled == 1 && server == format!("127.0.0.1:{port}")
    }
}

#[cfg(target_os = "macos")]
mod imp {
    //! Системный прокси на macOS через `networksetup`. Ставим web (HTTP),
    //! secure web (HTTPS) и SOCKS прокси на 127.0.0.1:<port> — sing-box
    //! `mixed`-inbound слушает и HTTP, и SOCKS на одном порту.
    use std::process::Command;

    /// Захват stdout команды `networksetup <args...>`.
    fn output(args: &[&str]) -> Option<String> {
        let out = Command::new("networksetup").args(args).output().ok()?;
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Включённые сетевые сервисы с реальным аппаратным интерфейсом (en0/en9/…).
    /// Отключённые (`*`) и VPN-псевдосервисы (пустой `Device`) пропускаем: прокси
    /// на loopback (127.0.0.1) имеет смысл ставить только на физические сервисы,
    /// и работает он независимо от того, какой интерфейс сейчас аплинк (в т.ч. если
    /// маршрут по умолчанию уже уводит другой VPN-туннель `utunN`).
    ///
    /// Формат `networksetup -listnetworkserviceorder`:
    /// ```text
    /// (1) Wi-Fi
    /// (Hardware Port: Wi-Fi, Device: en0)
    /// ```
    /// Отключённый сервис — `(*) Name`.
    fn hardware_services() -> Vec<String> {
        let Some(order) = output(&["-listnetworkserviceorder"]) else {
            return Vec::new();
        };
        let mut services = Vec::new();
        let mut pending: Option<String> = None; // имя сервиса, ждём строку деталей
        for line in order.lines() {
            let l = line.trim();
            if l.starts_with("(Hardware Port:") {
                // деталь текущего сервиса — оставляем сервис, только если есть Device
                let device = l
                    .split("Device:")
                    .nth(1)
                    .map(|d| d.trim().trim_end_matches(')').trim())
                    .unwrap_or("");
                if !device.is_empty() {
                    if let Some(name) = pending.take() {
                        services.push(name);
                    }
                } else {
                    pending = None;
                }
            } else if let Some(rest) = l.strip_prefix('(') {
                // строка сервиса "(N) Name" / "(*) Name"
                if let Some(idx) = rest.find(") ") {
                    let marker = &rest[..idx];
                    let name = rest[idx + 2..].trim().to_string();
                    pending = if marker.contains('*') { None } else { Some(name) };
                }
            }
        }
        services
    }

    /// Обход прокси для локальных адресов (аналог `ProxyOverride` на Windows):
    /// localhost/LAN/mDNS — напрямую, иначе ломаются локальные dev-серверы,
    /// принтеры, доступ к роутеру и т.п.
    const BYPASS: &[&str] = &[
        "localhost",
        "127.0.0.1",
        "*.local",
        "169.254/16",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
    ];

    /// Полный набор команд networksetup для включения прокси на сервисах.
    fn enable_cmds(services: &[String], port: u16) -> Vec<Vec<String>> {
        let p = port.to_string();
        let mut cmds = Vec::new();
        for s in services {
            for set in ["-setwebproxy", "-setsecurewebproxy", "-setsocksfirewallproxy"] {
                cmds.push(vec![set.into(), s.clone(), "127.0.0.1".into(), p.clone()]);
            }
            let mut b = vec!["-setproxybypassdomains".to_string(), s.clone()];
            b.extend(BYPASS.iter().map(|x| x.to_string()));
            cmds.push(b);
        }
        cmds
    }

    /// Набор команд для выключения прокси на сервисах.
    fn disable_cmds(services: &[String]) -> Vec<Vec<String>> {
        let mut cmds = Vec::new();
        for s in services {
            for set in [
                "-setwebproxystate",
                "-setsecurewebproxystate",
                "-setsocksfirewallproxystate",
            ] {
                cmds.push(vec![set.into(), s.clone(), "off".into()]);
            }
        }
        cmds
    }

    /// Запуск команды с аргументами-строками (как `run`, но для Vec<String>).
    fn run_args(args: &[String]) -> Result<(), String> {
        let status = Command::new("networksetup")
            .args(args)
            .status()
            .map_err(|e| format!("networksetup: {e}"))?;
        if !status.success() {
            return Err(format!(
                "networksetup {:?} завершился с кодом {:?}",
                args,
                status.code()
            ));
        }
        Ok(())
    }

    /// Фолбэк: весь батч команд одним `osascript ... with administrator
    /// privileges`. У админ-пользователей networksetup обычно работает и так,
    /// но на части систем set-команды требуют прав (exit 14) — тогда один
    /// нативный запрос пароля вместо запуска всего приложения от root.
    fn admin_batch(cmds: &[Vec<String>]) -> Result<(), String> {
        let q = |s: &str| s.replace('\'', "'\\''");
        let shell = cmds
            .iter()
            .map(|c| {
                let args = c
                    .iter()
                    .map(|a| format!("'{}'", q(a)))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("/usr/sbin/networksetup {args}")
            })
            .collect::<Vec<_>>()
            .join("; ");
        crate::elevation::osascript_admin(&shell).map(|_| ())
    }

    pub fn enable(port: u16) -> Result<(), String> {
        let services = hardware_services();
        if services.is_empty() {
            return Err("не найдено сетевых сервисов для установки прокси".into());
        }
        // Ставим на все физические сервисы (best-effort): какой из них активен —
        // тот и подхватит; set-команды одновременно включают прокси.
        let mut ok_any = false;
        let mut failed: Vec<Vec<String>> = Vec::new();
        for service in &services {
            let cmds = enable_cmds(std::slice::from_ref(service), port);
            let mut service_ok = true;
            for c in &cmds {
                if let Err(e) = run_args(c) {
                    eprintln!("[sysproxy] {service}: {e}");
                    service_ok = false;
                }
            }
            if service_ok {
                ok_any = true;
            } else {
                failed.extend(cmds);
            }
        }
        if ok_any {
            return Ok(());
        }
        // ни один сервис не настроился без прав — пробуем один раз от админа
        admin_batch(&failed)
            .map_err(|e| format!("не удалось включить системный прокси: {e}"))
    }

    pub fn disable() -> Result<(), String> {
        // снимаем на всех физических сервисах (best-effort)
        let services = hardware_services();
        let cmds = disable_cmds(&services);
        let mut all_ok = true;
        for c in &cmds {
            if run_args(c).is_err() {
                all_ok = false;
            }
        }
        if all_ok || !any_proxy_enabled(&services) {
            return Ok(());
        }
        // без прав снять не вышло, а прокси реально включён — просим пароль,
        // иначе у пользователя останется «мёртвый» прокси без интернета
        admin_batch(&cmds)
    }

    /// true, если на каком-то сервисе включён хотя бы один из наших прокси.
    fn any_proxy_enabled(services: &[String]) -> bool {
        for service in services {
            for get in ["-getwebproxy", "-getsecurewebproxy", "-getsocksfirewallproxy"] {
                if let Some(info) = output(&[get, service]) {
                    if info.lines().any(|l| l.trim() == "Enabled: Yes") {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// true, если на каком-либо сервисе web-прокси включён и указывает на наш порт.
    pub fn is_enabled_for(port: u16) -> bool {
        let want_port = format!("Port: {port}");
        for service in hardware_services() {
            let Some(info) = output(&["-getwebproxy", &service]) else {
                continue;
            };
            let enabled = info.lines().any(|l| l.trim() == "Enabled: Yes");
            let host_ok = info.lines().any(|l| l.trim() == "Server: 127.0.0.1");
            let port_ok = info.lines().any(|l| l.trim() == want_port);
            if enabled && host_ok && port_ok {
                return true;
            }
        }
        false
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn enable(_port: u16) -> Result<(), String> {
        Ok(()) // TODO Linux: системный прокси (gsettings/env) — позже
    }
    pub fn disable() -> Result<(), String> {
        Ok(())
    }
    pub fn is_enabled_for(_port: u16) -> bool {
        false
    }
}

pub use imp::{disable, enable, is_enabled_for};
