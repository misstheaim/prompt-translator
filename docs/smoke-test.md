# Smoke test

> A manual checklist to run on an installer built from the release branch before a release (development.md §5 step 4). It checks what no automated check covers. Tick each box; write down anything that fails with the step number. The expected behaviour of every step is in [`docs/spec/`](spec/README.md).

Result: build `<version>` on `<date>` by `<name>`: pass / fail (list failures here).

## 0. Setup

- [ ] Build the installer: see [`build.md`](build.md) (a local build needs the signing key, or `--config '{"bundle":{"createUpdaterArtifacts":false}}'`, see [`release.md`](release.md) §1). For a real release, use the installer from the CI draft release instead.
- [ ] A machine (or a user profile) where the app is not installed, with: Ollama running and a model pulled (`ollama list`), and Claude Code installed and signed in (`claude --version`).
- [ ] Sample texts ready:
  - Russian: `Исправь ошибку в функции parseConfig в файле src/config.ts, она падает на пустом значении`
  - English with mistakes: `Please fix the bug in function parseConfig, it crash when value are empty`
  - A text in another language, not Russian: Spanish `Arregla el error en la función parseConfig, falla con un valor vacío`, and German `Behebe den Fehler in der Funktion parseConfig, sie stürzt bei leerem Wert ab`.
  - Mixed: `Добавь retry в fetchUser и покажи error message пользователю`.

## 1. Install and first run

- [ ] Run the installer. It needs no admin prompt. Windows SmartScreen shows "More info, Run anyway", and the install finishes.
- [ ] The Start menu has "Prompt Translator"; the app opens and a tray icon appears.
- [ ] The setup wizard opens on first start (SETUP-008) and goes through Welcome, Local model, Claude Code, Defaults, Done.
- [ ] Local model step: the graphics card and recommended size show; download (if the model is missing) shows progress and Cancel works; the speed test shows a result.
- [ ] "Leave setup" shows the "Setup isn't finished" banner; "Resume setup" returns to the same step.
- [ ] Finish opens the Translate tab; the wizard doesn't come back after a restart.

## 2. Modes by provider

Run each cell on the Translate tab with the matching sample text. For every run check that the output is English, that code names (`parseConfig`, `src/config.ts`) are unchanged, that no token warning shows wrongly, and that "Copy" puts the (edited) output on the clipboard with the toast "Copied".

| | Ollama | Claude Code |
|---|---|---|
| Translate, Russian | [ ] | [ ] |
| Translate, mixed Russian and English | [ ] | [ ] |
| Translate, Spanish | [ ] | [ ] |
| Translate, German | [ ] | [ ] |
| Improve, English | [ ] | [ ] |
| Correct, English with mistakes | [ ] | [ ] |
| Correct, Russian or Spanish (non-English parts translated) | [ ] | [ ] |

- [ ] Correct and Improve show the "Changes" diff and an issue list with category chips; Translate shows neither.
- [ ] A text of 8001 characters turns the counter red, "(too long)", and Run is disabled.
- [ ] A text with a technical token that the model drops shows the token-warning banner; "Ignore" hides it and "Retry" runs again.
- [ ] Cancel during a run returns to the previous state without an error.
- [ ] Switching presets changes the default mode; a custom preset can be created, edited and deleted in Settings, Presets.

## 3. Popup and hotkey

- [ ] Select text in a browser, press Ctrl+Shift+Space: the popup appears centred on the monitor under the cursor, shows the source and translates at once; the mode matches the language (Russian gives Translate, English gives Correct).
- [ ] Enter copies the result and closes the popup; paste it into an editor to check; the History entry is marked accepted.
- [ ] Esc closes the popup and cancels a running request; Ctrl+R runs again; Tab and Shift+Tab change the mode and rerun.
- [ ] "Open in main window" shows the same input and result in the Translate tab.
- [ ] Clicking outside the popup hides it.
- [ ] With nothing selected, the popup uses the clipboard text; with an empty clipboard it shows "Select or copy some text first".
- [ ] Token warning: a text with a path or an identifier that the model changes shows the banner in the popup; Ignore and Retry work.
- [ ] Error in the popup: stop Ollama (see section 4), trigger the popup with the Ollama default: the error banner shows with Retry and "Start Ollama", and no "Open settings".
- [ ] Hotkey taken: set the popup hotkey to a combination another app owns (or start a second tool that holds it): the warning banner "The popup hotkey is off..." shows with "Open settings"; choosing a free hotkey in Settings clears it without a restart.

## 4. Failures

- [ ] Ollama stopped (quit it from its tray icon): Translate with Ollama shows "Can't reach Ollama" with "Start Ollama"; the button starts it, and Retry works.
- [ ] Model not pulled: set the Ollama model to a name that doesn't exist: the error says the model isn't installed with the `ollama pull` hint.
- [ ] claude.exe path wrong: set a wrong path in Settings, Claude Code: Translate with Claude shows "claude.exe was not found"; "Install help" opens the docs; fixing the path (or clearing it) works again.
- [ ] Claude Code not signed in (optional, on a profile where it isn't): "Claude Code is not signed in" and the status badge "Not signed in".
- [ ] Settings, Check/Test buttons show the right badges for each of the states above.

## 5. History and mistakes

- [ ] With history on, each successful run appears in History (mode badge, provider, source and output previews); search finds it; "Load more" appears after 50 entries (optional).
- [ ] The detail panel shows the issues; "Copy" marks the entry accepted; "Open in Translate" reloads it; "Delete" asks and removes it.
- [ ] Mistakes shows category bars, recurring mistakes and vocabulary after a few Correct runs; clicking a recurring pair opens History filtered to it.
- [ ] History off (Settings, Save translations to history, Save): new runs are not added, the History tab shows the "History is turned off" banner. Turn it on again: new runs are saved again.
- [ ] "Clear all history" asks for confirmation, then History and Mistakes are empty.

## 6. Tray, window and system

- [ ] Left-click on the tray icon shows the main window; right-click shows Open, Unload model and Quit. After a translation, "Unload model" empties `ollama ps` and the main window shows "Model unloaded" (with Ollama stopped: "No model loaded"). Quit after a translation: `ollama ps` is empty within a few seconds; with Ollama stopped the app quits at once. The top-bar "Unload model" button works the same and is disabled while a translation runs.
- [ ] Closing the window keeps the app in the tray; with "Keep running in the tray" off, closing exits the app.
- [ ] Starting the app a second time shows the running window and no second tray icon.
- [ ] "Start with Windows" on, then sign out and in (real machine, not the Sandbox): the app starts hidden in the tray. Off removes it (check Task Manager, Startup apps).
- [ ] Ctrl+Tab, Ctrl+Shift+Tab and Ctrl+, switch tabs.
- [ ] Settings, About shows the version, both folders open in Explorer, and "Check for updates" says "You're up to date." (or finds a newer release).

## 7. Auto-copy

Set a known text on the clipboard first (for example `CLIPBOARD-OLD`). After each step check what the popup shows as source, and afterwards check that the clipboard text is still `CLIPBOARD-OLD`.

- [ ] Browser (Chrome or Edge): select a sentence, press the hotkey: the popup shows the selection.
- [ ] Browser with no selection: the popup shows `CLIPBOARD-OLD`.
- [ ] Editor (Notepad): selection works the same way.
- [ ] VS Code with no selection: the popup shows `CLIPBOARD-OLD`, not the whole line.
- [ ] Windows Terminal, conhost (`cmd.exe`) and, if installed, WezTerm, Alacritty or mintty: with a command running (for example `ping -t localhost`), press the hotkey with some text selected: the running command is **not** interrupted and the popup shows the clipboard text.
- [ ] VS Code integrated terminal: the hotkey sends Ctrl+C (known limitation, see README); with a text selected the popup shows it.
- [ ] The default hotkey contains Shift: in Chrome with a selection, pressing it opens the popup with the selection and does not open DevTools (the app waits for the keys to be released before sending Ctrl+C).
- [ ] Auto-copy off in Settings: the popup always shows the clipboard.
- [ ] The restored clipboard text does not show in Windows clipboard history (Win+V).

## 8. Keyboard layout fix

Default hotkey Ctrl+Shift+` (an existing settings.json keeps its saved hotkey). Type `ghbdtn` with the English layout, and `руддщ` with the Russian layout.

- [ ] Editor (Notepad), selection: select `ghbdtn`, press the hotkey: it becomes `привет`, the input language switches to Russian. Select `руддщ`: it becomes `hello` and the input language switches to English.
- [ ] Editor, no selection: type `ghbdtn` and leave the cursor after it; the hotkey converts the last word only.
- [ ] Browser (an input field or a text area): the same two cases.
- [ ] VS Code, no selection: the last word is converted, not the whole line.
- [ ] Terminal (Windows Terminal): copy `ghbdtn`, press the hotkey: the notification "Layout fixed: paste with Ctrl+V" shows; Ctrl+V pastes `привет`. With an empty or already-correct clipboard: "Copy some text first".
- [ ] Input-language switch: with "Switch input language after fixing" off in Settings, the language does not change; on, it follows the converted text.
- [ ] Punctuation: `ghbdtn,` and `руддщ.` convert by physical key; digits and spaces stay.
- [ ] The clipboard text you had before is back after a replacement.
- [ ] Layout fix off in Settings: the hotkey does nothing. Setting it to the same hotkey as the popup is refused with "Must be different from the global hotkey".

## 9. Updates

Needs a published newer release than the installed build (or a local test release); the simplest way is to install the previous release, then check the new one from it. Do this on a real machine or in the Sandbox section.

- [ ] Within 10 seconds of starting the older build, the update banner "Version <x> is available." shows (with "Check for updates on startup" on); Dismiss hides it.
- [ ] "Install and restart": progress shows, the installer runs in passive mode (progress bar only), the app restarts as the new version (Settings, About shows it).
- [ ] After the update, settings, presets, history, the tray icon, shortcuts and the "Start with Windows" entry are unchanged.

## 10. Windows Sandbox

A clean machine with no GPU and no `winget`. How to start it, and what is installed in it: [`build.md`](build.md), "Clean-machine test in Windows Sandbox" (it uses [`docs/sandbox/no-gpu.wsb`](sandbox/no-gpu.wsb)). The sandbox is discarded when closed, so start a fresh one for each part below.

**A. Install and the wizard without a GPU or winget**
- [ ] Start `docs/sandbox/no-gpu.wsb` and run the installer from `Desktop\installer`. SmartScreen shows; the install finishes without admin rights; the WebView2 bootstrapper runs if needed.
- [ ] The wizard's Local model step says "No supported graphics card (...)" with "Set up anyway".
- [ ] After "Set up anyway", "Install Ollama" fails (no winget) and the button changes to "Open the Ollama download page" with a note to install it and "Check again". The page opens (or the link works).
- [ ] Skip both providers: Local model gives "Skip", Claude Code (not found) gives "Skip" and "Install instructions"; Defaults shows the "no working provider" warning; Done shows the warning that translating fails until a provider is set up; Finish works.
- [ ] The main window opens; a translation fails with an error banner and an "Open settings" button, and the app doesn't crash.
- [ ] Optional, on the same sandbox: install Ollama from the download page, run "Check again" in the wizard (Settings, About, "Run setup again"), and measure a CPU-only model if time allows.

**B. Update from the previous release**
- [ ] In a fresh sandbox, install the previous release's installer (download it in the sandbox, as the mapped folder holds only the new build), start the app and let it finish the first run.
- [ ] Change a setting and create a history entry (history on), then install the new build over it (run the new installer, choose "Uninstall before installing", the default), or use the in-app update when a published release exists.
- [ ] The new version starts with the same settings and history, the shortcut still works and the "Apps" entry shows the new version.

**C. Uninstall with data deletion (default)**
- [ ] Install, start the app once (so the data folders exist), turn "Start with Windows" on, then quit the app from the tray.
- [ ] Settings, About, "Uninstall Prompt Translator…" (installed build only; hidden in `npm run tauri dev`), confirm: the app closes and the uninstaller opens. Or Windows Settings, Apps, Prompt Translator, Uninstall: the confirm page shows "Delete the application data", checked.
- [ ] After it finishes, these are gone (check in Explorer and with the cmd.exe commands below): `%LOCALAPPDATA%\Prompt Translator\`, `%APPDATA%\com.misstheaim.prompttranslator\`, `%LOCALAPPDATA%\com.misstheaim.prompttranslator\` (including `EBWebView`), the Start menu shortcut and the Apps entry.

```
dir "%LOCALAPPDATA%\Prompt Translator"
dir "%APPDATA%\com.misstheaim.prompttranslator"
dir "%LOCALAPPDATA%\com.misstheaim.prompttranslator"
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "Prompt Translator"
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" /v "Prompt Translator"
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Prompt Translator"
reg query "HKCU\Software\misstheaim\Prompt Translator"
```

Each command must report that the folder or value is not found.

**D. Uninstall keeping the data**
- [ ] In a fresh sandbox, install, start once, turn "Start with Windows" on, quit from the tray.
- [ ] Uninstall and untick "Delete the application data".
- [ ] The two `com.misstheaim.prompttranslator` folders are still there (with `settings.json` and `history.db`); the program folder, shortcut, Apps entry and both autostart values are gone (the same commands as above).
- [ ] Install again: the app starts with the old settings and history.

## 11. Finish

- [ ] No unexpected console window opens at any point (release build).
- [ ] The log file in `%LOCALAPPDATA%\com.misstheaim.prompttranslator\logs\` has no panics or unexplained errors.
- [ ] Note the result at the top of this file in the release PR description (build, date, pass or fail, failures).
