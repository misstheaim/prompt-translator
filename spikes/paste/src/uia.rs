//! UI Automation: Windows Terminal tabs (`TabItem` elements) with their RuntimeId and
//! `SelectionItemPattern`.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, SAFEARRAY,
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
use windows::core::Result;

pub struct Uia {
    automation: IUIAutomation,
}

pub struct Tab {
    element: IUIAutomationElement,
    pub name: String,
    /// The RuntimeId as dot-separated decimal ints (what `--tab` takes).
    pub runtime_id: String,
}

impl Uia {
    /// Initialises COM (multithreaded, as UIA clients should) and creates the automation object.
    pub fn new() -> Result<Self> {
        // SAFETY: plain call; S_FALSE (already initialised) is fine.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        // SAFETY: plain COM activation.
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
        Ok(Self { automation })
    }

    /// Every `TabItem` under the window, in tree order.
    pub fn tabs(&self, hwnd: HWND) -> Result<Vec<Tab>> {
        // SAFETY: COM calls on valid interfaces; a dead HWND returns an error.
        unsafe {
            let root = self.automation.ElementFromHandle(hwnd)?;
            let condition = self.automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &VARIANT::from(UIA_TabItemControlTypeId.0),
            )?;
            let found = root.FindAll(TreeScope_Descendants, &condition)?;
            let count = found.Length()?;
            let mut tabs = Vec::new();
            for index in 0..count {
                let element = found.GetElement(index)?;
                let name = element
                    .CurrentName()
                    .map(|b| b.to_string())
                    .unwrap_or_default();
                let runtime_id = runtime_id(&element).unwrap_or_default();
                tabs.push(Tab {
                    element,
                    name,
                    runtime_id,
                });
            }
            Ok(tabs)
        }
    }
}

impl Tab {
    fn selection_item(&self) -> Result<IUIAutomationSelectionItemPattern> {
        // SAFETY: COM call on a valid element.
        unsafe {
            self.element
                .GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(
                    UIA_SelectionItemPatternId,
                )
        }
    }

    /// `None` when the tab has no SelectionItemPattern (or the call failed).
    pub fn is_selected(&self) -> Option<bool> {
        let pattern = self.selection_item().ok()?;
        // SAFETY: COM call on a valid pattern.
        unsafe { pattern.CurrentIsSelected() }
            .ok()
            .map(|b| b.as_bool())
    }

    /// `SelectionItemPattern.Select()`.
    pub fn select(&self) -> Result<()> {
        let pattern = self.selection_item()?;
        // SAFETY: COM call on a valid pattern.
        unsafe { pattern.Select() }
    }
}

/// The element's RuntimeId as "a.b.c.d".
fn runtime_id(element: &IUIAutomationElement) -> Result<String> {
    // SAFETY: GetRuntimeId returns a 1-D SAFEARRAY of VT_I4 that the caller owns and destroys.
    unsafe {
        let array: *mut SAFEARRAY = element.GetRuntimeId()?;
        if array.is_null() {
            return Ok(String::new());
        }
        let lower = SafeArrayGetLBound(array, 1)?;
        let upper = SafeArrayGetUBound(array, 1)?;
        let mut data: *mut core::ffi::c_void = core::ptr::null_mut();
        SafeArrayAccessData(array, &mut data)?;
        let len = usize::try_from(upper - lower + 1).unwrap_or(0);
        let values = core::slice::from_raw_parts(data as *const i32, len).to_vec();
        let _ = SafeArrayUnaccessData(array);
        let _ = SafeArrayDestroy(array);
        Ok(values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join("."))
    }
}
