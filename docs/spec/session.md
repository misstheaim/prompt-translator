# Claude sessions

The terminal windows that a Translate tab can be bound to, and delivering the accepted text into one. A session target is a terminal window running Claude Code: a Windows Terminal tab or a classic console window. VS Code sessions are not targets in v0.1.0; their text goes through the clipboard. The commands are used by the bound Translate tabs of the main window (SESSION-007).

> Code: `src-tauri/src/target/` (`commands.rs`, `sessions.rs`, `deliver.rs`), `src-tauri/src/system/` (`uia.rs`, `input.rs`, `process.rs`), `src/lib/api.ts`, `src/screens/Translate/` (`TranslateTabs.tsx`, `BindSessionDialog.tsx`, `sessionTabs.ts`)

## Behaviour

### SESSION-001 Session targets
`list_session_targets` (wrapper `listSessionTargets`) returns the targets that can be bound, front to back, as `{ kind: "terminal", hwnd, pid, tabRuntimeId?, title, host }`:
- every visible top-level Windows Terminal window gives one target per tab (`host` `windowsTerminal`, `tabRuntimeId` the tab's UI Automation id, `title` the tab's name); when the tabs cannot be read the window is listed once without `tabRuntimeId`, titled with the window title;
- every visible classic console window gives one target (`host` `conhost`, `title` the window title; `pid` is the console's client process, e.g. `powershell.exe`);
- no other windows: the other terminals of the auto-copy check (POPUP-007) and VS Code are not listed;
- a window whose process is one of the app's own translator runs, or an ancestor of one (e.g. the terminal the app was started from in dev mode), is never listed.
The title is a label only; it changes while Claude works.
Since: #73

### SESSION-002 Bind by focus
`bind_terminal_by_focus(timeoutMs)` (wrapper `bindTerminalByFocus`, default 10 s) waits for the user to bring a terminal window to the front, binds it (with its selected Windows Terminal tab) and returns the target. Only a window that comes to the front after the call counts: a terminal already in front must be left and entered again. The wait is at most 60 s whatever `timeoutMs` is; when no terminal came to the front the call fails with `Timeout`. When 5 bindings exist it fails at once with `InvalidInput` (SESSION-003). No separate `bind_target` call is needed.
Since: #73

### SESSION-003 Bind and unbind
`bind_target(target)` (`bindTarget`) binds a target from the list and returns it with its current title. It fails with `SessionClosed` when the window is gone, belongs to another process or window class, or its tab is gone; with `InvalidInput` "This window runs one of the app's own translations and can't be bound" for a translator run's window; with `Io` "Can't read the tabs of this Windows Terminal window" when the tabs cannot be read. At most 5 bindings exist at a time (counting every bind, also of the same target): the sixth fails with `InvalidInput` "At most 5 Translate tabs can be bound to Claude sessions". Bindings are counted per target, so two Translate tabs may share one; each bind needs one `unbind_target(target)` (`unbindTarget`), and unbinding an unknown target does nothing.
Since: #73

### SESSION-004 Closed targets
While at least one binding exists, a watcher checks every 2 s that each bound window still exists with the same process and window class and that a bound Windows Terminal tab is still there. For each one that is gone it emits the event `target://closed` with `{ hwnd, tabRuntimeId? }` and forgets the binding. A passing failure to read a window's tabs counts as alive. With no bindings the watcher is not running.
Since: #73

### SESSION-005 Deliver to a session
`deliver_to_target(target, text, historyId?, confirmed?)` (`deliverToTarget`) puts the text into the target terminal's input and does not send it: the user presses Enter in Claude. It is meant to follow the user's own action in the app (Accept click or Enter), so that Windows lets the app bring the terminal to the front.
- Blank text fails with `InvalidInput` "There is nothing to send".
- Text starting (after trimming) with `!`, `/` or `#` fails with `PrefixNeedsConfirmation` unless `confirmed` is true: in Claude Code these run a shell command, a slash command or write a memory note.
- The text is pasted with line breaks as `\n` and trailing white space removed. The terminal is brought to the front (restored if minimised), the bound Windows Terminal tab is selected, and the text is pasted: Ctrl+V for Windows Terminal, the console's own Edit > Paste for a classic console. A text of up to 3 lines is pasted once. A text of more than 3 lines is pasted twice (Claude Code collapses such a paste into a `[Pasted text]` placeholder), 200 ms apart; before the second paste the same checks as before the first run again: the terminal must still be in front and no key held or recently used, else the second paste is skipped and the delivery takes the clipboard fallback (SESSION-006) with the cause "The terminal lost the focus" or "The keyboard was in use".
- The text goes through the clipboard (hidden from clipboard history); afterwards, after the last paste, the previous clipboard text is put back, unless the clipboard changed meanwhile.
- On success the outcome is `{ kind: "delivered" }`, and when `historyId` is given the history entry is marked accepted with the session as target (HISTORY-009). A history problem never fails the delivery.
Since: #74 · Changed: #187, #199

### SESSION-006 Clipboard fallback
When the terminal cannot take the text, nothing is pasted, the text is copied to the clipboard instead, the outcome is `{ kind: "deliveredToClipboard", reason }` and the history entry is marked accepted with the session as target all the same. `reason` reads "<cause>. The prompt was copied; paste it with Ctrl+V." Causes: the window has closed, is no longer the bound one, or is a translator run's; the bound tab was not found, cannot be identified or cannot be selected, or is no longer selected; a classic console and a text of several lines ("A classic console window can't take several lines safely (line breaks would act as Enter)"); a modifier key or Enter still held down for 500 ms, or the keyboard in use (input within 300 ms) for 1.5 s; the terminal could not be restored or brought to the front, or lost the focus before the paste; the clipboard in use by another app; Windows blocked the keystroke (is the terminal running as administrator?). If the copy itself fails the call fails with `Io` "Can't write to the clipboard: ..." and the entry is not marked.
Since: #74

### SESSION-007 Bound Translate tabs
The Translate screen has a tab strip (TRANS-013): the default tab plus up to 5 tabs bound to a Claude session. Compact mode (TRANS-016) shows the selected tab without the strip, with a dropdown ("Translate" and each bound tab as ">_ <title>") to switch tabs; in a bound tab its copy button sends instead (a paper-plane icon, tooltip "Send to <title>").
- **Binding:** "+ Bind session" (disabled with the tooltip "At most 5 bound tabs" once 5 exist) opens the dialog "Bind a Claude session". It lists the session targets (SESSION-001: title and "Windows Terminal" or "Console window"; "Looking for terminal windows" while loading, "No terminal windows found." when empty) and has "Click into a terminal…", which binds by focus (SESSION-002) with a 10 s wait and the button text "Click into a terminal (<n> s)" counting down. Picking a target or a successful focus bind adds a tab and selects it, titled with the session's title at that moment (a label only). A failed bind shows an error toast and the dialog stays open; Cancel and Esc close it, but not while a bind is running.
- **Tabs:** each tab has its own text, mode, provider, preset, language pair, result and edited output, and starts empty with the settings' defaults and mode Translate. A tab's "×" ("Unbind and close this tab") closes it and releases its binding. Binding a target that already has a tab selects that tab and releases the extra binding. Tabs are not restored after a restart. Saved settings and preset changes reach every open tab as described in TRANS-013 (a changed language pair replaces the tab's pair; default provider or preset changes leave its selection alone; a provider no longer offered or a deleted preset falls back to the default).
- **Improve writes a Claude prompt** in a bound tab (tooltip "Improve: rewrite as a clearer prompt for Claude"): the run is sent with `purpose: "claudePrompt"` (TRANS-008), so the result is the prompt rewrite of TRANS-017.
- **Sending:** the result's accept button reads "Send to <title>" instead of "Copy" (Ctrl+Shift+Enter as before, disabled for a blank result) and delivers the edited text with the entry's `historyId` (SESSION-005). Success shows the toast "Sent to <title>: press Enter in Claude"; the clipboard fallback (SESSION-006) shows its `reason` as an info toast; other failures show an error toast. If the text starts with `!`, `/` or `#`, a dialog "Send this text?" asks first and sends only after "Send" (with `confirmed`).
- **Closed sessions:** when `target://closed` (SESSION-004) names a bound tab's window (and, if it names a tab id, that tab), the tab closes without unbinding and the toast "<title> closed" shows; the default tab is selected if it was the closed one.
- Opening an entry from History (HISTORY-010) or the popup (POPUP-006) selects the default tab and loads it there.
Since: #75 · Changed: #185, #199

## Known issues

- Only the clipboard's text is restored after a delivery; a clipboard holding other content (an image, files) is emptied instead of restored.

Retired IDs: none
