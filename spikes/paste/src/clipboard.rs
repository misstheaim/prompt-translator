//! Clipboard text read/write, hidden from clipboard history and monitors. Same approach as the
//! app's `system/input.rs` (`read_text`, `write_text`) and the layout fix's save/restore
//! (LAYOUT-004): only text is saved; an untouched clipboard is never rewritten.

use std::ptr;
use std::thread::sleep;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::core::HSTRING;

use crate::win::POLL_INTERVAL;

const CF_UNICODETEXT: u32 = 13;

/// Registered formats that keep text out of Windows clipboard history (Win+V) and away from
/// clipboard monitors; each gets a DWORD 0 (the app's `HIDDEN_MARKERS`).
const HIDDEN_MARKERS: &[&str] = &[
    "ExcludeClipboardContentFromMonitorProcessing",
    "CanIncludeInClipboardHistory",
];

/// `OpenClipboard` fails while another app has it open; retry this often (10 ms apart).
const OPEN_ATTEMPTS: u32 = 10;

#[derive(Debug)]
pub struct ClipboardBusy;

struct Opened;

impl Opened {
    fn open() -> Result<Self, ClipboardBusy> {
        for attempt in 0..OPEN_ATTEMPTS {
            if attempt > 0 {
                sleep(POLL_INTERVAL);
            }
            // SAFETY: a null owner is allowed; closed on drop.
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(Self);
            }
        }
        Err(ClipboardBusy)
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        // SAFETY: this thread opened the clipboard.
        let _ = unsafe { CloseClipboard() };
    }
}

pub fn sequence_number() -> u32 {
    // SAFETY: plain call.
    unsafe { GetClipboardSequenceNumber() }
}

/// The clipboard's text: `Ok(None)` when it holds no text.
pub fn read_text() -> Result<Option<String>, ClipboardBusy> {
    let _open = Opened::open()?;
    // SAFETY: the clipboard is open on this thread; the handle stays owned by the clipboard.
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
            return Ok(None);
        }
        let Ok(handle) = GetClipboardData(CF_UNICODETEXT) else {
            return Ok(None);
        };
        let memory = HGLOBAL(handle.0);
        let data = GlobalLock(memory) as *const u16;
        if data.is_null() {
            return Ok(None);
        }
        let units = std::slice::from_raw_parts(data, GlobalSize(memory) / 2);
        let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        let text = String::from_utf16_lossy(&units[..end]);
        let _ = GlobalUnlock(memory);
        Ok(Some(text))
    }
}

/// Replaces the clipboard with `text` (or leaves it empty), hidden from history and monitors.
pub fn write_text_hidden(text: Option<&str>) -> bool {
    let Ok(_open) = Opened::open() else {
        return false;
    };
    // SAFETY: the clipboard is open on this thread.
    if unsafe { EmptyClipboard() }.is_err() {
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
    for name in HIDDEN_MARKERS {
        // SAFETY: a valid string.
        let format = unsafe { RegisterClipboardFormatW(&HSTRING::from(*name)) };
        if format != 0 {
            set_data(format, &0u32.to_ne_bytes());
        }
    }
    true
}

fn set_data(format: u32, bytes: &[u8]) -> bool {
    // SAFETY: allocate, copy, hand the block to the open clipboard (which then owns it), or free
    // it when the clipboard refuses.
    unsafe {
        let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) else {
            return false;
        };
        let target = GlobalLock(memory) as *mut u8;
        if target.is_null() {
            let _ = GlobalFree(Some(memory));
            return false;
        }
        ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
        let _ = GlobalUnlock(memory);
        if SetClipboardData(format, Some(HANDLE(memory.0))).is_err() {
            let _ = GlobalFree(Some(memory));
            return false;
        }
        true
    }
}
