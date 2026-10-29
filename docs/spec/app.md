# App core

The application shell: what launches, the main window with its tabs, error banner and toasts, the tray, close-to-tray, single instance and autostart, app identity and icons, where data lives, logging, the error payload every command returns, the exported TypeScript contracts, and webview security. The UI is still a shell: the four tabs exist but their screens are placeholders (plus a dev-only mock panel on Translate). Behind it the core holds settings, presets, the history database, mode detection, the technical-token check and a translation service with the Ollama, Claude Code and mock providers (see the other area files), but no real screen uses them yet; the popup and hotkeys are not implemented.

> Code: `src-tauri/src/lib.rs`, `src-tauri/src/error.rs`, `src-tauri/src/system/`, `src-tauri/src/translation/model.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src/App.tsx`, `src/store/app.ts`, `src/components/`, `src/lib/api.ts`, `src/bindings/`, `package.json`

## Behaviour

### APP-001 Main window
The window `main` (title "Prompt Translator", 800x600) is created hidden and shown right after startup, unless the app was started by autostart (APP-029). It has a top bar with four tabs, Translate, History, Mistakes and Settings (Translate selected at start), and below it the selected screen. Every screen is a placeholder: the tab's name as a heading and the text "This screen isn't built yet." In dev builds of the UI (`npm run dev`) the Translate tab also shows the "Mock provider (dev)" panel above the placeholder (TRANS-011).
Since: #34 · Changed: #35, #44, #52, #57

### APP-003 Popup page is a placeholder
A second page (`popup.html`, title "Prompt Translator Popup") exists and shows the text "Popup placeholder". No popup window is created at startup, and nothing opens it yet.
Since: #34

### APP-004 App identity
Product name "Prompt Translator", version 0.1.0, bundle identifier `com.misstheaim.prompttranslator`. Bundle targets: all; bundle icons: 32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico (from `src-tauri/icons/`).
Since: #34 · Changed: #37

### APP-005 Icon set
`src-tauri/icons/` holds the app icon in the sizes above plus Square*Logo/StoreLogo/64x64 PNGs, and `tray.png`, the tray icon (APP-026).
Since: #37 · Changed: #57

### APP-006 Single data root per kind
All app paths come from one place. Settings and presets live under the Tauri app config dir (`settings.json`, `presets/`); history, logs, the Claude working dir and temp files live under the app local data dir (`history.db`, `logs/`, `claude-cwd/`, `tmp/`). The roaming app data dir is never used.
Since: #35

### APP-007 Directories created on first use
Each directory is created (recursively) the first time its path is requested; the `history.db` request creates its parent directory only, not the file. Requesting any path before startup initialisation fails with an `Io` error "app paths are not initialized". The logs directory is created at startup, as are `claude-cwd/` and `tmp/` (PROV-012).
Since: #35 · Changed: #49

### APP-008 Logging
At startup the app writes `prompt-translator.log` into `logs/` (see APP-006), level Info and above. Each file holds at most 1,000,000 bytes; rotation keeps 5 files. Debug builds also log to stdout. The first line logged is "Prompt Translator <version> started".
Since: #35

### APP-009 Startup failure
If path initialisation or logger setup fails, startup is aborted and the error is returned from setup; if the Tauri runtime fails to run, the process panics with "error while running tauri application".
Since: #35

### APP-010 Release builds have no console
In release builds on Windows the app starts without a console window.
Since: #34

### APP-011 Error payload shape
A failed command rejects with JSON `{ kind, message, hint, detail? }`. `kind` is the variant name (PascalCase), `message` a short text, `hint` a UI hint (empty string when none), `detail` is present only for `MalformedOutput` and `ProcessCrashed` and only when their payload is non-empty (raw model text / stderr excerpt). The UI helper `isAppError` treats any object with `kind`, `message` and `hint` as this shape.
Since: #35

### APP-012 Error kinds, messages and hints
| Kind | Message | Hint |
|---|---|---|
| OllamaUnreachable | Can't reach Ollama | Ollama isn't running. |
| ModelMissing | Model "<name>" is not installed | Run `ollama pull <name>` or pick another model. |
| ClaudeNotFound | claude.exe was not found | Set the claude.exe path in Settings. |
| ClaudeAuth | Claude Code is not signed in | Claude Code isn't signed in. Run `claude` once and sign in. |
| Timeout | The request timed out | Retry. |
| Cancelled | The request was cancelled | Retry. |
| EmptyOutput | The model returned an empty result | Retry, or try another provider. |
| MalformedOutput | The model returned an unreadable result | Retry, or try another provider. |
| ProcessCrashed | The provider process crashed | See the error details. |
| SessionClosed | The bound Claude session has ended | The bound Claude session has ended. Pick another session or unbind to copy instead. |
| SessionUnreachable | Can't write to the Claude session | Can't write to this session. Is its terminal running as administrator? |
| InjectionFailed | Sending to the Claude session failed | Sending failed. The prompt was copied to the clipboard instead; paste it manually. |
| PrefixNeedsConfirmation | The text starts with a special character | This starts with `!` / `/` / `#` and will run as a shell command / slash command / memory note in Claude Code. Send anyway? |
| Io | File error: <text> | (empty) |
| Db | Database error: <text> | (empty) |
| InvalidInput | Invalid input: <text> | (empty) |

I/O errors, Tauri errors, JSON errors and SQLite errors (including migration errors) convert to `Io`, `Io`, `InvalidInput` and `Db` respectively. Raised so far: `Io`, `InvalidInput`, `Db` (history commands when the database is unavailable, HISTORY-008), the translation kinds `Timeout`, `Cancelled`, `EmptyOutput` and `MalformedOutput` (TRANS-007/009), and the provider kinds `OllamaUnreachable`, `ModelMissing`, `ClaudeNotFound`, `ClaudeAuth` and `ProcessCrashed` (PROV-003, PROV-009). `SessionClosed`, `SessionUnreachable`, `InjectionFailed` and `PrefixNeedsConfirmation` are defined but no code raises them yet.
Since: #35 · Changed: #40, #41, #62, #46, #50, #63

### APP-013 Exported translation contracts
Rust types are exported to `src/bindings/*.ts` (one file per type, LF line endings). Field names are camelCase on the wire.
- `Mode`: `translate` | `improve` | `correct`.
- `ProviderKind`: `ollama` | `claudeCode` | `mock` (nothing restricts `mock` to debug builds yet).
- `TranslationRequest`: `requestId`, `sourceText`, `mode`, `provider`, `presetId`.
- `TranslationResult`: `historyId` (number or null when history is off), `output`, `issues`, `tokenWarnings`, `provider`, `model`, `durationMs` (number).
- `Issue`: `original`, `corrected`, `category`, `explanation`.
- `IssueCategory` (snake_case, matches the model output): `spelling`, `grammar`, `article`, `tense`, `preposition`, `word_choice`, `vocabulary`, `punctuation`, `style`, `other`.
- `TokenWarning`: `token`, `kind`; `TokenKind` (camelCase): `ticketId`, `identifier`, `fileName`, `path`, `url`, `codeSpan`, `number`.
- `ErrorKind` and `AppErrorPayload` as in APP-011/012.
- `ProviderStatus`: `ollama` (`OllamaStatus`: `reachable`, `models`, `selectedModelInstalled`) and `claude` (`ClaudeStatus`, or null if the check could not run: `found`, `path`, `version`, `signedIn`, the last three null when unknown).
`Mode` is used by `detect_mode` and `set_last_popup_mode`; `TranslationRequest` and `TranslationResult` by `translate` (TRANS-008).
Since: #35 · Changed: #60, #44, #49

### APP-014 Regenerating bindings
`npm run bindings` runs the Rust export tests (`cargo test export_bindings` in `src-tauri`), which write the TypeScript files into `src/bindings/`. Git keeps `src/bindings/*.ts` on LF endings regardless of `core.autocrlf`, so regeneration does not show modified files.
Since: #35 · Changed: #88

### APP-015 UI-to-core access
The UI calls Rust only through typed wrappers in `src/lib/api.ts`; a rejected call always throws an error of the APP-011 shape. Wrappers: `ping()` (returns "pong"; nothing in the UI calls it), `getSettings`, `saveSettings`, `setLastPopupMode`, `listPresets`, `savePreset`, `deletePreset`, `resetPresets`, `detectMode`, `translate`, `cancelTranslation`, `providerStatus`, `copyResult`, `historyList`, `historyGet`, `historyDelete`, `historyClear`, `mistakesSummary`, `pickFile` and `openUrl`. Registered commands: `ping`, `pick_file`, `open_url`, `get_settings`, `save_settings`, `set_last_popup_mode`, `detect_mode`, `translate`, `cancel_translation`, `provider_status`, `copy_result`, `list_presets`, `save_preset`, `delete_preset`, `reset_presets`, `history_list`, `history_get`, `history_delete`, `history_clear`, `mistakes_summary`.
Since: #35 · Changed: #40, #41, #60, #44, #49, #52, #63

### APP-016 Webview security
Release CSP: `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src ipc: http://ipc.localhost` (no remote network, no inline scripts). The dev CSP additionally allows inline scripts and connections to `http://localhost:1420` and `ws://localhost:1420`.
Since: #34

### APP-017 Capabilities
One capability `default` applies to windows `main` and `popup` with only `core:default` and `core:event:default`. No shell, fs, http or sql plugin permissions are granted to the webview.
Since: #34 · Changed: #35

### APP-018 Developer scripts
`npm run dev` (Vite on fixed port 1420, fails if taken), `build` (`tsc && vite build`, producing both `index.html` and `popup.html` pages into `dist/`), `lint` (`eslint .`), `typecheck` (`tsc --noEmit` for the app and the node config), `format` (`prettier --write .`), `bindings` (APP-014), `tauri` (Tauri CLI). `tauri dev` runs `npm run dev` first and `tauri build` runs `npm run build` first.
Since: #34

### APP-019 Continuous integration
A GitHub Actions workflow (`CI`) runs for release builds only, on pull requests into and pushes to `release/**` and `main` (not `develop`), on `windows-latest`: `npm ci`, lint, typecheck, frontend build, `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `npm run bindings`, then fails if `src/bindings` differs from the committed files. A newer run on the same ref cancels the older one. No tests, network or LLM calls. Ticket PRs into `develop` and pushes to `develop` do not run it.
Since: #36 · Changed: #103

### APP-020 Tabs and shortcuts
Clicking a tab selects it (the current one is marked as the current page). Ctrl+Tab selects the next tab and Ctrl+Shift+Tab the previous one, wrapping around in the order Translate, History, Mistakes, Settings; Ctrl+, selects Settings.
Since: #52

### APP-021 UI state loaded at start
When the window opens the UI loads settings, presets and the provider status (PROV-013). Whenever `settings://changed` fires it replaces its settings and re-checks the provider status; a failed status check keeps the previous status. If loading settings fails, an error banner (APP-022) with a Retry action is shown under the tab bar; Retry loads the settings again. If loading presets fails, an error toast shows the error's message.
Since: #52

### APP-022 Error banner
An error is shown as a banner with the message in bold, the hint below it when not empty, and a collapsed "Details" section when the error has a `detail`. By kind:
- red: OllamaUnreachable, ModelMissing, ClaudeNotFound, ClaudeAuth, Timeout, EmptyOutput, MalformedOutput, ProcessCrashed, Io, Db, InvalidInput; blue: Cancelled; yellow: SessionClosed, SessionUnreachable, InjectionFailed, PrefixNeedsConfirmation. An unknown kind looks like Io.
- "Retry" appears (when the screen gives a retry action) for OllamaUnreachable, ModelMissing, ClaudeNotFound, ClaudeAuth, Timeout, Cancelled, EmptyOutput, MalformedOutput and ProcessCrashed.
- "Open settings" (switches to the Settings tab) appears for OllamaUnreachable, ModelMissing and ClaudeNotFound; ClaudeNotFound also has "Install help", which opens the Claude Code install docs (APP-024).
- If a link fails to open, an error toast shows the message.
Since: #52

### APP-023 Toasts
A toast is a short message at the bottom centre of the window with a "×" button to dismiss it. At most 3 show at once (a newer one pushes out the oldest). Info and success toasts disappear after 3 s, error toasts after 6 s.
Since: #52

### APP-024 Opening links
`open_url(url)` (wrapper `openUrl`) opens a link in the default browser, only if it equals or continues (with `/`, `?` or `#`) one of: `https://ollama.com/download`, `https://docs.claude.com/en/docs/claude-code/setup`, `https://github.com/misstheaim/prompt-translator/releases`. Any other URL fails with `InvalidInput` "This link can't be opened from the app"; a failure to launch the browser fails with `Io` "Can't open the link: ...". The webview itself has no opener permission.
Since: #52

### APP-025 File picker
`pick_file(filter)` (wrapper `pickFile`) opens the native file dialog and returns the chosen path, or null if the user cancels. `filter` is a list of extensions without dots (e.g. `["exe", "cmd"]`, shown as "Files"); null or empty allows any file. A dialog failure fails with `Io`. Nothing in the UI calls it yet.
Since: #49

### APP-026 Tray icon
The app puts an icon in the notification area (monochrome `tray.png`, tooltip "Prompt Translator"). A left click shows the main window: it is shown, restored if minimized and focused. A right click opens a menu with "Open" (same as left click) and "Quit" (exits the app).
Since: #57

### APP-027 Close to tray
Closing the main window never destroys it. When `closeToTray` is on (default) the window is hidden and the app keeps running in the tray; when it is off the app exits. The setting is read at the moment of closing.
Since: #57

### APP-028 Single instance
Starting the app while it is already running does not open a second copy: the running one shows and focuses its main window, and the new process exits.
Since: #57

### APP-029 Autostart
The setting `autostart` is applied at startup and after every saved settings change. On: a value `Prompt Translator` is added under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` with the data `<exe path> --autostart`. Off: that value is removed. Nothing is changed when the entry already matches; a failure is only logged. A process started with `--autostart` keeps the main window hidden (the app sits in the tray); a normal start shows it.
Since: #57

## Known issues

None.

Retired IDs: APP-002
