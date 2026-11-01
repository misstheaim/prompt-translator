# Hotkey and popup

The global hotkey that captures text from any app and opens the popup window, with the auto-copy of the current selection. The popup page translates the captured text right away in the detected mode, shows the result and replaces the selection with it (Enter) or copies it (Ctrl+Enter). The layout-fix hotkey is registered by the same code (see `layout-fix.md`).

> Code: `src-tauri/src/system/hotkey.rs`, `src-tauri/src/system/popup.rs`, `src-tauri/src/system/clipboard.rs`, `src-tauri/src/system/input.rs`, `src-tauri/capabilities/popup.json`, `src/popup/`, `src/App.tsx`, `src/store/app.ts`

## Behaviour

### POPUP-001 Hotkey registration
The global hotkey is `settings.hotkey` (default `Ctrl+Shift+Space`), written as modifiers in the order Ctrl, Alt, Shift, Super and then one key (`Ctrl+Alt+Space`). It is registered at startup, after the settings are loaded, and again whenever `settings://changed` fires with a different value (the old one is unregistered first), so a new hotkey works without a restart. The value is trimmed; saving settings with an unchanged hotkey does nothing. The layout-fix hotkey is registered next to it (LAYOUT-003); a settings change touches only the hotkey that changed.
Since: #58 · Changed: #78

### POPUP-002 Registration failure
If the hotkey cannot be registered (not a valid hotkey string, or taken by another app), no hotkey is active (the popup is off until a working one is saved) and the error text is kept: `"<hotkey>" isn't a valid hotkey (<reason>).` or `Could not register "<hotkey>": <reason>.`, followed by "The popup hotkey is off. Choose another combination in Settings." If the layout-fix hotkey fails too, its message (same form, "The layout fix hotkey is off.") is appended after a space, so the one text holds both. The text is sent to the main window as the event `hotkey://error` (the text, or null once registration works) and is returned by the command `hotkey_error` (wrapper `hotkeyError`), which the window asks once when it opens, because a startup failure happens before it listens. While an error is set, the main window shows a warning banner with that text and an "Open settings" button under the tab bar; a successful registration of both clears it. A failure never stops the app from starting.
Since: #58 · Changed: #78

### POPUP-003 Pressing the hotkey
On key press (not release) the hotkey captures the source text and classifies where it was selected (POPUP-007, POPUP-012), picks the mode, shows the popup (POPUP-004) and sends it the event `popup://source` with `{ text, mode, sourceKind, languagePair? }`. `text` is the selection when auto-copy produced one, else the clipboard text; empty when the clipboard is empty or holds no text. The mode is chosen by the source kind:
- **read-only:** always Translate, into the opposite language of the text. The first real word decides (technical tokens, numbers and symbols are skipped): for a pair with one Latin-script language its script decides; for other pairs language detection on the first 5 words, limited to the two languages. Text that starts in the target language is translated into the native language: the event then carries `languagePair` `{ nativeLanguage, targetLanguage }` set to the settings' target and native codes. Anything else (native language, no words, undetectable) is translated into the target language with the settings' pair and no `languagePair`. This translation is for reading; it never goes to a Claude session.
- **editable or unknown:** the setting `popupStartMode` (SETTINGS-001): `auto` uses mode detection (TRANS-003) with `lastPopupMode` as the fallback for short text; `translate`, `improve` and `correct` are fixed; `lastUsed` is `lastPopupMode`.
Capturing can take up to about a second and does not freeze the app.
Since: #58 · Changed: #59, #152

### POPUP-004 Popup window
The window `popup` (title "Prompt Translator Popup", 520x360, not resizable, frameless, always on top, not in the taskbar, not minimizable or maximizable) is created hidden at startup from `popup.html`. It has no header or drag area, so it cannot be moved or resized by the mouse. Every show centres it in the work area of the monitor under the cursor (the primary monitor if that can't be found), then shows and focuses it. It hides when it loses focus, and closing it only hides it. `popup://source` is sent after it is shown.
Since: #58 · Changed: #145, #162

### POPUP-005 Popup page
Until it receives `popup://source` the page shows "Waiting for text...". After it, an empty or blank text shows "Select or copy some text first"; otherwise it shows, top to bottom:
- the source text, cut to 3 lines; with a "Show more" / "Show less" toggle when the text is over 180 characters or 3 lines;
- a mode switch (Translate / Improve / Correct) set to the mode from the event; when the event has a `languagePair` and the mode is Translate, a small label "<from> → <to>" (English names, e.g. "English → Russian", tooltip "Translation direction") beside it. Switching to Improve or Correct hides it and uses the settings' pair; switching back to Translate shows it again;
- the result area (POPUP-009);
- a footer "Enter: replace · Ctrl+Enter: copy · Esc: close · Ctrl+R: retry · Tab: mode" when the source kind is editable (POPUP-012), otherwise "Enter: copy · Esc: close · Ctrl+R: retry · Tab: mode", and an "Open in main window" button (POPUP-006).
Each new `popup://source` resets the page (cancelling a running request) and starts a run for a non-blank text. With a blank text the footer hint shows but there is no "Open in main window" button. Before the first source (and for an unknown or read-only source) the footer shows the copy form.
Since: #34 · Changed: #58, #61, #152, #153

### POPUP-006 Opening an entry in the main window
When the event `main://open-entry` with `{ input, mode, result }` (`result` a translation result or null) is emitted, the core shows and focuses the main window and hides the popup, and the main window selects the Translate tab and its default tab (never a bound tab, SESSION-007) and loads it: the input, the mode, the provider of the result (none, so the settings default, when `result` is null), and, when a result is given, that result as a finished translation with its output in the editable output box. The popup's "Open in main window" button sends it with the captured text, the current mode and the finished result (null while loading, after an error or when nothing ran yet); the popup does not cancel a request still running.
Since: #58 · Changed: #61, #75

### POPUP-007 Auto-copy of the selection
When `autoCopySelection` is on (default), pressing the hotkey first tries to copy what is selected in the foreground app, so no manual Ctrl+C is needed:
1. If the foreground window is a terminal, nothing is sent (Ctrl+C would interrupt the running command). A terminal is a window of class `CASCADIA_HOSTING_WINDOW_CLASS` (Windows Terminal) or `ConsoleWindowClass` (conhost), or of a process named `WindowsTerminal.exe`, `wezterm-gui.exe`, `alacritty.exe` or `mintty.exe` (names compared ignoring case). Terminals embedded in IDEs can't be told apart from the editor, so Ctrl+C is sent there.
2. It waits up to 500 ms for every modifier key (Shift, Ctrl, Alt, Win) to be released; if one is still down, nothing is sent (Ctrl+Shift+C would open DevTools in Chromium apps).
3. It saves the clipboard text, and gives up unless the same window is still in front and no modifier is down. It sends Ctrl+C and waits up to 300 ms for the clipboard to change.
4. The new clipboard text is the captured source, and the saved text is put back on the clipboard. The restore is marked (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory` = 0) so it does not appear in Windows clipboard history. If the copy is VS Code's (or another Monaco editor's) whole-line copy, made by the editor because nothing was selected (recognised by the `"isFromEmptySelection":true` marker in the copied data), it counts as no selection: the saved text is put back and POPUP-008 applies. Other editors that copy the whole line without that marker (JetBrains IDEs, Sublime) are not recognised.
Only one capture runs at a time, and not while a layout fix is using the clipboard (LAYOUT-004). When `autoCopySelection` is off, the clipboard is used as it is.
Since: #59 · Changed: #78, #128

### POPUP-008 Fallback to the clipboard
Whenever auto-copy gives nothing (a terminal, modifiers held, the clipboard busy, the input blocked by Windows, no selection so the clipboard did not change, a whole-line copy from VS Code or another Monaco editor (POPUP-007), or an empty copy), the source is the text that was on the clipboard before, left untouched. This is not an error.
Since: #59 · Changed: #128

### POPUP-009 Running and showing the result
For a non-blank text the page runs `translate` (TRANS-008) at once with a new request id, the starting mode, and the provider and preset from the current settings (`defaultProvider`, `defaultPresetId`); a Translate run of a reverse translation (POPUP-003) also sends its `languagePair` as `nativeLanguage` and `targetLanguage`. While it runs the area shows a spinner and "Working... Esc to cancel".
- Success: the output text (not editable), then the token-warning banner (Ignore hides it for this result, Retry runs again; TRANS-013) and, when the result has issues, a toggle "<n> fix" / "<n> fixes" that expands the issue list (APP-031, collapsed again on every new run).
- Failure: an error banner (APP-022) with Retry but without "Open settings", and "Start Ollama" for `OllamaUnreachable` (SETUP-002). `Cancelled` is not shown as an error; the page returns to idle.
- Changing the mode (switch or Tab) with a non-blank text saves it as `lastPopupMode`, except for a read-only source (POPUP-012), which never saves it (also not on Enter, POPUP-010). Each mode's finished result is kept for the current capture: switching to a mode that has one shows it without running (a run in progress is cancelled); switching to a mode without one runs it. A new capture clears the kept results; Ctrl+R reruns the current mode and replaces its result. A new run cancels the one in progress (its result is not kept), and an answer to a superseded or cancelled request is ignored.
Since: #61 · Changed: #145, #152

### POPUP-010 Popup keys
- Enter (without Shift or Alt) after a successful run accepts the output. Plain Enter replaces the selection in the source app when the source kind is editable (POPUP-013) and otherwise copies the output with its `historyId` (TRANS-010). Ctrl+Enter always copies. Both mark the history entry accepted, save the mode as `lastPopupMode` (not for a read-only source) and hide the popup. If the copy (or, for a replace, the fallback copy) fails the popup stays open with the error banner; Enter does nothing in other states.
- Esc cancels a running request and hides the popup.
- Ctrl+R runs the same text and mode again (also while one runs); it does not reload the page.
- Tab and Shift+Tab move the mode forward and back through Translate, Improve, Correct, wrapping around.
Since: #61 · Changed: #152, #153

### POPUP-012 Source kind
When `autoCopySelection` is on and the foreground window isn't a terminal (POPUP-007), pressing the hotkey asks UI Automation, before Ctrl+C is sent, for the focused element and classifies the capture:
- **editable:** the element has a value pattern that isn't read-only, or a text pattern whose document isn't read-only, or (with neither) is an Edit control;
- **read-only:** a read-only value or text pattern, or (with neither) a Document, Text or Hyperlink control (a web page or article);
- **unknown:** anything else, a failed call, no answer within about 100 ms of the hotkey (the capture never waits longer for it), an earlier query still stuck on a hung app, a terminal, `autoCopySelection` off, or a clipboard fallback (POPUP-008), which is always unknown.
The kind is sent as `sourceKind` (`editable`, `readOnly`, `unknown`) in `popup://source` (POPUP-003). Unknown is treated like editable.
Since: #152

### POPUP-013 Replace the selection
Plain Enter (POPUP-010) replaces the selection only when the source kind is editable (an unknown, read-only or clipboard-fallback source copies instead) and the window the text was selected in is recorded from the hotkey press. The command `replace_selection(text, historyId?)` (wrapper `replaceSelection`) hides the popup, brings that window to the front (restoring it if minimised; up to 1 s), waits for Enter and the modifier keys to be released (up to 500 ms), puts the result on the clipboard (hidden from clipboard history) and sends Ctrl+V. After 200 ms the previous clipboard text is put back, unless the clipboard changed meanwhile. Blank text fails with `InvalidInput` "There is nothing to paste".
- **Fallback:** if the window is gone, hidden or does not come to the front, a modifier stays down, the input is blocked or the clipboard cannot be used, nothing is pasted: the result is copied and the notification "Couldn't paste: the result was copied instead" shows (title: the app name).
- If even that copy fails the command fails with `Io` "Can't write to the clipboard: ...", the popup is shown again with the error banner and the history entry is not marked.
- Otherwise (pasted or copied) the entry with `historyId` is marked accepted with target `clipboard` (HISTORY-009).
Since: #153

Retired IDs: POPUP-011
