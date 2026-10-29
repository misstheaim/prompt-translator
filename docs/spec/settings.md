# Settings

The app's user settings, stored in one `settings.json`. The core loads them at startup and serves them to the webview through commands; the Settings screen (SETTINGS-007 to SETTINGS-010) edits most of them. The app reads them as follows: `lastPopupMode` (the popup's mode fallback), `hotkey` and `autoCopySelection` (`popup.md`), `closeToTray` (APP-027), `autostart` (APP-029), `historyEnabled` (HISTORY-007), `defaultProvider` and `defaultPresetId` (the Translate screen's initial choices, TRANS-013), `ollama.*` and `claude.*` (the providers, `providers.md`) and `devMockProvider` (TRANS-008). There is no per-mode provider default. `layoutFix.*`, `setupCompleted`, `autoCheckUpdates` and `sessionAppendHintShown` are not read or editable yet.

> Code: `src-tauri/src/settings/`, `src/screens/Settings/`, `src/lib/api.ts`, `src/bindings/Settings.ts`

## Behaviour

### SETTINGS-001 Settings fields and defaults
`Settings` (camelCase on the wire) has these fields and defaults:
- `defaultProvider` `ollama`; `defaultPresetId` `technical-development`; `historyEnabled` true; `hotkey` `Ctrl+Shift+Space`; `autostart` false; `closeToTray` true; `setupCompleted` false; `autoCheckUpdates` true; `autoCopySelection` true; `sessionAppendHintShown` false; `lastPopupMode` `correct`; `devMockProvider` false.
- `ollama`: `endpoint` `http://localhost:11434`, `model` `qwen3.5:9b`, `keepAlive` `30m`, `numCtx` 8192, `timeoutSecs` 60.
- `claude`: `executablePath` null (find claude.exe automatically), `model` `haiku`, `timeoutSecs` 90.
- `layoutFix`: `enabled` true, `hotkey` `Ctrl+Alt+Space`, `switchInputLanguage` true.
Since: #40 · Changed: #46

### SETTINGS-002 Loading settings.json
The file is read from the settings location (see APP-006) at startup. A missing file gives the defaults and nothing is written. A field missing from the file (also inside `ollama`, `claude`, `layoutFix`) takes its default; unknown fields are ignored and are not kept on the next save. Loaded values are not validated.
Since: #40

### SETTINGS-003 Corrupt settings.json
If the file is not valid for `Settings` (unparseable JSON, or a field of the wrong type), it is renamed to `settings.json.bak` next to it, the error is logged and the defaults are used. If the rename fails, the file stays, the error is logged and the defaults are used. If the file cannot be read for another reason, the defaults are used and the file is left alone. Loading never fails the startup.
Since: #40

### SETTINGS-004 Validation on save
A save is refused with `InvalidInput` (nothing written, in-memory settings unchanged) when:
- `ollama.endpoint` is not an `http://` or `https://` URL with a host and no whitespace: "The Ollama endpoint must be a URL like http://localhost:11434";
- `ollama.timeoutSecs` is outside 5-600: "The Ollama timeout must be 5 to 600 seconds";
- `claude.timeoutSecs` is outside 5-600: "The Claude timeout must be 5 to 600 seconds";
- `ollama.numCtx` is outside 2048-32768: "The context size must be 2048 to 32768";
- `hotkey` is empty or whitespace: "The hotkey must not be empty";
- `layoutFix.enabled` is true and `layoutFix.hotkey` is empty or whitespace: "The layout fix hotkey must not be empty".
The checks run in this order and the first failure is reported.
Since: #40

### SETTINGS-005 Saving
A valid save creates the settings folder if needed, writes `settings.json` as pretty-printed JSON through a temporary file that is then renamed over it, and replaces the current settings. Concurrent saves are applied one after another. `settings.json` first appears on the first save.
Since: #40

### SETTINGS-006 Settings commands
- `get_settings` returns the current settings.
- `save_settings(settings)` validates and saves (SETTINGS-004/005), emits the event `settings://changed` carrying the saved settings, and returns them. A refused save returns the `InvalidInput` error and emits nothing.
- `set_last_popup_mode(mode)` sets only `lastPopupMode` (`translate`, `improve` or `correct`) through the same save and event, and returns the saved settings.
Since: #40 · Changed: #60

### SETTINGS-007 Settings screen
The Settings tab has three sub-tabs: General, Presets (PRESETS-009) and About. About shows a placeholder, "Coming with the About tab (T33).". General holds the form and stays loaded when the user switches sub-tabs, so unsaved edits survive. Until the settings are loaded it shows "Loading settings...".
Since: #55

### SETTINGS-008 General form
The form edits a draft of the settings in these sections:
- **Ollama** ("The local model. Text generation only."): endpoint, model (free text with the installed models as suggestions), keep alive, context size and timeout (seconds).
- **Claude Code** ("Runs `claude -p` with your normal login."): executable path (empty means automatic), model, timeout (seconds).
- **Translation**: default provider (Ollama (local), Claude Code; Mock (dev) only while it is the saved value) and default preset (a preset id that no longer exists is still listed by its id).
- **History**: "Save translations to history" (`historyEnabled`; existing entries stay when turned off) and a red "Clear all history" button. It asks "Clear all history?" ("This deletes every saved translation and your mistakes log. It can't be undone."); on confirm it calls `history_clear` and shows "History cleared", or an error toast.
- **System**: the global hotkey (SETTINGS-010), "Copy the selected text when the hotkey is pressed" (`autoCopySelection`), "Start with Windows" (`autostart`), "Keep running in the tray when the window is closed" (`closeToTray`).
Since: #55

### SETTINGS-009 Saving, reverting and field errors
While the draft differs from the saved settings the bar at the bottom shows "Unsaved changes" with Revert (back to the saved values) and Save. Save is disabled without changes or with a field error; Enter in a field saves too. Field errors show next to the field before saving: endpoint "Use a URL like http://localhost:11434"; Ollama or Claude timeout "Enter 5 to 600 seconds"; context size "Enter 2048 to 32768"; hotkey "Press a key combination" (the same limits as SETTINGS-004). Save calls `save_settings`; success shows the toast "Settings saved" and re-checks the provider status; a refused save shows its error banner under the form and keeps the draft. When the saved settings change elsewhere, the form takes them unless it has unsaved edits.
Provider status: the Ollama "Test" and Claude "Check" buttons are disabled while there are unsaved changes (the tooltip says "Save your changes first; the test uses the saved settings.") and re-check both providers (PROV-013) with the saved settings. The badges show "Not checked" until a status is known, then Ollama as "Not reachable" (with the hint "Is Ollama running? Check the endpoint." after a test), "Reachable, model not installed" or "Ready (<n> models)", and Claude Code as "Not found", "Found (<version>)" plus "Signed in", "Not signed in" or "Sign-in unknown". A warning banner tells to run `claude` once and sign in (found, not signed in) or that Claude Code wasn't found. "Browse" next to the executable path opens a file picker for `.exe` files (APP-025) and fills the path in the draft.
Since: #55

### SETTINGS-010 Hotkey capture
The hotkey field is read-only: clicking it and pressing a combination records it as modifiers in the order Ctrl, Alt, Shift, Super and then the key, e.g. `Ctrl+Alt+Space`. Keys: letters, digits, function keys (F1...), Space, Enter, Tab, ` - = [ ] \ ; ' , . /, the arrow keys (`Up`, `Down`, `Left`, `Right`), Home, End, PageUp, PageDown, Insert and Delete. A press without a modifier, or with an unsupported key, is ignored; Tab alone still moves focus. Saving applies the new hotkey at once (POPUP-001).
Since: #55

## Known issues

None.

Retired IDs: none
