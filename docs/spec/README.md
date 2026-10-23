# Prompt Translator: behaviour spec

> What Prompt Translator does **now**: only implemented behaviour. Planned changes are in `docs/plans/`, as spec deltas on their tasks.
> Synced to: `6d1cd87` (2026-10-24): every change on `develop` up to this commit is reflected here.

## Overview

Prompt Translator is a Windows tray app for practising English while working with Claude Code: it will translate, improve or correct a prompt typed in Russian (or another language, or mixed with English), explain the mistakes, and keep a history. It is a Tauri 2 desktop app: a Rust core owns all I/O and a React webview renders the UI, talking to the core only through typed commands. So far only the app shell exists: the main window and popup entry, app identity and icons, data folders, logging, the error format and the shared data contracts. Translation, history, settings, the tray, hotkeys and setup are not implemented yet.

## Areas

| Area | File | Covers |
|---|---|---|
| App core | [app.md](app.md) | Windows, identity and icons, data folders, logging, error format, exported contracts, webview security, developer contracts |

## Glossary

| Term | Meaning |
|---|---|
| Core | The Rust side of the app (`src-tauri/`); it owns every file, process, network and OS call |
| Webview | The React UI (`src/`) running inside the app's windows; it reaches the core only through commands |
| Command | A typed call from the webview to the core (Tauri `invoke`), wrapped in `src/lib/api.ts` |
| Bindings | TypeScript types generated from the core's Rust contract types into `src/bindings/` |
| Popup | The app's second window entry (`popup.html`), separate from the main window |

## How to edit this spec

- Describe behaviour, never implementation: what a user or another component observes.
- One item per behaviour: `### <AREA>-<NNN> <short name>`, then 1-6 lines of plain statements with concrete values (defaults, limits, exact messages where they matter), then `Since: #<issue>` (and `Changed: #<issue>, ...` later).
- **New behaviour:** a new item with the next free ID of its area. **Changed:** edit the item in place and add the issue to `Changed:`. **Removed:** delete the item and add its ID to the area's `Retired IDs` line. IDs are never reused.
- Update the spec in the same change as the code. Tickets run by the `/coordinator` are the exception: it updates the spec after each wave.
- Every statement must be true in the code on `develop`. If you find one that isn't, fix it.
