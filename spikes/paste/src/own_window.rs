//! `--own-window`: a small top-level window of the spike's own, standing in for the app's
//! Translate tab. The paste starts when Enter is **released** in it, so the spike process owns the
//! foreground window and received the last input when it calls `SetForegroundWindow`, exactly
//! like the app after the user's Enter. The window stays open (not destroyed) until the paste is
//! done, so the foreground rights aren't lost to another window first.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GetMessageW, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, TranslateMessage, WINDOW_EX_STYLE, WM_CLOSE, WM_KEYDOWN, WM_KEYUP,
    WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::w;

const PENDING: u8 = 0;
const ENTER: u8 = 1;
const CLOSED: u8 = 2;

static RESULT: AtomicU8 = AtomicU8::new(PENDING);
static ENTER_PRESSED: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let enter = wparam.0 == usize::from(VK_RETURN.0);
    match message {
        WM_KEYDOWN if enter => {
            ENTER_PRESSED.store(true, Ordering::SeqCst);
            LRESULT(0)
        }
        // Only an Enter pressed in this window counts: the key-up of the Enter that started the
        // command in the console can arrive here once the window takes the foreground.
        WM_KEYUP if enter && ENTER_PRESSED.load(Ordering::SeqCst) => {
            RESULT.store(ENTER, Ordering::SeqCst);
            // SAFETY: plain call.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_CLOSE => {
            RESULT.store(CLOSED, Ordering::SeqCst);
            // SAFETY: plain call.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: default handling for everything else.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// The spike's window, open until dropped.
pub struct OwnWindow(HWND);

impl Drop for OwnWindow {
    fn drop(&mut self) {
        // SAFETY: created by `wait_for_enter`, destroyed once.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

/// Opens the window and pumps messages until Enter is released in it (`Some`) or it is closed
/// (`None`).
pub fn wait_for_enter() -> windows::core::Result<Option<OwnWindow>> {
    // SAFETY: plain Win32 window creation and a standard message loop on this thread.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("PasteSpikeWindow");
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("Paste spike: press Enter here to paste (close = cancel)"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            120,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let window = OwnWindow(hwnd);
        // May be refused (then the window flashes); the owner clicks it in that case.
        let _ = SetForegroundWindow(hwnd);
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        Ok((RESULT.load(Ordering::SeqCst) == ENTER).then_some(window))
    }
}
