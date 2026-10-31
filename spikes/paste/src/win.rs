//! Plain Win32 helpers: window facts, focus, key state and synthetic input. Mirrors the app's
//! `src-tauri/src/system/input.rs` where the two overlap (terminal classifier, modifier check,
//! Ctrl+V chord), so what the spike proves carries over.

use std::mem::size_of;
use std::thread::sleep;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LWIN,
    VK_MENU, VK_RETURN, VK_RWIN, VK_SHIFT, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, MONITORINFOF_PRIMARY,
    SW_RESTORE, SetForegroundWindow, ShowWindow,
};
use windows::core::{BOOL, PWSTR};

/// Window classes of terminal windows (the app's `TERMINAL_WINDOW_CLASSES`, POPUP-007).
pub const TERMINAL_WINDOW_CLASSES: &[&str] =
    &["CASCADIA_HOSTING_WINDOW_CLASS", "ConsoleWindowClass"];

/// Executable names of terminal apps (the app's `TERMINAL_EXE_NAMES`).
pub const TERMINAL_EXE_NAMES: &[&str] = &[
    "WindowsTerminal.exe",
    "wezterm-gui.exe",
    "alacritty.exe",
    "mintty.exe",
];

/// Windows Terminal's top-level window class: the only host whose tabs UIA exposes here.
pub const WINDOWS_TERMINAL_CLASS: &str = "CASCADIA_HOSTING_WINDOW_CLASS";

/// An unassigned virtual key (the app's `VK_MASK`): tapped while Alt is down so releasing Alt
/// doesn't open a menu bar.
const VK_MASK: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

pub const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub fn hwnd_from(value: usize) -> HWND {
    HWND(value as *mut core::ffi::c_void)
}

pub fn hwnd_value(hwnd: HWND) -> usize {
    hwnd.0 as usize
}

pub fn is_terminal(class_name: &str, exe: &str) -> bool {
    let exe_name = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    TERMINAL_WINDOW_CLASSES
        .iter()
        .any(|class| class.eq_ignore_ascii_case(class_name))
        || TERMINAL_EXE_NAMES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(exe_name))
}

/// Every top-level window, in Z order (front to back).
pub fn top_level_windows() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the `Vec<HWND>` passed by `top_level_windows`, alive for the call.
        let list = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
        list.push(hwnd);
        BOOL(1)
    }
    let mut list: Vec<HWND> = Vec::new();
    // SAFETY: the callback only touches `list`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut list as *mut Vec<HWND> as isize)) };
    list
}

pub fn exists(hwnd: HWND) -> bool {
    // SAFETY: plain call; any value is accepted.
    unsafe { IsWindow(Some(hwnd)) }.as_bool()
}

pub fn is_visible(hwnd: HWND) -> bool {
    // SAFETY: plain call.
    unsafe { IsWindowVisible(hwnd) }.as_bool()
}

pub fn is_minimised(hwnd: HWND) -> bool {
    // SAFETY: plain call.
    unsafe { IsIconic(hwnd) }.as_bool()
}

pub fn foreground() -> HWND {
    // SAFETY: plain call; null when no window is in front.
    unsafe { GetForegroundWindow() }
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: `buf` is writable; the result is the count written.
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..usize::try_from(len).unwrap_or(0)])
}

pub fn title(hwnd: HWND) -> String {
    let mut buf = vec![0u16; 1024];
    // SAFETY: `buf` is writable; the result is the count written.
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..usize::try_from(len).unwrap_or(0)])
}

/// The window's process id and thread id (0, 0 for a dead handle).
pub fn pid_and_thread(hwnd: HWND) -> (u32, u32) {
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out pointer.
    let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid, thread)
}

/// The process's executable path, "" when it can't be read.
pub fn exe(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    // SAFETY: plain call.
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return String::new();
    };
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` holds `len` u16s; `len` receives the count written.
    let ok = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    };
    // SAFETY: opened above, closed once.
    let _ = unsafe { CloseHandle(process) };
    if ok.is_err() {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// "primary" or "other@<left>,<top>" for the monitor that holds most of the window.
pub fn monitor(hwnd: HWND) -> String {
    // SAFETY: plain call; NEAREST always returns a monitor.
    let handle = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid MONITORINFO with its size set.
    if !unsafe { GetMonitorInfoW(handle, &mut info) }.as_bool() {
        return "unknown".into();
    }
    if info.dwFlags & MONITORINFOF_PRIMARY != 0 {
        "primary".into()
    } else {
        format!("other@{},{}", info.rcMonitor.left, info.rcMonitor.top)
    }
}

fn is_key_down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: plain call; the high bit means "down now".
    unsafe { GetAsyncKeyState(i32::from(key.0)) < 0 }
}

/// The held keys that would corrupt the paste or submit the prompt: the app's modifiers (Shift,
/// Ctrl, Alt, Win turn Ctrl+V into another shortcut) and Enter (an
/// Enter still held when the terminal takes focus would auto-repeat into Claude and send).
pub fn blocking_keys_down() -> Vec<&'static str> {
    let named: [(VIRTUAL_KEY, &str); 6] = [
        (VK_SHIFT, "Shift"),
        (VK_CONTROL, "Ctrl"),
        (VK_MENU, "Alt"),
        (VK_LWIN, "LWin"),
        (VK_RWIN, "RWin"),
        (VK_RETURN, "Enter"),
    ];
    named
        .iter()
        .filter(|(key, _)| is_key_down(*key))
        .map(|(_, name)| *name)
        .collect()
}

/// Polls until no blocking key is down; the keys still held after `timeout` otherwise.
pub fn wait_for_keys_released(timeout: Duration) -> Result<(), Vec<&'static str>> {
    let deadline = Instant::now() + timeout;
    loop {
        let held = blocking_keys_down();
        if held.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(held);
        }
        sleep(POLL_INTERVAL);
    }
}

/// Polls `check` until it's true or `timeout` passes.
pub fn wait_until(timeout: Duration, mut check: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if check() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(POLL_INTERVAL);
    }
}

/// `ShowWindow(SW_RESTORE)`; returns whether the window was visible before (the BOOL result).
pub fn restore(hwnd: HWND) -> bool {
    // SAFETY: plain call.
    unsafe { ShowWindow(hwnd, SW_RESTORE) }.as_bool()
}

/// How the spike asks Windows to bring the window to the front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusMethod {
    /// `SetForegroundWindow` only: what an app may do while it owns the foreground window.
    Plain,
    /// Alt down + unassigned key + Alt up with `SendInput` first (the common workaround for the
    /// foreground lock), then `SetForegroundWindow`.
    Alt,
    /// `AttachThreadInput` to the current foreground thread, `SetForegroundWindow` +
    /// `BringWindowToTop`, detach.
    Attach,
}

impl FocusMethod {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "plain" => Some(Self::Plain),
            "alt" => Some(Self::Alt),
            "attach" => Some(Self::Attach),
            _ => None,
        }
    }
}

/// Runs the focus method once; the BOOL `SetForegroundWindow` returned.
pub fn set_foreground(hwnd: HWND, method: FocusMethod) -> bool {
    match method {
        FocusMethod::Plain => {
            // SAFETY: plain call.
            unsafe { SetForegroundWindow(hwnd) }.as_bool()
        }
        FocusMethod::Alt => {
            send_inputs(&[
                key_input(VK_MENU, false),
                key_input(VK_MASK, false),
                key_input(VK_MASK, true),
                key_input(VK_MENU, true),
            ]);
            // SAFETY: plain call.
            unsafe { SetForegroundWindow(hwnd) }.as_bool()
        }
        FocusMethod::Attach => {
            let (_, front_thread) = pid_and_thread(foreground());
            // SAFETY: plain call.
            let own_thread = unsafe { GetCurrentThreadId() };
            let attached = front_thread != 0
                && front_thread != own_thread
                // SAFETY: plain call with two thread ids.
                && unsafe { AttachThreadInput(own_thread, front_thread, true) }.as_bool();
            // SAFETY: plain calls.
            let ok = unsafe { SetForegroundWindow(hwnd) }.as_bool();
            let _ = unsafe { BringWindowToTop(hwnd) };
            if attached {
                // SAFETY: undoes the attach above.
                let _ = unsafe { AttachThreadInput(own_thread, front_thread, false) };
            }
            ok
        }
    }
}

fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    // SAFETY: plain call; 0 when the key has no scan code.
    let scan = unsafe { MapVirtualKeyW(u32::from(key.0), MAPVK_VK_TO_VSC) };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: u16::try_from(scan).unwrap_or(0),
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// `false` if Windows blocked some events (e.g. the foreground app runs elevated: UIPI).
fn send_inputs(inputs: &[INPUT]) -> bool {
    // SAFETY: `inputs` is a valid slice of INPUT structs of the given size.
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}

/// Ctrl down, V down, V up, Ctrl up, in one `SendInput` call. Never anything else: no Enter.
pub fn send_ctrl_v() -> bool {
    send_inputs(&[
        key_input(VK_CONTROL, false),
        key_input(VK_V, false),
        key_input(VK_V, true),
        key_input(VK_CONTROL, true),
    ])
}
