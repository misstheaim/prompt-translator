# Hotkey and popup

The global hotkey that captures text from any app and opens the popup window, with the auto-copy of the current selection. Today the popup page is a placeholder that only shows what was captured: it does not translate, and nothing sends it to the main window yet.

> Code: `src-tauri/src/system/hotkey.rs`, `src-tauri/src/system/popup.rs`, `src-tauri/src/system/clipboard.rs`, `src-tauri/capabilities/popup.json`, `src/popup/main.tsx`, `src/App.tsx`, `src/store/app.ts`

## Behaviour

### POPUP-001 Hotkey registration
The global hotkey is `settings.hotkey` (default `Ctrl+Shift+Space`), written as modifiers in the order Ctrl, Alt, Shift, Super and then one key (`Ctrl+Alt+Space`). It is registered at startup, after the settings are loaded, and again whenever `settings://changed` fires with a different value (the old one is unregistered first), so a new hotkey works without a restart. The value is trimmed; saving settings with an unchanged hotkey does nothing. Only the popup hotkey is registered: the layout-fix hotkey (`layoutFix.hotkey`) is not.
Since: #58

### POPUP-002 Registration failure
If the hotkey cannot be registered (not a valid hotkey string, or taken by another app), no hotkey is active (the popup is off until a working one is saved) and the error text is kept: `"<hotkey>" isn't a valid hotkey (<reason>).` or `Could not register "<hotkey>": <reason>.`, followed by "The popup hotkey is off. Choose another combination in Settings." The text is sent to the main window as the event `hotkey://error` (the text, or null once registration works) and is returned by the command `hotkey_error` (wrapper `hotkeyError`), which the window asks once when it opens, because a startup failure happens before it listens. While an error is set, the main window shows a warning banner with that text and an "Open settings" button under the tab bar; a successful registration clears it. A failure never stops the app from starting.
Since: #58

### POPUP-003 Pressing the hotkey
On key press (not release) the hotkey captures the source text (POPUP-007), picks the mode with mode detection (TRANS-003) using `lastPopupMode` as the fallback, shows the popup (POPUP-004) and sends it the event `popup://source` with `{ text, mode }`. `text` is the selection when auto-copy produced one, else the clipboard text; empty when the clipboard is empty or holds no text. Capturing can take up to about a second and does not freeze the app.
Since: #58 · Changed: #59

### POPUP-004 Popup window
The window `popup` (title "Prompt Translator Popup", 520x360, not resizable, frameless, always on top, not in the taskbar, not minimizable or maximizable) is created hidden at startup from `popup.html`. Showing it centres it in the work area of the monitor under the cursor (the primary monitor if that can't be found), shows and focuses it. It hides when it loses focus, and closing it only hides it. `popup://source` is sent after it is shown.
Since: #58

### POPUP-005 Popup page
Until it receives `popup://source` the page shows "Popup placeholder". After it, an empty or blank text shows "Select or copy some text first"; otherwise the page shows "Mode: <mode>" and the captured text. Esc hides the popup. It has no translate, accept or other controls yet.
Since: #34 · Changed: #58

### POPUP-006 Opening an entry in the main window
When the event `main://open-entry` with `{ input, mode, result }` (`result` a translation result or null) is emitted, the core shows and focuses the main window and hides the popup, and the main window selects the Translate tab and loads it: the input, the mode, the provider of the result (none, so the settings default, when `result` is null), and, when a result is given, that result as a finished translation with its output in the editable output box. Nothing sends this event yet.
Since: #58

### POPUP-007 Auto-copy of the selection
When `autoCopySelection` is on (default), pressing the hotkey first tries to copy what is selected in the foreground app, so no manual Ctrl+C is needed:
1. If the foreground window is a terminal, nothing is sent (Ctrl+C would interrupt the running command). A terminal is a window of class `CASCADIA_HOSTING_WINDOW_CLASS` (Windows Terminal) or `ConsoleWindowClass` (conhost), or of a process named `WindowsTerminal.exe`, `wezterm-gui.exe`, `alacritty.exe` or `mintty.exe` (names compared ignoring case). Terminals embedded in IDEs can't be told apart from the editor, so Ctrl+C is sent there.
2. It waits up to 500 ms for every modifier key (Shift, Ctrl, Alt, Win) to be released; if one is still down, nothing is sent (Ctrl+Shift+C would open DevTools in Chromium apps).
3. It saves the clipboard text, and gives up unless the same window is still in front and no modifier is down. It sends Ctrl+C and waits up to 300 ms for the clipboard to change.
4. The new clipboard text is the captured source, and the saved text is put back on the clipboard. The restore is marked (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory` = 0) so it does not appear in Windows clipboard history.
Only one capture runs at a time. When `autoCopySelection` is off, the clipboard is used as it is.
Since: #59

### POPUP-008 Fallback to the clipboard
Whenever auto-copy gives nothing (a terminal, modifiers held, the clipboard busy, the input blocked by Windows, no selection so the clipboard did not change, or an empty copy), the source is the text currently on the clipboard, left untouched. This is not an error.
Since: #59

## Known issues

None.

Retired IDs: none
