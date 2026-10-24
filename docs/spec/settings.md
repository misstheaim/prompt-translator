# Settings

The app's user settings, stored in one `settings.json`. The core loads them at startup and serves them to the webview through commands. No screen edits them yet and, apart from `lastPopupMode`, nothing else in the app reads them.

> Code: `src-tauri/src/settings/`, `src/lib/api.ts`, `src/bindings/Settings.ts`

## Behaviour

### SETTINGS-001 Settings fields and defaults
`Settings` (camelCase on the wire) has these fields and defaults:
- `defaultProvider` `ollama`; `defaultPresetId` `technical-development`; `historyEnabled` true; `hotkey` `Ctrl+Shift+Space`; `autostart` false; `closeToTray` true; `setupCompleted` false; `autoCheckUpdates` true; `autoCopySelection` true; `sessionAppendHintShown` false; `lastPopupMode` `correct`; `devMockProvider` false.
- `ollama`: `endpoint` `http://localhost:11434`, `model` `qwen2.5:7b`, `keepAlive` `30m`, `numCtx` 8192, `timeoutSecs` 60.
- `claude`: `executablePath` null (find claude.exe automatically), `model` `haiku`, `timeoutSecs` 90.
- `layoutFix`: `enabled` true, `hotkey` `Ctrl+Alt+Space`, `switchInputLanguage` true.
Since: #40

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

## Known issues

None.

Retired IDs: none
