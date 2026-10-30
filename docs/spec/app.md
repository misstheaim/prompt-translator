# App core

The application shell: what launches, the main window with its tabs, error banner and toasts, the tray, close-to-tray, single instance and autostart, app identity and icons, where data lives, logging, the error payload every command returns, the exported TypeScript contracts, and webview security. The four tabs show real screens: Translate (`translation.md`), History and Mistakes (`history.md`) and Settings (`settings.md`, `presets.md`); the Settings About tab shows the version, folders, update check and "Run setup again" (SETTINGS-011), an update banner can appear under the tab bar (APP-035), the first-run setup wizard (`setup.md`) replaces the tabs until setup is completed, and in dev builds a mock panel sits above Translate. Behind them the core holds settings, presets, the history database, mode detection, the technical-token check and a translation service with the Ollama, Claude Code and mock providers (see the other area files). The global hotkeys and the popup window with its translate page are in `popup.md` and `layout-fix.md`; the commands for checking the machine and setting up Ollama are in `setup.md`.

> Code: `src-tauri/src/lib.rs`, `src-tauri/src/error.rs`, `src-tauri/src/system/`, `src-tauri/src/translation/model.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src/App.tsx`, `src/store/app.ts`, `src/components/`, `src/lib/api.ts`, `src/bindings/`, `package.json`

## Behaviour

### APP-001 Main window
The window `main` (title "Prompt Translator", 800x600) is created hidden and shown right after startup, unless the app was started by autostart (APP-029). It has a top bar with four tabs, Translate, History, Mistakes and Settings (Translate selected at start), and below it the selected screen: Translate (TRANS-013), History (HISTORY-010), Mistakes (HISTORY-011) or Settings (SETTINGS-007). While the hotkey registration has failed, a warning banner is shown under the tab bar (POPUP-002). When a newer release is known, the update banner shows there too (APP-035). In dev builds of the UI (`npm run dev`) the Translate tab also shows the "Mock provider (dev)" panel above the screen (TRANS-011). While `setupCompleted` is false the first-run setup wizard replaces this whole UI (SETUP-008), and after the user leaves it a "Resume setup" banner is shown under the tab bar (SETUP-009).
Since: #34 · Changed: #35, #44, #52, #57, #53, #55, #58, #64, #65, #71, #68

### APP-003 Popup page
A second page (`popup.html`, title "Prompt Translator Popup") is the content of the hidden `popup` window created at startup; the hotkey opens it (`popup.md`). The page translates the captured text (POPUP-005, POPUP-009).
Since: #34 · Changed: #58, #61

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
The UI calls Rust only through typed wrappers in `src/lib/api.ts`; a rejected call always throws an error of the APP-011 shape. Wrappers: `ping()` (returns "pong"; nothing in the UI calls it), `getSettings`, `saveSettings`, `setLastPopupMode`, `listPresets`, `savePreset`, `deletePreset`, `resetPresets`, `detectMode`, `translate`, `cancelTranslation`, `providerStatus`, `copyResult`, `historyList`, `historyGet`, `historyDelete`, `historyClear`, `mistakesSummary`, `hotkeyError`, `pickFile`, `openUrl` and the setup wrappers of `setup.md` (`environmentCheck`, `ollamaInstall`, `ollamaStart`, `ollamaPull`, `ollamaBenchmark`, `cancelSetupTask`, `completeSetup`, `onSetupProgress`). Registered commands: `ping`, `hotkey_error`, `pick_file`, `open_url`, `get_settings`, `save_settings`, `set_last_popup_mode`, `detect_mode`, `translate`, `cancel_translation`, `provider_status`, `copy_result`, `list_presets`, `save_preset`, `delete_preset`, `reset_presets`, `history_list`, `history_get`, `history_delete`, `history_clear`, `mistakes_summary`, `environment_check`, `ollama_install`, `ollama_start`, `ollama_pull`, `ollama_benchmark`, `cancel_setup_task`, `complete_setup`.
Since: #35 · Changed: #40, #41, #60, #44, #49, #52, #63, #58, #69, #70

### APP-016 Webview security
Release CSP: `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src ipc: http://ipc.localhost` (no remote network, no inline scripts). The dev CSP additionally allows inline scripts and connections to `http://localhost:1420` and `ws://localhost:1420`.
Since: #34

### APP-017 Capabilities
The capability `default` applies to windows `main` and `popup` with only `core:default` and `core:event:default`. The capability `popup` adds, for the window `popup` only, `core:window:allow-hide`, `allow-show` and `allow-set-focus`. No shell, fs, http, sql, clipboard, dialog or global-shortcut plugin permissions are granted to the webview.
Since: #34 · Changed: #35, #58

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
When the window opens the UI loads settings, presets, the provider status (PROV-013) and the hotkey error (POPUP-002), and listens to `hotkey://error`. Whenever `settings://changed` fires it replaces its settings and re-checks the provider status; a failed status check keeps the previous status. If loading settings fails, an error banner (APP-022) with a Retry action is shown under the tab bar; Retry loads the settings again. If loading presets fails, an error toast shows the error's message.
Since: #52 · Changed: #58

### APP-022 Error banner
An error is shown as a banner with the message in bold, the hint below it when not empty, and a collapsed "Details" section when the error has a `detail`. By kind:
- red: OllamaUnreachable, ModelMissing, ClaudeNotFound, ClaudeAuth, Timeout, EmptyOutput, MalformedOutput, ProcessCrashed, Io, Db, InvalidInput; blue: Cancelled; yellow: SessionClosed, SessionUnreachable, InjectionFailed, PrefixNeedsConfirmation. An unknown kind looks like Io.
- "Retry" appears (when the screen gives a retry action) for OllamaUnreachable, ModelMissing, ClaudeNotFound, ClaudeAuth, Timeout, Cancelled, EmptyOutput, MalformedOutput and ProcessCrashed.
- "Open settings" (switches to the Settings tab) appears for OllamaUnreachable, ModelMissing and ClaudeNotFound, except in the popup, which has no Settings screen; ClaudeNotFound also has "Install help", which opens the Claude Code install docs (APP-024).
- OllamaUnreachable also has "Start Ollama" (SETUP-002).
- If a link fails to open, an error toast shows the message.
Since: #52 · Changed: #61, #70

### APP-023 Toasts
A toast is a short message at the bottom centre of the window with a "×" button to dismiss it. At most 3 show at once (a newer one pushes out the oldest). Info and success toasts disappear after 3 s, error toasts after 6 s.
Since: #52

### APP-024 Opening links
`open_url(url)` (wrapper `openUrl`) opens a link in the default browser, only if it equals or continues (with `/`, `?` or `#`) one of: `https://ollama.com/download`, `https://docs.claude.com/en/docs/claude-code/setup`, `https://github.com/misstheaim/prompt-translator/releases`. Any other URL fails with `InvalidInput` "This link can't be opened from the app"; a failure to launch the browser fails with `Io` "Can't open the link: ...". The webview itself has no opener permission.
Since: #52

### APP-025 File picker
`pick_file(filter)` (wrapper `pickFile`) opens the native file dialog and returns the chosen path, or null if the user cancels. `filter` is a list of extensions without dots (e.g. `["exe", "cmd"]`, shown as "Files"); null or empty allows any file. A dialog failure fails with `Io`. The Settings screen calls it for the Claude executable "Browse" button (SETTINGS-009).
Since: #49 · Changed: #55

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

### APP-030 Eval harness
`cargo run --bin eval -- --provider ollama|claude-code` (in `src-tauri`) runs the cases of `eval/cases.jsonl` through the real translation service and a real provider, without a Tauri runtime, and writes a Markdown report. `cargo run` with no `--bin` still starts the app (`default-run`). Arguments: `--provider` (required), `--model` (default: the app's default model), `--cases <file>` (default `eval/cases.jsonl`), `--out <file>` (default `eval/reports/<UTC timestamp>.md`, git-ignored), `--preset <id>` (default `technical-development`); `--help` prints the usage; it, an unknown argument, an unknown provider, a missing value, an unreadable or empty cases file and an unknown preset all end with a message on stderr and a non-zero exit. A case is a JSON line with `id`, `mode`, `source`, `mustKeep` (tokens that must appear verbatim in the output) and `notes`. It uses its own scratch settings and presets under `%TEMP%\prompt-translator-eval` (presets re-seeded on every run), never the user's app data; history is off and timeouts are 180 s (Ollama) and 120 s (Claude). The report lists per case the source, output, issues, token warnings, `mustKeep` misses, duration or error, and a summary: JSON-valid rate (provider failures such as timeout are not counted as invalid), `mustKeep` pass rate, average and p95 latency of the successful cases. Details: `eval/README.md`.
Since: #47

### APP-031 Result components
Shared by the Translate screen, History and the popup:
- **Diff**: a word-level comparison of the source and the output, labelled "Changes": removed words struck through on a red background, added words on a green one.
- **Issue list**: one card per issue with a coloured category chip (the category name, `word_choice` shown as "word choice"), `original` struck through, an arrow, `corrected` in bold, and the explanation below when not empty. More than 5 issues show the first 5 and a "Show all <n>" button; expanded, it reads "Show fewer".
- **Token-warning banner** (yellow): "The result is missing text from your input that should stay unchanged:" with each lost token in code style, and the buttons "Ignore" and "Retry". It shows nothing when there are no warnings.
Since: #54

### APP-032 Windows installer
`npm run tauri build` produces one NSIS installer, `Prompt Translator_<version>_x64-setup.exe` (no MSI), in `src-tauri/target/release/bundle/nsis/`. It is per-user (no admin rights, no UAC prompt), English only with no language selector, and installs to `%LOCALAPPDATA%\Prompt Translator\` (changeable) with a Start menu shortcut (a desktop shortcut only if ticked on the last page) and an "Apps" entry. If the WebView2 Runtime is missing it downloads Microsoft's bootstrapper silently. It also installs `resources\presets\*.json` next to the exe (reference copies only).
- **Uninstall:** the confirm page shows "Delete the application data", checked by default. Checked removes `%APPDATA%\com.misstheaim.prompttranslator\` and `%LOCALAPPDATA%\com.misstheaim.prompttranslator\` (including the WebView2 profile); unchecked keeps both. A silent or passive uninstall (`/S`, `/P`) has no page and keeps the data. Either way the program files, shortcuts, "Apps" entry and the autostart values `Prompt Translator` under `HKCU\...\Run` and `HKCU\...\Explorer\StartupApproved\Run` are removed.
- **Update mode (`/UPDATE`):** replaces the program files only; it keeps all app data, the shortcuts and the autostart entry, and the uninstaller shows no "Delete the application data" checkbox. Installing a newer version by hand asks whether to uninstall the old one first; the default ("Uninstall before installing") runs the old uninstaller with `/UPDATE`, so data is kept either way.
Ollama and its models and the user's Claude Code configuration are never touched.
Since: #38

### APP-033 Release build and version
Pushing a tag `v*` runs the `Release` GitHub Actions workflow on `windows-latest`: the checks of APP-019, then a build of the NSIS installer (`--bundles nsis`) signed with the repo secret `TAURI_SIGNING_PRIVATE_KEY` (the run fails early with a clear message when it is missing), and a draft GitHub release named `Prompt Translator <tag>` with the installer, its `.sig` and `latest.json` (a tag containing `-` is marked a prerelease). The release stays a draft until the owner publishes it. A local `npm run tauri build` also needs the signing key in the environment, because `createUpdaterArtifacts` is on.
`node scripts/bump-version.mjs X.Y.Z` sets the version in `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`; without an argument it re-syncs the other files to `package.json`'s version. A version that is not `X.Y.Z` (optionally with a `-suffix`) is refused.
The updater settings in `tauri.conf.json` name the endpoint `https://github.com/misstheaim/prompt-translator/releases/latest/download/latest.json`, the release public key, and the Windows install mode `passive` (APP-034).
Since: #66 · Changed: #67

### APP-034 In-app updater
The core uses the updater plugin against the endpoint of APP-033; the webview gets no updater permission and only calls the commands below.
- `check_update()` asks the endpoint for a newer release and returns `{ available, version?, notes? }` (`version` and `notes` only when available; empty notes are omitted). A failure (offline, no release yet) is returned as an `Io` error "Update failed: <reason>".
- `install_update()` checks again, then downloads and installs the newest release. With no newer release it fails with `InvalidInput` "You're already on the latest version". While downloading it emits `update://progress` with `{ downloadedBytes, totalBytes?, finished }` at most every 150 ms (`totalBytes` only when the server sends it), and a last event with `finished: true` when the download is done. The NSIS installer then runs in passive mode (progress bar, no prompts), the app exits and the installer starts the new version (so the call normally never returns). A failure returns the same `Update failed: <reason>` error.
- Startup check: 10 s after launch, if `autoCheckUpdates` is true (read after the delay), the app checks once and, when a newer release exists, emits `update://available` with the same payload as `check_update`. It never installs by itself; "no update" and errors are only logged.
- `app_info()` returns `{ version, configDir, dataDir }` (the app version, the settings and presets folder, the history/logs/temp folder: APP-006). `open_folder(folder)` with `config` or `data` opens that folder in Explorer, or fails with `Io` "Can't open the folder: <reason>".
Since: #67

### APP-035 Update banner
When `update://available` arrives, or a manual check in Settings, About finds a release (SETTINGS-011), a dismissible info banner appears under the tab bar on every screen: "Version <x> is available." with the release notes as plain text (scrolling above 6rem), and the buttons "Install and restart" and "Dismiss". Dismiss hides it (Settings, About still shows the update); it comes back only when a different version is reported, and a check that finds no release clears the update. "Install and restart" calls `install_update`; while installing the buttons are replaced by a progress bar with "Downloading... <n>%" (a pulsing bar and "Downloading..." while the total is unknown) and, at the end, "Starting the installer..."; the banner stays visible while installing even if dismissed. A failure turns the banner red and adds "Update failed: <message>" with the buttons back.
Since: #68

## Known issues

- The app panics at startup with "No rustls crypto provider is configured" when the updater's HTTP client is created in the startup setup, so it does not launch (#138). Introduced with #67; the updater, the About tab and the banner are therefore unreachable in a build of this commit.

Retired IDs: APP-002
