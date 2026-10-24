# App core

The application shell: what launches, the windows and their content, app identity and icons, where data lives, logging, the error payload every command returns, the exported TypeScript contracts, and webview security. The UI is still a scaffold: one placeholder window (plus a dev-only mock panel). Behind it the core already holds settings, presets, the history database, mode detection, the technical-token check and a translation service with a mock provider (see the other area files), but no real screen uses them yet; real providers, tray, popup and hotkeys are not implemented.

> Code: `src-tauri/src/lib.rs`, `src-tauri/src/error.rs`, `src-tauri/src/system/`, `src-tauri/src/translation/model.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src/lib/api.ts`, `src/bindings/`, `package.json`

## Behaviour

### APP-001 Main window
On launch one window opens: label `main`, title "Prompt Translator", 800x600. It shows a centred card with the heading "Prompt Translator", the text "Placeholder window" and a line "Rust core: <result>". In dev builds of the UI a "Mock provider (dev)" panel is shown below the card (TRANS-011).
Since: #34 · Changed: #35, #44

### APP-002 Core round-trip check
On start the main window calls the `ping` command; the line shows "..." until it answers, then "Rust core: pong". If the call fails with an app error, the line shows that error's message; with any other failure it shows "Rust core: ping failed".
Since: #35

### APP-003 Popup page is a placeholder
A second page (`popup.html`, title "Prompt Translator Popup") exists and shows the text "Popup placeholder". No popup window is created at startup, and nothing opens it yet.
Since: #34

### APP-004 App identity
Product name "Prompt Translator", version 0.1.0, bundle identifier `com.misstheaim.prompttranslator`. Bundle targets: all; bundle icons: 32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico (from `src-tauri/icons/`).
Since: #34 · Changed: #37

### APP-005 Icon set
`src-tauri/icons/` holds the app icon in the sizes above plus Square*Logo/StoreLogo/64x64 PNGs, and `tray.png` for the future tray icon. Nothing loads `tray.png` yet: the app creates no tray icon.
Since: #37

### APP-006 Single data root per kind
All app paths come from one place. Settings and presets live under the Tauri app config dir (`settings.json`, `presets/`); history, logs, the Claude working dir and temp files live under the app local data dir (`history.db`, `logs/`, `claude-cwd/`, `tmp/`). The roaming app data dir is never used.
Since: #35

### APP-007 Directories created on first use
Each directory is created (recursively) the first time its path is requested; the `history.db` request creates its parent directory only, not the file. Requesting any path before startup initialisation fails with an `Io` error "app paths are not initialized". The logs directory is created at startup.
Since: #35

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

I/O errors, Tauri errors, JSON errors and SQLite errors (including migration errors) convert to `Io`, `Io`, `InvalidInput` and `Db` respectively. Raised so far: `Io` (paths, settings and preset files), `InvalidInput` (settings and preset validation, unknown preset) and `Db` (opening the history database, which is only logged, see HISTORY-001); the other kinds are defined but no code raises them yet.
Since: #35 · Changed: #40, #41, #62

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
`Mode` is used by `detect_mode` and `set_last_popup_mode`; `TranslationRequest` and `TranslationResult` by `translate` (TRANS-008).
Since: #35 · Changed: #60, #44

### APP-014 Regenerating bindings
`npm run bindings` runs the Rust export tests (`cargo test export_bindings` in `src-tauri`), which write the TypeScript files into `src/bindings/`. Git keeps `src/bindings/*.ts` on LF endings regardless of `core.autocrlf`, so regeneration does not show modified files.
Since: #35 · Changed: #88

### APP-015 UI-to-core access
The UI calls Rust only through typed wrappers in `src/lib/api.ts`: `ping()` (returns "pong"), `getSettings`, `saveSettings`, `setLastPopupMode`, `detectMode`, `translate`, `cancelTranslation` and `copyResult`. Registered commands: `ping`, `get_settings`, `save_settings`, `set_last_popup_mode`, `detect_mode`, `translate`, `cancel_translation`, `copy_result`, `list_presets`, `save_preset`, `delete_preset`, `reset_presets`. The four preset commands have no wrapper in `api.ts` yet.
Since: #35 · Changed: #40, #41, #60, #44

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
A GitHub Actions workflow (`CI`) runs on pull requests into and pushes to `develop`, `release/**` and `main`, on `windows-latest`: `npm ci`, lint, typecheck, frontend build, `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `npm run bindings`, then fails if `src/bindings` differs from the committed files. A newer run on the same ref cancels the older one. No tests, network or LLM calls.
Since: #36

## Known issues

None.

Retired IDs: none
