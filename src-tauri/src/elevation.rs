//! Права администратора (нужны для TUN-режима на Windows).

use tauri::AppHandle;

#[cfg(windows)]
mod imp {
    use super::*;

    /// Запущено ли приложение с правами администратора.
    pub fn is_elevated() -> bool {
        use std::mem;
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION {
                TokenIsElevated: 0,
            };
            let mut size = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                &mut elevation as *mut _ as *mut _,
                mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            );
            CloseHandle(token);
            ok != 0 && elevation.TokenIsElevated != 0
        }
    }

    /// Перезапускает приложение с правами администратора (UAC) и закрывает текущий процесс.
    pub fn relaunch_elevated(app: &AppHandle) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let to_wide = |s: &std::ffi::OsStr| -> Vec<u16> {
            s.encode_wide().chain(std::iter::once(0)).collect()
        };
        let exe_w = to_wide(exe.as_os_str());
        let verb_w = to_wide(std::ffi::OsStr::new("runas"));

        let h = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb_w.as_ptr(),
                exe_w.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecuteW: значение <= 32 — ошибка (в т.ч. отказ UAC)
        if (h as isize) <= 32 {
            return Err("не удалось перезапустить с правами администратора (отклонено?)".into());
        }
        app.exit(0);
        Ok(())
    }
}

#[cfg(unix)]
mod imp {
    use super::*;

    /// Запущено ли приложение от root (euid == 0). На macOS TUN-режим требует root.
    pub fn is_elevated() -> bool {
        // SAFETY: geteuid не имеет side-effects и всегда успешен.
        unsafe { libc::geteuid() == 0 }
    }

    /// Повышение прав на macOS: единого «перезапуска с UAC», как на Windows, нет.
    /// Root нужен только процессу sing-box в TUN-режиме — его поднимаем через
    /// `osascript ... with administrator privileges` в connection.rs (не весь GUI).
    /// Здесь возвращаем понятную ошибку, если код всё же сюда попал.
    pub fn relaunch_elevated(_app: &AppHandle) -> Result<(), String> {
        Err("на macOS права запрашиваются при подключении TUN, а не перезапуском приложения".into())
    }
}

pub use imp::{is_elevated, relaunch_elevated};

/// Выполняет shell-команду от root через osascript (нативный macOS-диалог
/// пароля администратора). Возвращает вывод команды при успехе.
#[cfg(target_os = "macos")]
pub fn osascript_admin(shell: &str) -> Result<std::process::Output, String> {
    // экранирование для строкового литерала AppleScript ("..." и \)
    let apple = format!(
        "do shell script \"{}\" with administrator privileges",
        shell.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&apple)
        .output()
        .map_err(|e| format!("не удалось запустить osascript: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // -128 / "User canceled" — пользователь отклонил запрос пароля
        if err.contains("-128") || err.to_lowercase().contains("cancel") {
            return Err("запрос прав администратора отклонён".into());
        }
        return Err(format!("osascript: {}", err.trim()));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Windows: запуск с правами администратора БЕЗ UAC-запроса — через задачу
// Планировщика с RunLevel=HighestAvailable. Задача создаётся один раз из
// elevated-процесса (одно подтверждение UAC), дальше любой запуск (автозапуск,
// двойной клик) идёт редиректом `schtasks /Run` → элевация без запроса.
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod admin_task {
    use std::os::windows::process::CommandExt;

    const TASK_NAME: &str = "UniGate";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn schtasks(args: &[&str]) -> Result<std::process::Output, String> {
        std::process::Command::new("schtasks")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("schtasks: {e}"))
    }

    pub fn exists() -> bool {
        schtasks(&["/Query", "/TN", TASK_NAME])
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Создаёт/пересоздаёт задачу под текущий путь exe (идемпотентно, лечит
    /// переезд приложения). Требует elevated-процесса. XML вместо флагов
    /// `/SC ONCE /ST` — без триггеров и без локале-зависимых дат; важно
    /// `ExecutionTimeLimit=PT0S` (иначе планировщик убьёт приложение через 72 ч)
    /// и `MultipleInstancesPolicy=Parallel` (иначе /Run откажет при живом инстансе).
    pub fn install() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Запуск UniGate с правами администратора без UAC</Description>
  </RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{cmd}</Command>
      <WorkingDirectory>{wd}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#,
            cmd = esc(&exe.to_string_lossy()),
            wd = esc(&exe_dir.to_string_lossy()),
        );

        // schtasks ждёт файл в UTF-16LE с BOM
        let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
        for u in xml.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let path = std::env::temp_dir().join("unigate-task.xml");
        std::fs::write(&path, &bytes).map_err(|e| format!("запись XML задачи: {e}"))?;

        let out = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &path.to_string_lossy(), "/F"])?;
        let _ = std::fs::remove_file(&path);
        if !out.status.success() {
            return Err(format!(
                "schtasks /Create: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(())
    }

    pub fn delete() -> Result<(), String> {
        let out = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])?;
        if !out.status.success() {
            return Err(format!(
                "schtasks /Delete: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(())
    }

    pub fn run() -> Result<(), String> {
        let out = schtasks(&["/Run", "/TN", TASK_NAME])?;
        if !out.status.success() {
            return Err(format!(
                "schtasks /Run: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(())
    }
}

/// Обслуживание «запуска от администратора» на старте приложения (Windows).
/// - включено + elevated: пересоздаём задачу под текущий путь (самолечение);
/// - включено + НЕ elevated + задача есть: перезапускаемся через неё (без UAC)
///   и завершаем текущий процесс;
/// - выключено + elevated: подчищаем задачу, если осталась.
/// Возвращает true, если текущий процесс нужно завершить (произошёл редирект).
#[cfg(windows)]
pub fn admin_launch_startup(app: &AppHandle) -> bool {
    let Ok(s) = crate::settings::load(app) else {
        return false;
    };
    if !s.admin_launch {
        if is_elevated() && admin_task::exists() {
            let _ = admin_task::delete();
        }
        return false;
    }
    if is_elevated() {
        if let Err(e) = admin_task::install() {
            eprintln!("не удалось обновить задачу планировщика: {e}");
        }
        return false;
    }
    if admin_task::exists() {
        // задача запускает exe без аргументов — оставляем маркер, чтобы
        // elevated-инстанс знал, что запуск пришёл из автозапуска (старт в трее)
        let via_autostart = std::env::args().any(|a| a == crate::AUTOSTART_ARG);
        let marker = crate::storage::path(app, crate::AUTOSTART_MARKER).ok();
        if via_autostart {
            if let Some(p) = &marker {
                let _ = std::fs::write(p, b"1");
            }
        }
        if admin_task::run().is_ok() {
            return true; // elevated-инстанс запущен задачей — этот процесс закрываем
        }
        if via_autostart {
            if let Some(p) = &marker {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    // задачи нет (удалили руками) — молча работаем без прав: UAC при автозапуске
    // показывать нельзя, а TUN при подключении сам предложит перезапуск
    false
}

/// Применяет настройку «запуск от администратора» (вызывается из UI).
/// Windows: включение из обычного процесса → один UAC-перезапуск, задачу
/// создаст elevated-инстанс на старте; из elevated — задача создаётся сразу.
/// Выключение — удаляем задачу (из-под обычного пользователя может не выйти —
/// тогда её подчистит следующий elevated-запуск).
#[tauri::command]
pub fn apply_admin_launch(app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        let s = crate::settings::load(&app)?;
        if s.admin_launch {
            if is_elevated() {
                admin_task::install()
            } else {
                relaunch_elevated(&app)
            }
        } else {
            if admin_task::exists() {
                let _ = admin_task::delete();
            }
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("запуск от администратора без запроса доступен только на Windows".into())
    }
}

#[tauri::command]
pub fn is_elevated_cmd() -> bool {
    is_elevated()
}

#[tauri::command]
pub fn relaunch_elevated_cmd(app: AppHandle) -> Result<(), String> {
    relaunch_elevated(&app)
}
