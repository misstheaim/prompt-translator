//! `system::uia`: UI Automation reads of Windows Terminal's tabs (`TabItem` elements: name,
//! RuntimeId, selected), as spike T58 found them (`docs/spikes/session-paste.md`), and selecting
//! one (`Tab::select`, used when delivering to a session). Nothing here types.
//!
//! It also classifies the focused element as editable or read-only for the popup hotkey
//! (POPUP-012, T54): [`spawn_source_kind`] runs [`Uia::focused_source_kind`] on its own thread so
//! the caller can stop waiting after a short cap.
//!
//! COM is initialised (multithreaded, as UIA clients should) for as long as a [`Uia`] lives, so
//! create it on a worker thread, never on the UI thread, and drop it on the same thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    SAFEARRAY,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};
use windows::Win32::System::Variant::{VARIANT, VT_BOOL};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomation2, IUIAutomationElement, IUIAutomationElementArray,
    IUIAutomationSelectionItemPattern, IUIAutomationTextPattern, IUIAutomationValuePattern,
    TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_DocumentControlTypeId,
    UIA_EditControlTypeId, UIA_HyperlinkControlTypeId, UIA_IsReadOnlyAttributeId,
    UIA_SelectionItemPatternId, UIA_TabItemControlTypeId, UIA_TextControlTypeId, UIA_TextPatternId,
    UIA_ValuePatternId,
};
use windows::core::Interface;

pub use windows::core::Result;

use super::popup::SourceKind;

/// UIA's own limits for the focused-element query, so a hung app ends the worker thread
/// eventually even though the caller stopped waiting long before.
const SOURCE_KIND_UIA_TIMEOUT_MS: u32 = 2000;

/// Set while a focused-element query runs; a new one isn't started until it ends, so threads
/// stuck on a hung app can't pile up.
static SOURCE_KIND_BUSY: AtomicBool = AtomicBool::new(false);

/// One Windows Terminal tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabInfo {
    /// The tab's name (Claude Code's task summary while it runs; it changes often).
    pub name: String,
    /// The RuntimeId as dot-separated decimal ints, e.g. `42.165152658.4.9`. Stable while the
    /// tab exists; the only way to tell tabs apart.
    pub runtime_id: String,
    pub selected: bool,
}

/// COM for this thread, uninitialised on drop when this call initialised it.
struct ComGuard {
    initialised: bool,
}

impl ComGuard {
    fn init() -> Self {
        // SAFETY: plain call. S_OK and S_FALSE (already initialised) both need a matching
        // CoUninitialize; RPC_E_CHANGED_MODE (an STA thread) doesn't, and UIA still works there.
        let initialised = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        Self { initialised }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialised {
            // SAFETY: balances the successful CoInitializeEx in `init` on the same thread.
            unsafe { CoUninitialize() };
        }
    }
}

/// A UI Automation client. Fields drop in order: the automation object before COM.
pub struct Uia {
    automation: IUIAutomation,
    _com: ComGuard,
}

impl Uia {
    pub fn new() -> Result<Self> {
        let com = ComGuard::init();
        // SAFETY: plain COM activation on a thread with COM initialised.
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
        Ok(Self {
            automation,
            _com: com,
        })
    }

    /// The `TabItem` elements under the top-level window `hwnd`, in tree order.
    fn tab_elements(&self, hwnd: usize) -> Result<IUIAutomationElementArray> {
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        // SAFETY: COM calls on valid interfaces; a dead HWND makes ElementFromHandle fail.
        unsafe {
            let root = self.automation.ElementFromHandle(hwnd)?;
            let condition = self.automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &VARIANT::from(UIA_TabItemControlTypeId.0),
            )?;
            root.FindAll(TreeScope_Descendants, &condition)
        }
    }

    /// The tab of window `hwnd` with this RuntimeId, `None` when there is none. An error when
    /// the window is gone or doesn't answer UIA.
    pub fn find_tab(&self, hwnd: usize, wanted: &str) -> Result<Option<Tab>> {
        let found = self.tab_elements(hwnd)?;
        // SAFETY: COM calls on a valid element array and its elements.
        unsafe {
            for index in 0..found.Length()? {
                let element = found.GetElement(index)?;
                if runtime_id(&element)? == wanted {
                    return Ok(Some(Tab { element }));
                }
            }
        }
        Ok(None)
    }

    /// Every `TabItem` under the top-level window `hwnd` (see `input::Window::id`), in tree
    /// order. An error when the window is gone or doesn't answer UIA.
    pub fn tabs(&self, hwnd: usize) -> Result<Vec<TabInfo>> {
        let found = self.tab_elements(hwnd)?;
        // SAFETY: COM calls on a valid element array and its elements.
        unsafe {
            let count = found.Length()?;
            let mut tabs = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
            for index in 0..count {
                let element = found.GetElement(index)?;
                let runtime_id = runtime_id(&element)?;
                if runtime_id.is_empty() {
                    continue;
                }
                let name = element
                    .CurrentName()
                    .map(|b| b.to_string())
                    .unwrap_or_default();
                tabs.push(TabInfo {
                    name,
                    runtime_id,
                    selected: is_selected(&element),
                });
            }
            Ok(tabs)
        }
    }

    /// Classifies the element that has the keyboard focus (POPUP-012):
    /// 1. a `ValuePattern` decides by its read-only flag (inputs, text areas, editors; Chromium
    ///    also reports web documents and links here, as read-only);
    /// 2. otherwise a `TextPattern` decides by its document's `IsReadOnly` text attribute;
    /// 3. otherwise the control type: Edit is editable; Document, Text and Hyperlink are
    ///    read-only; anything else is unknown.
    ///
    /// Every failed call gives `Unknown`. Can block on a slow or hung app (up to UIA's own
    /// timeouts), so callers use [`spawn_source_kind`].
    pub fn focused_source_kind(&self) -> SourceKind {
        if let Ok(automation) = self.automation.cast::<IUIAutomation2>() {
            // SAFETY: COM calls on a valid interface; only this client's limits change.
            unsafe {
                let _ = automation.SetConnectionTimeout(SOURCE_KIND_UIA_TIMEOUT_MS);
                let _ = automation.SetTransactionTimeout(SOURCE_KIND_UIA_TIMEOUT_MS);
            }
        }
        // SAFETY: COM calls on a valid interface and the element it returns.
        let Ok(element) = (unsafe { self.automation.GetFocusedElement() }) else {
            return SourceKind::Unknown;
        };
        if let Some(read_only) = value_read_only(&element).or_else(|| text_read_only(&element)) {
            return if read_only {
                SourceKind::ReadOnly
            } else {
                SourceKind::Editable
            };
        }
        // SAFETY: COM call on a valid element.
        match unsafe { element.CurrentControlType() } {
            Ok(control) if control == UIA_EditControlTypeId => SourceKind::Editable,
            Ok(control)
                if control == UIA_DocumentControlTypeId
                    || control == UIA_TextControlTypeId
                    || control == UIA_HyperlinkControlTypeId =>
            {
                SourceKind::ReadOnly
            }
            _ => SourceKind::Unknown,
        }
    }
}

/// Starts classifying the focused element ([`Uia::focused_source_kind`]) on a new thread and
/// returns where its answer arrives. `None` while an earlier query is still running (a hung
/// app): the caller treats that as `Unknown` without waiting.
pub fn spawn_source_kind() -> Option<mpsc::Receiver<SourceKind>> {
    if SOURCE_KIND_BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        log::debug!("Source kind: an earlier UIA query is still running");
        return None;
    }
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("uia-source-kind".into())
        .spawn(move || {
            let kind = Uia::new()
                .map(|uia| uia.focused_source_kind())
                .unwrap_or(SourceKind::Unknown);
            SOURCE_KIND_BUSY.store(false, Ordering::Release);
            // The caller may have stopped waiting; then nobody needs the answer.
            let _ = sender.send(kind);
        });
    if let Err(e) = spawned {
        log::warn!("Source kind: could not start the UIA thread: {e}");
        SOURCE_KIND_BUSY.store(false, Ordering::Release);
        return None;
    }
    Some(receiver)
}

/// `ValuePattern.CurrentIsReadOnly`; `None` when the element has no value pattern.
fn value_read_only(element: &IUIAutomationElement) -> Option<bool> {
    // SAFETY: COM calls on a valid element and pattern.
    unsafe {
        element
            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            .and_then(|pattern| pattern.CurrentIsReadOnly())
            .ok()
            .map(|read_only| read_only.as_bool())
    }
}

/// The `IsReadOnly` text attribute of the element's whole document; `None` when it has no text
/// pattern or the attribute is mixed or unsupported (UIA then returns a sentinel object, not a
/// boolean).
fn text_read_only(element: &IUIAutomationElement) -> Option<bool> {
    // SAFETY: COM calls on a valid element, pattern and range.
    unsafe {
        let pattern = element
            .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            .ok()?;
        let value = pattern
            .DocumentRange()
            .and_then(|range| range.GetAttributeValue(UIA_IsReadOnlyAttributeId))
            .ok()?;
        if value.vt() != VT_BOOL {
            return None;
        }
        bool::try_from(&value).ok()
    }
}

/// One Windows Terminal tab that can be selected.
pub struct Tab {
    element: IUIAutomationElement,
}

impl Tab {
    fn pattern(&self) -> Result<IUIAutomationSelectionItemPattern> {
        // SAFETY: COM call on a valid element.
        unsafe {
            self.element
                .GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(
                    UIA_SelectionItemPatternId,
                )
        }
    }

    /// Whether the tab is the selected one; `None` when it has no selection pattern or the call
    /// failed.
    pub fn is_selected(&self) -> Option<bool> {
        let pattern = self.pattern().ok()?;
        // SAFETY: COM call on a valid pattern.
        unsafe { pattern.CurrentIsSelected() }
            .ok()
            .map(|selected| selected.as_bool())
    }

    /// `SelectionItemPattern.Select()`: makes this the window's selected tab.
    pub fn select(&self) -> Result<()> {
        let pattern = self.pattern()?;
        // SAFETY: COM call on a valid pattern.
        unsafe { pattern.Select() }
    }
}

/// `SelectionItemPattern.CurrentIsSelected`; `false` when the tab has no such pattern.
fn is_selected(element: &IUIAutomationElement) -> bool {
    // SAFETY: COM calls on a valid element and pattern.
    unsafe {
        element
            .GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
            .and_then(|pattern| pattern.CurrentIsSelected())
            .is_ok_and(|selected| selected.as_bool())
    }
}

/// The element's RuntimeId as "a.b.c.d" ("" when it has none).
fn runtime_id(element: &IUIAutomationElement) -> Result<String> {
    // SAFETY: GetRuntimeId returns a 1-D SAFEARRAY of VT_I4 that the caller owns and destroys;
    // the data pointer is only read between Access and Unaccess.
    unsafe {
        let array: *mut SAFEARRAY = element.GetRuntimeId()?;
        if array.is_null() {
            return Ok(String::new());
        }
        let values = (|| -> Result<Vec<i32>> {
            let lower = SafeArrayGetLBound(array, 1)?;
            let upper = SafeArrayGetUBound(array, 1)?;
            let mut data: *mut core::ffi::c_void = core::ptr::null_mut();
            SafeArrayAccessData(array, &mut data)?;
            let len = usize::try_from(upper - lower + 1).unwrap_or(0);
            let values = core::slice::from_raw_parts(data as *const i32, len).to_vec();
            let _ = SafeArrayUnaccessData(array);
            Ok(values)
        })();
        let _ = SafeArrayDestroy(array);
        Ok(values?
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join("."))
    }
}
