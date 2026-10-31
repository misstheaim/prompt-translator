//! `system::uia`: UI Automation reads of Windows Terminal's tabs (`TabItem` elements: name,
//! RuntimeId, selected), as spike T58 found them (`docs/spikes/session-paste.md`). Read-only:
//! nothing here focuses, selects or types.
//!
//! COM is initialised (multithreaded, as UIA clients should) for as long as a [`Uia`] lives, so
//! create it on a worker thread, never on the UI thread, and drop it on the same thread.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    SAFEARRAY,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationSelectionItemPattern,
    TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_SelectionItemPatternId,
    UIA_TabItemControlTypeId,
};

pub use windows::core::Result;

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

    /// Every `TabItem` under the top-level window `hwnd` (see `input::Window::id`), in tree
    /// order. An error when the window is gone or doesn't answer UIA.
    pub fn tabs(&self, hwnd: usize) -> Result<Vec<TabInfo>> {
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        // SAFETY: COM calls on valid interfaces; a dead HWND makes ElementFromHandle fail.
        unsafe {
            let root = self.automation.ElementFromHandle(hwnd)?;
            let condition = self.automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &VARIANT::from(UIA_TabItemControlTypeId.0),
            )?;
            let found = root.FindAll(TreeScope_Descendants, &condition)?;
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
