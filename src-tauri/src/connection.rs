//! Менеджер подключения: запуск/останов sidecar sing-box, состояние, события.
//!
//! Состояние живёт в managed-state Tauri (`Connection`). При каждом изменении
//! шлём фронту событие `connection-state`. Если sing-box падает сам —
//! переводим в `Error` и снимаем системный прокси.

use crate::models::{ConnectionState, Mode, Outbound};
use crate::{config, elevation, profiles, settings, stats, storage, sysproxy};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

/// Порт локального mixed-inbound (proxy mode). Phase 3 — фиксированный.
pub const LOCAL_PORT: u16 = 2080;
/// Порт локального SOCKS5 у awg-shim (userspace AmneziaWG). Фиксированный.
pub const SHIM_PORT: u16 = 2081;
/// Конфиг для awg-shim (тот же .conf, что у legacy-движка, но своё имя файла).
const SHIM_CONF: &str = "awg-shim.conf";
const RUNNING_CONFIG: &str = "running-config.json";
const EVENT: &str = "connection-state";
/// Имя AmneziaWG-тоннеля (= имя .conf без расширения) для /installtunnelservice.
/// Только Windows: на macOS движок (awg-quick) адресуется путём к `.conf`.
#[cfg(target_os = "windows")]
const AWG_TUNNEL: &str = "UniGate";
const AWG_CONF: &str = "UniGate.conf";
/// macOS TUN: единого «перезапуска с UAC» нет, поэтому sing-box поднимаем от root
/// через osascript (нативный запрос пароля). Управляем им «сигнальным» файлом:
/// GUI создаёт его перед запуском, а при отключении удаляет — root-watcher внутри
/// той же привилегированной сессии убивает sing-box (disconnect без пароля).
#[cfg(target_os = "macos")]
const TUN_SENTINEL: &str = "tun-run.lock";
/// macOS AmneziaWG без root-приложения: тот же сигнальный механизм, что и у TUN
/// (awg-quick up от root через osascript + root-watcher, снимающий тоннель при
/// удалении файла — disconnect без повторного пароля).
#[cfg(target_os = "macos")]
const AWG_SENTINEL: &str = "awg-run.lock";

#[derive(Default)]
pub struct ConnState {
    /// sing-box (proxy/tun).
    child: Option<CommandChild>,
    /// активный AmneziaWG-тоннель (имя сервиса), если поднят legacy-движок.
    awg_tunnel: Option<String>,
    /// awg-shim (userspace AmneziaWG → локальный SOCKS5), когда AWG-профиль
    /// идёт через sing-box. Держим stdin-пайп: выход UniGate = EOF = шим
    /// завершается сам, даже если мы не успели его убить.
    shim: Option<std::process::Child>,
    /// PID sing-box, запущенного от root на macOS в TUN-режиме (управляется
    /// сигнальным файлом, не через CommandChild).
    #[cfg(target_os = "macos")]
    tun_root_pid: Option<u32>,
    state: ConnectionState,
}

pub struct Connection(pub Mutex<ConnState>);

impl Connection {
    pub fn new() -> Self {
        Self(Mutex::new(ConnState::default()))
    }
}

/// На старте: если мы (ещё) не подключены, а системный прокси указывает на наш
/// локальный порт — значит остался после нештатного выхода/краха. Снимаем его,
/// чтобы не сломать пользователю интернет мёртвым прокси.
pub fn reconcile_startup(_app: &AppHandle) {
    if sysproxy::is_enabled_for(LOCAL_PORT) {
        let _ = sysproxy::disable();
    }
    // macOS: убираем «застрявшие» сигнальные файлы после жёсткого краша GUI —
    // тогда оставшийся root-watcher (если жив) сам завершит sing-box/awg-тоннель.
    #[cfg(target_os = "macos")]
    for sentinel in [TUN_SENTINEL, AWG_SENTINEL] {
        if let Ok(sent) = storage::path(_app, sentinel) {
            let _ = std::fs::remove_file(sent);
        }
    }
    // macOS: если каталог настроек создавался под sudo — чиним владельца,
    // иначе обычный (не-root) запуск не может ничего сохранять.
    #[cfg(target_os = "macos")]
    heal_config_dir_ownership(_app);
}

/// macOS: после запуска приложения через `sudo` файлы в App Support остаются
/// root-owned — обычный запуск не может сохранять настройки/профили и создавать
/// сигнальные файлы (и пользователь вынужден снова запускать от админа).
/// Обнаруживаем это и чиним владельца одним запросом пароля.
#[cfg(target_os = "macos")]
fn heal_config_dir_ownership(app: &AppHandle) {
    use std::os::unix::fs::MetadataExt;
    if elevation::is_elevated() {
        return;
    }
    let Ok(probe) = storage::path(app, ".rw-probe") else { return };
    let Some(dir) = probe.parent().map(|d| d.to_path_buf()) else { return };

    let uid = unsafe { libc::getuid() };
    let dir_broken = match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            false
        }
        Err(e) => e.kind() == std::io::ErrorKind::PermissionDenied,
    };
    // отдельные root-owned файлы при user-owned каталоге (запускали sudo ПОСЛЕ
    // обычных запусков): write_json переживёт (rename), а копирование geoip и
    // лог ядра — нет, поэтому тоже чиним
    let files_broken = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .any(|e| e.metadata().map(|m| m.uid() != uid).unwrap_or(false))
        })
        .unwrap_or(false);
    if !dir_broken && !files_broken {
        return;
    }

    let gid = unsafe { libc::getgid() };
    let q = dir.to_string_lossy().replace('\'', "'\\''");
    if let Err(e) = elevation::osascript_admin(&format!("/usr/sbin/chown -R {uid}:{gid} '{q}'")) {
        eprintln!("не удалось починить владельца каталога настроек: {e}");
    }
}

/// Снимает прокси, убивает sing-box и снимает AmneziaWG-тоннель.
/// Вызывается при выходе из приложения (best-effort).
pub fn cleanup(app: &AppHandle) {
    let (child, awg, shim) = if let Some(conn) = app.try_state::<Connection>() {
        if let Ok(mut guard) = conn.0.lock() {
            let t = (guard.child.take(), guard.awg_tunnel.take(), guard.shim.take());
            guard.state = ConnectionState::Disconnected;
            t
        } else {
            (None, None, None)
        }
    } else {
        (None, None, None)
    };

    if let Some(child) = child {
        let _ = child.kill();
    }
    if let Some(mut shim) = shim {
        let _ = shim.kill();
    }
    if let Some(handle) = awg {
        awg_tunnel_down(app, &handle);
    }
    // macOS TUN: удаляем сигнальный файл — root-watcher (переживёт выход GUI) убьёт sing-box
    #[cfg(target_os = "macos")]
    if let Ok(sent) = storage::path(app, TUN_SENTINEL) {
        let _ = std::fs::remove_file(sent);
    }
    let _ = sysproxy::disable();
}

/// Путь к amneziawg.exe. ВАЖНО: запускаем именно ту копию, рядом с которой лежит
/// `wintun.dll` — иначе сервис тоннеля не создаст Wintun-адаптер (exit 3).
/// В dev это `src-tauri/binaries`, а НЕ `target/debug` (куда Tauri кладёт копию
/// sidecar без wintun.dll).
#[cfg(target_os = "windows")]
fn awg_exe() -> Option<std::path::PathBuf> {
    const TRIPLE: &str = "amneziawg-x86_64-pc-windows-msvc.exe";
    const PLAIN: &str = "amneziawg.exe";
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    // dev: рядом с wintun.dll
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("binaries").join(TRIPLE));
        candidates.push(cwd.join("src-tauri").join("binaries").join(TRIPLE));
    }
    // prod: рядом с основным exe (туда бандлится и wintun.dll)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(TRIPLE));
            candidates.push(dir.join(PLAIN));
        }
    }
    candidates.into_iter().find(|p| p.exists())
}

/// Находит файл в каталоге binaries (dev: src-tauri/binaries; prod: рядом с exe
/// или в resources/).
fn binaries_file(name: &str) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("binaries").join(name));
        candidates.push(cwd.join("src-tauri").join("binaries").join(name));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(name));
            candidates.push(dir.join("resources").join(name));
            // macOS .app: exe лежит в Contents/MacOS, а bundle.resources Tauri
            // кладёт в Contents/Resources — без этого кандидата установленное
            // приложение не находило geoip-ru.srs и RU-обход молча отключался
            #[cfg(target_os = "macos")]
            if let Some(contents) = dir.parent() {
                candidates.push(contents.join("Resources").join(name));
            }
        }
    }
    candidates.into_iter().find(|p| p.exists())
}

/// Путь к awg-shim — userspace AmneziaWG, торчащий локальным SOCKS5
/// (собирается scripts/build-awg-shim.ps1; prod: externalBin кладёт рядом с exe
/// без triple-суффикса). Пока только Windows: на macOS AmneziaWG работает через
/// awg-quick, мигрируем на шим после обкатки.
fn awg_shim_exe() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        binaries_file("awg-shim-x86_64-pc-windows-msvc.exe")
            .or_else(|| binaries_file("awg-shim.exe"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Запускает awg-shim: пишет .conf, стартует процесс со stdin-пайпом (умер
/// UniGate → закрылся пайп → шим выходит сам) и ждёт строку `READY` — к этому
/// моменту endpoint разрезолвлен и SOCKS5 слушает. Ошибка старта — из stderr.
async fn start_awg_shim(app: &AppHandle, conf: &str) -> Result<std::process::Child, String> {
    let exe = awg_shim_exe()
        .ok_or("awg-shim не найден рядом с приложением (scripts/build-awg-shim.ps1)")?;
    let conf_path = storage::path(app, SHIM_CONF)?;
    std::fs::write(&conf_path, conf)
        .map_err(|e| format!("не удалось записать {SHIM_CONF}: {e}"))?;

    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("--conf")
        .arg(&conf_path)
        .arg("--listen")
        .arg(format!("127.0.0.1:{SHIM_PORT}"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("не удалось запустить awg-shim: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let ready = tauri::async_runtime::spawn_blocking(move || {
        use std::io::{BufRead, BufReader, Read};
        let mut line = String::new();
        if let Some(out) = stdout {
            let _ = BufReader::new(out).read_line(&mut line);
        }
        if line.starts_with("READY") {
            return Ok(());
        }
        // stdout закрылся без READY — шим упал на старте, причина в stderr
        let mut err = String::new();
        if let Some(mut se) = stderr {
            let _ = se.read_to_string(&mut err);
        }
        let lines: Vec<&str> = err.trim().lines().collect();
        let tail = lines[lines.len().saturating_sub(3)..].join("\n");
        Err(if tail.is_empty() { line.trim().to_string() } else { tail })
    });
    match tokio::time::timeout(std::time::Duration::from_secs(10), ready).await {
        Ok(Ok(Ok(()))) => Ok(child),
        Ok(Ok(Err(msg))) => {
            let _ = child.kill();
            Err(format!("awg-shim не запустился: {msg}"))
        }
        _ => {
            let _ = child.kill();
            Err("awg-shim не ответил за 10 секунд".into())
        }
    }
}

/// Доступен ли awg-shim: AmneziaWG тогда идёт через sing-box (split-tunneling
/// и статистика работают). Иначе — legacy-движок, полный туннель.
#[tauri::command]
pub fn awg_shim_available() -> bool {
    awg_shim_exe().is_some()
}

/// Убивает awg-shim, если он ещё запущен (best-effort).
fn kill_shim(app: &AppHandle) {
    if let Some(conn) = app.try_state::<Connection>() {
        if let Ok(mut guard) = conn.0.lock() {
            if let Some(mut shim) = guard.shim.take() {
                let _ = shim.kill();
            }
        }
    }
}

/// Готовит `geoip-ru.srs` в каталоге данных приложения (App Support) и возвращает
/// путь к копии. На macOS папки `~/Desktop`/`~/Documents`/`~/Downloads` закрыты TCC:
/// процесс sing-box (даже от root) не может читать оттуда data-файлы (EPERM →
/// «operation not permitted», ядро падает на старте). App Support под TCC не попадает,
/// поэтому в dev-раскладке (проект на Рабочем столе) копируем geoip туда.
#[cfg(target_os = "macos")]
fn staged_geoip(app: &AppHandle) -> Option<String> {
    let src = match binaries_file("geoip-ru.srs") {
        Some(p) => p,
        None => {
            // источник не найден (нестандартная раскладка) — используем ранее
            // staged-копию, если есть, вместо тихого отключения RU-обхода
            let staged = storage::path(app, "geoip-ru.srs").ok().filter(|p| p.exists());
            if staged.is_none() {
                eprintln!("geoip-ru.srs не найден — RU-обход не будет применён");
            }
            return staged.map(|p| p.to_string_lossy().into_owned());
        }
    };
    let dst = match storage::path(app, "geoip-ru.srs") {
        Ok(p) => p,
        Err(_) => return Some(src.to_string_lossy().into_owned()),
    };
    // копируем, если целевого нет или он другого размера (обновился)
    let need_copy = match (std::fs::metadata(&dst), std::fs::metadata(&src)) {
        (Ok(d), Ok(s)) => d.len() != s.len(),
        _ => true,
    };
    if need_copy {
        if let Err(e) = std::fs::copy(&src, &dst) {
            eprintln!("не удалось скопировать geoip в data dir: {e}");
            return Some(src.to_string_lossy().into_owned());
        }
    }
    Some(dst.to_string_lossy().into_owned())
}

/// Путь к бинарнику sing-box для ПРЯМОГО запуска (не через Tauri-sidecar).
/// Нужно на macOS в TUN-режиме, где sing-box поднимается от root через osascript.
/// dev: `src-tauri/binaries/sing-box-<triple>`; prod: рядом с exe (`sing-box`).
#[cfg(target_os = "macos")]
fn singbox_path() -> Option<std::path::PathBuf> {
    const NAMES: [&str; 3] = [
        "sing-box-aarch64-apple-darwin",
        "sing-box-x86_64-apple-darwin",
        "sing-box",
    ];
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("binaries"));
        dirs.push(cwd.join("src-tauri").join("binaries"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            dirs.push(dir.join("resources"));
            // установленный macOS bundle: executable = Contents/MacOS/unigate,
            // ресурсы Tauri = Contents/Resources
            if let Some(contents) = dir.parent() {
                dirs.push(contents.join("Resources"));
            }
        }
    }
    for d in dirs {
        for n in NAMES {
            let p = d.join(n);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// Запускает sing-box от root через osascript (нативный запрос пароля админа).
/// Возвращает PID. В той же привилегированной сессии стартует root-watcher: он
/// убьёт sing-box, как только исчезнет сигнальный файл `sentinel` — это даёт
/// отключение без повторного запроса пароля (GUI просто удаляет файл).
#[cfg(target_os = "macos")]
fn spawn_singbox_root(
    singbox: &std::path::Path,
    cfg: &std::path::Path,
    log: &std::path::Path,
    sentinel: &std::path::Path,
) -> Result<u32, String> {
    // экранирование одинарных кавычек для sh: ' -> '\''
    let q = |p: &std::path::Path| p.to_string_lossy().replace('\'', "'\\''");
    let shell = format!(
        "'{sb}' run -c '{cfg}' > '{log}' 2>&1 & SB=$!; echo $SB; \
         ( while kill -0 $SB 2>/dev/null && [ -e '{sent}' ]; do sleep 0.3; done; \
           kill $SB 2>/dev/null ) >/dev/null 2>&1 &",
        sb = q(singbox),
        cfg = q(cfg),
        log = q(log),
        sent = q(sentinel),
    );
    let out = elevation::osascript_admin(&shell)?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<u32>()
        .map_err(|_| "не удалось получить PID sing-box (root)".to_string())
}

/// Ждёт освобождения локального порта (проверка bind'ом), poll 150 мс.
/// true — порт свободен; false — так и не освободился за timeout.
async fn wait_port_free(port: u16, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        // bind тут же закрываем — нам нужен только факт доступности порта
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

/// Последние строки лога ядра (`sing-box.log`) — чтобы показать причину падения
/// в сообщении об ошибке, а не только «завершился с кодом N».
fn log_tail(app: &AppHandle, max_lines: usize) -> Option<String> {
    let path = storage::path(app, "sing-box.log").ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines: Vec<&str> = text.lines().rev().take(max_lines).collect();
    lines.reverse();
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n"))
}

/// true, если процесс с данным PID жив. `kill -0` от пользователя к root-процессу
/// возвращает EPERM — это тоже «жив»; окончательно мёртв только ESRCH.
#[cfg(target_os = "macos")]
fn pid_alive(pid: u32) -> bool {
    if unsafe { libc::kill(pid as i32, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Запускает amneziawg.exe напрямую (как в консоли — без перенаправления stdio
/// и CREATE_NO_WINDOW, иначе GUI-exe не стартует сервис тоннеля).
#[cfg(target_os = "windows")]
fn run_awg(args: &[&str]) -> Result<(), String> {
    let exe = awg_exe().ok_or("amneziawg.exe не найден рядом с приложением")?;
    let status = std::process::Command::new(&exe)
        .args(args)
        .status()
        .map_err(|e| format!("не удалось запустить amneziawg: {e}"))?;
    if !status.success() {
        return Err(format!("amneziawg завершился с кодом {:?}", status.code()));
    }
    Ok(())
}

/// Каталог с движком AmneziaWG для macOS: `amneziawg-go` (userspace-датапас) +
/// `awg` (UAPI-конфигуратор) + `awg-quick` (bash-обёртка, поднимает utun+адреса+
/// маршруты+DNS из `.conf`). Prebuilt-бинарников нет — собираются из исходников,
/// см. scripts/fetch-awg-macos.sh; в dev кладутся в `src-tauri/binaries`.
#[cfg(target_os = "macos")]
fn awg_engine_dir() -> Option<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("binaries"));
        dirs.push(cwd.join("src-tauri").join("binaries"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            dirs.push(dir.join("resources"));
            if let Some(contents) = dir.parent() {
                dirs.push(contents.join("Resources"));
            }
        }
    }
    dirs.into_iter().find(|d| {
        d.join("amneziawg-go").exists() && d.join("awg").exists() && d.join("awg-quick").exists()
    })
}

/// macOS: `awg-quick up|down <conf>` от root. PATH дополняем каталогом движка,
/// чтобы awg-quick нашёл `awg` и userspace-реализацию (`amneziawg-go`).
#[cfg(target_os = "macos")]
fn run_awg_quick(action: &str, conf: &str) -> Result<(), String> {
    let dir = awg_engine_dir()
        .ok_or("движок AmneziaWG (amneziawg-go/awg/awg-quick) не найден рядом с приложением")?;
    let path_env = match std::env::var("PATH") {
        Ok(p) => format!("{}:{}", dir.display(), p),
        Err(_) => format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", dir.display()),
    };
    let status = std::process::Command::new("bash")
        .arg(dir.join("awg-quick"))
        .arg(action)
        .arg(conf)
        .env("PATH", path_env)
        .env("WG_QUICK_USERSPACE_IMPLEMENTATION", "amneziawg-go")
        .status()
        .map_err(|e| format!("не удалось запустить awg-quick: {e}"))?;
    if !status.success() {
        return Err(format!("awg-quick {action} завершился с кодом {:?}", status.code()));
    }
    Ok(())
}

/// true, если у системы есть реальный IPv6-маршрут по умолчанию.
/// `route -n get -inet6 default` возвращает 0 даже при отсутствии маршрута
/// («not in table» уходит в stderr) — поэтому смотрим на stdout: у найденного
/// маршрута там есть строка `interface: enX`.
#[cfg(target_os = "macos")]
fn has_ipv6_route() -> bool {
    std::process::Command::new("route")
        .args(["-n", "get", "-inet6", "default"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("interface:"))
        .unwrap_or(false)
}

/// Резолвит домен сервера профиля системным резолвером (best-effort, таймаут 4 с).
/// IPv4 — в начале списка (DNS-стратегия ядра ipv4_only). Пусто, если сервер
/// задан IP-литералом или резолв не удался.
#[cfg(not(target_os = "windows"))]
async fn resolve_server_ips(outbound: &Outbound) -> Vec<std::net::IpAddr> {
    use std::net::ToSocketAddrs;
    let Some(host) = config::server_domain(outbound) else {
        return Vec::new();
    };
    let host = host.to_string();
    // getaddrinfo блокирующий и может висеть — уводим в blocking-пул с таймаутом
    let resolve = tauri::async_runtime::spawn_blocking(move || {
        (host.as_str(), 443)
            .to_socket_addrs()
            .map(|addrs| {
                let (mut v4, mut v6): (Vec<_>, Vec<_>) =
                    addrs.map(|a| a.ip()).partition(|ip| ip.is_ipv4());
                v4.append(&mut v6);
                v4.dedup();
                v4
            })
            .unwrap_or_default()
    });
    match tokio::time::timeout(std::time::Duration::from_secs(4), resolve).await {
        Ok(Ok(ips)) => ips,
        _ => Vec::new(),
    }
}

fn set_state(app: &AppHandle, state: ConnectionState) {
    if let Some(conn) = app.try_state::<Connection>() {
        if let Ok(mut guard) = conn.0.lock() {
            guard.state = state.clone();
        }
    }
    let _ = app.emit(EVENT, state);
}

#[tauri::command]
pub fn get_connection_state(app: AppHandle) -> ConnectionState {
    app.state::<Connection>()
        .0
        .lock()
        .map(|g| g.state.clone())
        .unwrap_or_default()
}

/// true, если текущее состояние — Connected (используется поллером статистики).
pub fn is_connected(app: &AppHandle) -> bool {
    matches!(get_connection_state(app.clone()), ConnectionState::Connected)
}

/// Возвращает адрес локального прокси (для ручной проверки/отображения).
#[tauri::command]
pub fn local_proxy_addr() -> String {
    format!("127.0.0.1:{LOCAL_PORT}")
}

/// Windows + совместная работа с другим VPN: sing-box назначает TUN-адаптеру
/// служебный DNS (следующий адрес после 198.18.0.1), и Windows ставит этот
/// интерфейс первым. OpenVPN Connect с защитой от DNS-утечек блокирует запросы к
/// чужому TUN-DNS, хотя его собственные DNS через TAP/DCO доступны.
///
/// После создания адаптера очищаем его DNS и поднимаем interface metric: тогда
/// Windows выбирает DNS активного OpenVPN (либо физической сети), а более
/// специфичные auto_route-маршруты UniGate продолжают перехватывать обычный
/// трафик. Настройки относятся к ActiveStore и исчезают вместе с адаптером.
#[cfg(target_os = "windows")]
async fn release_tun_dns_to_system() -> Result<(), String> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$deadline = [DateTime]::UtcNow.AddSeconds(5)
do {
  $tun = Get-NetIPAddress -AddressFamily IPv4 -IPAddress '198.18.0.1' -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if ($null -ne $tun) {
    Set-DnsClientServerAddress -InterfaceIndex $tun.InterfaceIndex -ResetServerAddresses
    Set-NetIPInterface -InterfaceIndex $tun.InterfaceIndex -AddressFamily IPv4 -InterfaceMetric 5000
    exit 0
  }
  Start-Sleep -Milliseconds 100
} while ([DateTime]::UtcNow -lt $deadline)
Write-Error 'TUN interface 198.18.0.1 was not found'
exit 1
"#;

    tauri::async_runtime::spawn_blocking(move || {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("не удалось настроить DNS TUN: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if stderr.is_empty() {
                format!(
                    "настройка DNS TUN завершилась с кодом {:?}",
                    output.status.code()
                )
            } else {
                format!("не удалось освободить DNS TUN: {stderr}")
            })
        }
    })
    .await
    .map_err(|e| format!("задача настройки DNS TUN завершилась с ошибкой: {e}"))?
}

#[tauri::command]
pub async fn connect(app: AppHandle, profile_id: String) -> Result<ConnectionState, String> {
    {
        let conn = app.state::<Connection>();
        let guard = conn.0.lock().map_err(|e| e.to_string())?;
        if matches!(
            guard.state,
            ConnectionState::Connected | ConnectionState::Connecting
        ) {
            return Err("уже подключено".into());
        }
    }

    let profile = profiles::list(&app)?
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("профиль {profile_id} не найден"))?;

    // AmneziaWG: предпочтительный путь — awg-shim (userspace AWG → локальный
    // SOCKS5): sing-box остаётся мозгом маршрутизации, split-tunneling и
    // статистика работают как у остальных протоколов. Если шима нет рядом —
    // legacy-движок (amneziawg.exe / awg-quick): полный туннель без split.
    let awg_shim =
        matches!(profile.outbound, Outbound::AmneziaWg { .. }) && awg_shim_exe().is_some();
    if let Outbound::AmneziaWg { config, .. } = &profile.outbound {
        if !awg_shim {
            return connect_amneziawg(&app, config).await;
        }
    }

    // настройки: режим + маршрутизация; TUN требует прав администратора.
    // На Windows/Linux нужен elevated-процесс приложения; на macOS так нельзя —
    // там сам sing-box поднимаем от root через osascript (см. connect_tun_macos),
    // поэтому здесь мак не блокируем.
    let settings = settings::load(&app)?;
    let mode = settings.mode;
    #[cfg(not(target_os = "macos"))]
    if mode == Mode::Tun && !elevation::is_elevated() {
        let msg =
            "TUN-режим требует прав администратора — перезапустите приложение от имени администратора"
                .to_string();
        set_state(&app, ConnectionState::Error { message: msg.clone() });
        return Err(msg);
    }

    set_state(&app, ConnectionState::Connecting);

    // путь к geoip-ru.srs для RU-обхода (только TUN + bypass_ru).
    // На macOS отдаём копию из App Support (обход TCC на ~/Desktop), см. staged_geoip.
    let geoip = if mode == Mode::Tun && settings.routing.bypass_ru {
        #[cfg(target_os = "macos")]
        {
            staged_geoip(&app)
        }
        #[cfg(not(target_os = "macos"))]
        {
            binaries_file("geoip-ru.srs").map(|p| p.to_string_lossy().into_owned())
        }
    } else {
        None
    };
    // macOS/прочие + TUN: предрезолв домена сервера системным резолвером ДО
    // запуска ядра — подъём туннеля не зависит от доступности публичного
    // бутстрап-DNS (см. комментарий в config.rs). Best-effort с таймаутом:
    // не вышло — генератор оставит старую схему (77.88.8.8).
    #[cfg(not(target_os = "windows"))]
    let bootstrap_ips = if mode == Mode::Tun {
        resolve_server_ips(&profile.outbound).await
    } else {
        Vec::new()
    };
    #[cfg(target_os = "windows")]
    let bootstrap_ips: Vec<std::net::IpAddr> = Vec::new();

    // Перехватывать ли IPv6 в TUN: только если у системы реально есть
    // IPv6-маршрут. На IPv4-only сети фейковый IPv6 через tun ломает
    // direct-обход (bypass_ru): приложения выбирают IPv6, а доставить его
    // некуда → мгновенный разрыв. Windows не трогаем (флаг игнорируется).
    #[cfg(target_os = "macos")]
    let tun_ipv6 = mode == Mode::Tun && has_ipv6_route();
    #[cfg(not(target_os = "macos"))]
    let tun_ipv6 = false;

    // Профиль для генератора: AmneziaWG через шим для sing-box — обычный
    // SOCKS5-outbound на localhost
    let mut gen_profile = profile.clone();
    if awg_shim {
        gen_profile.outbound = Outbound::Socks {
            server: "127.0.0.1".into(),
            port: SHIM_PORT,
            username: None,
            password: None,
        };
    }

    // генерируем и пишем рабочий конфиг
    let mut cfg = config::generate(
        &gen_profile,
        mode,
        LOCAL_PORT,
        &settings.routing,
        geoip.as_deref(),
        &bootstrap_ips,
        tun_ipv6,
        settings.tun_stack,
    );

    // Трафик самого шима (UDP к AWG-серверу) обязан идти мимо TUN, иначе цикл:
    // правило по имени процесса + (если сервер задан IP-литералом) по адресу.
    // В proxy-режиме route-правил в конфиге нет — блок no-op.
    if awg_shim {
        if let Some(rules) = cfg
            .get_mut("route")
            .and_then(|r| r.get_mut("rules"))
            .and_then(|r| r.as_array_mut())
        {
            let mut extra: Vec<serde_json::Value> = Vec::new();
            if let Some(name) = awg_shim_exe()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            {
                extra.push(serde_json::json!({ "process_name": [name], "outbound": "direct" }));
            }
            if let Outbound::AmneziaWg { server, .. } = &profile.outbound {
                if let Ok(ip) = server.parse::<std::net::IpAddr>() {
                    let prefix = if ip.is_ipv4() { 32 } else { 128 };
                    extra.push(serde_json::json!({
                        "ip_cidr": [format!("{ip}/{prefix}")],
                        "outbound": "direct"
                    }));
                }
            }
            // после sniff + hijack-dns (первые два правила), до обходов/per-app
            let at = 2.min(rules.len());
            rules.splice(at..at, extra);
        }
    }
    // логи ядра в файл — уровень warn (без адресов подключений, для траблшутинга).
    // Файл обнуляем перед стартом, чтобы хвост лога относился к текущему запуску
    // (его показываем в сообщении об ошибке при падении ядра).
    if let Ok(log_path) = storage::path(&app, "sing-box.log") {
        let _ = std::fs::write(&log_path, b"");
        cfg["log"] = serde_json::json!({
            "level": "warn",
            "timestamp": true,
            "output": log_path.to_string_lossy()
        });
    }
    if let Err(e) = storage::write_json(&app, RUNNING_CONFIG, &cfg) {
        set_state(&app, ConnectionState::Error { message: e.clone() });
        return Err(e);
    }
    let path = storage::path(&app, RUNNING_CONFIG)?;

    // Пре-флайт: ядро слушает Clash API (9090), а в proxy-режиме ещё и 2080.
    // Порт может быть занят предыдущим ядром, которое завершается асинхронно
    // (root-ядро macOS TUN убивает watcher с задержкой), или чужим процессом —
    // тогда sing-box падает с невнятным «код 1». Ждём до 3 с и говорим внятно.
    {
        let mut ports = vec![config::CLASH_API_PORT];
        if mode == Mode::Proxy {
            ports.push(LOCAL_PORT);
        }
        if awg_shim {
            ports.push(SHIM_PORT);
        }
        for port in ports {
            if !wait_port_free(port, std::time::Duration::from_secs(3)).await {
                let msg = format!(
                    "порт 127.0.0.1:{port} занят другим процессом (возможно, предыдущее ядро ещё завершается) — попробуйте ещё раз"
                );
                set_state(&app, ConnectionState::Error { message: msg.clone() });
                return Err(msg);
            }
        }
    }

    // macOS + TUN: sing-box нужен root (создать utun + маршруты).
    // Если приложение УЖЕ запущено от админа (euid==0) — запускаем sing-box обычным
    // дочерним процессом (наследует root, есть отслеживание падений), как на Windows.
    // Если нет — поднимаем sing-box от root через osascript (см. connect_tun_macos).
    #[cfg(target_os = "macos")]
    if mode == Mode::Tun && !elevation::is_elevated() {
        return connect_tun_macos(&app, &path);
    }

    // AmneziaWG: шим поднимаем ДО ядра — он резолвит endpoint системным
    // резолвером (наш TUN ещё не активен) и начинает слушать SOCKS5
    let mut shim_child: Option<std::process::Child> = None;
    if awg_shim {
        if let Outbound::AmneziaWg { config: conf, .. } = &profile.outbound {
            match start_awg_shim(&app, conf).await {
                Ok(c) => shim_child = Some(c),
                Err(e) => {
                    set_state(&app, ConnectionState::Error { message: e.clone() });
                    return Err(e);
                }
            }
        }
    }

    // запускаем sing-box
    let spawn = app
        .shell()
        .sidecar("sing-box")
        .map_err(|e| format!("sidecar sing-box: {e}"))
        .and_then(|cmd| {
            cmd.args(["run", "-c", &path.to_string_lossy()])
                .spawn()
                .map_err(|e| format!("не удалось запустить sing-box: {e}"))
        });

    let (mut rx, child) = match spawn {
        Ok(pair) => pair,
        Err(e) => {
            if let Some(mut shim) = shim_child.take() {
                let _ = shim.kill();
            }
            set_state(&app, ConnectionState::Error { message: e.clone() });
            return Err(e);
        }
    };

    {
        let conn = app.state::<Connection>();
        let mut guard = conn.0.lock().map_err(|e| e.to_string())?;
        guard.child = Some(child);
        guard.shim = shim_child;
        guard.state = ConnectionState::Connected;
    }

    // При явно включённой совместимости корпоративные сети и DNS должны
    // оставаться за нативным OpenVPN-адаптером. Делаем после spawn: только
    // теперь существует tun0.
    #[cfg(target_os = "windows")]
    if mode == Mode::Tun
        && settings.routing.bypass_lan
        && settings.routing.vpn_compatibility
    {
        if let Err(e) = release_tun_dns_to_system().await {
            // Best-effort: без OpenVPN само подключение UniGate остаётся рабочим,
            // поэтому не роняем сессию из-за системной настройки DNS.
            eprintln!("[tun-dns] {e}");
        }
    }

    // системный прокси ставим только в proxy-режиме (в TUN роутит сам адаптер)
    if mode == Mode::Proxy {
        if let Err(e) = sysproxy::enable(LOCAL_PORT) {
            eprintln!("не удалось включить системный прокси: {e}");
        }
    }
    let _ = app.emit(EVENT, ConnectionState::Connected);

    // запускаем опрос статистики (живёт, пока Connected)
    stats::start_polling(app.clone());

    // следим за неожиданным завершением процесса
    let app_watch = app.clone();
    tauri::async_runtime::spawn(async move {
        // последние строки stderr/stdout ядра: FATAL при старте (например,
        // занятый порт) печатается в stderr ДО настройки файлового лога —
        // без этого буфера ошибка в UI была бы просто «код 1»
        let mut stderr_tail: std::collections::VecDeque<String> =
            std::collections::VecDeque::with_capacity(5);
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stderr(bytes) | CommandEvent::Stdout(bytes) => {
                    let line = String::from_utf8_lossy(&bytes);
                    let line = line.trim();
                    if !line.is_empty() {
                        eprintln!("[sing-box] {line}");
                        if stderr_tail.len() == 5 {
                            stderr_tail.pop_front();
                        }
                        stderr_tail.push_back(line.to_string());
                    }
                }
                CommandEvent::Terminated(payload) => {
                    let conn = app_watch.state::<Connection>();
                    let unexpected = {
                        let mut guard = conn.0.lock().unwrap();
                        guard.child = None;
                        !matches!(
                            guard.state,
                            ConnectionState::Disconnecting | ConnectionState::Disconnected
                        )
                    };
                    // ядро умерло — шим больше не нужен (при штатном
                    // disconnect его уже забрали, тогда это no-op)
                    kill_shim(&app_watch);
                    if unexpected {
                        let _ = sysproxy::disable();
                        let mut msg = format!("sing-box завершился (код {:?})", payload.code);
                        if let Some(tail) = log_tail(&app_watch, 5) {
                            msg = format!("{msg}\n{tail}");
                        } else if !stderr_tail.is_empty() {
                            let tail: Vec<String> = stderr_tail.iter().cloned().collect();
                            msg = format!("{msg}\n{}", tail.join("\n"));
                        }
                        set_state(&app_watch, ConnectionState::Error { message: msg });
                    }
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(ConnectionState::Connected)
}

/// macOS TUN: поднимаем sing-box от root через osascript. Системный прокси НЕ
/// ставим (роутит сам utun). Статистика идёт через Clash API как обычно.
#[cfg(target_os = "macos")]
fn connect_tun_macos(app: &AppHandle, cfg_path: &std::path::Path) -> Result<ConnectionState, String> {
    let fail = |app: &AppHandle, msg: String| -> Result<ConnectionState, String> {
        set_state(app, ConnectionState::Error { message: msg.clone() });
        Err(msg)
    };

    let singbox = match singbox_path() {
        Some(p) => p,
        None => return fail(app, "не найден бинарник sing-box".into()),
    };
    let log = storage::path(app, "sing-box.log")?;
    let sentinel = storage::path(app, TUN_SENTINEL)?;

    // создаём сигнальный файл ДО запуска — пока он есть, root-watcher держит sing-box
    if let Err(e) = std::fs::write(&sentinel, b"") {
        return fail(app, format!("не удалось создать сигнальный файл: {e}"));
    }

    let pid = match spawn_singbox_root(&singbox, cfg_path, &log, &sentinel) {
        Ok(pid) => pid,
        Err(e) => {
            let _ = std::fs::remove_file(&sentinel);
            return fail(app, e);
        }
    };

    // Даём ядру время на старт и проверяем, что оно не упало сразу (FATAL на
    // конфиге/DNS/TCC): иначе покажем ложный «Connected», а ошибка останется
    // только в логе. При падении — показываем хвост sing-box.log.
    std::thread::sleep(std::time::Duration::from_millis(1500));
    if !pid_alive(pid) {
        let _ = std::fs::remove_file(&sentinel);
        let mut msg = "sing-box (TUN) упал при старте".to_string();
        if let Some(tail) = log_tail(app, 5) {
            msg = format!("{msg}:\n{tail}");
        }
        return fail(app, msg);
    }

    {
        let conn = app.state::<Connection>();
        let mut guard = conn.0.lock().map_err(|e| e.to_string())?;
        guard.tun_root_pid = Some(pid);
        guard.state = ConnectionState::Connected;
    }
    let _ = app.emit(EVENT, ConnectionState::Connected);
    stats::start_polling(app.clone());

    // Следим за root-процессом: CommandChild-событий у него нет, поэтому поллим
    // PID. Если ядро умерло, а мы всё ещё «Connected» — показываем ошибку с логом.
    let app_watch = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let ours = {
                let conn = app_watch.state::<Connection>();
                let guard = match conn.0.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                guard.tun_root_pid == Some(pid)
            };
            if !ours {
                break; // штатный disconnect уже забрал PID
            }
            if !pid_alive(pid) {
                {
                    let conn = app_watch.state::<Connection>();
                    if let Ok(mut guard) = conn.0.lock() {
                        guard.tun_root_pid = None;
                    };
                }
                if let Ok(sent) = storage::path(&app_watch, TUN_SENTINEL) {
                    let _ = std::fs::remove_file(sent);
                }
                let mut msg = "sing-box (TUN) неожиданно завершился".to_string();
                if let Some(tail) = log_tail(&app_watch, 5) {
                    msg = format!("{msg}:\n{tail}");
                }
                set_state(&app_watch, ConnectionState::Error { message: msg });
                break;
            }
        }
    });
    Ok(ConnectionState::Connected)
}

/// Подключение через движок AmneziaWG. Windows — `amneziawg.exe
/// /installtunnelservice`; macOS — `awg-quick up` (amneziawg-go userspace).
/// Всегда требует прав администратора; статистики Clash API у него нет.
#[cfg(any(target_os = "macos", test))]
fn normalize_awg_config_for_macos(config: &str) -> String {
    let is_empty_concealment = |line: &str| {
        let Some((key, value)) = line.trim().split_once('=') else {
            return false;
        };
        let key = key.trim().as_bytes();
        let is_i1_i5 = key.len() == 2
            && key[0].eq_ignore_ascii_case(&b'i')
            && matches!(key[1], b'1'..=b'5');
        let value = value.trim();
        is_i1_i5 && (value.is_empty() || value == "''" || value == "\"\"")
    };

    let mut normalized = String::with_capacity(config.len());
    for segment in config.split_inclusive('\n') {
        let line = segment
            .strip_suffix('\n')
            .unwrap_or(segment)
            .strip_suffix('\r')
            .unwrap_or_else(|| segment.strip_suffix('\n').unwrap_or(segment));
        if !is_empty_concealment(line) {
            normalized.push_str(segment);
        }
    }
    normalized
}

async fn connect_amneziawg(app: &AppHandle, config: &str) -> Result<ConnectionState, String> {
    // macOS не блокируем: там awg-quick поднимается от root через osascript
    // (нативный запрос пароля), приложение может оставаться обычным.
    #[cfg(not(target_os = "macos"))]
    if !elevation::is_elevated() {
        let msg = "AmneziaWG требует прав администратора — перезапустите приложение от имени администратора".to_string();
        set_state(app, ConnectionState::Error { message: msg.clone() });
        return Err(msg);
    }
    set_state(app, ConnectionState::Connecting);

    let conf_path = storage::path(app, AWG_CONF)?;
    // awg(8) on Unix rejects explicit empty AWG 2.0 concealment fields (`I2 =`),
    // while an omitted field has the same zero-value semantics. Amnezia containers
    // commonly emit empty I2-I5, so remove only empty I1-I5 on macOS. Filled
    // concealment packets and the stored/exported profile remain untouched.
    #[cfg(target_os = "macos")]
    let config_to_write = normalize_awg_config_for_macos(config);
    #[cfg(not(target_os = "macos"))]
    let config_to_write = config;
    if let Err(e) = std::fs::write(&conf_path, config_to_write) {
        let m = format!("не удалось записать {AWG_CONF}: {e}");
        set_state(app, ConnectionState::Error { message: m.clone() });
        return Err(m);
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&conf_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("не удалось защитить {AWG_CONF}: {e}"))?;
    }

    let cp = conf_path.to_string_lossy().into_owned();

    // Поднимаем тоннель движком под конкретную ОС; возвращённый «handle» кладём в
    // состояние для последующего снятия (Windows: имя сервиса; macOS: путь conf).
    let handle = match awg_tunnel_up(app, &cp) {
        Ok(h) => h,
        Err(e) => {
            set_state(app, ConnectionState::Error { message: e.clone() });
            return Err(e);
        }
    };

    {
        let conn = app.state::<Connection>();
        let mut guard = conn.0.lock().map_err(|e| e.to_string())?;
        guard.awg_tunnel = Some(handle);
        guard.state = ConnectionState::Connected;
    }
    let _ = app.emit(EVENT, ConnectionState::Connected);
    Ok(ConnectionState::Connected)
}

/// Поднимает AmneziaWG-тоннель из `.conf` движком под текущую ОС. Возвращает
/// «handle» для снятия: Windows — имя сервиса тоннеля, macOS — путь к `.conf`.
fn awg_tunnel_up(app: &AppHandle, conf: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        // снимаем возможный «зависший» тоннель с тем же именем (best-effort)
        let _ = run_awg(&["/uninstalltunnelservice", AWG_TUNNEL]);
        run_awg(&["/installtunnelservice", conf])?;
        Ok(AWG_TUNNEL.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        if elevation::is_elevated() {
            // приложение уже root — управляем движком напрямую
            let _ = run_awg_quick("down", conf);
            run_awg_quick("up", conf)?;
        } else {
            // обычный запуск: up от root через osascript + сигнальный файл
            spawn_awg_quick_root(app, conf)?;
        }
        Ok(conf.to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, conf);
        Err("AmneziaWG на этой платформе не поддерживается".to_string())
    }
}

/// macOS без root: `awg-quick up` через osascript (запрос пароля) + root-watcher,
/// который снимет тоннель (`awg-quick down`), как только GUI удалит сигнальный
/// файл — disconnect и выход из приложения не требуют повторного пароля.
#[cfg(target_os = "macos")]
fn spawn_awg_quick_root(app: &AppHandle, conf: &str) -> Result<(), String> {
    let dir = awg_engine_dir()
        .ok_or("движок AmneziaWG (amneziawg-go/awg/awg-quick) не найден рядом с приложением")?;
    let sentinel = storage::path(app, AWG_SENTINEL)?;
    std::fs::write(&sentinel, b"")
        .map_err(|e| format!("не удалось создать сигнальный файл: {e}"))?;

    // экранирование одинарных кавычек для sh: ' -> '\''
    let q = |s: &str| s.replace('\'', "'\\''");
    let dirq = q(&dir.to_string_lossy());
    let confq = q(conf);
    let sentq = q(&sentinel.to_string_lossy());
    let shell = format!(
        "export PATH='{dirq}':\"$PATH\"; export WG_QUICK_USERSPACE_IMPLEMENTATION=amneziawg-go; \
         bash '{dirq}/awg-quick' down '{confq}' >/dev/null 2>&1; \
         bash '{dirq}/awg-quick' up '{confq}' && \
         ( ( while [ -e '{sentq}' ]; do sleep 1; done; \
             bash '{dirq}/awg-quick' down '{confq}' ) >/dev/null 2>&1 & )",
    );
    if let Err(e) = elevation::osascript_admin(&shell) {
        let _ = std::fs::remove_file(&sentinel);
        return Err(e);
    }
    Ok(())
}

/// Снятие AmneziaWG-тоннеля по сохранённому «handle» (Windows: имя сервиса;
/// macOS: путь `.conf`). Best-effort — ошибку только логируем.
fn awg_tunnel_down(app: &AppHandle, handle: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        if let Err(e) = run_awg(&["/uninstalltunnelservice", handle]) {
            eprintln!("не удалось снять AmneziaWG-тоннель: {e}");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if elevation::is_elevated() {
            if let Err(e) = run_awg_quick("down", handle) {
                eprintln!("не удалось снять AmneziaWG-тоннель: {e}");
            }
        } else {
            // тоннель поднимал root-watcher — снимаем удалением сигнального файла
            if let Ok(sent) = storage::path(app, AWG_SENTINEL) {
                let _ = std::fs::remove_file(sent);
            }
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, handle);
    }
}

#[tauri::command]
pub async fn disconnect(app: AppHandle) -> Result<ConnectionState, String> {
    let (child, awg, shim) = {
        let conn = app.state::<Connection>();
        let mut guard = conn.0.lock().map_err(|e| e.to_string())?;
        guard.state = ConnectionState::Disconnecting;
        (guard.child.take(), guard.awg_tunnel.take(), guard.shim.take())
    };
    let _ = app.emit(EVENT, ConnectionState::Disconnecting);

    if let Some(child) = child {
        let _ = child.kill();
    }
    if let Some(mut shim) = shim {
        let _ = shim.kill();
    }
    if let Some(handle) = awg {
        awg_tunnel_down(&app, &handle);
    }
    // macOS TUN: удаляем сигнальный файл — root-watcher сам убьёт sing-box (без
    // пароля). ВАЖНО дождаться реальной смерти ядра: оно держит порт Clash API
    // (9090), и мгновенный reconnect (например, в proxy-режиме) иначе падает с
    // «address already in use».
    #[cfg(target_os = "macos")]
    {
        let pid = {
            let conn = app.state::<Connection>();
            let mut guard = conn.0.lock().map_err(|e| e.to_string())?;
            guard.tun_root_pid.take()
        };
        if let Some(pid) = pid {
            if let Ok(sent) = storage::path(&app, TUN_SENTINEL) {
                let _ = std::fs::remove_file(sent);
            }
            // watcher поллит сигнальный файл раз в 0.3 c; ждём до ~5 с
            for _ in 0..50 {
                if !pid_alive(pid) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
    if let Err(e) = sysproxy::disable() {
        eprintln!("не удалось снять системный прокси: {e}");
    }

    set_state(&app, ConnectionState::Disconnected);
    Ok(ConnectionState::Disconnected)
}

#[cfg(test)]
mod tests {
    use super::normalize_awg_config_for_macos;

    #[test]
    fn macos_awg_drops_only_empty_concealment_packets() {
        let config = "[Interface]\r\nI1 = <b 0x01>\r\nI2 =\r\nI3 = ''\r\nI4 = \"\"\r\nI5 = <b 0x05>\r\nJc = 5\r\n[Peer]\r\nPublicKey = PUB\r\n";
        let normalized = normalize_awg_config_for_macos(config);

        assert!(normalized.contains("I1 = <b 0x01>\r\n"));
        assert!(normalized.contains("I5 = <b 0x05>\r\n"));
        assert!(normalized.contains("Jc = 5\r\n"));
        assert!(!normalized.contains("I2 ="));
        assert!(!normalized.contains("I3 ="));
        assert!(!normalized.contains("I4 ="));
    }

    #[test]
    fn macos_awg_preserves_config_without_empty_i_fields_exactly() {
        let config = "[Interface]\nI2 = <b 0x02>\n[Peer]\nEndpoint = 1.2.3.4:1234";
        assert_eq!(normalize_awg_config_for_macos(config), config);
    }
}
