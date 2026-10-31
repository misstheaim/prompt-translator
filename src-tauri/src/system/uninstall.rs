//! Uninstalling from inside the app (Settings, About): finds the NSIS uninstaller registered for
//! the current user and starts it detached.

use std::path::PathBuf;

use crate::error::{AppError, AppResult};

/// The uninstaller's executable, read from the per-user NSIS entry
/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Prompt Translator`. `None` when the
/// entry is missing (a dev run or a portable copy) or the file no longer exists.
pub fn uninstaller_path() -> Option<PathBuf> {
    let command = read_uninstall_string()?;
    let path = PathBuf::from(executable_of(&command));
    path.is_file().then_some(path)
}

/// The executable part of an `UninstallString` (`"C:\...\uninstall.exe"` with optional arguments).
fn executable_of(command: &str) -> &str {
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        return rest.split('"').next().unwrap_or(rest);
    }
    // Unquoted: the path may hold spaces, so keep everything up to the `.exe`.
    match command.to_ascii_lowercase().find(".exe") {
        Some(i) => &command[..i + 4],
        None => command,
    }
}

/// Starts the uninstaller as a separate process that outlives the app.
pub fn run_uninstaller() -> AppResult<()> {
    let path = uninstaller_path().ok_or_else(|| {
        AppError::Io("The uninstaller wasn't found. Uninstall from Windows Settings, Apps.".into())
    })?;
    spawn_detached(&path).map_err(|e| AppError::Io(format!("Couldn't start the uninstaller: {e}")))
}

#[cfg(windows)]
fn spawn_detached(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};

    std::process::Command::new(path)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
fn spawn_detached(_path: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::Error::other("Windows only"))
}

#[cfg(windows)]
fn read_uninstall_string() -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let subkey = wide(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Prompt Translator");
    let value = wide("UninstallString");
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;

    let mut size: u32 = 0;
    // SAFETY: valid NUL-terminated strings; a null data pointer only asks for the size.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            flags,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if status != 0 || size < 2 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: `buf` holds at least `size` bytes.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            flags,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let text = String::from_utf16_lossy(&buf[..len]);
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(not(windows))]
fn read_uninstall_string() -> Option<String> {
    None
}
