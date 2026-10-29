# Building the installer

> How to build the Windows installer, what it installs, and what install, update and uninstall do with the app's files (plan T6, decisions D24 and D27). Release steps (version bump, tag, signed build in CI): [`docs/development.md`](development.md) §5.

## Build

From the repo root, after `npm ci`:

```
npm run tauri build
```

- Runs `npm run build` (the frontend) first, then a release `cargo build` of `src-tauri`, then NSIS.
- Output: `src-tauri/target/release/bundle/nsis/Prompt Translator_<version>_x64-setup.exe` (or under `$CARGO_TARGET_DIR/release/bundle/nsis/` when `CARGO_TARGET_DIR` is set).
- The first build downloads NSIS and Tauri's NSIS plugins into `%LOCALAPPDATA%\tauri\`; that needs network access once.
- The version comes from `tauri.conf.json` (synced from `package.json`, see development.md §4).
- Only the NSIS target is built (`bundle.targets: ["nsis"]`); there is no MSI.

The release build is a Windows-subsystem app (`#![windows_subsystem = "windows"]` in `src-tauri/src/main.rs`), so no console window opens at runtime. Debug builds (`npm run tauri dev`) keep the console for logs.

## The installer

Configured in `src-tauri/tauri.conf.json` under `bundle`:

| Setting | Value | Effect |
|---|---|---|
| `windows.nsis.installMode` | `currentUser` | Per-user install, no admin rights and no UAC prompt |
| `windows.webviewInstallMode` | `downloadBootstrapper` (silent) | If the WebView2 Runtime is missing, the installer downloads and runs Microsoft's bootstrapper (needs network; Windows 11 already has the runtime) |
| `windows.nsis.languages` | `English`, no language selector | English installer and uninstaller |
| `resources` | `resources/presets/*.json` | The bundled presets, installed next to the exe. The app itself seeds presets from copies compiled into the binary, so these files are for reference only |
| `windows.nsis.template` | `windows/installer.nsi` | Custom NSIS template, see below |
| `windows.nsis.installerHooks` | `windows/hooks.nsh` | Removes the autostart entries on uninstall, see below |

What the installer puts where:

| What | Where |
|---|---|
| Program files (`prompt-translator.exe`, `uninstall.exe`, `resources\presets\`) | `%LOCALAPPDATA%\Prompt Translator\` (the user can pick another folder) |
| Start menu shortcut | `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Prompt Translator.lnk` |
| Desktop shortcut | only if the user ticks it on the last page |
| "Apps" entry (uninstall information) | `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Prompt Translator` |
| Install folder and installer language | `HKCU\Software\misstheaim\Prompt Translator` |

## App data locations (D27)

Everything the app creates lives in two folders; `src-tauri/src/system/paths.rs` is the only place that builds these paths.

| Folder | Contents |
|---|---|
| `%APPDATA%\com.misstheaim.prompttranslator\` | `settings.json`, `presets\` (seeded with the bundled presets on first start) |
| `%LOCALAPPDATA%\com.misstheaim.prompttranslator\` | `history.db`, `logs\`, `claude-cwd\` (empty working dir of the translator's `claude -p` runs), `tmp\` (their `TEMP`/`TMP`), `EBWebView\` (the WebView2 profile and cache) |

Plus one registry value when "Start with Windows" is on: `Prompt Translator` = `<exe path> --autostart` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, and the same name under `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run` when Windows has created that key (written by `tauri-plugin-autostart`, see `src-tauri/src/system/autostart.rs`).

Not app data, and never touched by the installer or uninstaller: Ollama and its models, the user's own Claude Code configuration, and whatever `winget` leaves behind when the setup wizard installs Ollama.

## Install, update and uninstall

- **First install:** copies the program files, creates the Start menu shortcut and the "Apps" entry. App data is created by the app on first start, not by the installer.
- **Update through the app** (`tauri-plugin-updater`, T31): runs the new installer with `/UPDATE`. It replaces the program files in place and keeps all app data, the shortcuts and the autostart entry.
- **Installing a newer version by hand** over an installed one: the installer asks whether to uninstall the old version first.
  - "Uninstall before installing" (the default) runs the old uninstaller with `/UPDATE`, so it removes only the program files; app data and the autostart entry stay.
  - "Do not uninstall" installs over the old files.
  - Either way all app data is kept.
- **Uninstall** (Windows "Apps", or `uninstall.exe` in the install folder): the confirm page shows **"Delete the application data"**, checked by default.
  - Checked: removes both data folders above (including `EBWebView`) and the installer's registry keys.
  - Unchecked: keeps both data folders, so a later install picks up the old settings, presets and history.
  - Either way: removes the program files, the shortcuts, the "Apps" entry and both autostart values (`Run` and `StartupApproved\Run`).
- **Silent or passive uninstall** (`uninstall.exe /S` or `/P`): no confirm page, so the data is kept (Tauri's behaviour); the autostart values are still removed.
- Update mode (`/UPDATE`) never deletes data and never removes the autostart entry.

## Why a custom NSIS template

Tauri's NSIS template already has the "Delete the application data" checkbox, and it deletes exactly `%APPDATA%\<identifier>` and `%LOCALAPPDATA%\<identifier>` (which holds `EBWebView`). But it creates the checkbox **unchecked** inside its own `un.ConfirmShow` page function, and installer hooks are only inserted into the install and uninstall sections, which run after that page. So hooks can't change the default, and D24 needs it checked.

`src-tauri/windows/installer.nsi` is therefore a copy of the template of the Tauri CLI in use (tag `@tauri-apps/cli-v2.12.4`, file `crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi`) with three changes, each marked `PROMPT TRANSLATOR`:

1. `un.ConfirmShow` checks the checkbox after creating it.
2. `un.ConfirmShow` shows no checkbox in update mode, where data is never deleted.
3. The installer always passes `/UPDATE` when it runs the old uninstaller before installing (by default Tauri passes it only during an update). Without this, the checked default would wipe the user's data during a manual upgrade.

The autostart cleanup stays in `src-tauri/windows/hooks.nsh` (`NSIS_HOOK_POSTUNINSTALL`), because hooks can do it. The template itself removes only the `Run` value.

**When upgrading `@tauri-apps/cli`** (pinned to an exact version in `package.json`): copy the new version's `installer.nsi` over `src-tauri/windows/installer.nsi`, re-apply the three marked changes and update the tag in the file header and above. The template is rendered by the CLI with its own variables, so an outdated copy can break the build or miss upstream fixes.

## Clean-machine test in Windows Sandbox

`docs/sandbox/no-gpu.wsb` starts Windows Sandbox with the GPU off (`<vGPU>Disable</vGPU>`, the CPU-only case for Ollama), networking on (WebView2 bootstrapper, Ollama, model downloads) and the installer folder mapped **read-only** to `Desktop\installer` in the sandbox.

1. Turn on the Windows feature "Windows Sandbox" (Windows 10/11 Pro or Enterprise; needs a restart).
2. Run `npm run tauri build` so `src-tauri/target/release/bundle/nsis/` exists (the mapped folder must exist, or the sandbox won't start).
3. Double-click `docs/sandbox/no-gpu.wsb`. The `HostFolder` path is relative to the `.wsb` file, which recent Windows 11 versions accept. If the sandbox won't start because it can't find the folder (older Windows), or the build output is elsewhere (`CARGO_TARGET_DIR`), set `HostFolder` to the absolute path of the `nsis` folder in a local copy of the file.
4. In the sandbox, run the installer from `Desktop\installer`.

The sandbox is discarded when it closes, so every run starts clean. It has **no `winget`**, so the setup wizard's "Install Ollama" step fails there; install Ollama from <https://ollama.com/download> inside the sandbox if a test needs it. A sign-out/sign-in isn't possible in the sandbox (it would end it), so "Start with Windows" is checked on a real machine.
