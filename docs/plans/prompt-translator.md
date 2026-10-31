# Prompt Translator: implementation plan

> Status: Tickets created
> Last updated: 2026-10-14
> Root issue: n/a (new project, top-level epics #1–#9; S6.3 added later as #76–#79). Repo: https://github.com/misstheaim/prompt-translator · Board: https://github.com/users/misstheaim/projects/2
> Workflow: `docs/development.md` (Git Flow: base branch `develop`, milestone `v0.1.0`)
> Source spec: `claude_prompt_translator_implementation (1).md` (project root). This plan supersedes it where they differ.

## 1. Context and goal

- **Problem / motivation:** the owner thinks in Russian (often mixed with English) and wants to **practise English** while working with Claude Code. The main Claude Code session must receive only English. The tool helps the user produce that English and **learn from it**: it translates, corrects the user's own English attempts, and explains the mistakes.
- **Users:** the owner plus other people on **other Windows 10/11 machines**, with any native language and different hardware (possibly no NVIDIA GPU, possibly no Claude Code). Each Windows user has their own install and data.
- **Main use cases:**
  1. Write a prompt in Russian, get a faithful English translation, copy it into Claude Code (**Translate**).
  2. Write a rough prompt, get a cleaner and more precise English prompt that keeps the intent (**Improve**).
  3. Write an English attempt, possibly with Russian words where stuck, get a corrected version, a diff and a short English explanation of each mistake (**Correct**). This is the main learning mode.
  4. From anywhere, select text (copy it by hand in terminals), press a global hotkey, review the result in a small popup and accept it with Enter.
  5. Review past translations and a "My mistakes" summary of recurring errors and new vocabulary.
  6. On a new machine: download the installer from GitHub Releases, install, and let the **first-run setup wizard** install Ollama, download a model that fits the hardware and (if needed) sign in the Claude translator profile.
  7. Get updates from inside the app; uninstall cleanly, with all app data removed by default.
  8. **Bind an open Claude Code session**: pick a running session in the main window's session picker. From then on every Accept (main window and popup) **types the English prompt straight into that session** and submits it. With no session picked, Accept copies to the clipboard. If the bound session closes, Accept is disabled and an error explains why. (Depends on spike T37; see the gate in §6.)
  9. **Fix the keyboard layout**: text typed with the wrong layout (`ghbdtn` instead of `привет`, or `руддщ` instead of `hello`) is fixed in place with a separate hotkey. This is a stand-alone tool, not part of the translate flow: no LLM, no popup, no history (D36–D38).
- **Goal (v1 "done"):** a daily-usable tray app that anyone can install on a Windows machine:
  - hotkey popup plus main window;
  - three modes;
  - two optional providers (a local Ollama model and an isolated Claude Code process), with at least one working;
  - editable presets;
  - technical-token check;
  - history and mistakes view;
  - first-run setup wizard;
  - installer published on GitHub Releases;
  - in-app updater;
  - uninstaller that clears app data;
  - sending accepted prompts into a bound, open Claude Code session (if spike T37 passes);
  - keyboard layout fix hotkey (Russian ↔ English).
- **Hard requirement:** the main Claude Code session never receives the native-language text or the translator's instructions. Only the final English text the user accepted is copied to the clipboard or written into the bound session.
- **In scope (v1):** everything in the goal above, plus CI that runs build and lint, and a release workflow.
- **Out of scope (v1):** focus-and-paste UI automation into terminals; Claude Code channels as a delivery method; a persistent Claude translation session; stopping Ollama when the app exits (the wizard may install and start it); bundling Ollama or models into the installer; streaming output; spaced-repetition quizzes; Authenticode code signing (installer is unsigned, so SmartScreen warns); macOS/Linux builds; translating Claude's replies back; UI localisation (UI is English).

## 2. Current state

New project in `C:\Users\User\Documents\Projects\prompt_translator` (empty except the source spec). GitHub owner: `misstheaim` (personal account). No repo yet. **The repo will be public, MIT license** (decided 2026-10-13). The updater downloads `latest.json` and installers from its GitHub Releases without auth.

The owner's machine below is the **development** machine. Target machines vary, so nothing may assume an NVIDIA GPU, Ollama, Claude Code or a Russian-speaking user.

**Machine (checked 2026-10-13):**
- Windows 11 Pro, Ryzen 7 7435HS, 32 GB RAM, **RTX 4060 Laptop GPU (8 GB VRAM)**. That fits 7–9B models at Q4 fully on the GPU; 12–14B models partly offload to the CPU and run slower.
- Installed: Python 3.14, .NET, Claude Code 2.1.296, gh (logged in as `misstheaim`).
- **Not installed:** Node.js, Rust/Cargo, MSVC Build Tools, Ollama. WebView2 ships with Windows 11.

**Related project:** `misstheaim/prompt-analyzer` (Python/tkinter) already runs `claude.exe` headless with no console window. You can use its approach as a reference for spawning on Windows (no console flash, path detection).

**Stack and practices:**
- **Tauri 2** desktop shell with a **Rust core** and a **React 19 + TypeScript + Vite** UI, **Tailwind CSS v4**, **Zustand** for UI state, npm.
- Official Tauri 2 plugins: `global-shortcut`, `clipboard-manager`, `single-instance`, `autostart`, `opener` (URLs and folders, called from Rust only), `dialog` (file picker), `log`, `updater`, `process`; tray via Tauri's `tray-icon` feature. Plugin overview: [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace). Pin exact versions of Tauri, plugins and `ts-rs` in T2/T3; upgrades are deliberate. Do I/O in Rust commands rather than giving the webview shell/SQL permissions; tray actions that call privileged commands need matching capabilities.
- **Ollama** HTTP API: `POST /api/chat` with `stream: false`, a top-level `format` JSON Schema for structured output, and `keep_alive` to keep the model loaded (the default is about 5 minutes, so set it longer). **Always set `options.num_ctx`**: the default context window (2–4k tokens depending on version) is smaller than the input limit plus prompt, and Ollama silently truncates the oldest tokens, which can drop the system prompt. `GET /api/tags` lists installed models; `GET /api/ps` shows loaded models with `size_vram` vs `size`, which tells whether a model really runs on the GPU. ([LiteLLM Ollama docs](https://docs.litellm.ai/docs/providers/ollama), [LangChain ChatOllamaAI](https://langchain.hexdocs.pm/LangChain.ChatModels.ChatOllamaAI.html); check against the official `docs/api.md` during the implementation tasks.)
- **Claude Code headless:** `claude -p` with `--safe-mode`, `--system-prompt-file`, `--json-schema`, `--tools`, `--strict-mcp-config`, `--disable-slash-commands`, `--setting-sources`, `--no-session-persistence`, `--model`, `--output-format json` (all checked against `claude --help` on 2.1.296).
  - Plain `-p` still loads `~/.claude/CLAUDE.md`.
  - `--bare` skips it but only accepts an API key, not a subscription login ([analysis](https://vijaychauhanseo.substack.com/p/i-reverse-engineered-claude-code), plus local `--help`).
  - **`--safe-mode`** disables CLAUDE.md, skills, plugins, hooks, MCP servers, custom commands/agents and output styles, while auth works normally. Spike T16 confirmed it gives isolation with the normal subscription login (D41).
  - Prefer the native `claude.exe` (`%USERPROFILE%\.local\bin\claude.exe`) over an npm `claude.cmd` shim: Rust refuses or mangles batch-file arguments with quotes/newlines. Long prompts go through files or stdin, never the command line.
  - `claude agents --json` lists running sessions (`pid` = `claude.exe` itself, `cwd`, `kind`, `startedAt`, `sessionId`, `name`, `status` idle/busy); verified by running it.
  - A running translator `claude -p` **is listed** by `claude agents --json` as `kind: "interactive"` for its 2–5 s (spike T16, D41).
  - Spike T16 settled the exact invocation: `docs/spikes/claude-invocation.json` (D41).
- Rust: `rusqlite` (bundled) + `rusqlite_migration`, `reqwest`, `tokio`, `serde`, `thiserror`, `ts-rs` for TS bindings; `whatlang` for language detection; `windows` crate (DXGI) for GPU memory detection.
- **Distribution:**
  - Tauri NSIS bundler, per-user install (no admin).
  - NSIS installer hooks (`bundle.windows.nsis.installerHooks`, e.g. `NSIS_HOOK_POSTUNINSTALL`) for extra uninstall cleanup. The uninstaller's "delete app data" option must be skipped in update mode (`$UpdateMode`).
  - `tauri-apps/tauri-action` in GitHub Actions builds the installer on a version tag and publishes a GitHub Release with the updater artifacts and `latest.json`.
  - `tauri-plugin-updater` checks `https://github.com/misstheaim/prompt-translator/releases/latest/download/latest.json`. Updates are verified with a minisign key pair (`tauri signer generate`); the private key lives in GitHub secrets. Check the exact config keys against the current Tauri v2 docs in T6/T31/T32.
- **Clean-machine testing:** Windows Sandbox (available on Windows 11 Pro) gives a throwaway Windows for testing install, setup wizard, update and uninstall. Two caveats:
  - Sandbox ships **without winget**, so test the "no winget" fallback there; when the winget path itself needs testing, install App Installer in the Sandbox first.
  - Its vGPU is on by default and can expose the host GPU, so use `docs/sandbox/no-gpu.wsb` (`<vGPU>Disable</vGPU>`, mapped read-only folder with the installer) to simulate a machine without a GPU.
- Frontend: ESLint + Prettier. Rust: `clippy`, `rustfmt`. No automated tests (§4.10); `cargo test` is used only by ts-rs to export the bindings.

## 3. Decisions

| # | Decision | Chosen option | Alternatives considered | Why |
|---|---|---|---|---|
| D1 | Purpose | English practice: translation **and** learning (corrections, explanations, mistakes log) | Pure translator (spec) | The user's stated motivation; a translator alone teaches little |
| D2 | Stack | Tauri 2 + React + TS + Vite + Tailwind | Python/tkinter (like prompt-analyzer); .NET WPF | User's choice; modern UI, small footprint. Cost: installing Node + Rust + MSVC (T1) |
| D3 | Where logic lives | **Rust core** owns all I/O (Ollama, claude.exe, SQLite, clipboard, files); React is UI only | Providers in TS via shell/http/sql plugins | Smaller webview permissions, one place for logic |
| D4 | Interaction model | Tray app with **both** a main window and a hotkey popup | Main window only; popup only | Popup for speed, main window for presets, settings, history and mistakes |
| D5 | Hotkey behaviour | **Auto-copy except in terminals:** if the foreground window isn't a terminal, the app sends Ctrl+C itself, reads the clipboard, then restores the previous clipboard text. In terminals (Windows Terminal, conhost) the user copies by hand first. Setting `autoCopySelection` (on by default). Enter in the popup = Accept (D13) | Clipboard only; always simulate Ctrl+C | "Select + hotkey" in most apps; Ctrl+C means interrupt in terminals, so those are excluded |
| D6 | Modes | **Translate, Improve, Correct** in v1 | Translate + Improve (spec); Translate + Correct | User's choice |
| D7 | Presets | A preset = domain rules + default mode. Drop the spec's `preserveIntent` / `preserveTechnicalTerms` / `improveNaturalness` booleans | Booleans as in the spec | The booleans overlap with modes and rules; "preserve intent: off" makes no sense |
| D8 | Output contract | Every provider returns JSON `{ "output": string, "issues": Issue[] }`; Ollama enforces it with `format` (JSON Schema) | Plain text output | Correct/Improve need structured explanations; one parser for all providers |
| D9 | Streaming | Not in v1; a spinner plus **Cancel** | Stream tokens | JSON output can't be shown usefully mid-stream; local 7–9B models answer in about 1–3 s |
| D10 | Ollama lifecycle | **Detect, and start on request**: the wizard and the `OllamaUnreachable` error offer **Start Ollama** (`ollama_start`). The app never stops Ollama and doesn't manage it beyond that. `keep_alive: "30m"`, `num_ctx` from settings (default 8192) | Full start/stop management as in the spec; detect only | On Windows Ollama runs as an autostart tray app; starting it on request covers the gap; `keep_alive` covers model lifetime **Changed by D46 (2026-10-31): `keep_alive` default 10m; the app unloads the model on exit.** |
| D11 | Local model | Chosen by spike T13; always configurable | Fixed model | Quality on Russian and mixed input must be measured on 8 GB VRAM |
| D12 | Claude provider | One `claude -p --no-session-persistence` process per translation (about 7–15 runs per typical 5–10-turn main session; each lives a few seconds and uses the user's subscription quota), default model `haiku`, system prompt via `--system-prompt-file`, output via `--json-schema`. Isolation per spike T16 (D41): `--safe-mode` + `--setting-sources ""` with the user's normal login (no extra sign-in, no dedicated config dir). Always one fixed empty cwd, auto-updater and nonessential traffic disabled via env. Runs never appear in the user's session history or `--resume` list, but they do show in `claude agents --json` while running, so the session picker and injection exclude them (D41) | Persistent session; Anthropic API with key; accept leakage | User's choice: spike first. Per-process gives no context build-up |
| D13 | Claude target | A `ClaudeTarget` abstraction with two targets: **Clipboard** (default, no session bound) and **Console session** (bound session). One **Accept** action (Ctrl+Shift+Enter in the main window, Enter in the popup); its label shows the target ("Send to <session name>" or "Copy") | Copy only; separate Copy and Send buttons | User wants direct sending; one action keeps the flow simple |
| D14 | Storage | `settings.json` + `presets/*.json` in the config dir (`app_config_dir()` = `%APPDATA%\<id>`); SQLite `history.db`, logs, Claude dirs in the **local** data dir (`app_local_data_dir()` = `%LOCALAPPDATA%\<id>`; note `app_data_dir()` is Roaming on Windows and is not used). All paths come from `system/paths.rs` | JSON only; tauri-plugin-sql | Queries for "recurring mistakes" need SQL; the plugin would expose raw SQL to the webview |
| D15 | History | On by default; can be turned off; Clear all | Off by default | It's the learning log; privacy is covered by the toggle and Clear |
| D16 | Explanation language | English, short and simple | Russian; configurable | More practice for the user |
| D17 | Token check | In v1 (pure Rust function) | Phase 6 as in the spec | Safety net against small models changing identifiers |
| D18 | TS types | Generated from Rust structs with `ts-rs` into `src/bindings/` | Hand-written mirrors | Contracts stay in sync |
| D19 | Distribution | **GitHub Releases** built by `tauri-action` on a `v*` tag; NSIS per-user installer | Local build shared by hand | Several users on several machines |
| D20 | Auto mode in popup | Strip code spans, URLs and identifiers. Then: **Latin-script words ≥ 60% of words → Correct** (covers English with some native words, the main learning case); otherwise → Translate. `whatlang` is the tie-breaker in the 40–60% band. Very short text (< 3 words) → last-used mode (`lastPopupMode` in settings); user can switch | Pure whatlang; Cyrillic ratio; always last-used mode | Users may write in any language; mixed text must go to Correct |
| D21 | Source languages | **Any**, auto-detected; output and explanations always English; the Mistakes view calls them "native → English" | Russian only | Other users have other native languages |
| D22 | Updates | In-app updater (`tauri-plugin-updater`), checks on startup (setting) and from Settings → About; signed with an updater key | Manual download | Several machines; the updater key is free (unlike Authenticode) |
| D23 | Code signing | **Unsigned** for v1; README documents SmartScreen "More info → Run anyway" | Azure Trusted Signing (about $10/month) | Few users; can be added to the release workflow later |
| D24 | Uninstall | Uninstaller shows "Delete all app data" **checked by default**. It removes `%APPDATA%\<id>` and `%LOCALAPPDATA%\<id>` (settings, presets, history, translator Claude profile, WebView cache, temp) and **always** removes the autostart entry. It never touches Ollama or its models. Update mode never deletes data | Always delete; never delete | User asked for full cleanup; the checkbox protects history on reinstall |
| D25 | New-machine setup | **First-run setup wizard** in the app: checks the environment, installs Ollama via `winget` (fallback: open the download page), recommends a model by hardware, downloads it with progress, benchmarks it, detects Claude Code and checks it is signed in (normal login, D41). Re-runnable from Settings | Bundle Ollama in the installer; README instructions only | Ollama + models are several GB and shared with other software; a wizard works for non-technical users |
| D26 | Provider availability | Both providers are optional; the app needs at least one working provider and says so clearly when none is | Ollama required | Some machines are too weak for local models; some users don't have Claude Code |
| D27 | App data locations | Everything under `%APPDATA%\com.misstheaim.prompttranslator` (config: settings, presets) and `%LOCALAPPDATA%\com.misstheaim.prompttranslator` (history.db, logs, `claude-cwd`, `tmp`, WebView cache). Nothing in `%TEMP%` or elsewhere. `system/paths.rs` (T3) is the single source of these paths | Scattered defaults | So the uninstaller can remove everything |
| D28 | Local model per machine | Not tied to the dev machine. T13 measures and produces a **tier table** keyed on a dedicated GPU's VRAM. Integrated GPUs count as no GPU. **Changed in #45 (owner):** no CPU tiers: a machine without a usable GPU gets tier `none` and Claude Code as the recommended provider. No per-mode provider default (Ollama and Claude are equal choices in every mode). The wizard recommends the matching tier, then runs a **quick benchmark** on the downloaded model; if it's slow, it recommends Claude Code as the default provider (local stays as an offline fallback). Always changeable in Settings | One fixed model; tier by VRAM only | Laptops with only integrated graphics run models on the CPU, where RAM sets the size limit and real speed varies too much to predict |
| D29 | Repo and license | Public GitHub repo `misstheaim/prompt-translator`, MIT license | Private repo + public releases repo | User's choice; needed for the updater |
| D30 | Session discovery | `claude agents --json` (lists interactive and background sessions with `pid`, `cwd`, `name`, `status` idle/busy), polled about every 2 s **only while the main window or popup is visible**, plus a fresh check when the popup opens and right before each Accept. No polling while hidden (each poll spawns `claude.exe`) | Scanning processes; window titles | Official scripting output of the CLI; gives exactly the data the picker needs |
| D31 | Delivery into a session | **Console input injection** (`AttachConsole(pid)` + `WriteConsoleInputW`, then Enter), targeted by PID, with no focus change, run in a **short-lived helper process** (the app re-runs its own exe with `--inject`) so console attach, Ctrl+C/close events and unsafe FFI can't affect the app. Only into a PID that `claude agents --json` reports as interactive **and** that is not one of the app's own translator runs (D41). Must be proven by spike T37. **If the spike fails, epic E9 goes on hold** and v1 ships with Clipboard only | Injection inside the app process; Claude Code channels (preview, needs a special startup flag, arrives as a `<channel>` event); focus-and-paste (fragile, can't target a Windows Terminal tab) | Works with any already-open session, needs no restart or flags; helper process isolates the risk **Superseded by D47 (2026-10-31).** |
| D32 | Busy session | Send anyway; Claude Code queues it. Show "Session busy: your prompt will be queued" | Wait for idle; block | User's choice |
| D33 | Existing draft in Claude's input box | Never clear it; the prompt is appended. A one-time hint explains this | Send a clear-line key first | Never delete the user's text silently |
| D34 | Binding scope | One binding for the whole app, chosen in the main window's session picker. The popup shows the target and follows it. Not persisted across app restarts (sessions are short-lived) | Per-window binding; persisted binding | User's choice; simple mental model **Superseded by D47 (2026-10-31): bindings are per Translate tab.** |
| D35 | Prompt prefix guard | If the accepted text starts with `!`, `/` or `#` (after trimming whitespace) and the target is a session, Accept asks for explicit confirmation ("This will run as a shell command / slash command / memory note in Claude Code. Send anyway?"). Never sent silently | Block outright; strip the character | These prefixes trigger bash mode, slash commands and memory in Claude Code; model output from untrusted source text could start with them, but a user may also want to send a slash command on purpose |
| D36 | Keyboard layout fix | A **stand-alone tool** with its own hotkey, outside the translate flow. A fixed key-position mapping between the Windows Russian (ЙЦУКЕН) and US (QWERTY) layouts: no LLM, instant, offline. Direction by majority of Cyrillic vs Latin letters in the text. Only characters of the source layout change, so correctly typed words in the other script stay as they are. No history, no window. Only ru ↔ en in v1; the tables are data so other pairs (e.g. Ukrainian) can be added later | Convert through the LLM; per-word direction | Deterministic and instant; per-word direction is ambiguous for short words |
| D37 | Layout fix input and output | Selection via Ctrl+C (same rules as D5). **No selection** → send Ctrl+Shift+Left to select the previous word, then convert it. The result is pasted with Ctrl+V and the previous clipboard text is restored. **In terminals** (or when `autoCopySelection` is off) it converts the clipboard text in place and shows a notification; the user pastes | Keyboard hook that buffers typed keys (Punto Switcher style); terminals unsupported | User's choice. Reuses T25b's code; a global keyboard hook is heavier and looks like a keylogger to antivirus tools |
| D38 | Layout fix extras | After converting, switch the Windows input language to the target layout (only if it's installed; setting, on by default). Default hotkey **Shift+Pause** (changed in #79, owner decision: Ctrl+Alt+Space was taken by another app on the owner's machine), configurable, must differ from the translate hotkey | Shift+Home (rejected: it's "select to line start" in every app); Ctrl+Alt+Space (the first default; often taken by other apps) | User's choice; works on every keyboard **Changed 2026-10-31 (owner, T56): default hotkey `` Ctrl+Shift+` `` (takes VS Code's "Create New Terminal" while the app runs; accepted).** |
| D39 | Branching and releases | Git Flow as in `docs/development.md`: `main` holds released code only; `develop` is the default and integration branch; one ticket = one branch = one PR squash-merged into `develop`; `release/X.Y.Z` and `hotfix/*` merge into `main` with a merge commit and a `vX.Y.Z` tag. **One milestone, `v0.1.0`, for the whole v1 plan** | Trunk-based on `main`; several smaller milestones | The coordinator needs a stable integration branch, and `main` stays releasable for hotfixes; v1 ships once, after T7's smoke test |
| D40 | Prompt templates vs. the local model spike | T11 writes provider-neutral templates from §4.4 without waiting for T13. T13 runs in wave 4, once Ollama is installed, with T11's templates as its baseline; T44 applies its findings to the templates | T11 waits for T13 | Ollama wasn't ready in wave 0; this unblocks the core path and the Claude Code provider, and the templates are plain files that are cheap to change later |
| D41 | Claude isolation (spike T16 outcome, owner decision 2026-10-24) | Invocation **candidate 1**: `claude -p --safe-mode --setting-sources ""` plus the flags in `docs/spikes/claude-invocation.json` (the contract T17b builds from: args, env added, explicit `envRemove` list, cwd under the app data dir via `system/paths.rs`, `TEMP`/`TMP` redirect, UTF-8 stdin, `CREATE_NO_WINDOW`), using the user's normal subscription login. No dedicated `CLAUDE_CONFIG_DIR`, no separate sign-in, no `claude_open_login`. Sign-in check: `claude auth status` (exit 0 = signed in); its output contains the e-mail, so **never log it**. **Safety:** a running translator run is listed by `claude agents --json` as `kind: "interactive"` (name from `--name prompt-translator`, via a transient `~/.claude/sessions/<pid>.json`). Session discovery (D30), injection (D31, D35) and the picker therefore exclude the app's own translator runs by **both** `name == "prompt-translator"` **and** PIDs in the translator-run registry (`TranslatorPids`, T17a) | Dedicated config dir (separate sign-in, hides runs from `agents`, but costs the user a login); `--bare` + API key; accept leakage | Spike showed candidate 1 loads none of the user's CLAUDE.md, hooks, skills, plugins or MCP servers, leaves no history, and needs no extra step; the `agents` listing is handled by filtering, with two independent checks so one failing (rename, registry race) can't send text into a translator run |
| D42 | E9 on hold (spike T37 outcome, owner decision 2026-10-24) | Spike T37 (#72) could not run: the implementing agent's safety classifier blocked writing the console injector, so there is **no verdict**. The owner put T37 and story S9.2 (#31, T38-T40 = #73-#75) **on hold** (label `status:on-hold`, removed from milestone v0.1.0). **v0.1.0 ships with Clipboard only**; `copy_result` stays the only Accept path. The T38-T40 tickets and D30-D35 are kept for the future. **What would change it:** the owner writes and runs the injector by hand per T37 step 2 and the matrix passes, or a path that doesn't write into another process's console exists (e.g. Claude Code channels once out of preview) | Keep E9 in v0.1.0 and wait; drop E9 for good | The only risky part (writing into another process's console) needs the owner's hands; Clipboard already delivers the core value. **Superseded by D47 (2026-10-31).** |
| D43 | Improve mode (owner, manual test 2026-10-31) | Improve becomes a **practice** mode: a natural, advanced rewrite (higher-level vocabulary and constructions, idioms where a native speaker would use them), up to 12 explained changes, new issue categories `idiom` and `phrasing`. **Disabled in Translate tabs bound to a Claude session** (D47), because idioms make prompts less precise for an LLM (T51) | Keep Improve conservative and add a style option | Owner's choice; the app is for English practice, and bound tabs keep prompts precise |
| D44 | Language pair (owner, 2026-10-31) | Two settings, native (default `ru`) and target (default `en`) language, chosen in the wizard and in Settings; the Translate tab can switch the pair for the session only. Languages = those `whatlang` detects. Templates, mode detection and history use the pair; explanations, the UI and the layout fix (US ↔ Russian) stay as they are (T52, T53) | Hard-wired ru → en; all ISO languages | Detection bounds the list; with the defaults nothing changes |
| D45 | Popup starting mode and Enter (owner, 2026-10-31) | At capture, UI Automation classifies the source as editable, read-only or unknown. Read-only → Translate into the **opposite** language of the text (first word decides); editable/unknown → the "Popup starts in" setting (Auto = TRANS-003, fixed mode, or Last used). In the popup, **Enter replaces** an editable selection (paste + clipboard restore) and **Ctrl+Enter copies**; otherwise Enter copies (T54, T55) | Language-only detection; Enter = copy | Owner's choice; reading foreign text and fixing one's own text are different jobs |
| D46 | Local model memory (owner, 2026-10-31) | `ollama.keepAlive` default **10m** (was 30m, D10); the app unloads the model on exit and offers an always-visible "Unload model" button (top bar and tray) (T56) | 30m only; stop Ollama on exit | A 9b model holds about 6.4 GB of VRAM; unloading isn't stopping Ollama, which stays out of scope |
| D47 | Delivery into a session, revised (owner, 2026-10-31; replaces D31, D34 and D42) | **No console injection.** VS Code extension sessions get the text through the extension's link `vscode://anthropic.claude-code/open?session=<id>&prompt=<text>` (opens the session, fills its input); terminal sessions: a Translate tab is bound to **one terminal window** (one session per window), and delivery brings it to the front, selects the bound Windows Terminal tab (UI Automation) and pastes once with Ctrl+V, restoring the clipboard. **Never sends Enter**: the user reviews and sends in Claude. Bindings are **per Translate tab**: one default unbound tab plus at most 5 bound ones, not restored after a restart; a tab closes when its terminal window closes. D32/D33/D35 still apply. Spike T58 (#148) confirms the recipe; T37 (#72) is dropped | Console input injection (blocked, D42); Claude Code channels (research preview, start flag, arrives as an event, not as reviewable input); Remote Control (no API) | Uses only actions a user could do (focus, paste, open a link), so nothing writes into another process; the owner checked by hand that a paste waits in Claude's input while it works |

## 4. Architecture and design

### 4.1 Components

```mermaid
flowchart LR
  subgraph Webview [React UI]
    MW[Main window: Translate / History / Mistakes / Settings]
    PU[Popup window]
  end
  subgraph Core [Rust core]
    CMD[Tauri commands]
    SVC[TranslationService]
    PB[PromptBuilder + OutputParser]
    TC[TokenChecker]
    PS[PresetStore]
    ST[SettingsStore]
    HR[HistoryRepo - SQLite]
    OP[OllamaProvider]
    CP[ClaudeCodeProvider]
    MP[MockProvider]
    SYS[Tray / Hotkey / Clipboard]
  end
  MW -- invoke --> CMD
  PU -- invoke --> CMD
  CMD --> SVC
  SVC --> PB
  SVC --> TC
  SVC --> PS
  SVC --> HR
  SVC --> OP & CP & MP
  OP -- HTTP localhost:11434 --> OL[(Ollama)]
  CP -- spawn, stdin/stdout --> CC[claude.exe isolated]
  SYS -- events --> PU
```

Flow of one request: UI calls `translate(req)`. `TranslationService` loads the preset and builds the prompt (mode template + preset rules + delimited source text). It calls the selected provider and parses the JSON output, then runs the token check and saves to history when that's enabled. It returns a `TranslationResult`. The UI shows the result. **Accept** sends `output` through the `ClaudeTarget`: into the bound session if one is picked, otherwise to the clipboard. It also marks the history entry as accepted with that target.

### 4.2 Rust module layout (`src-tauri/src/`)

```
main.rs / lib.rs          app builder, plugin + command registration
error.rs                  AppError (serializable)
settings/                 Settings model + SettingsStore (settings.json)
presets/                  Preset model + PresetStore; built-ins in src-tauri/resources/presets/*.json
translation/
  model.rs                Mode, ProviderKind, TranslationRequest, TranslationResult, Issue, IssueCategory, TokenWarning
  prompts/                translate.md, improve.md, correct.md (include_str!)
  prompt_builder.rs
  output_parser.rs
  token_check.rs
  mode_detect.rs          whatlang-based auto mode
  service.rs              TranslationService, provider trait, cancellation registry
  providers/{ollama.rs, claude_code.rs, mock.rs}
history/                  db.rs (migrations), repo.rs
system/                   tray.rs, hotkey.rs, popup.rs, clipboard.rs, paths.rs (all app dirs, D27)
setup/                    environment.rs (GPU/VRAM, Ollama, winget, claude checks), model_tiers.rs, ollama_setup.rs (install, start, pull)
updater.rs                check/install via tauri-plugin-updater
layout_fix/               convert.rs (layout tables, direction, convert), hotkey.rs (capture → convert → paste, input-language switch) (T41, T42)
target/                   mod.rs (ClaudeTarget trait, accept routing), clipboard.rs, sessions.rs (discovery + binding), console_inject.rs (Win32 console input)
<module>/commands.rs      each module owns its #[tauri::command] fns (thin); lib.rs only adds one registration line per module
logging                   tauri-plugin-log → %LOCALAPPDATA%\<id>\logs (T3)
bin/eval.rs               quality regression harness (T15); Cargo.toml sets `default-run` to the app binary
```

**Conventions to avoid merge conflicts between parallel tasks:**
- Commands live in their module's `commands.rs`; `lib.rs` gets a single line per module when registered.
- Each backend task adds its own typed wrappers to `src/lib/api.ts` (one section per module) and its own permissions to the capability files.
- Each task regenerates `src/bindings/` (`npm run bindings`) and commits the result; CI fails if they're stale.
- Fields carrying `u64`/`i64` (ids, durations, byte counts, RAM) use `#[ts(type = "number")]` so TS gets `number`, not `bigint`. Bindings are exported via `TS_RS_EXPORT_DIR` in `src-tauri/.cargo/config.toml`.

Frontend (`src/`): `bindings/` (generated), `lib/api.ts` (typed invoke wrappers), `store/` (Zustand), `components/`, `screens/{Translate,History,Mistakes,Settings,Setup}/`, `popup/` (second entry point `popup.html`).

Packaging (`src-tauri/`): `tauri.conf.json` (bundle, updater pubkey and endpoint, strict `app.security.csp`), `capabilities/default.json` (windows `main` and `popup`, minimal permissions, created in T3 and extended by later tasks), `windows/hooks.nsh` (uninstall cleanup), `resources/presets/`. Release: `.github/workflows/release.yml`. Sandbox configs: `docs/sandbox/*.wsb`.

### 4.3 Data contracts (Rust, serde `camelCase`, exported to TS via ts-rs)

```rust
enum Mode { Translate, Improve, Correct }
enum ProviderKind { Ollama, ClaudeCode, Mock }   // Mock only in debug builds / dev setting

struct TranslationRequest {
    request_id: String,        // UUID from UI, used for cancel
    source_text: String,
    mode: Mode,
    provider: ProviderKind,
    preset_id: String,
}

enum IssueCategory { Spelling, Grammar, Article, Tense, Preposition, WordChoice, Vocabulary, Punctuation, Style, Other }
// Vocabulary = a non-English word/phrase the user didn't know, replaced by English.

struct Issue { original: String, corrected: String, category: IssueCategory, explanation: String }

struct TokenWarning { token: String, kind: TokenKind }  // token present in source but missing verbatim from output
enum TokenKind { TicketId, Identifier, FileName, Path, Url, CodeSpan, Number }

struct TranslationResult {
    history_id: Option<i64>,   // None when history is off
    output: String,
    issues: Vec<Issue>,        // empty for Translate
    token_warnings: Vec<TokenWarning>,
    provider: ProviderKind,
    model: String,
    duration_ms: u64,
}
```

**LLM output schema** (sent as Ollama `format`, and as `--json-schema` to Claude; Claude's result is read from the envelope's structured-output field, per the T16 spike):

```json
{ "type": "object", "required": ["output", "issues"],
  "properties": {
    "output": { "type": "string" },
    "issues": { "type": "array", "items": { "type": "object",
      "required": ["original", "corrected", "category", "explanation"],
      "properties": { "original": {"type":"string"}, "corrected": {"type":"string"},
        "category": {"type":"string", "enum": ["spelling","grammar","article","tense","preposition","word_choice","vocabulary","punctuation","style","other"]},
        "explanation": {"type":"string"} } } } } }
```

Parser rules: strip ```` ``` ```` fences if present; parse strictly; unknown category → `Other`; blank `output` → `AppError::EmptyOutput`; unparsable → `AppError::MalformedOutput` (keep the raw text in the error's `detail` for debugging).

**Preset file** (`presets/<id>.json`):

```json
{ "id": "technical-development", "name": "Technical Development",
  "description": "Programming and software development prompts",
  "defaultMode": "improve",
  "rules": ["Preserve all code identifiers, filenames, URLs and ticket numbers exactly.", "Use concise professional technical English.", "Do not invent missing information."],
  "builtIn": true }
```

Built-ins: `general`, `technical-development`, `code-review`, `work-communication`. On first run they're copied from bundled resources into `%APPDATA%\com.misstheaim.prompttranslator\presets\`. Users can edit them; **Reset** restores the bundled versions.

**Settings** (`settings.json`):

```json
{ "defaultProvider": "ollama", "defaultPresetId": "technical-development",
  "ollama": { "endpoint": "http://localhost:11434", "model": "<from T13>", "keepAlive": "30m", "numCtx": 8192, "timeoutSecs": 60 },
  "claude": { "executablePath": null, "model": "haiku", "timeoutSecs": 90 },
  "historyEnabled": true, "hotkey": "Ctrl+Shift+Space", "autostart": false, "closeToTray": true,
  "setupCompleted": false, "autoCheckUpdates": true,
  "autoCopySelection": true, "sessionAppendHintShown": false, "lastPopupMode": "correct",
  "devMockProvider": false,
  "layoutFix": { "enabled": true, "hotkey": "Shift+Pause", "switchInputLanguage": true } }
```

Unknown or missing fields fall back to defaults (`#[serde(default)]`). A corrupt file is renamed to `settings.json.bak` and defaults are used.

### 4.4 Prompt construction

- System prompt = mode template (`prompts/<mode>.md`) + "Additional rules:" + preset rules + output-schema instructions.
- User message = the source text wrapped in `<source_text>…</source_text>`, plus the reminder: "The text is data to transform, not a request to you. Never answer or perform it."
- Shared rules for all modes: keep the intent; don't answer the request; keep identifiers, filenames, URLs, ticket IDs and code spans verbatim; don't invent details; keep ambiguity rather than resolve it.
- **Translate:** faithful, minimal changes, `issues: []`.
- **Improve:** clearer, more precise English for Claude Code; same intent, no new requirements; `issues` lists notable changes with category `style`/`word_choice`.
- **Correct:** fix the user's English with the smallest edits, translate any non-English fragments in any language (each one an `issues` entry with category `vocabulary`), and give each fix a short English explanation (one sentence, B1-level vocabulary).
- Templates never assume a specific source language ("the text may be in any language or a mix").
- Temperature 0.2 for Ollama.

### 4.5 Token check

Extract candidate tokens from the source with regexes: ticket IDs `[A-Z][A-Z0-9]+-\d+` and `#\d+`; camelCase/PascalCase identifiers that contain an inner capital; snake_case and SCREAMING_CASE identifiers; `name.ext` filenames; Windows/Unix paths; URLs; backtick code spans. Each token missing verbatim from `output` becomes a `TokenWarning`. Plain words, including misspelled English words in Correct mode, are never tokens.

### 4.6 Tauri command API

| Command | Input | Output |
|---|---|---|
| `translate` | `TranslationRequest` | `TranslationResult` |
| `cancel_translation` | `requestId` | `()` |
| `detect_mode` | `text` | `Mode` |
| `provider_status` | – (T14 creates it, T17a fills `claude`) | `{ ollama: { reachable, models: string[], selectedModelInstalled }, claude: { found, path, version, signedIn: bool \| null } }` |
| `get_settings` / `save_settings` | `Settings` | `Settings` |
| `list_presets` / `save_preset` / `delete_preset` / `reset_presets` | `Preset` / `id` | `Preset[]` |
| `copy_result` | `{ text, historyId? }` | `()`: writes the clipboard (T12), marks the entry accepted with target `clipboard` (T28). Used until E9 lands; afterwards the UI calls `accept_result` |
| `list_sessions` | – | `ClaudeSession[]` = `{ pid, sessionId, name, cwd, status: 'idle' \| 'busy' \| string, kind }` (interactive only) |
| `bind_session` / `unbind_session` | `{ sessionId }` / – | `BindingState { bound: ClaudeSession \| null, alive: bool }` |
| `get_binding` | – | `BindingState` |
| `accept_result` | `{ text, historyId?, confirmedPrefix?: bool }` | `{ deliveredTo: 'clipboard' \| 'session', sessionName?, queued: bool }`. Routes to the bound session or the clipboard; errors `SessionClosed`, `SessionUnreachable` (e.g. elevated terminal), `InjectionFailed`, `PrefixNeedsConfirmation` (D35) |
| `history_list` | `{ query?, issue?: {original, corrected}, limit, offset }` | `HistoryEntrySummary[]` (the `issue` filter returns entries containing that mistake; used by the Mistakes screen) |
| `history_get` / `history_delete` | `id` | `HistoryEntry` / `()` |
| `history_clear` | – | `()` |
| `mistakes_summary` | `{ sinceDays? }` | `{ byCategory: {category, count}[], recurring: {original, corrected, category, count}[], vocabulary: {original, corrected, count, lastSeen}[] }` |
| `environment_check` | – | `EnvironmentReport { os, ramMb, ramAvailableMb, usableGpu: bool, gpus: {name, vramMb, integrated, usable}[], recommendation: { tier, model, downloadMb } (`model`/`downloadMb` null for tier `none`, changed in #69), winget: bool, ollama: { installed, running, version, models }, claude: { found, path, version, signedIn } }` |
| `ollama_install` | – | `()`: runs winget; progress via `setup://progress` |
| `ollama_start` | – | `()`: launches the installed Ollama app if not running (wizard and the `OllamaUnreachable` error's **Start Ollama** button) |
| `ollama_benchmark` | `{ model }` | `{ translateMs, correctMs, gpuOffloadPercent }` (offload from `/api/ps`) |
| `ollama_pull` | `{ model }` | `()`: progress via `setup://progress { phase, completedBytes, totalBytes }` |
| `cancel_setup_task` | – | `()` |
| `complete_setup` | – | `()`: sets `setupCompleted` (T35) |
| `app_info` | – | `{ version, configDir, dataDir }` (T32) |
| `check_update` | – | `{ available, version?, notes? }` |
| `install_update` | – | `()`: download + install + restart; progress via `update://progress` |
| `open_folder` | `'config' \| 'data'` | `()` (T32, via `tauri-plugin-opener` from Rust) |
| `open_url` | `{ url }` | `()`: only allow-listed URLs (ollama.com download, Claude Code install docs, the repo's releases); via `tauri-plugin-opener` (T19) |
| `pick_file` | `{ filter }` | `string \| null` (native file dialog via `tauri-plugin-dialog`; used for the claude.exe path in T22) |

Events: `popup://source` (Rust → popup: captured text when the hotkey fires), `settings://changed`, `setup://progress`, `update://available`, `update://progress`, `sessions://changed` (session list or binding state changed: bound session closed, status idle↔busy), `main://open-entry` (popup → main window: show this input/result; payload `{ input, mode, result: TranslationResult | null }`, changed in #58), `hotkey://error` (Rust → main: popup hotkey registration error, string or null; also readable via the `hotkey_error` command, changed in #58).

**Errors** (`AppError`, serialized as `{ kind, message, hint }`):

| kind | hint shown in UI |
|---|---|
| `OllamaUnreachable` | "Ollama isn't running." + **Start Ollama** button (`ollama_start`) + Open settings |
| `ModelMissing` | "Run `ollama pull <model>` or pick another model." |
| `ClaudeNotFound` | "Set the claude.exe path in Settings." |
| `ClaudeAuth` | "Claude Code isn't signed in. Run `claude` once and sign in" (the translator uses the normal login, D41) |
| `Timeout` / `Cancelled` | Retry |
| `EmptyOutput` / `MalformedOutput` | Retry; try another provider |
| `ProcessCrashed` | Show stderr excerpt |
| `SessionClosed` | "The bound Claude session has ended. Pick another session or unbind to copy instead." |
| `SessionUnreachable` | "Can't write to this session. Is its terminal running as administrator?" |
| `InjectionFailed` | "Sending failed. The prompt was copied to the clipboard instead; paste it manually." (fallback: copy) |
| `PrefixNeedsConfirmation` | "This starts with `!` / `/` / `#` and will run as a shell command / slash command / memory note in Claude Code. Send anyway?" (D35) |
| `Io` / `Db` / `InvalidInput` | message |

### 4.7 Data model (SQLite, `history.db`)

```sql
CREATE TABLE translations (
  id INTEGER PRIMARY KEY,
  created_at TEXT NOT NULL,            -- ISO 8601 UTC
  mode TEXT NOT NULL, provider TEXT NOT NULL, model TEXT NOT NULL, preset_id TEXT NOT NULL,
  source_text TEXT NOT NULL, output_text TEXT NOT NULL,
  duration_ms INTEGER NOT NULL,
  token_warnings TEXT NOT NULL DEFAULT '[]',   -- JSON
  accepted_target TEXT,                -- NULL = not accepted; 'clipboard' or 'session'
  accepted_session_id TEXT,            -- Claude sessionId when sent to a session
  accepted_session_name TEXT
);
CREATE TABLE issues (
  id INTEGER PRIMARY KEY,
  translation_id INTEGER NOT NULL REFERENCES translations(id) ON DELETE CASCADE,
  category TEXT NOT NULL, original TEXT NOT NULL, corrected TEXT NOT NULL, explanation TEXT NOT NULL
);
CREATE INDEX idx_translations_created ON translations(created_at);
CREATE INDEX idx_issues_category ON issues(category);
```

Migrations via `rusqlite_migration`; `PRAGMA foreign_keys = ON`. Stored at `app_local_data_dir()/history.db` (D14). Recurring mistakes = group by `lower(original), lower(corrected)`.

### 4.8 UI

- **Main window** (about 720×640, resizable). A left nav or top tabs switches between **Translate**, **History**, **Mistakes** and **Settings** (with a Presets sub-section). Light and dark themes follow the OS.
- **Session picker** (top bar of the main window, always visible): "Target: [ None (copy to clipboard) ▾ ]". The list shows active interactive sessions as `name — folder — idle/busy`, refreshed live. Picking one binds it for the whole app (D34). States: no Claude Code installed (picker hidden); no sessions running ("No open Claude sessions"); bound + idle (green dot); bound + busy (amber dot, "prompt will be queued"); **bound session closed** (red, "Session ended": Accept disabled until the user picks another session or None).
- **Translate screen:** mode segmented control (Translate / Improve / Correct), provider select, preset select (choosing a preset sets its default mode), input textarea, **Run** (Ctrl+Enter) / **Cancel**. Output panel: English text (editable), **Accept** (Ctrl+Shift+Enter; label "Copy" or "Send to <session>"), duration and model. In Improve and Correct the panel adds a word-level diff and the issues list (category chip + original → corrected + explanation). Token warnings appear as a yellow banner with **Ignore** / **Retry** (not "Accept", which is the main action).
- **Popup:** frameless, always on top, about 520×360, centered on the monitor under the cursor. It shows the source excerpt and auto-runs with the detected mode (mode chips switch and re-run). It shows the result and an issues count with expandable explanations. A footer shows the target ("Enter: send to <session>" or "Enter: copy"). **Enter** accepts (sends or copies) and closes; if the bound session has ended it shows the error and stays open. **Esc** closes, **Ctrl+R** retries, and a link opens the entry in the main window.
- **States everywhere:** idle, loading (spinner + Cancel), success, error (message + hint + Retry / Open settings), empty (History and Mistakes).
- **Setup wizard** (main window, shown when `setupCompleted` is false; re-runnable from Settings): Welcome → **Local model** (install or start Ollama; recommended model for this machine with alternatives; download with progress; benchmark; or Skip) → **Claude Code** (detected or not; signed in or not, with a hint to run `claude` once if not; or Skip) → **Defaults** (default provider limited to working ones, hotkey, history on/off, autostart) → **Done** (quick tour of the hotkey flow). If no provider works at the end, Done stays enabled but shows a clear warning and Translate shows a "Set up a provider" banner.
- **Settings → About:** version, "Check for updates", update available banner with release notes and **Install and restart**, "Run setup again", "Open data folder".
- **Keyboard layout fix** (D36–D38): no window of its own. Settings → System has a "Keyboard layout fix" group (enabled, hotkey, switch input language). In terminals the only feedback is a Windows notification ("Layout fixed: paste with Ctrl+V").
- **Tray:** icon; menu: Open, Quit. Closing the main window hides it to the tray (setting). Single instance: a second launch focuses the existing window. When started by autostart (`--autostart` argument), the app starts hidden in the tray.

### 4.9 Non-functional

- **Privacy:** in Ollama mode nothing leaves the machine. **In Claude mode the native text is sent to Anthropic** (through the isolated translator process, under the user's own Claude account), never into the user's working sessions. The only thing ever written into a bound session is the accepted English text, and only on an explicit Accept. Auto-copy may capture whatever is selected, including secrets, which would then be translated and stored in history; the README says so. The keyboard layout fix is entirely local and stores nothing.
- **Console injection safety:** inject only into a PID that `claude agents --json` currently reports as an interactive session (re-checked right before writing), **and never into one of the app's own translator runs**: those are also listed as interactive while they run (D41), so exclude entries named `prompt-translator` **and** PIDs in `TranslatorPids`, both checks, in discovery and again in the pre-write re-check. Never inject into any other process. Injection runs in a short-lived helper process (D31). The text is written exactly as accepted, with control characters other than newline stripped. Leading `!`, `/`, `#` need confirmation (D35).
- **History:** local and unencrypted, with an off switch and Clear all.
- **Local model is text-only (owner rule, 2026-10-29):** the Ollama provider only generates text. The `/api/chat` request never includes `tools`, tool calls in a reply are ignored, and no setting or request field may give the model tools or let it execute anything. The Claude provider runs with `--tools ""` (T17b).
- **Webview permissions:** no shell, fs, http or sql plugin permissions for the webview. Only the app's own commands, `core:event` (listen/emit) and the window APIs each window needs. Clipboard, opener, dialog, global-shortcut and updater are used from Rust only. Capabilities in `src-tauri/capabilities/` list windows `main` and `popup` explicitly; the popup's window permissions (hide, show, setFocus) are in their own `popup.json`, scoped to `popup` only (changed in #58). A strict CSP is set in `tauri.conf.json`.
- **Process hygiene (Windows):** spawn `claude.exe` (native exe preferred over `.cmd`) with `CREATE_NO_WINDOW`; pass the system prompt as a file and the user text over stdin (avoids command-line length and quoting limits); set `DISABLE_AUTOUPDATER=1` and `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` (confirmed in T16), redirect `TEMP`/`TMP` to the app tmp dir, and remove the parent-session `CLAUDE_*` variables listed in the contract's `envRemove`, so translator runs never update the shared `claude.exe` or inherit a parent session; kill the **whole process tree** on timeout or cancel (Job Object, or `taskkill /T /F` fallback).
- **Performance:** Ollama Translate under 3 s for a typical prompt with a warm model on the dev machine (T13 measured 3.9 s median with `qwen3.5:9b` at `num_ctx` 8192, 3.3 s at 4096: accepted by the owner, changed in #45); UI stays responsive (all I/O async).
- **Portability:** Windows 10 22H2+ and 11, x64. No admin rights needed (per-user install; Ollama's installer is per-user too). Works without a dedicated GPU (small model tier) and without Claude Code.
- **Updates and data:** an update never deletes or breaks user data. Settings, presets and DB migrations are forward-compatible (`serde(default)`, ordered migrations).
- **Uninstall:** removes everything the app created (D24, D27). The only things left are Ollama and its models, which the user manages separately.

### 4.10 Verification

No automated tests (owner's decision, 2026-10-15). A ticket is done when build and lint pass and its acceptance criteria are met; behaviour criteria with example inputs and outputs are checked by hand (dev mode, the dev mock provider, or a scratch `main`).

- **Quality regression:** `cargo run --bin eval` (T15) runs `eval/cases.jsonl` through the real providers and writes a Markdown report. Run it manually when prompts or models change.
- **Manual smoke test:** `docs/smoke-test.md` checklist (T7), run before each release, on the dev machine **and in Windows Sandbox** using `docs/sandbox/no-gpu.wsb` (clean machine without winget or GPU: install, setup wizard, translate, update from the previous version, uninstall with and without data deletion, check that the folders and autostart entries are gone).
- **CI:** GitHub Actions on `windows-latest`: `npm ci`, lint, typecheck, **`npm run build`** (Tauri's `generate_context!` needs `frontendDist` to exist), then in `src-tauri`: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, then `npm run bindings` and `git diff --exit-code src/bindings` (generated bindings must be committed). No automated tests, and no network or LLM calls in CI.
- **Release:** `release.yml` runs on tag `v*`: the same checks, then `tauri-action` builds and publishes the release (T31).

## 5. Work breakdown

```
[Epic] E1 Project setup and delivery (#1)
├── [Story] S1.1 Developers can build and check the app (#10)
│   ├── [Task][cloud]    T0 Create the GitHub repo and license (#32)
│   ├── [Task][cloud]    T1 Install toolchain and document dev setup (#33)
│   ├── [Task][frontend] T2 Scaffold Tauri 2 + React + TS app (#34)
│   ├── [Task][backend]  T3 Rust core skeleton, AppError, ts-rs bindings (#35)
│   └── [Task][cloud]    T4 GitHub Actions CI for build and lint (#36)
└── [Story] S1.2 Users install, update and cleanly uninstall the app (#11)
    ├── [Task][assets]   T5 App icon set (#37)
    ├── [Task][cloud]    T6 NSIS installer and uninstall data cleanup (#38)
    ├── [Task][cloud]    T31 Release workflow and updater signing (#66)
    ├── [Task][backend]  T32 In-app updater backend (#67)
    ├── [Task][frontend] T33 About section and update UI (#68)
    ├── [Task][backend]  T46 Fix startup panic: no rustls crypto provider for reqwest (#138)
    ├── [Task][backend]  T56 Free the local model's memory, uninstall from the app, new layout-fix hotkey default (#146)
    └── [Task][docs]     T7 README, install guide and smoke-test checklist (#39)
[Epic] E2 Translation core (#2)
├── [Story] S2.1 Settings persist between runs (#12)
│   └── [Task][backend]  T8 Settings model and store (#40)
├── [Story] S2.2 Presets are editable JSON files (#13)
│   └── [Task][backend]  T9 Preset model, built-ins and PresetStore (#41)
├── [Story] S2.3 Prompts are built per mode and outputs are checked (#14)
│   ├── [Task][backend]  T10 Technical token checker (#42)
│   ├── [Task][backend]  T11 Prompt templates, builder and output parser (#43)
│   ├── [Task][backend]  T47 Validate model mistakes and fix Correct/Improve template failures (#142)
│   ├── [Task][backend]  T51 Improve: idiomatic rewrite with an explanation for each change (#149)
│   └── [Task][backend]  T52 Configurable language pair in the translation core (#150)
└── [Story] S2.4 One service runs any provider (#15)
    └── [Task][backend]  T12 TranslationService, provider trait, mock provider, translate/cancel commands (#44)
[Epic] E3 Local LLM provider (Ollama) (#3)
├── [Story] S3.1 A local model is chosen on evidence (#16)
│   ├── [Task][research] T13 Spike: evaluate local models and draft prompts (#45)
│   └── [Task][backend]  T44 Tune prompt templates for local models (#81)
├── [Story] S3.2 Translate with a local Ollama model (#17)
│   └── [Task][backend]  T14 Ollama provider and status (#46)
└── [Story] S3.3 Translation quality can be re-measured (#18)
    └── [Task][testing]  T15 Eval harness binary (#47)
[Epic] E4 Claude Code provider (#4)
├── [Story] S4.1 Claude translation runs are proven isolated (#19)
│   └── [Task][research] T16 Spike: isolated headless claude.exe invocation (#48)
└── [Story] S4.2 Translate with a separate Claude Code process (#20)
    ├── [Task][backend]  T17a Claude Code detection, status, translator-run registry and cleanup (#49)
    └── [Task][backend]  T17b Claude Code provider: spawn, parse, kill tree (#50)
[Epic] E5 Main window (#5)
├── [Story] S5.1 Translate, improve and correct from the main window (#21)
│   ├── [Task][design]   T18 UI design spec and Tailwind theme (#51)
│   ├── [Task][frontend] T19 App shell, API layer, error banner (#52)
│   ├── [Task][frontend] T20 Translate screen (#53)
│   ├── [Task][frontend] T21 Result details: diff, issues, token warnings (#54)
│   └── [Task][frontend] T49 Fix the Settings save bar gap and add a clear button to the Translate box (#144)
└── [Story] S5.2 Configure providers, defaults and presets (#22)
    ├── [Task][frontend] T22 Settings screen (#55)
    ├── [Task][frontend] T23 Preset editor (#56)
    └── [Task][frontend] T53 Language pair in setup, Settings and the Translate tab (#151)
[Epic] E6 Tray and hotkey popup (#6)
├── [Story] S6.1 The app lives in the tray (#23)
│   └── [Task][backend]  T24 Tray, close-to-tray, single instance, autostart (#57)
├── [Story] S6.2 Translate the clipboard from anywhere with a hotkey (#24)
│   ├── [Task][backend]  T25a Global hotkey and popup window (#58)
│   ├── [Task][backend]  T25b Auto-copy selection outside terminals (#59)
│   ├── [Task][backend]  T25c Mode detection (#60)
│   ├── [Task][frontend] T26 Popup UI (#61)
│   ├── [Task][backend]  T45 Auto-copy: treat VS Code's whole-line copy as no selection (#128)
│   ├── [Task][frontend] T50 Popup: keep each mode's result, movable, resizable, remembers size and position (#145)
│   ├── [Task][backend]  T54 Popup starting mode from where the text was selected (#152)
│   └── [Task][backend]  T55 Popup: Enter replaces the selection, Ctrl+Enter copies (#153)
└── [Story] S6.3 Fix text typed in the wrong keyboard layout with a hotkey (#76)
    ├── [Task][backend]  T41 Keyboard layout conversion module (#77)
    ├── [Task][backend]  T42 Layout-fix hotkey: capture, replace and switch layout (#78)
    ├── [Task][frontend] T43 Layout-fix settings (#79)
    └── [Task][backend]  T57 Layout fix: convert every wrong-layout word before the cursor (#147)
[Epic] E7 History and learning log (#7)
├── [Story] S7.1 Translations and mistakes are saved (#25)
│   ├── [Task][database] T27 SQLite schema, migrations and repository (#62)
│   └── [Task][backend]  T28 Wire history into the service and expose commands (#63)
├── [Story] S7.2 Review and search past translations (#26)
│   └── [Task][frontend] T29 History screen (#64)
└── [Story] S7.3 See my recurring mistakes and new vocabulary (#27)
    ├── [Task][frontend] T30 Mistakes screen (#65)
    └── [Task][backend]  T48 Count mistakes once per text and filter History by mode, category and period (#143)
[Epic] E8 First-run setup (#8)
├── [Story] S8.1 The app knows what this machine can run (#28)
│   └── [Task][backend]  T34 Environment check and model tier recommendation (#69)
└── [Story] S8.2 A new user is guided to a working provider (#29)
    ├── [Task][backend]  T35 Ollama install, start and model download (#70)
    └── [Task][frontend] T36 Setup wizard UI (#71)
[Epic] E9 Send prompts into an open Claude session (#9)   (revised, D47)
├── [Story] S9.1 Writing into a running session is proven possible (#30)
│   ├── [Task][research] T37 Spike: console input injection into claude.exe (#72)   (DROPPED, D47)
│   └── [Task][research] T58 Spike: paste into a bound terminal window and open VS Code sessions by link (#148)
└── [Story] S9.2 Bind a session and send accepted prompts to it (#31)
    ├── [Task][backend]  T38 Session targets: list, bind and watch Claude sessions (#73)
    ├── [Task][backend]  T39 Deliver accepted text into a bound Claude session (#74)
    └── [Task][frontend] T40 Translate tabs bound to Claude sessions (#75)
```

Totals: **9 epics, 23 stories, 59 tasks** (T37 dropped).

---

### T1. Install toolchain and document dev setup  `cloud` · parent: S1.1 · issue: #33
- **Goal:** the machine can build a Tauri 2 app, and the steps are documented.
- **Details:** install with `winget`: Node.js LTS, Rust via rustup (stable, `x86_64-pc-windows-msvc`), Visual Studio 2022 Build Tools with the "Desktop development with C++" workload. Confirm WebView2 is present. Don't install Ollama here (T13 does). Write `docs/dev-setup.md`: step-by-step install commands, how to verify, common errors (MSVC linker missing, PATH not refreshed). Check the requirements table in `docs/development.md` §1 against what was actually installed and correct the versions there; `dev-setup.md` links to it instead of repeating it. Ask the user before running installers that need admin rights.
- **Acceptance criteria:**
  - [ ] `node -v`, `npm -v`, `rustc -V`, `cargo -V` work in a new shell.
  - [ ] `docs/dev-setup.md` exists and a fresh reader can follow it; `docs/development.md` §1 matches the installed versions.
- **Depends on:** none. **Can run in parallel with:** T13, T16, T18.
- **Suggested model:** Sonnet 5.5 (confidence: High). Mechanical installs and a short doc.

### T0. Create the GitHub repo and license  `cloud` · parent: S1.1 · issue: #32
- **Goal:** the public repo `misstheaim/prompt-translator` exists with an MIT license, so CI, releases and the updater have a home.
- **Details:** **ask the user for approval first** (it's a remote change): create the public repo `misstheaim/prompt-translator` with `gh repo create` (description "Windows tray app that turns your prompts into good English for Claude Code, and teaches you while it does"), no auto-generated files. Locally: `git init` if needed, add `LICENSE` (MIT, copyright holder from `git config user.name`, year 2026), connect `origin`. Don't commit or push unless the user asks.
- **Acceptance criteria:**
  - [ ] Repo exists and is public; `origin` is set locally; `LICENSE` exists.
- **Depends on:** none. **Can run in parallel with:** T1, T13, T16, T18, T37.
- **Suggested model:** Haiku 5.5 (confidence: High). Two commands and a file.

### T2. Scaffold Tauri 2 + React + TS app  `frontend` · parent: S1.1 · issue: #34
- **Goal:** an empty app that runs with `npm run tauri dev`, with the frontend tooling configured.
- **Details:** **The project root already contains files (the spec, `docs/`, possibly `eval/`, `spikes/`). Never delete or overwrite existing files.** Run `npm create tauri-app@latest` (React + TS + Vite) into a **temp subfolder** (e.g. `_scaffold/`), then move its contents into the root, stopping if any file would be overwritten, and delete `_scaffold/`. App name "Prompt Translator", identifier `com.misstheaim.prompttranslator`. Pin exact versions of Tauri CLI/API (no `^`). Add Tailwind CSS v4 (Vite plugin), Zustand, ESLint (typescript-eslint, react-hooks), Prettier. npm scripts: `dev`, `build`, `lint`, `typecheck`, `format`. Add a second Vite entry `popup.html` → `src/popup/main.tsx` (placeholder) for T25a/T26. `.gitignore` for Node, Rust `target/`, `.env`, `eval/reports/`. The repo already has `README.md`, `LICENSE`, `CLAUDE.md` and `docs/`: keep them. **This is the bootstrap task:** create the npm scripts with the names planned in `docs/development.md` §2, and update that table and `CLAUDE.md` wherever a command ends up different.
- **Acceptance criteria:**
  - [ ] `npm run tauri dev` opens a window with a Tailwind-styled placeholder.
  - [ ] `npm run lint`, `npm run typecheck` and `npm run build` pass.
  - [ ] `popup.html` builds as a second entry.
  - [ ] Every file that existed before the task is unchanged, except `docs/development.md` and `CLAUDE.md` where commands differ.
- [ ] Every frontend command in `docs/development.md` §2 exists and runs.
- **Depends on:** T1. **Can run in parallel with:** T13, T16, T18.
- **Suggested model:** Sonnet 5.5 (confidence: High). Standard scaffolding.

### T3. Rust core skeleton, AppError, ts-rs bindings  `backend` · parent: S1.1 · issue: #35
- **Goal:** the Rust module layout from §4.2 exists, with a shared error type and generated TS bindings.
- **Details:**
  - Create the module folders from §4.2 as empty modules, following the conventions there (per-module `commands.rs`, one registration line per module in `lib.rs`).
  - `error.rs`: `AppError` enum with the kinds in §4.6 (`thiserror`), `Serialize` as `{ kind, message, hint, detail? }`, `type AppResult<T>`.
  - Add deps (exact versions): `serde`, `serde_json`, `tokio`, `thiserror`, `ts-rs`, `uuid`, `tauri-plugin-log`.
  - Put the `Mode`/`ProviderKind`/`TranslationRequest`/`TranslationResult`/`Issue`/`IssueCategory`/`TokenWarning`/`TokenKind` types from §4.3 in `translation/model.rs` with `#[derive(TS)] #[ts(export)]`. Set the export dir with `TS_RS_EXPORT_DIR` in `src-tauri/.cargo/config.toml` pointing at `../src/bindings`. Use serde `camelCase` (enums `rename_all = "camelCase"`, IssueCategory `snake_case` to match the LLM schema). Mark `u64`/`i64` fields `#[ts(type = "number")]`.
  - **`system/paths.rs`**: the single source of every app path (D14, D27): `config_dir()` (= `app_config_dir()`), `local_dir()` (= `app_local_data_dir()`), `history_db()`, `logs_dir()`, `claude_cwd()`, `tmp_dir()`, `presets_dir()`. Creates dirs on first use. Never use `app_data_dir()` (Roaming on Windows).
  - **Logging**: `tauri-plugin-log` writing to `logs_dir()` (rotating, max 5 × 1 MB) and stdout in dev.
  - **Capabilities**: `src-tauri/capabilities/default.json` for windows `["main", "popup"]` with only `core:default`/`core:event` and the window permissions each window needs. Set a strict `app.security.csp` in `tauri.conf.json` (`default-src 'self'; style-src 'self' 'unsafe-inline'` as needed by Tailwind; `connect-src ipc: http://ipc.localhost`).
  - Add a `ping` command and call it from the frontend placeholder to prove the round trip.
  - ts-rs exports the bindings through the `export_bindings_*` tests it generates; add an npm script `bindings` that runs `cargo test export_bindings` (the only use of `cargo test`; no hand-written tests).
  - Configure `rustfmt.toml` and clippy.
- **Acceptance criteria:**
  - [ ] `npm run bindings` writes `src/bindings/*.ts`; `u64` fields come out as `number`.
  - [ ] `AppError` serializes to the `{kind,message,hint}` shape.
  - [ ] Every `paths.rs` path is under the config or local dir.
  - [ ] A log line from startup appears in the log file.
  - [ ] `cargo clippy -- -D warnings` is clean.
  - [ ] The placeholder UI shows the `ping` result.
- **Depends on:** T2. **Can run in parallel with:** T5.
- **Suggested model:** Sonnet 5.5 (confidence: High). Clear contract, boilerplate.

### T4. GitHub Actions CI for build and lint  `cloud` · parent: S1.1 · issue: #36
- **Goal:** every release build (PR or push into `release/**` or `main`) runs the checks in §4.10.
- **Details:** `.github/workflows/ci.yml` on `windows-latest`, triggered by `pull_request` into `release/**` and `main` and by `push` to those branches, release builds only (changed 2026-10-27; `docs/development.md` §9), `permissions: contents: read`, actions pinned to commit SHAs. Steps:
  1. checkout;
  2. setup-node (LTS, npm cache);
  3. `dtolnay/rust-toolchain@stable` with clippy and rustfmt;
  4. `Swatinem/rust-cache` (workspace `src-tauri`);
  5. `npm ci`; `npm run lint`; `npm run typecheck`;
  6. **`npm run build`**, because `generate_context!` needs `frontendDist` to exist before any cargo step compiles;
  7. with `working-directory: src-tauri`: `cargo fmt --check`; `cargo clippy --all-targets --locked -- -D warnings`;
  8. `npm run bindings`, then `git diff --exit-code src/bindings` (bindings must be committed).

  No secrets, no automated tests and no network calls. Pushing the workflow follows the user's push rule.
- **Acceptance criteria:**
  - [ ] The workflow runs and passes on this task's PR into `develop`.
  - [ ] A deliberate lint error makes CI fail (check locally with `act`, or reason it through and note it in the PR).
- **Depends on:** T0, T2, T3. **Can run in parallel with:** T8, T9, T10, T27.
- **Suggested model:** Sonnet 5.5 (confidence: High). Standard workflow.

### T5. App icon set  `assets` · parent: S1.2 · issue: #37
- **Goal:** the app has its own icon for the window, tray and installer.
- **Details:** create a simple 1024×1024 SVG/PNG source (`assets/icon.svg`): speech bubble with "EN", flat and legible at 16 px. Generate the set with `npm run tauri icon assets/icon.png`. Also add a monochrome 32×32 tray variant `src-tauri/icons/tray.png` for T24.
- **Acceptance criteria:**
  - [ ] `src-tauri/icons/` holds the generated set; the dev window shows the icon.
  - [ ] Tray icon file exists and is legible on light and dark taskbars.
- **Depends on:** T2. **Can run in parallel with:** T3.
- **Suggested model:** Haiku 5.5 (confidence: Medium). Mechanical; the SVG design may need one iteration.

### T6. NSIS installer and uninstall data cleanup  `cloud` · parent: S1.2 · issue: #38
- **Goal:** `npm run tauri build` produces a per-user Windows installer whose uninstaller removes all app data by default.
- **Details:**
  - In `tauri.conf.json`: bundle target `nsis`; product name; version `0.1.0`; `resources` include `resources/presets/*.json`; NSIS `installMode: "currentUser"`; WebView2 `downloadBootstrapper`; Start menu shortcut; installer language English.
  - Release build has no console window (`windows_subsystem = "windows"`).
  - **Uninstall (D24, D27):** add `src-tauri/windows/hooks.nsh` via `bundle.windows.nsis.installerHooks`. When **not** in update mode, it always deletes the autostart entries: the value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` and, if present, under `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`. Use the exact value name that `tauri-plugin-autostart` writes (check it in T24's code and the plugin source). **Changed in #57:** the value name is `Prompt Translator` (set explicitly), data `<exe path> --autostart` **unquoted**; the plugin sets the StartupApproved value only when that key exists and never removes it. If the install folder contains a space, check autostart after a sign-out/sign-in in the Sandbox test; if it fails, quote the path or use a folder without spaces.
  - Use Tauri's built-in "Delete application data" checkbox if it removes both `%APPDATA%\com.misstheaim.prompttranslator` and `%LOCALAPPDATA%\com.misstheaim.prompttranslator` (including the WebView2 `EBWebView` folder). Make the checkbox **checked by default**. If hooks can't change the default, use a minimal custom NSIS template (`bundle.windows.nsis.template`, copied from the Tauri version in use) and document why.
  - Never delete data in update mode, and never touch Ollama.
  - Check that `system/paths.rs` (D27) puts every app-created file under those two folders.
  - Add `docs/sandbox/no-gpu.wsb` (`<vGPU>Disable</vGPU>`, networking on, the `src-tauri/target/release/bundle/nsis` folder mapped read-only) for clean-machine tests. Note that Sandbox has no winget.
  - Write `docs/build.md`: build command, output path, data locations, what install, update and uninstall do, and how to use the Sandbox config.
- **Acceptance criteria:**
  - [ ] In Windows Sandbox: install works without admin; the app starts from the Start menu; presets are seeded.
  - [ ] Uninstall with the default (checked) removes both data folders and the autostart entries. Uninstall with it unchecked keeps data but still removes the autostart entries.
  - [ ] Installing a newer version over an older one keeps all data.
  - [ ] No console window appears at runtime.
- **Depends on:** T5, T9, T24. **Can run in parallel with:** T34, T35.
- **Suggested model:** Opus 5.5 (confidence: Medium). NSIS hooks and template behaviour across Tauri versions needs investigation; Sandbox testing is fiddly.

### T7. README, install guide and smoke-test checklist  `docs` · parent: S1.2 · issue: #39
- **Goal:** a new user can install and use the app on their own machine, and the owner can check a release before publishing it.
- **Details:**
  - `README.md` for **end users first**:
    - what the app does (English practice; only English reaches Claude);
    - download from GitHub Releases;
    - the **SmartScreen warning** ("More info → Run anyway") and why it appears (unsigned);
    - the setup wizard;
    - the three modes;
    - the hotkey flow;
    - the privacy model (§4.9), stating plainly that Claude mode sends the text to Anthropic and that auto-copy can capture anything selected;
    - updates;
    - uninstall and what it removes (Ollama and models stay; how to remove them with `ollama rm` or Windows Apps);
    - troubleshooting (Ollama not running, model missing, Claude not signed in, hotkey taken);
    - a developer section linking to `docs/development.md` (workflow and commands), `docs/dev-setup.md`, `docs/build.md` and `docs/release.md`. It replaces the current stub `README.md`.
  - `docs/smoke-test.md`: a step checklist covering:
    - each mode × provider, in a non-Russian language too;
    - the popup flow, token warning, cancel;
    - Ollama stopped; claude.exe path wrong;
    - history off/on; clear history;
    - tray behaviour;
    - auto-copy from a browser vs a terminal;
    - keyboard layout fix: selection and last word in an editor and a browser, the terminal clipboard flow, input-language switch;
    - if E9 shipped: bind a session, send from the main window and the popup, busy queueing, a closed session disables Accept, unbinding returns to copy;
    - a **Windows Sandbox section** using `docs/sandbox/no-gpu.wsb`: install, wizard with no GPU and no winget (download-page fallback), wizard skipping both providers, update from the previous release, uninstall with and without data deletion, and a check that the folders and autostart entries are gone.
- **Acceptance criteria:**
  - [ ] README covers every item above; every command in it was run once.
  - [ ] Running the smoke test on the final build, including the Sandbox section, passes (result noted in the PR).
- **Depends on:** T6, T21, T22, T23, T25b, T26, T29, T30, T31, T33, T36, T43, T44, and T40 if E9 is GO. **Can run in parallel with:** none (last).
- **Suggested model:** Sonnet 5.5 (confidence: High). Docs from a finished app.

### T8. Settings model and store  `backend` · parent: S2.1 · issue: #40
- **Goal:** settings load and save as `settings.json` with safe defaults.
- **Details:** `settings/` module: `Settings` struct exactly as in §4.3, `#[serde(default)]` on every field, exported via ts-rs. `SettingsStore` in Tauri managed state (`RwLock<Settings>`), path = `paths::config_dir()/settings.json`. It must also be constructible without a Tauri `AppHandle` (from a plain path) so T12/T15 can use it. Load on startup: missing → defaults; corrupt → rename to `settings.json.bak`, log, use defaults. Save writes atomically (temp file + rename). Validate on save: endpoint is a URL, timeouts 5–600 s, `numCtx` 2048–32768, hotkey non-empty. Commands `get_settings`, `save_settings`. Emit a `settings://changed` event after save so T24/T25a can react.
- **Acceptance criteria:**
  - [ ] Defaults when the file is missing; a partial file merges with defaults; a corrupt file is backed up; validation errors map to `AppError::InvalidInput`.
  - [ ] Values round-trip through the commands from the UI placeholder (manual).
- **Depends on:** T3. **Can run in parallel with:** T9, T10, T27.
- **Suggested model:** Sonnet 5.5 (confidence: High). Self-contained.

### T9. Preset model, built-ins and PresetStore  `backend` · parent: S2.2 · issue: #41
- **Goal:** presets live as editable JSON files with four built-ins.
- **Details:** `Preset` struct per §4.3 (ts-rs). Write the 4 built-in files in `src-tauri/resources/presets/` (`general`, `technical-development`, `code-review`, `work-communication`), each with 4–8 sensible rules and a `defaultMode` (technical-development: improve; code-review: improve; work-communication: correct; general: translate). `PresetStore` (constructible from plain paths, without an `AppHandle`): on startup, if `paths::presets_dir()` is missing, copy the bundled ones in. `list` (sorted by name), `get(id)`, `save` (validate id slug `^[a-z0-9-]+$`, name non-empty, ≤ 30 rules, each ≤ 500 chars), `delete` (refuses the last remaining preset), `reset` (restores built-ins and keeps user-created ones). Commands per §4.6.
- **Acceptance criteria:**
  - [ ] Seeding, list, save new, edit built-in, delete, delete-last refused and reset work; an invalid file is skipped with a logged warning.
  - [ ] Built-in rule texts don't contradict the mode templates in §4.4.
- **Depends on:** T3. **Can run in parallel with:** T8, T10, T27.
- **Suggested model:** Sonnet 5.5 (confidence: High). Clear CRUD.

### T10. Technical token checker  `backend` · parent: S2.3 · issue: #42
- **Goal:** a pure function finds technical tokens in the source that are missing from the output.
- **Details:** `translation/token_check.rs`: `fn check_tokens(source: &str, output: &str) -> Vec<TokenWarning>`, rules in §4.5. Deduplicate tokens. Use the `regex` crate with patterns compiled once (`LazyLock`). Must not flag plain words, misspellings, or Russian words.
- **Acceptance criteria:**
  - [ ] `PD-3693`, `#42`, `MigrationQueue`, `GetStudentData`, `get_student_data`, `MAX_RETRIES`, `appsettings.json`, `src/app/main.rs`, `C:\Users\x\file.txt`, `https://x.y/z?q=1`, `` `foo()` `` are detected; `MigrationQueue → Migration Queue` gives a warning; "undesrand", "Hello", "проверь" are not tokens; tokens kept verbatim give no warnings.
- **Depends on:** T3. **Can run in parallel with:** T8, T9, T27.
- **Suggested model:** Sonnet 5.5 (confidence: High). Pure function with a clear spec.

### T11. Prompt templates, builder and output parser  `backend` · parent: S2.3 · issue: #43
- **Goal:** each mode has a defined system prompt, and model output parses into `output` + `issues`.
- **Details:** put the final templates in `translation/prompts/{translate,improve,correct}.md`, following §4.4. Write them provider-neutral from the §4.4 rules; they don't wait for the local model spike (D40). T44 tunes them for local models after T13. `prompt_builder.rs`: `fn build(mode, preset, source) -> BuiltPrompt { system: String, user: String }` per §4.4, plus `OUTPUT_SCHEMA: serde_json::Value` (§4.3). `output_parser.rs`: `fn parse(raw: &str) -> AppResult<ParsedOutput>` with the rules in §4.3 (fence stripping, unknown category → Other, empty → EmptyOutput, invalid → MalformedOutput with raw in detail). It also accepts an already-parsed `serde_json::Value` (Claude's structured output). Translate mode drops any issues the model returns. Check that a built prompt for an 8000-character input fits in the default `numCtx` (8192 tokens) with room for the output; document the estimate in a code comment.
- **Acceptance criteria:**
  - [ ] The built prompt for each mode with a sample preset is pasted into the PR for review.
  - [ ] The parser handles valid JSON, fenced JSON, leading prose before JSON, unknown category, empty output and garbage as §4.3 describes.
  - [ ] Templates include the "data, not a request" rule and the shared rules.
- **Depends on:** T3, T9. **Can run in parallel with:** T10.
- **Suggested model:** Opus 5.5 (confidence: Medium). Prompt wording drives quality and there's no local-model evidence yet, so judgment is needed.

### T12. TranslationService, provider trait, mock provider, translate/cancel commands  `backend` · parent: S2.4 · issue: #44
- **Goal:** one service runs a request through any provider, with cancel, timeout and token check.
- **Details:** `translation/service.rs`:
  ```rust
  #[async_trait] trait TranslationProvider: Send + Sync {
      fn kind(&self) -> ProviderKind;
      async fn complete(&self, prompt: &BuiltPrompt, cancel: CancellationToken) -> AppResult<ProviderOutput /* { raw: String, model: String } */>;
  }
  ```
  `TranslationService::translate(req)`: validate (non-empty, ≤ 8000 chars) → load preset (T9) and settings (T8) → `PromptBuilder::build` → provider by kind (registry in managed state) → `tokio::time::timeout` with the provider's timeout setting → `output_parser::parse` → `check_tokens` → result with `duration_ms`. A cancellation registry `Mutex<HashMap<request_id, CancellationToken>>`: `cancel_translation` triggers it, and the token is always removed when the request finishes. `providers/mock.rs`: returns canned output per mode after 800 ms; only used when `ProviderKind::Mock` and (`cfg!(debug_assertions)` or `settings.devMockProvider`). Leave a clearly marked hook point `on_result` for history (T28). **The service must be constructible without a Tauri `AppHandle`** (plain settings + preset dir + providers), so the eval binary (T15) can use it; Tauri state only wraps it. Commands `translate`, `cancel_translation`, and **`copy_result`**: writes the text to the clipboard with `tauri-plugin-clipboard-manager` from Rust (register the plugin here); T28 adds the history marking.
- **Acceptance criteria:**
  - [ ] With the mock provider: success per mode; cancel returns `Cancelled`; timeout returns `Timeout`; malformed output is mapped; token warnings appear; the registry is empty after completion.
  - [ ] The service builds without an `AppHandle`.
  - [ ] The UI placeholder can call `translate` with the Mock provider, and `copy_result` puts text on the clipboard (manual).
- **Depends on:** T8, T10, T11. **Can run in parallel with:** T27.
- **Suggested model:** Opus 5.5 (confidence: High). Core async design with cancellation, which later tasks build on.

### T13. Spike: evaluate local models and draft prompts  `research` · parent: S3.1 · issue: #45
- **Goal:** pick the default Ollama model and prove the prompt approach works on this machine.
- **Details:** install Ollama (`winget install Ollama.Ollama`; ask the user first). Build `eval/cases.jsonl` with about 25 cases, fields `{ id, mode, source, mustKeep: string[], notes }`. Cover: Russian dev requests; mixed RU/EN; 3–4 cases in other languages (e.g. Ukrainian, German, Spanish) for other users; English with typical learner errors (articles, tenses, prepositions, typos like "undesrand", "analize"); prompts with identifiers, ticket IDs and paths; **question-like prompts that tempt the model to answer** ("почему этот метод возвращает null?"); long multi-paragraph input. Ask the user for 10+ real prompts of their own. Use the templates in `translation/prompts/` (T11) as the baseline if T11 has merged; otherwise draft the three mode templates (§4.4) in `eval/prompts/`. Try template variants only in `eval/prompts/`; applying them to the app is T44. Write a throwaway script (`eval/spike_eval.py`, Python 3 stdlib only, calls `/api/chat` with `format` schema, `temperature: 0.2`, `keep_alive: "30m"`, **`num_ctx: 8192`**; measure the VRAM cost of 8192 vs 4096 per model and recommend the default). Compare 3 candidates that fit 8 GB VRAM (e.g. current Qwen 7–9B instruct, Gemma 12B-class at Q4, one smaller 3–4B model; check the Ollama library for the best current options). Record per model: valid-JSON rate, token preservation rate, "answered instead of translated" count, cold and warm latency, VRAM use, and a human quality rating per mode. Also measure the small model **CPU-only** (`options.num_gpu: 0`) to simulate machines without a GPU.

Write `docs/spikes/local-model-eval.md` with:
  - the results;
  - the recommended default model;
  - recommended template changes, with the failing cases each one fixes (applied by T44);
  - whether Correct mode needs Claude as the recommended provider;
  - a **model tier table** for T34: tier → Ollama model tag, download size, expected latency per mode. Tiers:
    - `cpu-low`: no usable GPU, RAM under 12 GB (about 1–2B models);
    - `cpu`: no usable GPU, RAM 12 GB or more (about 3–4B);
    - `gpu-small`: dedicated GPU under 6 GB VRAM;
    - `gpu-medium`: 6–10 GB;
    - `gpu-large`: over 10 GB.

    Measure both CPU tiers CPU-only (`num_gpu: 0`) on the dev machine and note that a typical laptop CPU will be slower. For each tier, state whether Correct mode is good enough locally or whether Claude Code should be the recommended provider.

  **Changed in #45 (owner decision 2026-10-29):** one model only (`qwen3.5:9b`, Q4_K_M, 6.6 GB), measured on the dev RTX 4060 (8 GB); no CPU tiers and no CPU-only runs: machines without a usable GPU use Claude Code, not Ollama. The tier table has `gpu-small` (`qwen3.5:4b`, estimate), `gpu-medium` (`qwen3.5:9b`, measured), `gpu-large` (`qwen3.5:9b`, estimate) and a "no usable GPU → Claude Code" row. 13 of the owner's own English prompts (from earlier Claude sessions, owner-approved) are in the case set (35 cases). Results: `docs/spikes/local-model-eval.md`.
- **Acceptance criteria:**
  - [x] Report contains a results table for 1 model × 3 modes (changed in #45), a clear recommendation and the tier table.
  - [ ] `eval/cases.jsonl` and the draft templates are committed-ready (reused by T44 and T15).
- **Depends on:** none (needs Ollama installed); runs in wave 4 (D40). **Can run in parallel with:** T12, T17a, T19, T24, T28.
- **Suggested model:** Opus 5.5 (confidence: Medium). Judging translation and explanation quality needs a strong model; fall back to Fable 5.1 if the results are ambiguous.

### T14. Ollama provider and status  `backend` · parent: S3.2 · issue: #46
- **Goal:** translations run on the configured local Ollama model.
- **Details:** `providers/ollama.rs` with `reqwest`. `complete`: `POST {endpoint}/api/chat` with `{ model, messages: [system, user], stream: false, format: OUTPUT_SCHEMA, keep_alive, options: { temperature: 0.2, num_ctx: settings.ollama.numCtx } }`. **Always send `num_ctx`**: Ollama's default context is smaller than the input limit plus prompt, and it silently truncates the oldest tokens (the system prompt). Return `message.content` as raw. Cancellation: `tokio::select!` on the request future and `cancel.cancelled()`. Error mapping: connection refused → `OllamaUnreachable`; 404 / "model not found" body → `ModelMissing`; other HTTP errors → `Io` with body excerpt. Status: `GET /api/tags` with a 2 s timeout → `{ reachable, models, selectedModelInstalled }`, used by the `provider_status` command (create the command here with `claude: null`-safe fields; T17a fills the Claude part). The `OllamaUnreachable` error's **Start Ollama** button calls `ollama_start` (T35); until T35 lands, the UI shows only the hint. Default model in `Settings` = the T13 recommendation.
  **Changed in #46 (from T13):** default model `qwen3.5:9b`; the request also sends `think: false` (Qwen 3.5 is 2-3x slower when it thinks first); `numCtx` default stays 8192, and a prompt whose estimated size (system + user + answer reserve per mode) exceeds `numCtx` is refused with `InvalidInput` instead of being sent. `reqwest` is built without TLS (Ollama is http on localhost). The `format` schema is per mode since T44 (`output_schema(mode)`).
- **Acceptance criteria:**
  - [ ] Success, unreachable, model missing, 500 and cancel mid-request map to the right results; the request body contains `options.num_ctx` and `format`.
  - [ ] Manual: real translation via the UI placeholder or the `translate` command with Ollama running.
- **Depends on:** T12, T13. **Can run in parallel with:** T17a, T17b, T19, T24, T28.
- **Suggested model:** Sonnet 5.5 (confidence: High). Well-defined HTTP client.

### T15. Eval harness binary  `testing` · parent: S3.3 · issue: #47
- **Goal:** `cargo run --bin eval` re-measures quality with the real pipeline.
- **Details:** `src-tauri/src/bin/eval.rs`. Args: `--provider ollama|claude-code`, `--model <name>`, `--cases eval/cases.jsonl`, `--out eval/reports/<timestamp>.md`. Runs each case through `TranslationService` (no Tauri runtime: construct the service with plain settings and a preset dir). Report: per case source, output, issues, token warnings, `mustKeep` misses, duration; summary: JSON-valid rate, `mustKeep` pass rate, average/p95 latency. Add `eval/reports/` to `.gitignore`. Document usage in `eval/README.md`.
- **Acceptance criteria:**
  - [ ] Runs end to end against Ollama and writes a report.
  - [ ] Works with `--provider claude-code` once T17b is done (if T17b lands later, note it in the PR and leave a follow-up).
  - [ ] `cargo run` (no `--bin`) still starts the app: `default-run` is set in `Cargo.toml`.
- **Depends on:** T14. **Can run in parallel with:** T20–T23.
- **Suggested model:** Sonnet 5.5 (confidence: High). The service is already decoupled from Tauri (T12).

### T16. Spike: isolated headless claude.exe invocation  `research` · parent: S4.1 · issue: #48
- **Goal:** find and document the exact `claude` invocation that translates with **no** user/project CLAUDE.md, hooks, plugins, skills, MCP servers, memory or tools, using the user's normal subscription.
- **Outcome (2026-10-24, D41):** candidate 1 adopted (`--safe-mode` + `--setting-sources ""`, normal login, no dedicated config dir); results in `docs/spikes/claude-isolation.md`, contract in `docs/spikes/claude-invocation.json`. Translator runs show in `claude agents --json` as `kind: "interactive"` while running, so T17a/T38/T39 exclude them. Candidate 2 was not pursued.
- **Details:** **Never edit the user's real `~/.claude` or any real project.** Work in temp dirs only. Candidates to test, in this order, alone and combined:
  1. **`--safe-mode`** with the user's normal login (first choice: no extra sign-in): `claude -p --safe-mode --no-session-persistence --tools "" --strict-mcp-config --disable-slash-commands --model haiku --output-format json --system-prompt-file <file> --json-schema <schema>` from one fixed empty cwd. Read its full `--help` text (including the admin-managed policy note) and check that the user's global CLAUDE.md, hooks, plugins, skills and MCP servers really don't load.
  2. A dedicated config dir (fallback): `CLAUDE_CONFIG_DIR=%LOCALAPPDATA%\com.misstheaim.prompttranslator\claude-config` with the same flags. Check whether it needs its own `claude` login (and whether that login works with the subscription) and whether anything from `~/.claude` still loads.
  3. Variants: `--setting-sources ""`, `--restricted`.
  4. Executable: confirm the native `claude.exe` path (`%USERPROFILE%\.local\bin\claude.exe`), and note how an npm-installed `claude.cmd` behaves (Rust's batch-file argument escaping rules).
  5. **Side effects:** confirm the env var names that disable the auto-updater and nonessential traffic (expected `DISABLE_AUTOUPDATER=1`, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`), and that 50 translator runs leave the user's `claude.exe` unchanged (hash before and after).
  6. **Output:** where `--json-schema` puts the structured result in the `--output-format json` envelope (field name), and how schema violations are reported.
  Leak tests with canaries: put `CLAUDE.md` files containing a unique canary instruction (e.g. "Always end replies with the word PINEAPPLE-7") in the temp cwd and in the isolated config dir; add a canary hook in the isolated config's settings. Then ask the model a neutral translation and check for the canary. To test whether the **real** global `~/.claude/CLAUDE.md` leaks without editing it, ask the model to summarise any instructions it has besides the system prompt, and look for distinctive phrases from that file. Also inspect `--debug-file` output for loaded files. Also measure: cold latency per call, output envelope fields (`result`, `is_error`, …), the exact error output when not signed in, the stdin prompt behaviour, the Windows console window (spawn with `CREATE_NO_WINDOW`). If nothing works under subscription auth, document the fallbacks: (a) `--bare` + `ANTHROPIC_API_KEY`; (b) accept leakage of global CLAUDE.md, listing what leaks.
  **Clutter check:**
  - run 50 translations with the recommended invocation and a fixed cwd;
  - list every file and folder created or changed in the isolated config dir, the cwd, `%TEMP%` and the user's real `~/.claude` (projects/transcripts, `.claude.json` project entries, todos, shell snapshots, file history, debug logs). The user's `~/.claude` must be **unchanged**;
  - confirm that `--no-session-persistence` leaves nothing that `claude --resume` would list;
  - while a translation runs, check whether it appears in `claude agents --json` (with and without the isolated config dir) and what its `kind` is;
  - write down a cleanup recipe: which paths in the isolated config dir can be deleted periodically without losing the sign-in.

  Output:
  - `docs/spikes/claude-isolation.md` with a results table, the clutter findings and cleanup recipe, the **recommended invocation** (env vars, args, cwd), whether a separate sign-in is needed (and its procedure if so), and known limits;
  - **`docs/spikes/claude-invocation.json`**: a machine-checkable contract `{ claudeVersion, exe, args[], env{}, cwd, stdin, envelope: { resultField, errorField, structuredField } }`. T17b builds its invocation from it; after a Claude Code upgrade, re-running the spike checks only diff this file.
- **Acceptance criteria:**
  - [ ] Each candidate has documented results, with canary evidence.
  - [ ] A single recommended invocation is given, or a clear statement that full isolation isn't possible plus the user's options.
  - [ ] The user's real Claude config is unchanged (state this explicitly in the report).
  - [ ] Clutter check done for 50 runs, with a cleanup recipe; nothing appears in `claude --resume`.
- **Depends on:** none. **Can run in parallel with:** T1–T13, T18.
- **Suggested model:** Opus 5.5 (confidence: Medium). Exploratory, version-dependent CLI behaviour; fall back to Fable 5.1 if results conflict.

### T17a. Claude Code detection, status, translator-run registry and cleanup  `backend` · parent: S4.2 · issue: #49
- **Goal:** the app finds Claude Code, reports its status, and has the shared pieces the provider and session picker need.
- **Details:** `providers/claude_code/detect.rs` and `.../housekeeping.rs`, following `docs/spikes/claude-isolation.md` and `claude-invocation.json` (T16).
  - **Executable detection order:** `settings.claude.executablePath`; `%USERPROFILE%\.local\bin\claude.exe`; `claude.exe` on `PATH`; only then `claude.cmd` on `PATH` (logged as limited: no newlines or quotes in args). Exposed as `fn find_claude() -> Option<ClaudeExe { path, kind: Native | CmdShim }>` for T17b, T34 and T38.
  - **Status** for `provider_status.claude`: `found`, `path`, `version` (`claude --version`, 5 s timeout, `CREATE_NO_WINDOW`), `signedIn` (`claude auth status`: exit 0 = signed in, exit 1 = not, anything else or a timeout = `null`; no API call; **never log its output**, it contains the e-mail address).
  - **`TranslatorPids`** (the translator-run registry): a shared set in managed state that T17b fills while translator runs are alive. T38 and T39 read it, together with the name `prompt-translator`, to exclude the app's own runs from session discovery and injection (D41: they are listed by `claude agents --json` as interactive while running). Expose a read helper such as `is_translator_run(pid, name) -> bool`.
  - **Housekeeping:** create the fixed cwd `paths::claude_cwd()` and `paths::tmp_dir()` (D27). On startup and after every 50 runs, apply the spike's cleanup recipe (`claude-cwd` must stay empty; clear old files in the app tmp dir, such as system-prompt files).
  - No `claude_open_login` command and no translator sign-in (D41): the translator uses the user's normal login; when not signed in the UI says to run `claude` once.
  - Register `tauri-plugin-dialog` and add a `pick_file` command (§4.6) for the Settings Browse button.
- **Acceptance criteria:**
  - [ ] Detection order: settings path wins, native beats `.cmd`, none found; the cleanup recipe deletes only the listed paths (checked on a temp copy).
  - [ ] Manual: status shows the right path and version on the dev machine.
- **Depends on:** T12, T16. **Can run in parallel with:** T14, T19, T24, T28.
- **Suggested model:** Sonnet 5.5 (confidence: High). Clear rules from the spike.

### T17b. Claude Code provider: spawn, parse, kill tree  `backend` · parent: S4.2 · issue: #50
- **Goal:** translations run in a separate, isolated, short-lived `claude` process.
- **Details:** `providers/claude_code/provider.rs` implementing `TranslationProvider` with the exact invocation in `docs/spikes/claude-invocation.json`.
  - Spawn the exe from `find_claude()` (T17a) with `tokio::process::Command`, `creation_flags(CREATE_NO_WINDOW)`, cwd = `paths::claude_cwd()` (one fixed, reused, empty dir under the app data dir), and build args and env exactly from the contract (D41): the args list (`--safe-mode`, `--setting-sources ""`, `--no-session-persistence`, `--tools ""`, `--strict-mcp-config`, `--disable-slash-commands`, `--name prompt-translator`, `--model`, `--output-format json`, ...), the added env (auto-updater and nonessential-traffic switches, `TEMP`/`TMP` redirected to `paths::tmp_dir()`), and the explicit `envRemove` list (not a `CLAUDE_CODE_*` prefix sweep).
  - Write the system prompt to a UTF-8 file under `paths::tmp_dir()` and pass it with `--system-prompt-file`. Pass `OUTPUT_SCHEMA` **inline** (minified JSON string) with `--json-schema`; a file path is rejected. Send the user message over **stdin** as UTF-8 and close it; stdin must be non-empty.
  - Add the PID to `TranslatorPids` while it runs and remove it when the process is gone (T17a); this registry plus the `prompt-translator` name is how session discovery excludes the run (D41).
  - Put the process in a Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) so cancel or timeout kills the whole tree (fallback `taskkill /T /F /PID`).
  - Parse the `--output-format json` envelope using the contract's field names: `is_error: true` or the `Not logged in` result → `ClaudeAuth` (exit 1 with a stdout envelope); exit 1 with empty stdout and an `Error: ...` stderr line (bad schema, missing prompt file, empty stdin) → `ProcessCrashed` with a stderr excerpt (≤ 500 chars), as for any other non-zero exit; exit 0 with `structured_output` → the parsed output; exit 0 **without** `structured_output` (the model answered in free text) → `MalformedOutput`, with no fallback to `result` through `output_parser`.
  - **Changed in #50:** an `is_error` envelope maps to `ClaudeAuth` only when its text is a sign-in problem ("Not logged in", "/login", invalid API key, authentication, OAuth token); other error envelopes (usage limits, API errors) → `ProcessCrashed` with the error text, and a `structured_output` error subtype → `MalformedOutput`. The process starts suspended, joins the Job Object, then resumes; the job is closed after every run. A blank `settings.claude.model` falls back to `haiku`.
- **Acceptance criteria:**
  - [ ] Success, auth error, crash, timeout (process tree gone, including the child) and cancel map to the right results; the spawned args match the contract.
  - [ ] Success reads `structured_output`; exit 0 without it gives `MalformedOutput`; the schema is passed inline.
  - [ ] 🖐 Manual: watch one spawned translation to confirm no console window flashes (the spike could not check this visually).
  - [ ] Manual: a real translation via Claude; Task Manager shows no leftover `claude`/`node` processes after cancel; nothing appears in `claude --resume`.
- **Depends on:** T17a. **Can run in parallel with:** T14, T19, T24, T28.
- **Suggested model:** Opus 5.5 (confidence: Medium). Windows process-tree handling and CLI quirks are easy to get subtly wrong.

### T18. UI design spec and Tailwind theme  `design` · parent: S5.1 · issue: #51
- **Goal:** a short design spec so all UI tasks look consistent.
- **Details:** `docs/design/ui.md`: wireframes (ASCII) for the main window (Translate, History, Mistakes, Settings, Presets) and the popup, following §4.8; all states (idle, loading, success, error, empty); keyboard map; component inventory (Button, Select, SegmentedControl, TextArea, Banner, IssueItem, CategoryChip, DiffView, Spinner, ConfirmDialog, Toast); the colour per issue category; spacing and type scale. Tailwind theme tokens (CSS variables for light and dark via `prefers-color-scheme`) as a ready-to-paste snippet for `src/index.css`.
- **Acceptance criteria:**
  - [ ] Every screen and state in §4.8 has a wireframe.
  - [ ] Token snippet covers background, surface, text, muted, accent, danger, warning, success, and the 10 category colours, in both themes.
- **Depends on:** none. **Can run in parallel with:** T1–T17b.
- **Suggested model:** Sonnet 5.5 (confidence: High). Small app with a clear UI description.

### T19. App shell, API layer, error banner  `frontend` · parent: S5.1 · issue: #52
- **Goal:** the main window has navigation, typed backend calls and shared building blocks.
- **Details:** apply the theme from `docs/design/ui.md` to `src/index.css`. Layout with nav: Translate, History, Mistakes, Settings (placeholders for screens not yet built). `src/lib/api.ts`: a shared `call<T>()` helper that converts rejected `invoke` calls into a typed `AppError`, plus typed wrappers for the commands that **already exist** (ping, settings, presets, translate, cancel, copy_result). Later backend tasks add their own wrappers in their own section of the file (§4.2 conventions). Add an `open_url` command (Rust, `tauri-plugin-opener`, allow-listed URLs only; §4.6) for links in the UI. Zustand store `src/store/app.ts`: settings, presets, provider status (loaded on start, refreshed on `settings://changed`). Shared components from the inventory: Button, Select, SegmentedControl, TextArea, Spinner, Banner (variants error/warning/info, with optional actions), ConfirmDialog, Toast. `ErrorBanner` maps `AppError.kind` to message + hint + actions (Retry, Open settings) per §4.6.
- **Acceptance criteria:**
  - [ ] The api wrapper maps errors; ErrorBanner renders each kind; nav switching works.
  - [ ] Light and dark themes both render correctly (manual).
- **Depends on:** T2, T3, T12, T18. **Can run in parallel with:** T14, T17a, T17b, T24, T28.
- **Suggested model:** Sonnet 5.5 (confidence: High). Standard React work with a spec.

### T20. Translate screen  `frontend` · parent: S5.1 · issue: #53
- **Goal:** the user can run any mode and copy the result from the main window.
- **Details:** `src/screens/Translate/`. Controls per §4.8: mode SegmentedControl, provider Select (Ollama, Claude Code; Mock in dev), preset Select (selecting one sets its `defaultMode`; defaults come from settings). Input TextArea (autofocus; character counter, max 8000). Run (Ctrl+Enter) generates a `requestId` and calls `translate`; Cancel calls `cancel_translation`. Output: an editable TextArea with the result, metadata line (provider · model · duration), an **Accept** button labelled "Copy" (Ctrl+Shift+Enter) → `copy_result({ text: editedOutput, historyId })` + toast "Copied". Name the component and handler `Accept…` so T40 only changes the label and routing. Error → ErrorBanner with Retry. Keep the last input/output in the store when switching screens. Leave a slot below the output for T21.
- **Acceptance criteria:**
  - [ ] Run, loading, cancel, success, error + retry, shortcuts, and preset sets mode all work.
  - [ ] Manual: works end to end with the Mock and Ollama providers.
- **Depends on:** T19, T12. **Can run in parallel with:** T22, T23, T25a–c, T29, T30.
- **Suggested model:** Sonnet 5.5 (confidence: High). Clear spec.

### T21. Result details: diff, issues, token warnings  `frontend` · parent: S5.1 · issue: #54
- **Goal:** the user sees what changed and why, which is the core of the learning value.
- **Details:** `DiffView`: word-level diff between source and output using the `diff` npm package (`diffWords`), with insertions and deletions highlighted. Shown only in Improve and Correct. `IssueList`: each item shows a CategoryChip (colour from the design spec), `original → corrected`, explanation; collapsible when there are more than 5. `TokenWarningBanner`: lists missing tokens with **Ignore** (dismiss) and **Retry** (re-run the same request). Don't call it "Accept", which is the main action (D13). Reusable by the popup (T26), so keep components prop-driven and free of store access.
- **Note:** The History screen (#64) has a temporary `src/screens/History/IssueList.tsx`: make this task's issues list the shared component and switch History to it, deleting the copy (changed in #64).
- **Acceptance criteria:**
  - [ ] The diff renders correctly for a known pair; the issue list renders and collapses; warning banner actions work.
  - [ ] Translate mode shows no diff and no issues section.
- **Depends on:** T20. **Can run in parallel with:** T22, T23, T29, T30.
- **Suggested model:** Sonnet 5.5 (confidence: High). Self-contained components.

### T22. Settings screen  `frontend` · parent: S5.2 · issue: #55
- **Goal:** the user can configure providers, defaults, history, the hotkey and autostart.
- **Details:** `src/screens/Settings/` with sections: **Ollama** (endpoint, model Select filled from `provider_status.ollama.models` plus free text, keep-alive, context size `numCtx`, timeout, Test button → status badge); **Claude Code** (executable path with **Browse** → `pick_file`, model, timeout, status: found/version/signed-in, with a "run `claude` once to sign in" hint when not signed in); **Translation** (default provider, default preset); **History** (enabled toggle, "Clear all history" with confirm → `history_clear`, hidden until T28 commands exist, or behind a feature check); **System** (hotkey capture input, auto-copy selection toggle, autostart toggle, close-to-tray toggle). Save via `save_settings`; validation errors shown inline. A "Presets" sub-tab hosts T23, and an "About" sub-tab is added by T33 (leave a placeholder tab).
- **Acceptance criteria:**
  - [ ] Form load and save, validation display and Test button states work.
  - [ ] Manual: changing the Ollama model takes effect on the next translation.
- **Depends on:** T19, T8, T14, T17a. **Can run in parallel with:** T20, T21, T23.
- **Suggested model:** Sonnet 5.5 (confidence: High). Form work.

### T23. Preset editor  `frontend` · parent: S5.2 · issue: #56
- **Goal:** the user can view, edit, create, delete and reset presets.
- **Details:** Settings → Presets sub-tab. List of presets (built-in badge). The editor has name, id (auto slug on create, read-only after), description, default mode, and rules as a list with add/remove/reorder, or one textarea with one rule per line (choose the simpler one; one-per-line textarea is fine). Save → `save_preset`; Delete with confirm; "Reset built-ins" with confirm → `reset_presets`. Validation errors come from the backend.
- **Acceptance criteria:**
  - [ ] Create, edit, delete confirm and reset confirm work; a backend validation error is shown.
  - [ ] Edited rules take effect in the next translation (manual).
- **Depends on:** T19, T9. **Can run in parallel with:** T20–T22.
- **Suggested model:** Sonnet 5.5 (confidence: High). CRUD form.

### T24. Tray, close-to-tray, single instance, autostart  `backend` · parent: S6.1 · issue: #57
- **Goal:** the app runs quietly in the tray and starts once.
- **Details:** enable the `tray-icon` feature; tray with `icons/tray.png` (fallback: app icon), tooltip "Prompt Translator", menu Open / Quit; left click → show and focus the main window. Main window close → hide when `closeToTray`, else exit. `tauri-plugin-single-instance`: a second launch focuses the main window. `tauri-plugin-autostart`: apply `settings.autostart` on startup and on `settings://changed`, registered with the argument `--autostart`. When launched with `--autostart`, the app starts **hidden in the tray** (no main window, no popup). Record in a code comment the exact registry value name(s) the plugin writes (Run and, if used, `Explorer\StartupApproved\Run`) for the uninstaller (T6). Capabilities: add only what the tray needs to `capabilities/default.json`.
- **Acceptance criteria:**
  - [ ] Manual: close hides to tray; tray Open restores; Quit exits; second launch focuses; autostart toggle adds or removes the startup entry; after a sign-out/sign-in with autostart on, the app is only in the tray.
- **Depends on:** T3, T8. **Can run in parallel with:** T14, T17a, T17b, T19, T28.
- **Suggested model:** Sonnet 5.5 (confidence: High). Documented plugin wiring.

### T25a. Global hotkey and popup window  `backend` · parent: S6.2 · issue: #58
- **Goal:** pressing the hotkey anywhere opens the popup with the clipboard text.
- **Details:**
  - `system/hotkey.rs` with `tauri-plugin-global-shortcut`: register `settings.hotkey` at startup; on `settings://changed`, unregister the old one and register the new one. A registration failure (hotkey taken) surfaces as a banner in the main window with an "Open settings" button (event `hotkey://error`, command `hotkey_error`); the old hotkey is unregistered first, so a failure leaves no popup hotkey (changed in #58).
  - `system/popup.rs`: a `popup` window from `popup.html`, created hidden at startup, frameless, always-on-top, `skip_taskbar`, about 520×360, centered on the monitor under the cursor when shown; hide on blur.
  - A separate `capabilities/popup.json` for the `popup` window only, with the window permissions the popup's JS needs (hide, show, setFocus); `default.json` keeps `core:default`/`core:event` for both windows (changed in #58).
  - On hotkey, a `capture_source()` function returns the text. In this task it reads the clipboard (`tauri-plugin-clipboard-manager`, Rust side); it runs in `spawn_blocking` so T25b's Ctrl+C wait doesn't block the event loop, and T25b plugs auto-copy into it (changed in #58). Then show the popup and emit `popup://source { text, mode }`, with `mode` from `detect_mode` (T25c; until it lands, use `settings.lastPopupMode`). Empty or non-text input → emit with empty text (the popup shows "Select or copy some text first").
  - Handle the `main://open-entry` event (payload `{ input, mode, result: TranslationResult | null }`): show the main window, hide the popup, load the entry into the Translate screen (changed in #58).
- **Acceptance criteria:**
  - [ ] Manual: the hotkey works from a terminal, browser and IDE; the popup appears on the monitor under the cursor; changing the hotkey takes effect without restart; a conflicting hotkey shows an error; the popup can receive events and hide itself (capabilities correct).
- **Depends on:** T24, T8. **Can run in parallel with:** T20–T23, T25c, T29, T30.
- **Suggested model:** Sonnet 5.5 (confidence: Medium). Window focus and positioning on Windows can need iteration.

### T25b. Auto-copy selection outside terminals  `backend` · parent: S6.2 · issue: #59
- **Goal:** "select + hotkey" works in non-terminal apps without manual copying (D5).
- **Details:** in `system/clipboard.rs`, extend `capture_source()` when `settings.autoCopySelection` is on:
  1. Get the foreground window (`GetForegroundWindow`) and its process. **Terminal classifier** (one constant list): window class `CASCADIA_HOSTING_WINDOW_CLASS` (Windows Terminal), `ConsoleWindowClass` (conhost); exe names `WindowsTerminal.exe`, `wezterm-gui.exe`, `alacritty.exe`, `mintty.exe`. If it's a terminal, skip auto-copy and use the clipboard.
  2. Otherwise:
     - save the current clipboard text;
     - wait until the hotkey's modifier keys are released (poll `GetAsyncKeyState`, max 500 ms). **If they're still held after 500 ms, skip auto-copy** and use the clipboard; never send Ctrl+C while Shift is down, because Ctrl+Shift+C opens DevTools in Chromium apps;
     - send Ctrl+C with `SendInput`;
     - wait for `GetClipboardSequenceNumber` to change (max 300 ms);
     - read the text, then restore the saved clipboard text. Mark the restore with `ExcludeClipboardContentFromMonitorProcessing` so it doesn't pile up in Windows clipboard history.
  3. If nothing changed (no selection), fall back to the existing clipboard content.

  IDE-integrated terminals can't be detected; their Ctrl+C with a selection copies, which is acceptable (documented in the README).
- **Acceptance criteria:**
  - [ ] The terminal classifier (class name + exe name → terminal or not) is a pure function and recognises the terminals in use.
  - [ ] Manual: select text in a browser and an IDE editor, press the hotkey, and the popup gets the selection; the previous clipboard text is restored. In Windows Terminal no Ctrl+C is sent (a running command is **not** interrupted). Holding the modifiers longer than 500 ms falls back to the clipboard without opening DevTools.
- **Depends on:** T25a. **Can run in parallel with:** T25c, T26.
- **Suggested model:** Opus 5.5 (confidence: Medium). Win32 input simulation and clipboard timing are easy to get subtly wrong.

### T25c. Mode detection  `backend` · parent: S6.2 · issue: #60
- **Goal:** the popup picks Translate or Correct from the text, including the main learning case (English with some native words).
- **Details:** `translation/mode_detect.rs` (D20), a pure function `fn detect_mode(text: &str, last: Mode) -> Mode`, plus the `detect_mode` command:
  1. strip code spans, URLs, paths and identifiers (reuse the token regexes from T10);
  2. split into words; if fewer than 3 words remain → `last`;
  3. Latin-script words ≥ 60% → **Correct**; ≤ 40% → **Translate**;
  4. in between, use `whatlang`: English → Correct, otherwise Translate.

  Save the mode the user finally used to `settings.lastPopupMode` (popup calls `save_settings`, or a small `set_last_popup_mode` command).
- **Acceptance criteria:**
  - [ ] Expected modes: pure Russian → Translate; Ukrainian → Translate; German → Correct (Latin script; acceptable, the user can switch, and documented); Spanish → Correct (same); pure English → Correct; English with typos → Correct; "Check why метод returns null" → Correct; mostly Russian with one English term → Translate; code-heavy English → Correct; 2 words → last; empty → last.
- **Depends on:** T3, T10. **Can run in parallel with:** T25a, T25b.
- **Suggested model:** Sonnet 5.5 (confidence: High). Pure function with explicit expectations.

### T26. Popup UI  `frontend` · parent: S6.2 · issue: #61
- **Goal:** a fast keyboard-only review-and-copy flow.
- **Details:** `src/popup/`. On `popup://source`: show a source excerpt (3 lines, expandable), mode chips (detected mode preselected), and auto-run `translate` with the default provider and preset. While loading: spinner + Esc to cancel and close. Result: output text, compact issue count ("3 fixes") expanding to `IssueList`, and `TokenWarningBanner` (reuse T21 components). Keys: **Enter** → `copy_result` then hide (T40 switches this to `accept_result`); **Esc** → hide (cancels if running); **Ctrl+R** → retry; **Tab** cycles mode and re-runs; link "Open in main window" emits `main://open-entry` with this input and result. Remember the mode the user ended on (`lastPopupMode`, T25c). Errors via ErrorBanner; `OllamaUnreachable` shows a **Start Ollama** button once T35 exists (done in #70: `ErrorBanner` has the button, so the main window and the popup both get it; the popup hides "Open settings" via `showSettingsAction={false}`).
- **Note:** Popup permissions are in `capabilities/popup.json` (popup only); `main://open-entry` takes `{ input, mode, result: TranslationResult | null }`; #58 left a placeholder in `src/popup/main.tsx` that shows the captured text and hides on Esc (changed in #58).
- **Acceptance criteria:**
  - [ ] Auto-run on event, keyboard handling, cancel on Esc and the error state work.
  - [ ] Manual: copy text in a terminal → hotkey → Enter → paste gives the English result; the whole flow works without the mouse.
- **Depends on:** T25a, T25c, T21. **Can run in parallel with:** T25b, T29, T30.
- **Suggested model:** Sonnet 5.5 (confidence: High). Clear flow, reuses components.

### T27. SQLite schema, migrations and repository  `database` · parent: S7.1 · issue: #62
- **Goal:** a data layer for history and mistakes.
- **Details:** `history/db.rs`: open `paths::history_db()` (= `app_local_data_dir()/history.db`; never `app_data_dir()`, which is Roaming) with `rusqlite` (feature `bundled`), `PRAGMA foreign_keys = ON`, `journal_mode = WAL`; migrations with `rusqlite_migration` using the schema in §4.7. `history/repo.rs` (sync functions; callers use `spawn_blocking`): `insert(result, request) -> id` (translation + issues in one transaction), `mark_accepted(id, target: AcceptedTarget)` (`Clipboard` or `Session { id, name }`, written to the three `accepted_*` columns), `list(query, issue, limit, offset)` (search `LIKE` on source/output; optional `issue {original, corrected}` filter that returns entries containing that mistake, case-insensitive; newest first; summaries with the first 120 chars), `get(id)` (with issues), `delete(id)`, `clear()`, `mistakes_summary(since_days)` per §4.6 (by category; recurring = group by lower(original), lower(corrected) having count ≥ 2, top 50; vocabulary = category 'vocabulary', top 100). Export summary types via ts-rs.
- **Acceptance criteria:**
  - [ ] Migrations apply; insert/get round trip, search, issue filter, delete (cascading issues), clear, summary counts and the `since_days` filter work.
- **Depends on:** T3. **Can run in parallel with:** T8–T12.
- **Suggested model:** Sonnet 5.5 (confidence: High). Defined schema and queries.

### T28. Wire history into the service and expose commands  `backend` · parent: S7.1 · issue: #63
- **Goal:** each successful translation is saved when history is enabled, and the UI can query it.
- **Details:** use the `on_result` hook from T12: when `settings.historyEnabled`, insert the entry and set `history_id`; a failure to save logs a warning and doesn't fail the translation. `copy_result` marks the entry accepted with target `clipboard` (`accept_result` from T39 reuses this). Commands: `history_list`, `history_get`, `history_delete`, `history_clear`, `mistakes_summary` (§4.6). Turning history off doesn't delete existing data (Clear does).
- **Acceptance criteria:**
  - [ ] Saved when enabled; not saved when disabled; a DB error doesn't fail the translation; copy marks the entry accepted.
- **Depends on:** T27, T12. **Can run in parallel with:** T14, T17a, T17b, T19, T24.
- **Suggested model:** Sonnet 5.5 (confidence: High). Glue code.

### T29. History screen  `frontend` · parent: S7.2 · issue: #64
- **Goal:** the user can search and reuse past translations.
- **Details:** `src/screens/History/`: a search box (debounced 300 ms), a list (date, mode badge, provider, source excerpt → output excerpt) with "Load more" paging (50 per page). Detail panel: full source, output, issues (reuse `IssueList`), token warnings, Copy, Delete (confirm), "Open in Translate" (loads into the Translate screen). Empty state: "No history yet"; history-disabled hint with a link to Settings.
- **Acceptance criteria:**
  - [ ] Search debounce, paging, delete confirm, and the empty and disabled states work.
- **Depends on:** T19, T28. **Can run in parallel with:** T20–T26, T30.
- **Suggested model:** Sonnet 5.5 (confidence: High). List and detail UI.

### T30. Mistakes screen  `frontend` · parent: S7.3 · issue: #65
- **Goal:** the user sees their recurring mistakes and the vocabulary they've learned.
- **Details:** `src/screens/Mistakes/`: period filter (7 days, 30 days, all). Three sections from `mistakes_summary`: **By category** (horizontal bars with counts, category colours from the design spec), **Recurring mistakes** (`original → corrected ×n`, click → History filtered with `history_list({ issue: {original, corrected} })`), **Vocabulary** (native → English, any source language, count, last seen). Empty state explains that data comes from Correct and Improve runs.
- **Acceptance criteria:**
  - [ ] The summary renders; the period switch refetches; the empty state shows.
- **Depends on:** T19, T28. **Can run in parallel with:** T20–T26, T29.
- **Suggested model:** Sonnet 5.5 (confidence: High). Read-only views.

### T31. Release workflow and updater signing  `cloud` · parent: S1.2 · issue: #66
- **Goal:** pushing a `v*` tag publishes a GitHub Release with the installer and updater artifacts.
- **Details:**
  - The **user** generates the updater key pair with `npm run tauri signer generate -- -w <path>`. Guide them through it; the private key never goes into the repo or chat. Document backing it up: losing it means existing installs can't be updated.
  - The user adds `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as repo secrets. That's a remote change, so ask first.
  - Put the public key in `tauri.conf.json` (`plugins.updater.pubkey`) and set `bundle.createUpdaterArtifacts: true`.
  - `.github/workflows/release.yml`: on tag `v*`, on `windows-latest`, `permissions: contents: write` (nothing else), actions pinned to commit SHAs. Run the CI checks, then `tauri-apps/tauri-action` with `tagName`, `releaseName`, `releaseDraft: true`, `includeUpdaterJson: true`. The user publishes the draft after checking it.
  - Version bump procedure: one source of truth (`package.json` version synced to `tauri.conf.json` and `Cargo.toml` by a small `scripts/bump-version.mjs`).
  - `docs/release.md`: the signing key (generation, offline backup, repo secrets), the bump script, and what to do when `release.yml` fails. The release flow itself (`release/X.Y.Z` from `develop` → PR into `main` → tag `vX.Y.Z` on `main` → draft → publish) is in `docs/development.md` §5: link to it instead of repeating it, and correct §5 if the workflow ends up different.
  **Changed in #66 (PR #133, repo part; the ticket stays open for the owner's key, secrets and test tag):** `release.yml` also fails early when `TAURI_SIGNING_PRIVATE_KEY` is unset and marks tags containing `-` as prereleases; `tauri-action` runs with `--bundles nsis`. `tauri.conf.json` has `createUpdaterArtifacts: true` and `plugins.updater` (endpoint as in T32) with a **placeholder** `pubkey` until the owner pastes the real one. Because of `createUpdaterArtifacts`, a local `npm run tauri build` needs the signing key (unsigned workaround in `docs/release.md`).
  **Changed in #66 (PR #135):** the real `pubkey` is set; a test tag `v0.0.1-test` produced a correct signed draft (deleted afterwards).
- **Acceptance criteria:**
  - [ ] A test tag (e.g. `v0.0.1-test`, on a commit of this task's branch, never on `main`; deleted afterwards with the user's approval) produces a draft release with the `-setup.exe`, its `.sig` and `latest.json`.
  - [ ] `latest.json` points to the release asset URLs.
  - [ ] No secrets in the repo or logs.
- **Depends on:** T4, T6. **Can run in parallel with:** T34–T36.
- **Suggested model:** Sonnet 5.5 (confidence: Medium). Standard tauri-action setup; key handling needs care.

### T32. In-app updater backend  `backend` · parent: S1.2 · issue: #67
- **Goal:** the app can find, download and install a newer release.
- **Details:**
  - Add `tauri-plugin-updater`. Endpoint `https://github.com/misstheaim/prompt-translator/releases/latest/download/latest.json`; pubkey from T31; set `plugins.updater.windows.installMode: "passive"` explicitly.
    **Changed in #66:** the endpoint, `createUpdaterArtifacts`, `plugins.updater` and the real pubkey are already in `tauri.conf.json`.
  - `updater.rs` commands: `check_update` → `{ available, version, notes }`; `install_update` downloads with `update://progress` events and installs. On Windows the NSIS installer exits and relaunches the app itself (verify against the current plugin docs); `tauri-plugin-process` relaunch is only a fallback if that isn't the case.
  - On startup, if `autoCheckUpdates`, check once after 10 s in the background. If an update exists, emit `update://available` (no auto-install).
  - Network errors are silent at startup and shown when the user checks manually.
  - `app_info` and `open_folder` commands (§4.6); `open_folder` uses `tauri-plugin-opener` from Rust.
  **Changed in #67 (PR #136):** `tauri-plugin-updater` `=2.14.1`, registered from Rust only (no webview permission). `check_update` → `UpdateInfo { available, version?, notes? }`; `install_update` re-checks, then downloads with throttled `update://progress` (`UpdateProgress { downloadedBytes, totalBytes?, finished }`) and the NSIS installer exits and relaunches the app (no process plugin). Startup check emits `update://available` with `UpdateInfo`. `app_info` → `AppInfo { version, configDir, dataDir }` (`dataDir` = `paths::local_dir()`), `open_folder(folder: "config"|"data")`, both in `system/commands.rs`. Updater errors map to `AppError::Io("Update failed: …")`. Adding the plugin brought rustls into `reqwest`, which panics at startup without a crypto provider: fixed by T46.
- **Acceptance criteria:**
  - [ ] Version comparison and notes mapping use the plugin's types where possible.
  - [ ] Manual (Sandbox): install release N-1, start it, see the update, install it, and the app restarts on N with history kept.
- **Depends on:** T3, T8, T31. **Can run in parallel with:** T34–T36.
- **Suggested model:** Sonnet 5.5 (confidence: Medium). Plugin wiring; end-to-end testing needs two real releases.

### T33. About section and update UI  `frontend` · parent: S1.2 · issue: #68
- **Goal:** users see the version, can update with one click, and can re-run setup.
- **Details:**
  - Settings → About tab: version; config and data folder paths with "Open" buttons; "Check for updates" (spinner, "You're up to date" or update card); "Run setup again" (navigates to the wizard from T36, or a placeholder until it exists); "Check for updates on startup" toggle (`autoCheckUpdates`).
    **Changed in #71:** the About tab placeholder already has a working "Run setup again" button (`openSetup` in `src/store/app.ts`); keep it.
    **Changed in #68 (PR #137):** shared state in `src/store/update.ts`; the startup toggle saves immediately (no Save button in About); the banner lives in the main UI only (not the wizard), stays visible while installing, and a dismissed banner reappears for a different version.
  - A global dismissible **update banner** on `update://available`: version, notes (Markdown rendered as plain text), **Install and restart** with a progress bar from `update://progress`.
- **Acceptance criteria:**
  - [ ] Check states (checking, none, available, error), install progress and banner dismiss work.
- **Depends on:** T19, T22, T32. **Can run in parallel with:** T36.
- **Suggested model:** Sonnet 5.5 (confidence: High). Small UI.

### T34. Environment check and model tier recommendation  `backend` · parent: S8.1 · issue: #69
- **Goal:** one command reports what this machine has and which local model fits it.
- **Details:** `setup/environment.rs` → `environment_check` returns the `EnvironmentReport` from §4.6.
  - **GPUs:** enumerate with DXGI (`windows` crate, `IDXGIFactory1::EnumAdapters1`, `DedicatedVideoMemory`), skipping software adapters.
  - **winget:** found on PATH, plus `winget --version`.
  - **Ollama:** installed if `ollama.exe` is on PATH or at `%LOCALAPPDATA%\Programs\Ollama\ollama.exe`; running via `GET /api/tags`; version via `/api/version`; models from the tags list.
  - **Claude:** use `find_claude()` and the status function from T17a.
  - **System RAM** via `GlobalMemoryStatusEx` (total and available).
  - **Usable GPU (initial guess)** = a discrete NVIDIA or AMD GPU (vendor ID 0x10DE or 0x1002) with ≥ 4 GB `DedicatedVideoMemory`. **Integrated GPUs** (Intel vendor 0x8086, or AMD with under 1 GB dedicated memory, or a known iGPU name such as "Radeon Graphics", "UHD", "Iris") count as no GPU. Report them, but mark them as not used. This is only a guess: the real check is `ollama_benchmark`'s GPU offload from `/api/ps` (T35), which the wizard uses to correct the tier.
  - `setup/model_tiers.rs`: the tier table from `docs/spikes/local-model-eval.md` (T13) as data, with `fn recommend(gpus) -> Recommendation { tier, model, downloadMb }` (**changed in #69:** no RAM argument, since the CPU tiers are gone; `model` and `downloadMb` are null for `none`; VRAM bounds are tolerant: usable ≥ 3900 MB, `gpu-medium` ≥ 5800 MB, `gpu-large` ≥ 10500 MB). A usable GPU picks a `gpu-*` tier by VRAM; **changed in #45:** without a usable GPU the tier is `none` (no model, Claude Code recommended); there are no `cpu-low`/`cpu` tiers, and no `preferClaudeForCorrect` field (D5: Ollama and Claude are equal choices in every mode).
  - Every check has a short timeout (≤ 3 s) and checks run in parallel.
- **Acceptance criteria:**
  - [ ] `recommend` picks correctly for: every tier; integrated-only GPU with 8 GB and 16 GB RAM; integrated + dedicated (dedicated wins); two dedicated GPUs; GPU under 4 GB.
  - [ ] Manual: correct report on the dev machine (RTX 4060 → `gpu-medium`) and in Windows Sandbox started with `docs/sandbox/no-gpu.wsb` (no GPU → tier `none`, Claude Code recommended, Ollama not installed, winget missing).
- **Depends on:** T3, T8, T13, T17a. **Can run in parallel with:** T14–T30.
- **Suggested model:** Sonnet 5.5 (confidence: Medium). The DXGI FFI part may need iteration.

### T35. Ollama install, start and model download  `backend` · parent: S8.2 · issue: #70
- **Goal:** the app can get a working local model on a fresh machine without the terminal.
- **Details:** `setup/ollama_setup.rs`:
  - `ollama_install`: run `winget install -e --id Ollama.Ollama --accept-package-agreements --accept-source-agreements --silent` with `CREATE_NO_WINDOW`, stream output lines as `setup://progress { phase: "install", message }`, and map exit codes to errors.
  - If winget is missing, return `AppError::InvalidInput` with a hint, and the UI opens `https://ollama.com/download` via `open_url`.
  - `ollama_start`: launch `%LOCALAPPDATA%\Programs\Ollama\ollama app.exe` (or `ollama serve` detached, without a window) and poll `/api/tags` until ready (30 s timeout). Also used by the **Start Ollama** button on the `OllamaUnreachable` error. The app doesn't stop Ollama on exit (D10).
  - `ollama_pull`: `POST /api/pull { model, stream: true }`, parse NDJSON, emit `setup://progress { phase: "download", completedBytes, totalBytes, status }`.
  - `cancel_setup_task` aborts the current install wait or pull.
  - On success, set `settings.ollama.model` to the pulled model.
  - `ollama_benchmark`: run 2 fixed eval prompts (one Translate, one Correct) through `TranslationService` with the new model and the configured `num_ctx`. The first run warms the model and isn't timed. Then read `GET /api/ps` and compute `gpuOffloadPercent = size_vram / size × 100`. Return `{ translateMs, correctMs, gpuOffloadPercent }`. The wizard calls it after the download.
  - `complete_setup`: sets `settings.setupCompleted = true`.
- **Acceptance criteria:**
  - [ ] Pull stream parsing handles progress, success and error lines; the start poll times out.
  - [ ] Manual in Windows Sandbox (`no-gpu.wsb`, no winget): the winget-missing path opens the download page; after installing Ollama manually, start it, pull the `cpu`-tier model, and a translation works with `gpuOffloadPercent = 0`.
- **Depends on:** T14, T34. **Can run in parallel with:** T31–T33.
- **Suggested model:** Sonnet 5.5 (confidence: Medium). External installers and process launching on Windows can surprise.

### T36. Setup wizard UI  `frontend` · parent: S8.2 · issue: #71
- **Goal:** a new user reaches a working provider in a few clicks.
- **Details:** `src/screens/Setup/`, shown instead of the main UI when `setupCompleted` is false, and from Settings → About "Run setup again". Steps per §4.8:
  1. **Welcome:** what the app does and the privacy note.
  2. **Local model:** shows `environment_check` results in plain words, e.g. "No supported graphics card, so the local model isn't recommended: use Claude Code" (changed in #45: tier `none`, Ollama stays available under "Set up anyway"). It shows the recommended model with download size; other tiers are selectable, and choosing one above the machine's tier shows a warning. Buttons: Install Ollama / Start Ollama / Download model with progress bar and Cancel, or Skip. After the download, run `ollama_benchmark` and show the times. If Translate takes over 8 s or Correct over 20 s (thresholds in one constant), or `gpuOffloadPercent` is low on a machine guessed as `gpu-*`, say so and preselect Claude Code as the default provider in step 4 (when available).
  3. **Claude Code:** found or not, with a link to install instructions (`open_url`); signed in or not (with a hint to run `claude` once if not); a re-check button; or Skip.
  4. **Defaults:** default provider (only working ones), hotkey, history toggle, autostart.
  5. **Done:** calls `complete_setup`. Warns if no provider works.

  Errors use ErrorBanner. The wizard can be left and resumed (step kept in the store).
- **Acceptance criteria:**
  - [ ] Each step's states, skip paths, the no-provider warning and resume work.
  - [ ] Manual in Windows Sandbox: full flow from a fresh install to a first translation.
- **Depends on:** T19, T34, T35, T17a. **Can run in parallel with:** T31–T33.
- **Suggested model:** Sonnet 5.5 (confidence: High). Multi-step form on top of ready commands.

### T37. Spike: console input injection into claude.exe  `research` · parent: S9.1 · issue: #72
> **DROPPED (D47, 2026-10-31):** never ran (D42). Closed as not planned; replaced by T58 (#148), which tests the VS Code link and focus-and-paste instead of console input injection. The old recipe and matrix are in the git history of this file.

### T38. Session targets: list, bind and watch Claude sessions  `backend` · parent: S9.2 · issue: #73
> **Rewritten 2026-10-31 (D47).** The console-injection version (`claude agents --json` poller, app-wide binding) is replaced.

- **Goal:** the core lists the Claude sessions a Translate tab can be bound to (VS Code extension sessions and terminal windows), binds one, and notices when a bound terminal window closes.
- **Details:** `target/sessions.rs` + `target/commands.rs`. Contract `SessionTarget` = `{ kind: "vscode", sessionId, title, cwd }` | `{ kind: "terminal", hwnd, pid, tabRuntimeId?, title }`. `list_session_targets()`: top-level terminal windows (POPUP-007's terminal check) with titles, Windows Terminal tabs via UI Automation (`TabItem` name, RuntimeId); VS Code sessions as T58 recommends (fallback: "New VS Code conversation" only); translator runs (D41) are never listed. `bind_terminal_by_focus(timeoutMs)` returns the next terminal window brought to the front (~10 s). While terminal bindings exist, a ~2 s liveness check (`IsWindow`, same pid, WT tab still there) emits `target://closed`; no polling without bindings.
- **Acceptance criteria:**
  - [ ] Two WT windows and a conhost window are listed with titles and WT tabs; translator runs aren't.
  - [ ] Bind by focus works within the timeout; closing a bound window emits `target://closed` within ~2 s.
  - [ ] VS Code sessions listed as T58 recommends.
- **Depends on:** T58. **Can run in parallel with:** T47–T57.
- **Suggested model:** Opus 5.5 (confidence: Medium). Win32 windows, UI Automation and a watcher; depends on the spike.

### T39. Deliver accepted text into a bound Claude session  `backend` · parent: S9.2 · issue: #74
> **Rewritten 2026-10-31 (D47).** Replaces console injection and the app-wide `accept_result` routing.

- **Goal:** Accept in a bound tab puts the text into the bound session's input box without sending it.
- **Details:** `deliver_to_target(target, text, historyId, confirmed?)`: D35 prefix guard first. VS Code → open `vscode://anthropic.claude-code/open?session=<id>&prompt=<url-encoded>` through `open_url` (fall back to copy above the URL limit T58 found). Terminal → under the input lock: check the window (same pid), restore if minimised, `SetForegroundWindow`, select the WT tab, verify foreground + no modifier, write the text hidden from clipboard history, Ctrl+V once, restore the clipboard after the layout fix's delay (LAYOUT-004). **No Enter.** Any failure → copy + message. Marks the entry accepted with `Session { id, name }` (HISTORY-009). Only the accepted target-language output is delivered (§4.9).
- **Acceptance criteria:**
  - [ ] Terminal: idle and busy Claude, window minimised / behind / bound tab not selected: the text waits in Claude's input; clipboard restored.
  - [ ] VS Code: the session opens with the text in its input.
  - [ ] Prefix confirmation; failure → copied with a message; history marked.
- **Depends on:** T38. **Can run in parallel with:** T40's UI against mocked commands.
- **Suggested model:** Sonnet 5.5 (confidence: Medium). Follows T58's recipe and the layout fix's paste code.

### T40. Translate tabs bound to Claude sessions  `frontend` · parent: S9.2 · issue: #75
> **Rewritten 2026-10-31 (D47).** Replaces the app-wide session picker (D34).

- **Goal:** the Translate screen has a default, unbound tab plus up to 5 tabs bound to Claude sessions; Accept in a bound tab delivers into its session.
- **Details:** tab strip ("Translate" + one tab per binding, ×, "+ Bind session" up to 5); bind dialog (list from T38, "Click into a terminal…"); per-tab state (text, mode, provider, preset, language pair, result); in bound tabs Improve is disabled and the result button is "Send to <session>" (T39, D35 dialog); `target://closed` closes the tab with a toast; tabs aren't restored after a restart; History/popup "Open in main window" load into the default tab.
- **Acceptance criteria:**
  - [ ] Binding by click and from the list; no 6th binding; per-tab state kept.
  - [ ] Improve disabled in bound tabs; Send delivers; a closed terminal closes its tab; restart shows only the default tab.
- **Depends on:** T39, T53. **Can run in parallel with:** none in E9.
- **Suggested model:** Sonnet 5.5 (confidence: Medium). Per-tab state refactor plus dialogs.

### T41. Keyboard layout conversion module  `backend` · parent: S6.3 · issue: #77
- **Goal:** a pure function that turns text typed with the wrong keyboard layout into what the user meant (D36).
- **Details:** new module `src-tauri/src/layout_fix/` with `convert.rs` (and `mod.rs`):
  - **One table of physical keys**, each entry `(us_char, ru_char)`, for the Windows US (QWERTY) and Russian (ЙЦУКЕН) layouts, unshifted and shifted:
    - letters row by row: `q↔й w↔ц e↔у r↔к t↔е y↔н u↔г i↔ш o↔щ p↔з [↔х ]↔ъ`, `a↔ф s↔ы d↔в f↔а g↔п h↔р j↔о k↔л l↔д ;↔ж '↔э`, `z↔я x↔ч c↔с v↔м b↔и n↔т m↔ь ,↔б .↔ю /↔.`, `` ` ``↔`ё`;
    - uppercase letters map to uppercase (`Q↔Й` … `M↔Ь`, `~↔Ё`);
    - shifted symbols that differ: `{↔Х }↔Ъ :↔Ж "↔Э <↔Б >↔Ю ?↔, @↔" #↔№ $↔; ^↔: &↔? |↔/`.
    Characters that are the same on both layouts (digits, space, newlines, `! % * ( ) - _ = + \`) and any character not in the source column stay unchanged.
  - `enum Direction { ToRussian, ToEnglish }` and `fn detect_direction(text: &str) -> Option<Direction>`: count Cyrillic letters and Latin letters, ignore everything else. More Latin → `ToRussian`; more Cyrillic → `ToEnglish`; equal (including no letters) → `None`.
  - `fn convert(text: &str, dir: Direction) -> String`: map each char through the column for that direction. The two directions are separate lookups, not one involution: `ToRussian` maps `.` → `ю` and `/` → `.`, while `ToEnglish` maps `.` → `/` and `ю` → `.`.
  - Keep the table as data in one place (a `const` array), so another layout pair can be added later without changing the logic. v1 ships only ru ↔ en.
- **Acceptance criteria:**
  - [ ] Conversions: `ghbdtn` → `привет`; `Ghbdtn? rfr ltkf/` → `Привет, как дела.`; `руддщ цщкдв` → `hello world`; `Ш тфьу` → `I name` (case kept); digits, spaces and newlines unchanged; mixed `ghbdtn мир` → `привет мир` (Latin majority; Cyrillic letters are not in the source column, so they stay).
  - [ ] Round trip: for every char in the US column, `convert(convert(c, ToRussian), ToEnglish) == c`, and the same the other way for the Russian column. The table has no duplicate characters in either column.
  - [ ] `detect_direction`: Latin majority, Cyrillic majority, tie, empty, punctuation only.
- **Depends on:** T3. **Can run in parallel with:** T8, T9, T10, T25a–c, and anything in waves 2–5.
- **Suggested model:** Sonnet 5.5 (confidence: High). A pure function, but the table must be checked key by key against the real layouts.

### T42. Layout-fix hotkey: capture, replace and switch layout  `backend` · parent: S6.3 · issue: #78
- **Goal:** pressing the layout-fix hotkey anywhere replaces the selected text (or the last typed word) with the converted text, as a stand-alone tool outside the translate flow (D36–D38).
- **Details:**
  - **Settings** (extend T8's model, `#[serde(default)]`): `"layoutFix": { "enabled": true, "hotkey": "Shift+Pause", "switchInputLanguage": true }` (§4.3; default changed from Ctrl+Alt+Space in #79, no migration). `save_settings` rejects a layout-fix hotkey equal to the translate hotkey with a validation error naming the field.
  - **Hotkey** (`system/hotkey.rs`, from T25a): register both shortcuts and dispatch by which one fired. On `settings://changed`, re-register what changed; when `layoutFix.enabled` is false, the layout-fix shortcut is unregistered. Registration failures are reported the same way as in T25a.
  - **Reuse T25b's Win32 helpers** (terminal classifier, waiting for modifiers to be released, clipboard save/restore with `ExcludeClipboardContentFromMonitorProcessing`, waiting on `GetClipboardSequenceNumber`). If they're private to `system/clipboard.rs`, move the shared pieces into `system/input.rs`; T25b's behaviour must not change.
  - **Flow** in `layout_fix/hotkey.rs`, run off the main thread; ignore the hotkey while a previous run is still in progress:
    1. Foreground window is a **terminal**, or `autoCopySelection` is off → read the clipboard text, convert it, write the result to the clipboard (no restore) and show a notification "Layout fixed: paste with Ctrl+V" (`tauri-plugin-notification`, used from Rust only). Empty clipboard or no direction → notification "Copy some text first". Then stop.
    2. Otherwise wait until the hotkey's modifiers are released (max 500 ms; if still held, stop without sending any keys), save the clipboard text, send Ctrl+C and wait for the clipboard sequence number to change (max 300 ms).
    3. **No selection** (the number didn't change): send Ctrl+Shift+Left to select the previous word, then Ctrl+C again. Still nothing → restore the clipboard and stop silently.
    4. `detect_direction`; `None`, or the converted text equals the input → restore the clipboard and stop.
    5. Put the converted text on the clipboard, send Ctrl+V, wait a fixed delay (a named constant, start at 200 ms; the target app reads the clipboard asynchronously), then restore the saved clipboard text.
    6. If `switchInputLanguage`: find an installed layout for the target language with `GetKeyboardLayoutList` (primary language Russian for `ToRussian`, English for `ToEnglish`) and post `WM_INPUTLANGCHANGEREQUEST` with that HKL to the foreground window. Not installed → skip silently.
  - Never writes history, never shows the popup or the main window, never calls a provider.
  - Add `tauri-plugin-notification` and only the permissions it needs from Rust; no webview permission.
- **Note:** Extend `system/hotkey.rs`: its shortcut handler already checks which shortcut fired. The Win32 SendInput/clipboard helpers from T25b are in a private `win` module of `system/clipboard.rs`; move them to `system/input.rs` (changed in #58, #59).
- **Acceptance criteria:**
  - [ ] `layoutFix` defaults, and an old `settings.json` without `layoutFix` loads with defaults; the same-hotkey validation; the decision logic (terminal or auto-copy off → clipboard flow; selection; no selection → word select; no direction → no paste), with the Win32 calls behind a small trait so the logic stays separate from them.
  - [ ] Manual, in Notepad, a browser text field and the VS Code editor: select `ghbdtn` → hotkey → `привет` and the input language switches to Russian; with the cursor right after a mistyped word and no selection, that word is converted; the app's own Ctrl+Z undoes it; the previous clipboard text is restored afterwards.
  - [ ] Manual, in Windows Terminal: copy `ghbdtn`, press the hotkey → notification, Ctrl+V pastes `привет`; no Ctrl+C is sent to the terminal.
  - [ ] Manual: changing the hotkey takes effect without restart; disabling the tool unregisters it; the translate hotkey keeps working.
- **Depends on:** T41, T25b, T8. **Can run in parallel with:** T26, T29, T30, T34, T35.
- **Suggested model:** Opus 5.5 (confidence: Medium). Win32 input simulation, clipboard timing and input-language switching across different apps need careful iteration.

### T43. Layout-fix settings  `frontend` · parent: S6.3 · issue: #79
- **Goal:** the user can turn the layout fix on or off, change its hotkey and choose whether the input language switches.
- **Details:** in Settings (T22), add a "Keyboard layout fix" group (changed in #79: its own section right after System, not inside it): enabled toggle; hotkey capture input (the same component as the translate hotkey); "Switch input language after fixing" toggle; one help line: "Select mistyped text, or put the cursor right after a word, and press the hotkey. In terminals: copy the text first, then paste the result." The duplicate-hotkey validation error from `save_settings` is shown inline. The hotkey and language toggles are disabled while the tool is off.
- **Acceptance criteria:**
  - [ ] Load and save the group, toggles and the disabled state work; a duplicate-hotkey error is shown.
  - [ ] Manual: changes take effect without restarting the app.
- **Depends on:** T22, T42. **Can run in parallel with:** T33, T36.
- **Suggested model:** Haiku 5.5 (confidence: High). A small form section that reuses T22's components.
  **Changed in #79:** also changed the default hotkey to `Shift+Pause` (D38) and taught the hotkey recorder the Pause key; a hotkey equal to the translate hotkey shows an inline error and blocks Save before `save_settings` rejects it.

### T45. Auto-copy: treat VS Code's whole-line copy as no selection  `backend` · parent: S6.2 · issue: #128
- **Goal:** with auto-copy on and nothing selected in VS Code, the popup hotkey uses the clipboard instead of the whole current line.
- **Details:** follow-up from #78. In `system/clipboard.rs` `copy_selection_win()`, after Ctrl+C changed the clipboard, check `input::clipboard_is_empty_selection_copy()` (Chromium's `"isFromEmptySelection":true` marker, the T42 helper) before the saved clipboard is restored; when present, restore and return no selection, so `capture_source()` falls back to the clipboard (POPUP-007, POPUP-008).
- **Acceptance criteria:**
  - [x] The marker check runs before the restore; a normal copy still returns the selection.
  - [ ] Manual: in VS Code with no selection, the popup shows the earlier clipboard text and the clipboard is unchanged.
- **Depends on:** T25b, T42. **Can run in parallel with:** any wave 7 task.
- **Suggested model:** Sonnet 5.5 (confidence: High). Small, contained change that reuses #78's helper.

### T46. Fix startup panic: no rustls crypto provider for reqwest  `backend` · parent: S1.2 · issue: #138
- **Goal:** the app starts again after T32 added the updater plugin.
- **Details:** follow-up from the wave 7 smoke check. `tauri-plugin-updater` pulls rustls into the tree, so feature unification turns on rustls in our `reqwest` (`default-features = false`), and `reqwest::Client::new()` in `OllamaProvider::new` panics with "No rustls crypto provider is configured". Install one default provider at the start of `run()` (and in the `eval` binary), matching the backend the updater already uses; no second crypto backend.
- **Acceptance criteria:**
  - [ ] The custom-protocol debug build starts, stays running, and the startup update check logs its result.
  - [ ] The `eval` binary doesn't panic when it builds providers.
  - [ ] No second crypto backend in `Cargo.lock`.
- **Depends on:** T32. **Must run before:** T7 and the release branch.
- **Suggested model:** Sonnet 5.5 (confidence: High). Small dependency/startup fix with a known cause.

### T44. Tune prompt templates for local models  `backend` · parent: S3.1 · issue: #81
- **Goal:** the prompt templates work well with the local Ollama models chosen by T13, without making them worse for Claude Code.
- **Details:** read `docs/spikes/local-model-eval.md` (T13): its template findings, failure cases (especially "answered instead of translated", invalid JSON, dropped technical tokens) and the recommended default model. Apply the changes to `translation/prompts/{translate,improve,correct}.md`. Keep the shared templates provider-neutral where possible; only if the report shows a local model needs wording that would hurt Claude, add local-model variants selected by `ProviderKind::Ollama` in `prompt_builder.rs` (every variant keeps the "data, not a request" rule and the shared rules). Re-run `eval/spike_eval.py` against the recommended model with the final templates and record before/after numbers in the report's template section. Re-check T11's `numCtx` estimate comment if the templates grew.
  **Changed in #81:** besides the Improve/Correct template changes, two code changes from T13: a per-mode output schema `output_schema(mode)` (Improve issues only `style`/`word_choice`, at most 5; Translate no issues; Correct the full schema), sent by Ollama as `format`; and `output_parser` drops issues whose `original` equals `corrected` (exact match). Templates stay provider-neutral (no Ollama variants).
- **Acceptance criteria:**
  - [ ] Every template change the T13 report recommends is applied, or rejected with a reason in the PR.
  - [ ] Before/after eval numbers (valid-JSON rate, token preservation, answered-instead-of-translated count) for the recommended model are in `docs/spikes/local-model-eval.md`.
  - [ ] Built prompts for the Claude Code provider are unchanged, or the PR shows each changed prompt and why it's still fine for Claude.
- **Depends on:** T11, T13. **Can run in parallel with:** T14.
- **Suggested model:** Sonnet 5.5 (confidence: Medium). Applies measured findings to existing templates with a ready eval script; fall back to Opus 5.5 if the report leaves wording choices open.

### T47. Validate model mistakes and fix Correct/Improve template failures  `backend` · parent: S2.3 · issue: #142
- **Goal:** issues that break the issue rules are dropped before saving, and the templates stop the failures found in the 2026-10-31 manual test (F1, F2, F3, F5).
- **Details:** new issue validation after parsing (TRANS-014): drop (and log) an issue whose `original` isn't in the source or `corrected` isn't in the output (case-insensitive, whitespace collapsed), whose `original` has more than 8 words, or, in Improve, a spelling-only change; the output is never changed. `correct.md`: capitalisation → `spelling`; never delete words or change correct names/headings; one fix per issue, verbatim from the text. `improve.md`: no spelling/grammar issues. Add the five failing texts to `eval/`, before/after numbers in the PR.
- **Acceptance criteria:**
  - [ ] Validation rules in code; templates updated; the five texts no longer give the bad issues or the "Windows Sandbox → sandbox" output on `qwen3.5:9b`.
- **Depends on:** none. **Must run before:** T51, T52.
- **Suggested model:** Sonnet 5.5 (confidence: Medium).

### T48. Count mistakes once per text and filter History by mode, category and period  `backend` · parent: S7.3 · issue: #143
- **Goal:** re-runs no longer inflate Mistakes (F4); History filters by mode (I12); every Mistakes item opens History filtered, for the same period (I14).
- **Details:** `mistakes_summary` counts a pair once per distinct normalised source text; `history_list` gets optional `mode`, `category`, `sinceDays`; History mode select and a combined filter banner; Mistakes category bars, recurring pairs and vocabulary items set the History filter with the period.
- **Acceptance criteria:**
  - [ ] The same text run 5 times counts ×1; mode filter; each Mistakes item opens the matching History list for its period.
- **Depends on:** none. **Can run in parallel with:** T47, T49, T50, T56, T57.
- **Suggested model:** Sonnet 5.5 (confidence: High).

### T49. Fix the Settings save bar gap and add a clear button to the Translate box  `frontend` · parent: S5.1 · issue: #144
- **Goal:** the Settings save bar sits on the window's bottom edge (F6); the Translate text box has a clear (×) button that clears only the input (I7).
- **Details:** `sticky bottom-0` inside `main`'s `p-4` leaves a strip; use `-bottom-4` and `-mx-4 px-4`. × shown while the box isn't empty, disabled while running, keeps the result, refocuses.
- **Acceptance criteria:**
  - [ ] Nothing shows under the bar; × behaves as described.
- **Depends on:** none.
- **Suggested model:** Haiku 5.5 (confidence: High).

### T50. Popup: keep each mode's result, movable, resizable, remembers size and position  `frontend` · parent: S6.2 · issue: #145
- **Goal:** switching back to a mode that already ran shows its result without a new run (I1); the popup can be dragged and resized and reopens where it was left (I17).
- **Details:** per-capture map mode → result (cleared by a new capture, Ctrl+R replaces the current mode's); drag region (`core:window:allow-start-dragging` for `popup` only), `resizable(true)` with a minimum size; new setting `popupBounds`, used when on a connected monitor, else centred as today.
- **Acceptance criteria:**
  - [ ] Mode switch back is instant; move/resize kept across restarts; off-screen bounds fall back to centring.
- **Depends on:** none. **Must run before:** T54, T55.
- **Suggested model:** Sonnet 5.5 (confidence: Medium).

### T51. Improve: idiomatic rewrite with an explanation for each change  `backend` · parent: S2.3 · issue: #149
- **Goal:** Improve becomes a practice mode (D43): a natural, advanced rewrite with idioms where they fit, every change explained.
- **Details:** new `improve.md`; Improve issues up to 12, categories `style`, `word_choice`, new `idiom`, `phrasing` (enum, schema, parser, bindings, APP-031 colours, Mistakes bars); explanations say why the wording is used and sounds more natural; tokens kept; eval cases. Improve is disabled in bound tabs (T40).
- **Acceptance criteria:**
  - [ ] Clearly rewritten output with explained changes on both providers; tokens kept; new categories end to end.
- **Depends on:** T47. **Must run before:** T52.
- **Suggested model:** Opus 5.5 (confidence: Medium).

### T52. Configurable language pair in the translation core  `backend` · parent: S2.3 · issue: #150
- **Goal:** native (default Russian) and target (default English) language settings replace the hard-wired pair (D44); with the defaults nothing changes.
- **Details:** language list = what `whatlang` detects (~70, code + English name); settings `nativeLanguage`, `targetLanguage` (must differ); optional pair on `translate`; template placeholders (explanations stay in English); `detect_mode` = "is it the target language?"; history migration 2 adds `source_language`, `target_language` (null = ru → en). Layout fix and UI language untouched.
- **Acceptance criteria:**
  - [ ] Default-pair prompts equivalent to today's; a `uk` → `de` pair works; migration and old settings load.
- **Depends on:** T51. **Must run before:** T53, T54.
- **Suggested model:** Opus 5.5 (confidence: Medium).

### T53. Language pair in setup, Settings and the Translate tab  `frontend` · parent: S5.2 · issue: #151
- **Goal:** the pair is chosen in the wizard, changed in Settings, and switched per session on the Translate tab without touching the settings.
- **Details:** searchable `LanguageSelect`; Defaults step and a Settings "Languages" group; Translate tab pair control with swap, kept in the screen store across tab switches, reset to settings at app start; the popup uses the settings' pair.
- **Acceptance criteria:**
  - [ ] Wizard and Settings save the pair; the tab switch translates en → ru without changing settings and resets after restart.
- **Depends on:** T52. **Must run before:** T40.
- **Suggested model:** Sonnet 5.5 (confidence: High).

### T54. Popup starting mode from where the text was selected  `backend` · parent: S6.2 · issue: #152
- **Goal:** read-only sources are translated into the opposite language (first word decides for mixed text); editable sources open in the new "Popup starts in" setting (D45).
- **Details:** UI Automation classifies the focused element at capture (editable / read-only / unknown, ~100 ms cap); `popup://source` carries `sourceKind` and an optional reversed pair; setting `popupStartMode` (`auto` default, `translate`, `improve`, `correct`, `lastUsed`); direction label in the popup. Reverse translations are never sent to a session.
- **Acceptance criteria:**
  - [ ] Article text → opposite language; text fields follow the setting; terminals/clipboard fallback count as editable.
- **Depends on:** T50, T52. **Must run before:** T55.
- **Suggested model:** Opus 5.5 (confidence: Medium).

### T55. Popup: Enter replaces the selection, Ctrl+Enter copies  `backend` · parent: S6.2 · issue: #153
- **Goal:** Enter pastes the result over the selection in the source app and puts the previous clipboard back; Ctrl+Enter copies (D45).
- **Details:** keep the source window from the capture; replace only for editable selections (T54), else Enter copies; hide popup, refocus the source, paste hidden from clipboard history, restore (LAYOUT-004 code); failure → copy + notification; footer text per case.
- **Acceptance criteria:**
  - [ ] Notepad, browser field, VS Code: replaced, clipboard restored; read-only/terminal/no selection: Enter copies; closed source window → copy + notification.
- **Depends on:** T54.
- **Suggested model:** Sonnet 5.5 (confidence: Medium).

### T56. Free the local model's memory, uninstall from the app, new layout-fix hotkey default  `backend` · parent: S1.2 · issue: #146
- **Goal:** Ollama doesn't hold VRAM longer than needed (D46), the app can start its own uninstaller, and the layout-fix default is `` Ctrl+Shift+` `` (D38).
- **Details:** defaults `keepAlive` `10m` and `` Ctrl+Shift+` `` (defaults only, no migration); `unload_model` (`keep_alive: 0`, 3 s); always-visible "Unload model" button in the main window's top bar and a tray item; best-effort unload on exit (~2 s, not on close-to-tray); About → "Uninstall Prompt Translator…" starting `UninstallString` from HKCU and exiting (hidden in dev); README and smoke-test updated.
- **Acceptance criteria:**
  - [ ] New defaults; button and tray item unload (`ollama ps` empty); quit unloads; uninstall button opens the uninstaller.
- **Depends on:** none.
- **Suggested model:** Sonnet 5.5 (confidence: High).

### T57. Layout fix: convert every wrong-layout word before the cursor  `backend` · parent: S6.3 · issue: #147
- **Goal:** with no selection, the layout fix extends leftwards over all wrong-layout words and stops at the first right-layout word (I6).
- **Details:** pure scorer in `convert.rs` with built-in English/Russian letter-bigram tables (word in its language vs its conversion in the other); the hotkey flow repeats Ctrl+Shift+Left + copy, shrinks back one word at the first right-layout word, stops at line start, a newline or 15 words; 1–2 letter words follow their neighbours.
- **Acceptance criteria:**
  - [ ] `Привет ghbdtn vbh` → `Привет привет мир`; `fix the ghbdtn vbh` → `fix the привет мир` in Notepad, Chrome, VS Code.
- **Depends on:** none.
- **Suggested model:** Opus 5.5 (confidence: Medium).

### T58. Spike: paste into a bound terminal window and open VS Code sessions by link  `research` · parent: S9.1 · issue: #148
- **Goal:** GO / NO-GO and a recipe for the two delivery routes of D47, replacing T37.
- **Details:** throwaway CLI in `spikes/paste/` (`list-terminals`, `paste --hwnd [--tab] --text`, `vscode --session --text`); matrix: WT window in front / behind / minimised / other monitor / bound tab not selected, conhost, several windows; Claude idle / busy / with a draft; one line, multi-line, 4,000 chars, `!` `/` `#`; focus rules; user typing; closed window; VS Code: open/closed session, several VS Code windows, draft replaced?, URL length, where session ids come from. Report: `docs/spikes/session-paste.md`. Needs the owner present; its findings update T38–T40.
- **Acceptance criteria:**
  - [ ] Every cell has a result; GO/NO-GO per route; recipe documented.
- **Depends on:** none. **Must run before:** T38.
- **Suggested model:** Opus 5.5 (confidence: Medium).

## 6. Order of work

Done: waves 0-8 (see git history of this file). T37 was dropped (D47).

| Wave | Tasks (parallel within a wave) | Notes |
|---|---|---|
| 9 | T47, T48, T49, T50, T56, T57, T58 | Follow-ups from the owner's manual test (2026-10-31). T58 needs the owner present |
| 10 | T51, T38 | T51 after T47; T38 after T58 |
| 11 | T52, T39 | Language pair core after T51; delivery after T38 |
| 12 | T53, T54 | UI for the pair; popup starting mode (needs T50, T52) |
| 13 | T55, T40 | Enter replaces (after T54); bound tabs (after T39, T53) |
| 14 | T7 | Docs for the new behaviour (README, smoke-test), then the full smoke test including Windows Sandbox; #39 closes when it passes |

Critical path: T1 → T2 → T3 → T11 → T12 → T20 → T21 → T26 → T7; for the follow-ups T47 → T51 → T52 → T53 → T40 → T7. The distribution chain is T24 → T6 → T31 → T32 → T33 → T7.

**First public release** (`v0.1.0`) only after T47–T58 and T38–T40 are merged and T7's smoke test passes (the manual test of 2026-10-31 did not pass). A test tag in T31 is fine before that, but it must stay a draft. The release follows `docs/development.md` §5: `release/0.1.0` is cut from `develop`, merged into `main` and tagged there.

## 7. Risks and open questions

| Risk / question | Impact | Mitigation / owner |
|---|---|---|
| The translator **answers** the prompt instead of translating it (especially questions) | High: wrong text pasted into Claude | Delimited source + explicit rule (§4.4); dedicated eval cases (T13, T15); the user always reviews before copying |
| Full isolation of `claude -p` isn't possible with subscription auth | Low (resolved by spike T16, D41): `--safe-mode` + `--setting-sources ""` loaded none of the canaries | Re-run the spike after a Claude Code upgrade and diff `claude-invocation.json` |
| Injecting into the app's own translator run, which `claude agents --json` lists as interactive for 2–5 s | High: text written into a short-lived process | D41: exclude by name `prompt-translator` and by `TranslatorPids` in discovery (T38) and in the pre-write re-check (T39) |
| Small local models give weak Correct-mode explanations in English | Medium: lower learning value | Measured in T13; may recommend Claude for Correct mode; the model is configurable |
| Claude CLI flags or output change between versions | Medium: provider breaks after an update | Invocation in one place; version shown in Settings; spike doc records the tested version; the invocation comes from the spike's contract file |
| Orphan `claude`/`node` processes on cancel or timeout | Medium | Job Object kill-tree (T17b), manual check in acceptance |
| Hotkey Ctrl+Shift+Space conflicts with another app or IME | Low | Configurable; registration error shown (T25a) |
| History stores sensitive code in plain SQLite | Low (per-user data folder) | Toggle + Clear all; documented in README |
| Unsigned installer: SmartScreen warning; some antivirus tools may flag it | Medium: other users may not trust it | README explains it; signing (D23) can be added to `release.yml` later |
| Updater private key lost or leaked | High: lost = no more updates for existing installs; leaked = malicious updates possible | Only in GitHub secrets + an offline backup by the owner (T31); never in the repo or chat |
| Repo must be public for the updater | Medium | Confirm at repo creation; alternative: private source + public releases repo |
| Uninstaller deletes data during an update by mistake | High: users lose history | Hooks check `$UpdateMode`; the Sandbox update test in T6/T32/T7 |
| Target machines without winget (old Windows 10) or with AMD/Intel GPUs | Medium | Fallback to the ollama.com download page; DXGI works for any vendor; `cpu` tier always available |
| Weak machines (integrated graphics only) give slow or poor local translations, especially in Correct mode | Medium | Changed in #45: no CPU tiers; without a usable GPU the wizard recommends Claude Code (tier `none`); with a GPU the wizard benchmarks the model and recommends Claude Code when it's slow (D28); the local model stays as an offline fallback |
| Ollama adds Vulkan/iGPU support later, so integrated GPUs become usable | Low (upside) | Usable-GPU rule lives in one function (T34); revisit when Ollama's Windows iGPU support is stable |
| Other users' Claude subscriptions: a separate translator sign-in per machine | Low | Wizard step + README |
| Console injection doesn't work under Windows Terminal/ConPTY, or multi-line text submits early | High for E9 | Realised as "unproven" (D42): T37 couldn't run, E9 is on hold and v0.1.0 ships with Clipboard. Revisit only if the owner runs the injector by hand or channels leave preview |
| Injected text starting with `!`, `/` or `#` runs a shell command, slash command or memory write in Claude Code (e.g. after prompt injection from copied web text) | High | D35: explicit confirmation before sending; prefix guard in T39; matrix case in T37 |
| Attaching to a session's console exposes the injector to that console's Ctrl+C/close events | High (app crash) | D31: injection runs in a short-lived helper process that ignores console control events |
| Ollama silently truncates long prompts when `num_ctx` is too small, dropping the system prompt | High (wrong output) | `num_ctx` always sent (default 8192, setting); measured in T13; checked in T14 |
| Translator runs auto-update the user's shared `claude.exe` | Medium | Auto-updater and nonessential traffic disabled via env (names confirmed in T16, T17b sets them) |
| `--safe-mode` is restricted by an admin policy or changes in a Claude Code update | Medium | Confirmed working in T16 (D41); a failed run maps to `ProcessCrashed` / `ClaudeAuth`; re-run the spike to diff the contract |
| `claude agents --json` output or session behaviour changes in a Claude Code update | Medium: picker or sending breaks | Lenient parser; the version is shown in Settings; `InjectionFailed` falls back to clipboard so text is never lost |
| User types in the terminal at the moment of injection, so the input interleaves | Low | Injection is a single fast write; documented; the user can undo in the terminal |
| Auto-copy (Ctrl+C simulation) hits an undetected terminal (e.g. an IDE terminal with nothing selected), which could interrupt a command | Medium | Terminal detection list; setting to turn auto-copy off; README note |
| Layout fix: the target app reads the clipboard after it was already restored, so the old text is pasted | Medium | Fixed delay after Ctrl+V as a named constant, tuned during T42's manual tests; the app's Ctrl+Z undoes a wrong paste |
| Layout fix: Ctrl+Shift+Left stops at punctuation, so a mistyped word containing `, . ; ' [ ]` (Russian б ю ж э х ъ) is converted only partly | Low | Documented in the README; the user selects the whole word by hand |
| Auto-copy briefly replaces the user's clipboard | Low | Previous clipboard text is restored; non-text clipboard content (images) is not restored, which is documented |
| Rust/Tauri is a new stack for the owner | Medium: harder to maintain by hand | Thin commands, documented modules; each task is self-contained |
| Open: the user should supply 10+ real prompts (Russian and mixed) for the eval set | Affects T13 quality | Ask the user at the start of T13 |
| Open: should the default provider differ per mode (e.g. Claude for Correct)? | UX | Decide after T13 and T16; it's a settings default only |
| Open: should the source spec move to `docs/spec/`? | Tidiness | Ask the user; not required |

## 8. Review notes

- **Changed 2026-10-31 (owner's manual test of the v0.1.0 candidate):** follow-up tasks T47–T58 (#142–#153) added in milestone v0.1.0; decisions D43–D47 (Improve as a practice rewrite, language pair, popup starting mode and Enter-replace, local model memory, revised session delivery); D10, D31, D34, D38, D42 marked changed or superseded. E9 revived: T37 (#72) dropped, T38–T40 (#73–#75) rewritten for VS Code link / terminal focus-and-paste, new spike T58. Waves 9–14; T7 last.
- **Changed 2026-10-30 (wave 7, part 2):** T31 (#66): real pubkey set, test tag checked. T32 (#67): updater commands, events and `AppInfo` as merged. T33 (#68): `store/update.ts`, banner rules. Follow-up T46 (#138, startup panic from the updater's rustls) added under S1.2, first in wave 8. Wave 7 collapsed.
- **Changed 2026-10-30 (wave 7, part 1):** D38 default layout-fix hotkey → Shift+Pause (owner, #79; §4.3, T42, T43). T31 (#66) repo part: early secret check, prerelease tags, `createUpdaterArtifacts` + `plugins.updater` with a placeholder pubkey (T31, T32). T36 (#71): "Run setup again" already in the About placeholder (T33). Follow-up T45 (#128) added under S6.2.
- **Changed 2026-10-30:** wave 6 outcomes folded in. T34 (#69): `recommend(gpus)` without RAM, nullable `model`/`downloadMb` for tier `none`, `ramAvailableMb` in `EnvironmentReport`, tolerant VRAM bounds. T26/T35 (#61, #70): Start Ollama button in the shared `ErrorBanner` (main window and popup).
- **Changed 2026-10-29:** wave 4 outcomes folded in. Spike T13 (#45, owner decisions): one model `qwen3.5:9b`, no CPU tiers (no usable GPU → tier `none`, Claude Code), no per-mode provider default (D28, T13, T34, T36, §4.6, risks). T14 (#46): `think: false`, too-long guard. T44 (#81): per-mode output schema and parser drop of no-op issues. T17b (#50): `is_error` → `ClaudeAuth` only for sign-in errors. T6: autostart value name `Prompt Translator`, unquoted path (from T24, #57).
- **Changed 2026-10-27:** CI runs for release builds only (owner's decision): PRs into and pushes to `release/**` and `main`, no longer `develop`; ticket PRs rely on the local full check (T4, `docs/development.md` §9, APP-019).
- **Changed 2026-10-24:** spike T37 (#72) outcome folded in as D42 (owner decision): the spike could not run (the agent's safety classifier blocked the console injector), so E9 (S9.2 #31, T38-T40 #73-#75, and T37 itself) is on hold and removed from v0.1.0, which ships with Clipboard only. Also dropped the unused `claude-config` app folder and `claude_config_dir()` (D41 removed the dedicated config dir) from D27 and T3.
- **Changed 2026-10-24:** spike T16 (#48) outcome folded in as D41 (owner decision): `--safe-mode` + `--setting-sources ""` with the normal login, contract in `docs/spikes/claude-invocation.json`; no dedicated config dir or `claude_open_login`; translator runs are visible to `claude agents --json`, so T38/T39 exclude them by name and `TranslatorPids`; T17b gained a no-console-flash manual check.
- **Added 2026-10-14:** the Git Flow workflow (D39, `docs/development.md`, project `CLAUDE.md`) and milestone `v0.1.0`; T1, T2, T4, T7 and T31 were aligned with it. No test cases were backfilled into the tickets (owner's choice).
- **Changed 2026-10-15:** automated tests dropped (owner's decision): §4.10 became Verification, CI runs build and lint only, Vitest/RTL/`mockito`/`insta` and the `fake_claude` bin were removed, and test acceptance criteria became behaviour criteria checked by hand. The eval harness (T15) and the manual smoke test stay.
- **Added 2026-10-14:** story S6.3 (keyboard layout fix, T41–T43, D36–D38) was added through the skill's light path after the plan was approved, so it had no independent review.

An independent review (Opus, 2026-10-13) found 28 issues and 7 improvements. It verified CLI claims by running `claude --help`, `claude agents --help` and `claude agents --json` on 2.1.296. The owner accepted all of them, applied as follows:

- **Critical:** T2 scaffolds into a temp folder and never overwrites existing files.
- **Isolation:** `--safe-mode` became T16's first candidate (D12 is provisional); `--system-prompt-file` and `--json-schema`; native `claude.exe` preferred over the `.cmd` shim; auto-updater and nonessential traffic disabled for translator runs; a machine-checkable invocation contract (`claude-invocation.json`).
- **Ollama:** `num_ctx` is always sent (setting, default 8192); the GPU check uses `/api/ps` offload; a **Start Ollama** button on the error.
- **Session injection:** injection runs in a helper process (D31). Leading `!`, `/`, `#` need confirmation (D35); this was chosen over blocking, because a user may want to send a slash command on purpose.
- **Paths and plumbing:** history in `app_local_data_dir()` (not Roaming); `system/paths.rs` owned by T3; logging; capability files and CSP; ts-rs `number` types; per-module command files and API wrappers to avoid merge conflicts; opener and dialog plugins.
- **Ownership and dependencies:** `copy_result` → T12, `complete_setup` → T35; T37 depends on T1; T38 ∥ T39 claim removed; new T0 (repo + MIT license, with approval).
- **CI and release:** frontend build before cargo; `--locked`; binding staleness check; minimal permissions; pinned action SHAs; passive updater install mode; `default-run` and an explicit `fake_claude` bin.
- **Testing:** Windows Sandbox config without vGPU, and the no-winget path tested there.
- **Behaviour:** auto-copy skips when modifier keys are still held; mode detection uses a Latin-word ratio with an expected mode per test case; session polling only while visible; token-warning buttons renamed to Ignore/Retry; autostart starts hidden and both registry entries are cleaned on uninstall; `history_list` issue filter; `lastPopupMode`.
- **Wording:** privacy text says plainly that Claude mode sends text to Anthropic; stale references fixed.
- **Splits:** T17 → T17a/T17b; T25 → T25a/T25b/T25c.

Rejected: nothing. One point was adapted rather than taken as proposed: the prefix guard asks for confirmation instead of refusing (see above).
