//! `system::clipboard`: auto-copy of the selection for the popup hotkey (plan D5, T25b).
//!
//! With `autoCopySelection` on, [`copy_selection`] copies what is selected in the foreground app
//! so "select + hotkey" works without a manual Ctrl+C:
//! 1. Terminals (see [`is_terminal`]) are skipped: Ctrl+C there interrupts the running command.
//! 2. It waits until every modifier key is released (at most [`MODIFIER_RELEASE_TIMEOUT`]); if
//!    they are still held it gives up. Ctrl+C is never sent while Shift is down, because
//!    Ctrl+Shift+C opens DevTools in Chromium apps.
//! 3. It saves the clipboard text, sends Ctrl+C with `SendInput` and waits (at most
//!    [`COPY_TIMEOUT`]) for the clipboard sequence number to change.
//! 4. It reads the new text, then puts the saved text back, marked so that Windows clipboard
//!    history and clipboard monitors ignore the restore.
//!
//! The caller checks the setting. `None` means "use the clipboard as it is": a terminal,
//! modifiers held, the input was blocked, or nothing was selected.
//! IDE-integrated terminals can't be told apart from the editor; Ctrl+C with a selection copies
//! there, which is acceptable.

use std::time::Duration;

/// Window classes of terminal windows: Windows Terminal, classic console host (conhost).
pub const TERMINAL_WINDOW_CLASSES: &[&str] =
    &["CASCADIA_HOSTING_WINDOW_CLASS", "ConsoleWindowClass"];

/// Executable names of terminal apps.
pub const TERMINAL_EXE_NAMES: &[&str] = &[
    "WindowsTerminal.exe",
    "wezterm-gui.exe",
    "alacritty.exe",
    "mintty.exe",
];

/// How long to wait for the hotkey's modifier keys to be released before giving up.
pub const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_millis(500);

/// How long to wait for the foreground app to put the copied selection on the clipboard.
pub const COPY_TIMEOUT: Duration = Duration::from_millis(300);

/// Whether a window is a terminal, from its window class and its process's executable (a bare
/// file name or a full path; empty when unknown). Both comparisons ignore ASCII case, as Windows
/// does for class and file names.
pub fn is_terminal(class_name: &str, exe: &str) -> bool {
    let exe_name = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    TERMINAL_WINDOW_CLASSES
        .iter()
        .any(|class| class.eq_ignore_ascii_case(class_name))
        || TERMINAL_EXE_NAMES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(exe_name))
}

/// Copies the foreground app's selection (see the module docs). `Some` only when Ctrl+C put
/// non-empty text on the clipboard; the clipboard then holds its previous text again.
pub fn copy_selection() -> Option<String> {
    #[cfg(windows)]
    {
        win::copy_selection()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
mod win {
    use std::mem::size_of;
    use std::ptr;
    use std::sync::Mutex;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{CloseHandle, GlobalFree, HGLOBAL, HWND};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LWIN,
        VK_MENU, VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId,
    };

    use super::{COPY_TIMEOUT, MODIFIER_RELEASE_TIMEOUT, is_terminal};

    /// `CF_UNICODETEXT` (kept local to avoid the `Win32_System_Ole` feature for one constant).
    const CF_UNICODETEXT: u32 = 13;

    /// Any modifier still down would turn the Ctrl+C into another shortcut.
    const MODIFIERS: &[VIRTUAL_KEY] = &[VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN];

    /// Registered clipboard formats that keep the restored text out of Windows clipboard history
    /// and away from clipboard monitors, with the data each one gets (a DWORD 0).
    const RESTORE_MARKERS: &[&str] = &[
        "ExcludeClipboardContentFromMonitorProcessing",
        "CanIncludeInClipboardHistory",
    ];

    const POLL_INTERVAL: Duration = Duration::from_millis(10);
    /// Extra time after the sequence number changes, for apps that write in several steps.
    const SETTLE_TIME: Duration = Duration::from_millis(20);
    /// `OpenClipboard` fails while another app has the clipboard open; retry this often.
    const OPEN_ATTEMPTS: u32 = 10;

    /// One capture at a time: a second hotkey press must not save the first one's copied text.
    static CAPTURE_LOCK: Mutex<()> = Mutex::new(());

    pub fn copy_selection() -> Option<String> {
        let _guard = CAPTURE_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // SAFETY: no arguments; returns null when no window is in the foreground.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return None;
        }
        let class_name = window_class(hwnd);
        let exe = window_exe(hwnd);
        if is_terminal(&class_name, &exe) {
            log::debug!("Auto-copy skipped: the foreground window is a terminal ({class_name})");
            return None;
        }

        if !wait_for_modifiers_released(MODIFIER_RELEASE_TIMEOUT) {
            log::info!("Auto-copy skipped: modifier keys still held; using the clipboard");
            return None;
        }

        // Save before copying; without a readable clipboard nothing could be restored.
        let saved = match read_text() {
            Ok(saved) => saved,
            Err(()) => {
                log::warn!("Auto-copy skipped: the clipboard is busy");
                return None;
            }
        };
        let before = sequence_number();

        // While we waited, a modifier may have been pressed again or another window (possibly a
        // terminal) may have come to the front; the classification above is only for `hwnd`.
        // SAFETY: no arguments.
        let still_foreground = unsafe { GetForegroundWindow() } == hwnd;
        if !still_foreground || any_modifier_down() {
            log::debug!("Auto-copy skipped: focus or modifiers changed before Ctrl+C");
            return None;
        }
        if !send_ctrl_c() {
            return None;
        }
        if !wait_for_sequence_change(before, COPY_TIMEOUT) {
            log::debug!("Auto-copy: nothing was copied (no selection); using the clipboard");
            return None;
        }
        sleep(SETTLE_TIME);

        let copied = read_text().ok().flatten();
        if !write_restored(saved.as_deref()) {
            log::warn!("Auto-copy: could not restore the previous clipboard text");
        }
        copied.filter(|text| !text.is_empty())
    }

    fn window_class(hwnd: HWND) -> String {
        let mut buf = [0u16; 256];
        // SAFETY: `buf` is writable for `buf.len()` u16s; the result is the count written.
        let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..usize::try_from(len).unwrap_or(0)])
    }

    /// The full path of the window's executable, or "" when it can't be read (e.g. a protected
    /// process); the class name alone decides then.
    fn window_exe(hwnd: HWND) -> String {
        let mut pid = 0u32;
        // SAFETY: `pid` is a valid out pointer.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid == 0 {
            return String::new();
        }
        // SAFETY: plain call; returns null on failure.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: `process` is a valid handle with query rights; `buf` holds `len` u16s and `len`
        // receives the count written.
        let ok = unsafe {
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len)
        };
        // SAFETY: `process` was opened above and is closed exactly once.
        unsafe { CloseHandle(process) };
        if ok == 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len as usize])
    }

    fn is_key_down(key: VIRTUAL_KEY) -> bool {
        // SAFETY: plain call; the high bit of the result means "down now".
        let state = unsafe { GetAsyncKeyState(i32::from(key)) };
        state < 0
    }

    fn any_modifier_down() -> bool {
        MODIFIERS.iter().any(|&key| is_key_down(key))
    }

    /// Polls until no modifier is down; `false` if one is still held after `timeout`.
    fn wait_for_modifiers_released(timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if !any_modifier_down() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            sleep(POLL_INTERVAL);
        }
    }

    fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
        // SAFETY: plain call; maps the virtual key to its scan code (0 if there is none).
        let scan = unsafe { MapVirtualKeyW(u32::from(key), MAPVK_VK_TO_VSC) };
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: u16::try_from(scan).unwrap_or(0),
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Sends Ctrl+C to the foreground window. `false` if Windows blocked the input (e.g. the
    /// foreground app runs elevated).
    fn send_ctrl_c() -> bool {
        let inputs = [
            key_input(VK_CONTROL, false),
            key_input(VK_C, false),
            key_input(VK_C, true),
            key_input(VK_CONTROL, true),
        ];
        // SAFETY: `inputs` is a valid array of `inputs.len()` INPUT structs of the given size.
        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<INPUT>() as i32,
            )
        };
        if sent as usize != inputs.len() {
            log::warn!(
                "Auto-copy: SendInput sent {sent} of {} events",
                inputs.len()
            );
            return false;
        }
        true
    }

    fn sequence_number() -> u32 {
        // SAFETY: plain call.
        unsafe { GetClipboardSequenceNumber() }
    }

    fn wait_for_sequence_change(before: u32, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if sequence_number() != before {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            sleep(POLL_INTERVAL);
        }
    }

    /// The clipboard, open for this thread until dropped.
    struct OpenedClipboard;

    impl OpenedClipboard {
        fn open() -> Result<Self, ()> {
            for attempt in 0..OPEN_ATTEMPTS {
                if attempt > 0 {
                    sleep(POLL_INTERVAL);
                }
                // SAFETY: a null owner is allowed; the clipboard is closed on drop.
                if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
                    return Ok(Self);
                }
            }
            Err(())
        }
    }

    impl Drop for OpenedClipboard {
        fn drop(&mut self) {
            // SAFETY: this thread opened the clipboard in `open`.
            unsafe { CloseClipboard() };
        }
    }

    /// The clipboard's text: `Ok(None)` when it holds no text, `Err` when it can't be opened.
    fn read_text() -> Result<Option<String>, ()> {
        let _open = OpenedClipboard::open()?;
        // SAFETY: the clipboard is open on this thread.
        if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } == 0 {
            return Ok(None);
        }
        // SAFETY: the clipboard is open; the handle stays owned by the clipboard.
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
        if handle.is_null() {
            return Ok(None);
        }
        // SAFETY: `handle` is a global memory handle from the open clipboard.
        let data = unsafe { GlobalLock(handle) }.cast::<u16>();
        if data.is_null() {
            return Ok(None);
        }
        // SAFETY: the locked block is `GlobalSize` bytes long and stays valid until unlocked.
        let text = unsafe {
            let units = std::slice::from_raw_parts(data, GlobalSize(handle) / 2);
            let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
            String::from_utf16_lossy(&units[..end])
        };
        // SAFETY: locked above.
        unsafe { GlobalUnlock(handle) };
        Ok(Some(text))
    }

    /// Replaces the clipboard with `text` (or leaves it empty), marked as excluded from
    /// clipboard history and monitors. `false` if the clipboard couldn't be written.
    fn write_restored(text: Option<&str>) -> bool {
        let Ok(_open) = OpenedClipboard::open() else {
            return false;
        };
        // SAFETY: the clipboard is open on this thread.
        if unsafe { EmptyClipboard() } == 0 {
            return false;
        }
        let Some(text) = text else {
            return true;
        };
        let bytes: Vec<u8> = text
            .encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_ne_bytes)
            .collect();
        if !set_data(CF_UNICODETEXT, &bytes) {
            return false;
        }
        for name in RESTORE_MARKERS {
            let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            // SAFETY: `wide` is a NUL-terminated UTF-16 string.
            let format = unsafe { RegisterClipboardFormatW(wide.as_ptr()) };
            if format != 0 {
                set_data(format, &0u32.to_ne_bytes());
            }
        }
        true
    }

    /// Puts a copy of `bytes` on the open clipboard in `format`.
    fn set_data(format: u32, bytes: &[u8]) -> bool {
        let Some(memory) = global_copy(bytes) else {
            return false;
        };
        // SAFETY: the clipboard is open and emptied by this thread; on success the system owns
        // `memory`.
        if unsafe { SetClipboardData(format, memory) }.is_null() {
            // SAFETY: the clipboard didn't take ownership, so the block is still ours to free.
            unsafe { GlobalFree(memory) };
            return false;
        }
        true
    }

    /// A movable global memory block holding a copy of `bytes`.
    fn global_copy(bytes: &[u8]) -> Option<HGLOBAL> {
        // SAFETY: plain allocation; returns null on failure.
        let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) };
        if memory.is_null() {
            return None;
        }
        // SAFETY: `memory` is a valid movable block of `bytes.len()` bytes.
        let target = unsafe { GlobalLock(memory) }.cast::<u8>();
        if target.is_null() {
            // SAFETY: the block was allocated above and isn't used afterwards.
            unsafe { GlobalFree(memory) };
            return None;
        }
        // SAFETY: `target` points to `bytes.len()` writable bytes that don't overlap `bytes`.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
            GlobalUnlock(memory);
        }
        Some(memory)
    }
}
