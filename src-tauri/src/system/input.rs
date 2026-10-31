//! `system::input`: Win32 helpers shared by the hotkeys (auto-copy for the popup, T25b, and the
//! layout fix, T42) and the session targets (T38): the terminal classifier, the foreground window,
//! top-level window facts (exists, visible, title, pid), waiting for modifier keys to be released,
//! synthetic key chords with `SendInput`, clipboard text read/write (optionally hidden from
//! clipboard history and monitors), the clipboard sequence number and input-language switching.
//!
//! Callers that send keys and touch the clipboard hold [`lock`] for the whole sequence, so two
//! hotkeys can't interleave their copies, pastes and restores.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// Windows Terminal's top-level window class (all its windows usually share one process).
pub const WINDOWS_TERMINAL_CLASS: &str = "CASCADIA_HOSTING_WINDOW_CLASS";

/// The classic console host (conhost) window class.
pub const CONHOST_CLASS: &str = "ConsoleWindowClass";

/// Window classes of terminal windows: Windows Terminal, classic console host (conhost).
pub const TERMINAL_WINDOW_CLASSES: &[&str] = &[WINDOWS_TERMINAL_CLASS, CONHOST_CLASS];

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

/// Extra time after the clipboard sequence number changes, for apps that write in several steps.
pub const SETTLE_TIME: Duration = Duration::from_millis(20);

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

/// The clipboard couldn't be opened (another app holds it open).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardBusy;

/// One synthetic-input + clipboard sequence at a time, across both hotkeys: a second press must
/// not save the first one's copied text as "the user's clipboard".
static INPUT_LOCK: Mutex<()> = Mutex::new(());

/// Holds the input lock until the guard is dropped (see the module docs).
pub fn lock() -> MutexGuard<'static, ()> {
    INPUT_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(windows)]
pub use win::*;

#[cfg(windows)]
mod win {
    use std::mem::size_of;
    use std::ptr;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{CloseHandle, GlobalFree, HGLOBAL, HWND, LPARAM};
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
        GetAsyncKeyState, GetKeyboardLayout, GetKeyboardLayoutList, HKL, INPUT, INPUT_0,
        INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC,
        MapVirtualKeyW, SendInput, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LEFT, VK_LWIN, VK_MENU,
        VK_RIGHT, VK_RWIN, VK_SHIFT, VK_V,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
        IsWindow, IsWindowVisible, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
    };
    use windows_sys::core::BOOL;

    use super::{ClipboardBusy, is_terminal};

    /// `CF_UNICODETEXT` (kept local to avoid the `Win32_System_Ole` feature for one constant).
    const CF_UNICODETEXT: u32 = 13;

    /// Any modifier still down would turn a synthetic chord into another shortcut.
    const MODIFIERS: &[VIRTUAL_KEY] = &[VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN];

    /// An unassigned virtual key: pressing it while Alt or Win is down keeps the release of those
    /// keys from opening the window's menu bar or the Start menu (the "menu mask" technique).
    const VK_MASK: VIRTUAL_KEY = 0xE8;

    /// Registered clipboard formats that keep hidden text out of Windows clipboard history and
    /// away from clipboard monitors, with the data each one gets (a DWORD 0).
    const HIDDEN_MARKERS: &[&str] = &[
        "ExcludeClipboardContentFromMonitorProcessing",
        "CanIncludeInClipboardHistory",
    ];

    /// Chromium's clipboard format for `DataTransfer.setData` with custom types (a pickle of
    /// UTF-16 type/value pairs). VS Code and Monaco editors put their copy metadata there.
    const CHROMIUM_CUSTOM_DATA_FORMAT: &str = "Chromium Web Custom MIME Data Format";

    /// What VS Code (and Monaco) write into their copy metadata when Ctrl+C copied the whole
    /// current line because nothing was selected (`editor.emptySelectionClipboard`).
    const EMPTY_SELECTION_COPY_MARKER: &str = "\"isFromEmptySelection\":true";

    const POLL_INTERVAL: Duration = Duration::from_millis(10);
    /// `OpenClipboard` fails while another app has the clipboard open; retry this often.
    const OPEN_ATTEMPTS: u32 = 10;

    /// A top-level window handle. Win32 validates handles itself, so a stale one only makes the
    /// calls below fail.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Window(HWND);

    /// The foreground window, or `None` when no window is in the foreground.
    pub fn foreground_window() -> Option<Window> {
        // SAFETY: no arguments; returns null when no window is in the foreground.
        let hwnd = unsafe { GetForegroundWindow() };
        (!hwnd.is_null()).then_some(Window(hwnd))
    }

    impl Window {
        /// The handle as a plain number, e.g. to pass it across a trait boundary.
        pub fn id(self) -> usize {
            self.0 as usize
        }

        /// The window whose handle is `id` (see [`Window::id`]).
        pub fn from_id(id: usize) -> Self {
            Self(id as HWND)
        }

        /// Whether this is still the foreground window.
        pub fn is_foreground(self) -> bool {
            foreground_window() == Some(self)
        }

        pub fn class_name(self) -> String {
            let mut buf = [0u16; 256];
            // SAFETY: `buf` is writable for `buf.len()` u16s; the result is the count written.
            let len = unsafe { GetClassNameW(self.0, buf.as_mut_ptr(), buf.len() as i32) };
            String::from_utf16_lossy(&buf[..usize::try_from(len).unwrap_or(0)])
        }

        /// Whether the handle still names a window (`IsWindow`). A closed window's handle may
        /// be reused later, so callers also compare the pid.
        pub fn exists(self) -> bool {
            // SAFETY: plain call; any handle value is accepted.
            unsafe { IsWindow(self.0) != 0 }
        }

        /// Whether the window has the visible style (minimised windows count as visible).
        pub fn is_visible(self) -> bool {
            // SAFETY: plain call.
            unsafe { IsWindowVisible(self.0) != 0 }
        }

        /// The id of the process that created the window, 0 for a dead handle. For a classic
        /// console this is the console's client (e.g. `powershell.exe`), not `conhost.exe`.
        pub fn pid(self) -> u32 {
            let mut pid = 0u32;
            // SAFETY: `pid` is a valid out pointer.
            unsafe { GetWindowThreadProcessId(self.0, &mut pid) };
            pid
        }

        /// The window's title ("" when it has none or the handle is dead).
        pub fn title(self) -> String {
            let mut buf = vec![0u16; 1024];
            // SAFETY: `buf` is writable for `buf.len()` u16s; the result is the count written.
            let len = unsafe { GetWindowTextW(self.0, buf.as_mut_ptr(), buf.len() as i32) };
            String::from_utf16_lossy(&buf[..usize::try_from(len).unwrap_or(0)])
        }

        /// The full path of the window's executable, or "" when it can't be read (e.g. a
        /// protected process); the class name alone decides then.
        pub fn exe(self) -> String {
            let pid = self.pid();
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
            // SAFETY: `process` is a valid handle with query rights; `buf` holds `len` u16s and
            // `len` receives the count written.
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

        /// [`is_terminal`] for this window: its class and its process's executable.
        pub fn is_terminal(self) -> bool {
            is_terminal(&self.class_name(), &self.exe())
        }

        /// Asks the window to switch its input language to an installed keyboard layout whose
        /// primary language is `language` (`LANG_RUSSIAN`, `LANG_ENGLISH`, ...). Does nothing
        /// when the window already uses that language or no such layout is installed. `true` if
        /// a request was posted.
        pub fn switch_input_language(self, language: u16) -> bool {
            // SAFETY: a window handle and a null pid pointer, which the call accepts.
            let thread = unsafe { GetWindowThreadProcessId(self.0, ptr::null_mut()) };
            // SAFETY: plain call.
            let current = unsafe { GetKeyboardLayout(thread) };
            if primary_language(current) == language {
                return false;
            }

            // SAFETY: with a zero-sized buffer the call only returns the number of layouts.
            let count = unsafe { GetKeyboardLayoutList(0, ptr::null_mut()) };
            let Ok(len) = usize::try_from(count) else {
                return false;
            };
            let mut layouts: Vec<HKL> = vec![ptr::null_mut(); len];
            // SAFETY: `layouts` has room for `count` HKLs.
            let filled = unsafe { GetKeyboardLayoutList(count, layouts.as_mut_ptr()) };
            layouts.truncate(usize::try_from(filled).unwrap_or(0));

            let Some(&target) = layouts
                .iter()
                .find(|&&hkl| primary_language(hkl) == language)
            else {
                log::debug!("No keyboard layout installed for language {language:#x}");
                return false;
            };
            // SAFETY: plain call; the message carries the HKL in lParam.
            let posted =
                unsafe { PostMessageW(self.0, WM_INPUTLANGCHANGEREQUEST, 0, target as isize) } != 0;
            if !posted {
                log::debug!("Could not post the input-language change request");
            }
            posted
        }
    }

    /// Every top-level window, in Z order (front to back), hidden ones included.
    pub fn top_level_windows() -> Vec<Window> {
        unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // SAFETY: `lparam` is the `Vec<Window>` passed below, alive for the whole call.
            let list = unsafe { &mut *(lparam as *mut Vec<Window>) };
            list.push(Window(hwnd));
            1
        }
        let mut list: Vec<Window> = Vec::new();
        // SAFETY: the callback only touches `list`, which outlives the call.
        unsafe { EnumWindows(Some(collect), &mut list as *mut Vec<Window> as LPARAM) };
        list
    }

    /// The primary language (`LANG_*`, the low 10 bits of the LANGID in the HKL's low word).
    fn primary_language(hkl: HKL) -> u16 {
        (hkl as usize & 0x3ff) as u16
    }

    fn is_key_down(key: VIRTUAL_KEY) -> bool {
        // SAFETY: plain call; the high bit of the result means "down now".
        let state = unsafe { GetAsyncKeyState(i32::from(key)) };
        state < 0
    }

    pub fn any_modifier_down() -> bool {
        MODIFIERS.iter().any(|&key| is_key_down(key))
    }

    /// Polls until no modifier is down; `false` if one is still held after `timeout`.
    pub fn wait_for_modifiers_released(timeout: Duration) -> bool {
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

    /// While Alt or Win is still held from the hotkey, taps an unassigned key so that releasing
    /// them doesn't open the foreground window's menu bar or the Start menu. Does nothing
    /// otherwise.
    pub fn mask_modifier_release() {
        if [VK_MENU, VK_LWIN, VK_RWIN].into_iter().any(is_key_down) {
            send_inputs(&[key_input(VK_MASK, false), key_input(VK_MASK, true)]);
        }
    }

    fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
        // SAFETY: plain call; maps the virtual key to its scan code (0 if there is none).
        let scan = unsafe { MapVirtualKeyW(u32::from(key), MAPVK_VK_TO_VSC) };
        let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
        // Arrow keys share scan codes with the numpad; without this flag apps may read Numpad 4 or 6.
        if key == VK_LEFT || key == VK_RIGHT {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: u16::try_from(scan).unwrap_or(0),
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Sends the events with `SendInput`. `false` if Windows blocked some of them (e.g. the
    /// foreground app runs elevated).
    fn send_inputs(inputs: &[INPUT]) -> bool {
        // SAFETY: `inputs` is a valid array of `inputs.len()` INPUT structs of the given size.
        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<INPUT>() as i32,
            )
        };
        if sent as usize != inputs.len() {
            log::warn!("SendInput sent {sent} of {} events", inputs.len());
            return false;
        }
        true
    }

    /// Presses `modifiers` in order, taps `key`, then releases the modifiers in reverse order.
    fn send_chord(modifiers: &[VIRTUAL_KEY], key: VIRTUAL_KEY) -> bool {
        let mut inputs: Vec<INPUT> = modifiers.iter().map(|&m| key_input(m, false)).collect();
        inputs.push(key_input(key, false));
        inputs.push(key_input(key, true));
        inputs.extend(modifiers.iter().rev().map(|&m| key_input(m, true)));
        send_inputs(&inputs)
    }

    /// Sends Ctrl+C to the foreground window. Never call it for a terminal.
    pub fn send_ctrl_c() -> bool {
        send_chord(&[VK_CONTROL], VK_C)
    }

    /// Sends Ctrl+V to the foreground window.
    pub fn send_ctrl_v() -> bool {
        send_chord(&[VK_CONTROL], VK_V)
    }

    /// Sends Ctrl+Shift+Left (select the previous word) to the foreground window.
    pub fn send_select_previous_word() -> bool {
        send_chord(&[VK_CONTROL, VK_SHIFT], VK_LEFT)
    }

    /// Sends Ctrl+Shift+Right (move the selection's moving end one word right) to the foreground
    /// window.
    pub fn send_select_next_word() -> bool {
        send_chord(&[VK_CONTROL, VK_SHIFT], VK_RIGHT)
    }

    pub fn sequence_number() -> u32 {
        // SAFETY: plain call.
        unsafe { GetClipboardSequenceNumber() }
    }

    /// Polls until the clipboard sequence number differs from `before`; `false` after `timeout`.
    pub fn wait_for_sequence_change(before: u32, timeout: Duration) -> bool {
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
        fn open() -> Result<Self, ClipboardBusy> {
            for attempt in 0..OPEN_ATTEMPTS {
                if attempt > 0 {
                    sleep(POLL_INTERVAL);
                }
                // SAFETY: a null owner is allowed; the clipboard is closed on drop.
                if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
                    return Ok(Self);
                }
            }
            Err(ClipboardBusy)
        }
    }

    impl Drop for OpenedClipboard {
        fn drop(&mut self) {
            // SAFETY: this thread opened the clipboard in `open`.
            unsafe { CloseClipboard() };
        }
    }

    fn register_format(name: &str) -> u32 {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string.
        unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
    }

    /// A copy of the open clipboard's data in `format`, or `None` when it has none.
    fn data_bytes(format: u32) -> Option<Vec<u8>> {
        // SAFETY: the caller opened the clipboard on this thread.
        if unsafe { IsClipboardFormatAvailable(format) } == 0 {
            return None;
        }
        // SAFETY: the clipboard is open; the handle stays owned by the clipboard.
        let handle = unsafe { GetClipboardData(format) };
        if handle.is_null() {
            return None;
        }
        // SAFETY: `handle` is a global memory handle from the open clipboard.
        let data = unsafe { GlobalLock(handle) }.cast::<u8>();
        if data.is_null() {
            return None;
        }
        // SAFETY: the locked block is `GlobalSize` bytes long and stays valid until unlocked.
        let bytes = unsafe { std::slice::from_raw_parts(data, GlobalSize(handle)) }.to_vec();
        // SAFETY: locked above.
        unsafe { GlobalUnlock(handle) };
        Some(bytes)
    }

    /// The clipboard's text: `Ok(None)` when it holds no text, `Err` when it can't be opened.
    pub fn read_text() -> Result<Option<String>, ClipboardBusy> {
        let _open = OpenedClipboard::open()?;
        let Some(bytes) = data_bytes(CF_UNICODETEXT) else {
            return Ok(None);
        };
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_ne_bytes(pair))
            .collect();
        let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        Ok(Some(String::from_utf16_lossy(&units[..end])))
    }

    /// Whether the clipboard holds a VS Code / Monaco "copy the current line" result, i.e. the
    /// editor had no selection when Ctrl+C was pressed.
    pub fn clipboard_is_empty_selection_copy() -> bool {
        let Ok(_open) = OpenedClipboard::open() else {
            return false;
        };
        let format = register_format(CHROMIUM_CUSTOM_DATA_FORMAT);
        if format == 0 {
            return false;
        }
        let Some(bytes) = data_bytes(format) else {
            return false;
        };
        // The pickle stores strings as raw UTF-16, so the JSON marker appears verbatim.
        let marker: Vec<u8> = EMPTY_SELECTION_COPY_MARKER
            .encode_utf16()
            .flat_map(u16::to_ne_bytes)
            .collect();
        bytes.windows(marker.len()).any(|w| w == marker.as_slice())
    }

    /// Replaces the clipboard with `text` (or leaves it empty). With `hidden`, the text is marked
    /// as excluded from clipboard history and monitors. `false` if the clipboard couldn't be
    /// written.
    pub fn write_text(text: Option<&str>, hidden: bool) -> bool {
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
        if hidden {
            for name in HIDDEN_MARKERS {
                let format = register_format(name);
                if format != 0 {
                    set_data(format, &0u32.to_ne_bytes());
                }
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
