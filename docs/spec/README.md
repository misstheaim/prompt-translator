# Prompt Translator: behaviour spec

> What Prompt Translator does **now**: only implemented behaviour. Planned changes are in `docs/plans/`, as spec deltas on their tasks.
> Synced to: `1ed188b` (2026-11-01): every change on `develop` up to this commit is reflected here.

## Overview

Prompt Translator is a Windows tray app for practising English while working with Claude Code: it translates, improves or corrects a prompt typed in Russian (or another language, or mixed with English), explains the mistakes, and keeps a history. It is a Tauri 2 desktop app: a Rust core owns all I/O and a React webview renders the UI, talking to the core only through typed commands. Implemented so far: the app shell (main window with tabs, error banner and toasts, tray icon, close-to-tray, single instance and autostart, app identity and icons, data folders, logging, the error format, shared data contracts); the Translate screen (run a mode, diff, issues and token warnings, copy the result), the History and Mistakes screens, and the Settings screen with the preset editor (including the About tab); the settings and preset stores; the history database; the translation pipeline with the Ollama, Claude Code and mock providers and a provider status check; the technical-token check and mode detection; the global hotkey with auto-copy of the selection and the popup window that translates the captured text; the layout-fix hotkey (default Ctrl+Shift+`) that replaces wrong-layout text in the foreground app; the setup commands (environment check and model recommendation, Ollama install, start, model download and benchmark) with the first-run setup wizard; the layout-fix section of the Settings screen; the NSIS installer and the release workflow; the `eval` quality-report binary; and the in-app updater (manual check and install, a silent startup check, the update banner). Also: unloading the Ollama model (button, tray, on quit), uninstalling from the About tab, and a fixed-size popup (520x360, centred on the monitor under the cursor, not movable or resizable). Also: a configurable native and target language pair (default Russian to English; set in the setup wizard and Settings, switchable per session on the Translate tab), a popup that starts in a mode chosen by where the text was selected (read-only text is translated into the opposite language), a compact mode of the Translate screen (a small complete window per tab), theme and accent colour settings, Translate tabs bound to Claude sessions (terminal windows: listing, binding and watching them, and delivering accepted text into one), and Enter in the popup replacing the selection in the source app.

## Areas

| Area | File | Covers |
|---|---|---|
| App core | [app.md](app.md) | Main window shell (tabs, error banner, toasts), tray, close-to-tray, single instance, autostart, identity and icons, data folders, logging, error format, exported contracts, webview security, developer contracts, CI, the Windows installer, the release build and version script, the in-app updater and update banner, the eval harness |
| Settings | [settings.md](settings.md) | `settings.json`: fields, defaults, loading, validation, saving, commands; the Settings screen, including the layout-fix section and the About tab |
| Presets | [presets.md](presets.md) | Preset files, the nine built-ins (four working, five joke), seeding record, per-preset history setting, validation, list/save/delete/reset; the preset editor |
| History | [history.md](history.md) | SQLite history database, saving results, search, "my mistakes" summary, history commands; the History and Mistakes screens |
| Translation | [translation.md](translation.md) | Prompt building, per-mode output schema, output parsing, translate/cancel/copy commands, mock provider, technical-token check, mode detection; the Translate screen with its diff, issues and token warnings |
| Providers | [providers.md](providers.md) | Ollama and Claude Code providers (text generation only), Claude Code detection and isolation, translator-run registry, provider status |
| Hotkey and popup | [popup.md](popup.md) | Global hotkey, registration errors, the popup window and its translate page and keys, auto-copy of the selection |
| Layout fix | [layout-fix.md](layout-fix.md) | US/Russian wrong-layout text conversion and the layout-fix hotkey (replace, previous word, terminals, input-language switch) |
| Claude sessions | [session.md](session.md) | Terminal session targets (Windows Terminal tabs, classic consoles): listing, binding by focus, closed-target events, delivering accepted text with the prefix guard and the clipboard fallback, the bound Translate tabs |
| Setup | [setup.md](setup.md) | Environment check and model tier, Ollama install, start, model download and benchmark, setup progress events, the setup wizard |

## Glossary

| Term | Meaning |
|---|---|
| Core | The Rust side of the app (`src-tauri/`); it owns every file, process, network and OS call |
| Webview | The React UI (`src/`) running inside the app's windows; it reaches the core only through commands |
| Command | A typed call from the webview to the core (Tauri `invoke`), wrapped in `src/lib/api.ts` |
| Bindings | TypeScript types generated from the core's Rust contract types into `src/bindings/` |
| Popup | The app's second window (`popup.html`), created hidden at startup and shown by the global hotkey |

## How to edit this spec

- Describe behaviour, never implementation: what a user or another component observes.
- One item per behaviour: `### <AREA>-<NNN> <short name>`, then 1-6 lines of plain statements with concrete values (defaults, limits, exact messages where they matter), then `Since: #<issue>` (and `Changed: #<issue>, ...` later).
- **New behaviour:** a new item with the next free ID of its area. **Changed:** edit the item in place and add the issue to `Changed:`. **Removed:** delete the item and add its ID to the area's `Retired IDs` line. IDs are never reused.
- Update the spec in the same change as the code. Tickets run by the `/coordinator` are the exception: it updates the spec after each wave.
- Every statement must be true in the code on `develop`. If you find one that isn't, fix it.
