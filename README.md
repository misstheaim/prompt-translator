# Prompt Translator

A Windows tray app for practising English while you work with Claude Code. You type a prompt in Russian (or any language, or a mix with English); the app translates it, improves it or corrects it, and explains your mistakes. You review the English result and accept it. **Only the English text you accept ever reaches Claude Code**: your original text and the app's instructions are never sent to your own Claude sessions.

It also keeps a history of your translations and a "my mistakes" log, so you can see which errors repeat.

Windows 10 and 11 (64-bit) only.

## Install

1. Download `Prompt Translator_<version>_x64-setup.exe` from the [latest release](https://github.com/misstheaim/prompt-translator/releases/latest).
2. Run it. It installs for your user only: no administrator rights and no UAC prompt. Everything goes to `%LOCALAPPDATA%\Prompt Translator\` and a Start menu shortcut.
3. If Windows shows "Windows protected your PC" (SmartScreen), choose **More info**, then **Run anyway**. The warning appears because the installer is not code-signed with a paid certificate, so Windows doesn't know the publisher yet. The installer is built by the project's GitHub Actions workflow from this repository's source.
4. If the WebView2 Runtime is missing (Windows 11 has it), the installer downloads it, which needs network access.

## First run: the setup wizard

The first start opens a short wizard. You can leave it at any time ("Leave setup") and resume from the banner, or reopen it later in Settings, About, "Run setup again".

1. **Welcome**: what the app does and what it sends where.
2. **Local model**: checks your graphics card and memory, suggests a model size (Small, Medium or Large), installs Ollama with `winget` if it's missing (or opens its download page), starts it, downloads the model and measures how fast it is on your machine.
3. **Claude Code**: finds `claude` and checks that you are signed in.
4. **Defaults**: the default provider (the slow-model case preselects Claude Code), the global hotkey, "Start with Windows" and "Save translations to history".
5. **Done**.

You need at least one working provider: a local model through [Ollama](https://ollama.com), or [Claude Code](https://docs.claude.com/en/docs/claude-code/setup) signed in. Neither is required to install the app, and both can be set up later.

## Using it

### The three modes

| Mode | What it does |
|---|---|
| **Translate** | Faithful translation into English. No rewriting. |
| **Improve** | Makes English (or a translation) clearer and more precise, keeping your intent. Lists the most useful changes. |
| **Correct** | Smallest possible fixes to your English; any non-English words are translated. Every fix is explained in a simple sentence. |

The hotkey popup picks Translate or Correct for you from the text (mostly non-English text gives Translate, mostly English gives Correct; very short text keeps your last mode). Improve is never picked automatically. You can switch the mode with one click. Code, file names, paths, URLs and numbers are kept exactly as you wrote them, and a warning appears if the result lost any of them.

### The main window

Four tabs: **Translate** (type or paste text, pick mode, provider and preset, Run with Ctrl+Enter, edit the result, Copy with Ctrl+Shift+Enter), **History** (search, reopen, copy or delete past translations), **Mistakes** (your most frequent categories and repeated mistakes, and new vocabulary) and **Settings**. Ctrl+Tab and Ctrl+Shift+Tab switch tabs; Ctrl+, opens Settings.

Presets (General, Technical Development, Code Review, Work Communication, and your own) add domain rules to the prompt. Edit them in Settings, Presets.

### The hotkey flow

Select text in any app and press **Ctrl+Shift+Space** (changeable in Settings). A small popup opens next to your cursor and translates the text at once in the detected mode.

- **Enter**: copy the English result to the clipboard (and mark it accepted in History), then paste it where you want.
- **Esc**: cancel and close. **Ctrl+R**: run again. **Tab**: change mode.
- "Open in main window" continues in the full Translate screen.

If nothing is selected, the popup uses what is already on the clipboard.

**Auto-copy.** To grab your selection the app sends **Ctrl+C** to the foreground window for you. Turn this off in Settings (System) if you prefer to copy yourself. Notes:

- Standalone terminals (Windows Terminal, the classic console, WezTerm, Alacritty, mintty) are detected and never receive Ctrl+C, because it would interrupt a running command. In a terminal, copy the text yourself first; the popup then uses the clipboard.
- Terminals **embedded in an IDE** (for example the VS Code integrated terminal) can't be told apart from the editor. There the hotkey does send Ctrl+C, which copies the selection if there is one. If a command is running in that terminal, Ctrl+C can interrupt it, so turn auto-copy off if you use the hotkey there.
- Nothing is sent while Shift, Ctrl, Alt or Win is held, so the app never triggers Ctrl+Shift+C.
- The app restores your previous clipboard text afterwards and keeps the temporary copy out of Windows clipboard history.

### Keyboard layout fix

Typed `ghbdtn` instead of `привет`? Select the text (or put the cursor right after the word) and press **Shift+Pause**. The app replaces it with the same keys on the other layout (US and Russian, both directions) and switches the input language. It uses no model and no history. In terminals it converts the clipboard text instead and tells you to paste with Ctrl+V. Change or disable the hotkey in Settings, "Keyboard layout fix".

### Tray

The app lives in the notification area. Left-click opens the main window; right-click offers Open and Quit. Closing the window keeps the app running in the tray (turn off in Settings to quit on close). Starting the app again just shows the running one.

## Privacy

- **Ollama (local)**: your text goes only to the Ollama server at the address in Settings (by default on your own machine). Nothing leaves your computer.
- **Claude Code**: the text is sent to **Anthropic** through your own `claude` login, like any prompt you type into Claude Code. Use the local model if the text must stay on your machine. Each run is an isolated, short-lived `claude -p` process in an empty folder, with no tools, no MCP servers and none of your Claude settings, and it never touches your own Claude sessions or folders.
- **Auto-copy can capture anything selected** in the foreground app when you press the hotkey, including passwords or private messages. Don't press the hotkey with sensitive text selected, or turn auto-copy off.
- History and the mistakes log are stored only in a local database on your computer. You can turn saving off, or clear everything, in Settings, History.
- The app has no telemetry and no account. Its only network traffic is to your chosen provider, to Ollama's download servers when the wizard downloads a model, and to GitHub for update checks.
- Only the English text you accept (copy) reaches your clipboard; nothing is injected into other apps except Ctrl+C for auto-copy and the layout fix's paste.

## Updates

The app checks GitHub for a new release 10 seconds after it starts (turn this off in Settings, About) and shows a banner: **Install and restart**. The update is downloaded, checked against the project's signing key and installed in passive mode (a progress bar, no questions); your settings, presets and history are kept. You can also click "Check for updates" in Settings, About.

## Uninstall

Open Windows Settings, Apps, **Prompt Translator**, Uninstall (or run `uninstall.exe` in the install folder). The confirm page has **"Delete the application data"**, ticked by default.

- Ticked: removes the settings, presets, history, logs and the app's WebView2 profile (`%APPDATA%\com.misstheaim.prompttranslator\` and `%LOCALAPPDATA%\com.misstheaim.prompttranslator\`).
- Unticked: keeps them, so a later install picks them up.
- Either way: removes the program files, shortcuts, the Apps entry and the "Start with Windows" entry.

**Ollama and its models stay installed**, because other tools may use them. To remove a model, list and remove it in a terminal:

```
ollama list
ollama rm <model>
```

(for example `ollama rm qwen3.5:4b`). To remove Ollama itself, uninstall "Ollama" in Windows Settings, Apps. Your own Claude Code configuration is never touched.

## Troubleshooting

| Problem | What to do |
|---|---|
| "Can't reach Ollama" | Ollama isn't running. Start it from the error banner, from the setup wizard, or from the Start menu. Check the endpoint in Settings (default `http://localhost:11434`). |
| `Model "<name>" is not installed` | Run `ollama pull <name>` (see `ollama list` for what you have), or pick another model in Settings, or repeat the wizard's model step. |
| "claude.exe was not found" | Install [Claude Code](https://docs.claude.com/en/docs/claude-code/setup), or set the path to `claude.exe` in Settings, Claude Code ("Browse"). Check with `claude --version`. |
| "Claude Code is not signed in" | Run `claude` once in a terminal, sign in, then choose "Check" in Settings. |
| "The popup hotkey is off" banner | Another app uses that combination. Choose another one in Settings (System). The layout fix hotkey reports the same way. |
| Slow answers | The local model may not fit your graphics card. Pick a smaller model, or switch the default provider to Claude Code. |
| "The request timed out" | Retry, or raise the timeout in Settings. A model that has to load from disk is slower on the first request. |
| The popup shows the old clipboard, not my selection | You are in a terminal, held a modifier key, or nothing was selected; copy the text first, or release the keys before pressing the hotkey. |
| Something else | The log files are in `%LOCALAPPDATA%\com.misstheaim.prompttranslator\logs\` (Settings, About has an Open button for the data folder). |

## Developing

- Workflow, branches, commands and releases: [`docs/development.md`](docs/development.md)
- Setting up a machine: [`docs/dev-setup.md`](docs/dev-setup.md)
- Building the installer and the Windows Sandbox test: [`docs/build.md`](docs/build.md)
- Signing keys and the release workflow: [`docs/release.md`](docs/release.md)
- Checking a build before a release: [`docs/smoke-test.md`](docs/smoke-test.md)
- How the app behaves: [`docs/spec/README.md`](docs/spec/README.md); plan: [`docs/plans/prompt-translator.md`](docs/plans/prompt-translator.md)

Quick start from a clone, after the tools in `dev-setup.md` are installed:

```
npm ci
npm run tauri dev
```

## License

[MIT](LICENSE)
