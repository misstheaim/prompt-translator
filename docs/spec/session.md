# Claude sessions

The terminal windows that a Translate tab can be bound to, and delivering the accepted text into one. A session target is a terminal window running Claude Code: a Windows Terminal tab or a classic console window. VS Code sessions are not targets; their text goes through the clipboard. The commands exist in the core and in `api.ts`; no screen uses them yet.

> Code: `src-tauri/src/target/` (`commands.rs`, `sessions.rs`, `deliver.rs`), `src-tauri/src/system/` (`uia.rs`, `input.rs`, `process.rs`), `src/lib/api.ts`

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
- The text is pasted with line breaks as `\n` and trailing white space removed. The terminal is brought to the front (restored if minimised), the bound Windows Terminal tab is selected, and the text is pasted once: Ctrl+V for Windows Terminal, the console's own Edit > Paste for a classic console.
- The text goes through the clipboard (hidden from clipboard history); afterwards the previous clipboard text is put back, unless the clipboard changed meanwhile.
- On success the outcome is `{ kind: "delivered" }`, and when `historyId` is given the history entry is marked accepted with the session as target (HISTORY-009). A history problem never fails the delivery.
Since: #74

### SESSION-006 Clipboard fallback
When the terminal cannot take the text, nothing is pasted, the text is copied to the clipboard instead, the outcome is `{ kind: "deliveredToClipboard", reason }` and the history entry is marked accepted with the session as target all the same. `reason` reads "<cause>. The prompt was copied; paste it with Ctrl+V." Causes: the window has closed, is no longer the bound one, or is a translator run's; the bound tab was not found, cannot be identified or cannot be selected, or is no longer selected; a classic console and a text of several lines ("A classic console window can't take several lines safely (line breaks would act as Enter)"); a modifier key or Enter still held down for 500 ms, or the keyboard in use (input within 300 ms) for 1.5 s; the terminal could not be restored or brought to the front, or lost the focus before the paste; the clipboard in use by another app; Windows blocked the keystroke (is the terminal running as administrator?). If the copy itself fails the call fails with `Io` "Can't write to the clipboard: ..." and the entry is not marked.
Since: #74

## Known issues

- Only the clipboard's text is restored after a delivery; a clipboard holding other content (an image, files) is emptied instead of restored.

Retired IDs: none
