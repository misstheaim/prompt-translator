# Prompt Translator

A Windows tray app for practising English while working with Claude Code. It translates, improves or corrects a prompt typed in Russian (or another language, or mixed with English), explains the mistakes, and keeps a history and a "my mistakes" log. Only the accepted English text ever reaches the user's Claude Code session. Public repo, MIT license, distributed to several Windows machines through GitHub Releases. Current milestone: **v0.1.0** (the whole v1 plan).

## Stack

- Tauri 2 desktop app: Rust core (`src-tauri/`) + React 19 / TypeScript / Vite frontend (`src/`), Tailwind CSS v4, Zustand.
- Providers: local Ollama (HTTP API) and an isolated, short-lived `claude -p` process.
- SQLite history (`rusqlite`), settings and presets as JSON files.
- Tests: `cargo test` (unit + integration with `mockito` and a fake `claude` binary), Vitest + React Testing Library.
- Windows 10/11 x64 only; NSIS per-user installer, `tauri-plugin-updater`.

## Workflow (must follow)

- **Base branch: `develop`.** Create every ticket branch from the latest `origin/develop` (`git fetch origin develop` first). Hotfix tickets (label `hotfix`) branch from the latest `origin/main` instead.
- PRs target `develop`, except release and hotfix PRs, which target `main`.
- Never commit directly to `main` or `develop`. Never merge into `main` or `release/*`, push tags, publish releases or add repo secrets without the owner's explicit approval.
- Full workflow, commands and requirements:

@docs/development.md

## Plan and tickets

- Plan: `docs/plans/prompt-translator.md` (decisions D1–D39 in §3, architecture and contracts in §4, every task's details in §5, order of work in §6). It wins over the original spec `claude_prompt_translator_implementation (1).md` where they differ.
- Tickets: GitHub issues in `misstheaim/prompt-translator`, GitHub Project "Prompt Translator" (https://github.com/users/misstheaim/projects/2), milestone `v0.1.0`. Task IDs like `T12` in the plan map to issue numbers in its hierarchy.
- Spike E9 gate: if T37 (#72) is NO-GO, #31 and #73–#75 are on hold; don't implement them.

## Conventions

- **Rust owns all I/O** (Ollama, `claude.exe`, SQLite, clipboard, files, Win32). React is UI only and talks to Rust through `src/lib/api.ts` typed wrappers (D3).
- Each Rust module keeps its `#[tauri::command]` fns in its own `commands.rs`; `lib.rs` gets one registration line per module (§4.2).
- Contracts are Rust structs with serde `camelCase`, exported to TS with ts-rs into `src/bindings/`. Run `npm run bindings` and commit the result whenever an exported type changes; `u64`/`i64` fields use `#[ts(type = "number")]`.
- Errors go through the serializable `AppError`; user-facing messages follow §4.6 of the plan.
- All app directories come from `system/paths.rs` (D27); never hard-code `%APPDATA%` paths.
- Settings fields use `#[serde(default)]`; old `settings.json` files must keep loading.
- Webview permissions stay minimal: no shell, fs, http or sql plugin permissions for the webview; capabilities list windows `main` and `popup` explicitly (§4.9).
- Never send native-language text or translator instructions to the user's own Claude sessions; never inject into a process that `claude agents --json` doesn't report as an interactive session (§4.9, D31, D35).
- Tests live next to the code (`#[cfg(test)]` modules) or in `src-tauri/tests/` for integration; frontend tests as `*.test.tsx` next to components. Test case IDs (`T12-TC03`), when a ticket has them, go in the test name.

## Gotchas

- `npm run build` must run before any cargo step: Tauri's `generate_context!` needs `frontendDist` to exist.
- Auto-copy (D5) must never send Ctrl+C to a terminal, and never while Shift is held (Ctrl+Shift+C opens DevTools in Chromium apps).
