# Spike T58: paste into a bound terminal window, open VS Code sessions by link

Task T58 (#148), story S9.1 (#30), epic E9 (#9), decision D47. Goal: a GO / NO-GO and a working recipe for the two delivery routes of D47, without writing into another process's console:

1. **Terminal:** bring the bound terminal window to the front, select the bound Windows Terminal tab (UI Automation), paste once with Ctrl+V through the clipboard, restore the clipboard. **Never send Enter.**
2. **VS Code extension:** open `vscode://anthropic.claude-code/open?session=<id>&prompt=<text>`.

- **Status:** the matrix was run **automatically by an agent** on 2026-10-31 (owner decision on #148: the owner was off the keyboard and mouse). Every cell has a result; cells that would need a submit, a second monitor or a saved VS Code session are marked **not checkable** with the reason. The verdicts below are **proposed**: the owner reviews them before T38–T40 (#73–#75) start.
- **Date:** 2026-10-31. **Machine:** Windows 11 Pro 10.0.26200, one monitor, Windows Terminal (all windows in one `WindowsTerminal.exe` process), Claude Code CLI 2.1.316, VS Code extension `anthropic.claude-code` 2.1.314.
- **Safety rules of the automated run:** Enter (or any other submit) was **never** pressed in any Claude session, throwaway ones included. Only the agent's own windows were targeted: new Windows Terminal windows (`wt -w new`), a conhost window, a WinForms "occluder" window standing in for Notepad / a browser, and VS Code windows on scratch folders, with throwaway `claude` sessions in a scratch folder, started with an environment cleaned of the parent session's `CLAUDE*` variables. Every pre-existing window and process was recorded first and never targeted; before each paste the driver checked the target HWND was one it had created. All test windows and processes were closed at the end and the clipboard put back.
- **Tool:** `spikes/paste/` (throwaway Rust CLI, its own Cargo project, not part of the app or its checks).

## Result in short

| Route | Verdict | Basis |
|---|---|---|
| Terminal focus-and-paste (Windows Terminal) | **Proposed GO** | AC2 met: the bound window and tab got the text in front (H-1), behind another window (H-2), minimised (H-3) and with another tab selected (H-5) or another WT window last active (H-7); nothing reached other windows or tabs. Multi-line arrives as **one** input (X-2: a single `[Pasted text #1 +4 lines]` placeholder; a second paste of the same text expands it to all 5 lines in one input box); that the user's Enter then sends it as one prompt was **not checked** (no Enter by rule). Failures are detectable: closed window 10, closed tab 13, keys held 21, foreground refused 20, all with nothing pasted. Conditions: deliver only right after the user's own input in the app (F-1; F-2 is refused), guard `!` (X-4 switches Claude to shell mode) and `/`, and check that no key at all was pressed recently (R-1b). |
| Terminal focus-and-paste (conhost) | **Proposed GO with a different paste** | Ctrl+V does **not** paste into Claude Code in a classic console (H-6); conhost's own Edit > Paste (`WM_COMMAND 0xFFF1` to the console window) does, one line only: multi-line text is refused (its line breaks could act as Enter; not tried, by rule). Low priority: the owner's setup is Windows Terminal. |
| VS Code link | **Proposed NO-GO for delivery into an open session; GO only as "open a new conversation with this text"** | AC3 not met for the T38–T40 use case: the link never put text into an **existing** session's input in this run, and per the extension code a session already open in an editor tab drops the prompt (V-1 for a saved session not checkable, see below). What works (V-4, V-7, V-8, V-9): a **new** conversation tab with the text in its input, in the **last active** VS Code window (V-5), up to roughly 16,000 characters (a 42,041-char URL fails with exit 40), and only after VS Code's "Allow 'Claude Code for VS Code' extension to open this URI?" confirmation, which appears on every link until the user ticks "Do not ask me again". |

### Found before the owner run (read-only)

- **Windows Terminal exposes tabs through UIA**, confirmed by `list-terminals` (see [list-terminals output](#list-terminals-output)): each tab is a `TabItem` with a name, a RuntimeId and a working `SelectionItemPattern.IsSelected`. The window title and the tab name are Claude's task summary with a spinner glyph (`◐ …`), so neither identifies a session; binding has to be by HWND + pid + tab RuntimeId, as the ticket says.
- **VS Code link, open sessions (static read of `extension.js` 2.1.314, to be confirmed in V-1/V-2):** the `/open` handler validates `session` (a UUID; an invalid id makes the link do nothing at all) and calls `claude-vscode.primaryEditor.open(session, prompt)` → `createPanel`. In `createPanel`:
  - session **already open in an editor tab** (or a remembered tab being revived): the tab is revealed and, if a prompt was given, VS Code shows the info message *"Session is already open. Your prompt was not applied — enter it manually."* **The prompt is dropped.**
  - session **held by the sidebar**: the sidebar is revealed and gets `activate_session` with `initialPrompt` = the prompt, **and** the same "not applied" message is shown. Whether the sidebar webview fills the input then is unknown (V-2).
  - session **not open anywhere** (closed): a new editor tab is created with the session and the prompt (`setupPanel(..., prompt, ...)`), so the text should land in its input (V-3).
  - no `session`: a new conversation with the prompt (the owner's check from the ticket, V-4).

  If V-1 confirms this, the VS Code route only works for sessions that are **not** open, which is the opposite of the T38–T40 use case (deliver into the session the user is working in). That would be a NO-GO for "open session" delivery unless the sidebar case (V-2) works.

### Found in the automated run

- **Windows Terminal:** focus-and-paste works for every host state that could be produced on one monitor; the paste lands only in the bound tab. All WT windows (the agent's and the coordinator's) were one `WindowsTerminal.exe` pid, so only the HWND tells windows apart; tab RuntimeIds stayed stable while other windows and tabs opened and closed.
- **Claude Code's paste handling** (CLI 2.1.316): one line is inserted as text at the cursor (an existing draft is kept, C-3); multi-line and long text become a `[Pasted text #N]` placeholder with the hint "paste again to expand", and pasting the **same** text again expands it in place instead of adding a second copy. A paste starting with `!` switches the input to shell mode (`! for shell mode`); `/` and `#` stay plain text.
- **Focus:** `SetForegroundWindow` works right after the spike's own window got the user's input (F-1, every click-triggered run); after a delay with another app in front it is refused (F-2, exit 20). The Alt-tap workaround (`--focus alt`) succeeds there (F-3), `AttachThreadInput` does not (F-4).
- **Typing race:** modifiers are caught (R-1a, exit 21) but plain letters typed while the window switches land in Claude's input around the pasted text (R-1b). The app should also check `GetLastInputInfo` (no input for ~300 ms) before pasting.
- **conhost:** Ctrl+V reaches Claude Code as a key, not a paste (nothing appears); conhost's system-menu Paste command works, also with the window in the background.
- **VS Code:** every `vscode://anthropic.claude-code/...` link first shows VS Code's "Allow 'Claude Code for VS Code' extension to open this URI?" dialog (Open / Cancel, "Do not ask me again for this extension"). The link goes to the **last active** VS Code window, even with a session id from another window's folder. A session id of an open session that has no saved transcript yet opens a **new** conversation tab (with a new session id) and leaves the open session's input alone. VS Code sessions **do** appear in `claude agents --json` and `~/.claude/sessions/` (`entrypoint: "claude-vscode"`), unlike the ticket assumed.
- **Not checkable in this run:** anything that needs a submitted prompt (Enter, Claude busy) and anything that needs a **saved** VS Code session (V-1 proper, V-2, V-3, V-6): creating one needs a submit, and writing a synthetic transcript into `~/.claude/projects` was refused by the agent's permission classifier. Those cells keep the static reading of the extension code above.

## The spike CLI

Build (from the repo root; any `CARGO_TARGET_DIR` works, default is `spikes/paste/target/`, ignored by git):

```powershell
cargo build --manifest-path spikes/paste/Cargo.toml --locked
$spike = "spikes\paste\target\debug\paste-spike.exe"   # or <CARGO_TARGET_DIR>\debug\paste-spike.exe
```

| Subcommand | What it does | Touches |
|---|---|---|
| `list-terminals [--all] [--title-chars <n>]` | Top-level terminal windows (the app's terminal classifier, POPUP-007: classes `CASCADIA_HOSTING_WINDOW_CLASS`, `ConsoleWindowClass`; exes `WindowsTerminal.exe`, `wezterm-gui.exe`, `alacritty.exe`, `mintty.exe`) with HWND, pid, exe, class, state (normal / minimised / hidden), foreground, monitor (`primary` or `other@x,y`), Z position among visible windows, title; for Windows Terminal every tab (UIA `TabItem` name, RuntimeId, selected). Prints a ready `bind:` command line per window or tab. `--all` adds hidden windows; `--title-chars` cuts titles. | Nothing (read-only) |
| `paste --hwnd <h> --text <file> [--pid <p>] [--tab <runtime-id>] [--own-window \| --delay <ms>] [--focus plain\|alt\|attach] [--restore-delay <ms>] [--via auto\|ctrl-v\|menu] [--dry-run]` | The terminal recipe below. Prints a timestamped line per step. `--dry-run` runs only the read-only checks (window, pid, terminal, tab found). `--via`: `ctrl-v`, or `menu` = conhost's Edit > Paste; `auto` (default) picks `menu` for conhost windows. | Focus, tab selection, clipboard, one Ctrl+V (or one conhost Paste command) |
| `vscode --text <file> [--session <id>] [--print-only]` | Builds the link and opens it with `ShellExecuteW("open")` (what `Start-Process` does). `--print-only` prints the link and its length without opening it. | Opens a `vscode://` link |
| `sessions [--limit <n>] [--entrypoint <name>]` | Newest session transcripts in `~/.claude/projects/<project>/<id>.jsonl` with their `entrypoint` (`claude-vscode`, `cli`, …). Never prints message content. | Nothing (read-only) |

`--hwnd` takes hex (`0x500dc2`) or decimal. The text file is UTF-8; a BOM is dropped, line breaks are normalised, **trailing line breaks are removed** (a paste ending in a line break could act as Enter in a host without bracketed paste), and the text goes on the clipboard with CRLF line breaks (what a copy from Notepad gives). The CLI prints the text's length and line count, and a note when it starts with `!`, `/` or `#` (D35; the spike does not guard, the app will).

### Terminal recipe as implemented (`spikes/paste/src/paste.rs`)

| # | Step | API / check | Timing | Failure → exit |
|---|---|---|---|---|
| 1 | Read the text file | UTF-8, BOM, trim trailing line breaks | – | 3 |
| 2 | Bound window still valid | `IsWindow(hwnd)`; `GetWindowThreadProcessId` = `--pid`; class/exe is a terminal | – | 10 / 11 / 12 |
| 3 | Bound tab found (Windows Terminal, `--tab`) | `CoInitializeEx(MTA)`, `CUIAutomation.ElementFromHandle(hwnd)`, `FindAll(Descendants, ControlType = TabItem)`, match `GetRuntimeId()` | – | 13 (1 on UIA failure) |
| 4 | Trigger | `--own-window`: the spike's own top-level (topmost) window, triggers on Enter **key-up** after an Enter key-down in that window (so the key-up of the Enter that started the command doesn't count), or on a left-button **up** after a down in it (the automated run clicked it, never pressing Enter); the process then owns the foreground and got the last input, like the app after the user's Enter. The window stays open until the end. Otherwise wait `--delay` ms. | – | 4 (window closed) |
| 5 | No key held | `GetAsyncKeyState` for Shift, Ctrl, Alt, LWin, RWin **and Enter** (an Enter still down when the terminal takes focus would auto-repeat into Claude and send) | poll 10 ms, up to 500 ms | 21 |
| 6 | Re-check the window (closed since binding?) | step 2 again | – | 10 / 11 / 12 |
| 7 | Restore if minimised | `IsIconic` → `ShowWindow(SW_RESTORE)`, poll `IsIconic` | up to 1000 ms | 15 |
| 8 | Bring to front | `--focus plain` (default): `SetForegroundWindow`. `alt`: `SendInput` Alt down, unassigned key 0xE8, Alt up, then `SetForegroundWindow`. `attach`: `AttachThreadInput` to the foreground thread, `SetForegroundWindow` + `BringWindowToTop`, detach. Then poll `GetForegroundWindow() == hwnd`. | up to 500 ms | 20 |
| 9 | Select the bound tab | `SelectionItemPattern.Select()` if `CurrentIsSelected` is false, poll `CurrentIsSelected` | up to 500 ms | 14 |
| 10 | Settle | let the terminal route input to the tab | 100 ms | – |
| 11 | Last checks | step 5 again; `GetForegroundWindow() == hwnd`; tab still selected | – | 21 / 22 |
| 12 | Save the clipboard | `OpenClipboard` (10 tries, 10 ms apart), read `CF_UNICODETEXT` (text only, like the app's layout fix) | – | 30 |
| 13 | Put the text there, hidden | `EmptyClipboard`, `SetClipboardData(CF_UNICODETEXT)` + `ExcludeClipboardContentFromMonitorProcessing` and `CanIncludeInClipboardHistory` = DWORD 0 (not in Win+V history, not seen by monitors) | – | 30 (saved text put back) |
| 14 | Paste | one `SendInput`: Ctrl down, V down, V up, Ctrl up. **No Enter, ever.** conhost (`--via menu`, the default there): `SendMessageW(hwnd, WM_COMMAND, 0xFFF1, 0)` instead (conhost's Edit > Paste); multi-line text is refused right after step 2 for this route | – | 31 (UIPI: elevated target; saved text put back); 33 (multi-line for conhost) |
| 15 | Restore the clipboard | wait, then if `GetClipboardSequenceNumber` is unchanged since step 13, write the saved text back (hidden); if it changed (the user copied something), leave it | `--restore-delay`, default 200 ms (the layout fix's `PASTE_DELAY`) | 32 |

Exit codes (every outcome has its own; `main.rs` `Exit`):

| Code | Meaning | Pasted? |
|---|---|---|
| 0 | `paste`: Ctrl+V sent, clipboard restored (or left alone because the user copied meanwhile). `vscode`: link handed to the shell (does **not** prove delivery). | yes |
| 1 | Unexpected COM / UIA failure | no |
| 2 | Bad arguments | no |
| 3 | Text file missing, not UTF-8 or empty | no |
| 4 | `--own-window` closed instead of Enter | no |
| 10 | Window gone (`IsWindow` false): terminal closed | no |
| 11 | HWND belongs to another pid (reused handle) | no |
| 12 | Not a terminal window | no |
| 13 | Tab RuntimeId not found (tab closed, or the id changed) | no |
| 14 | Tab select failed / didn't become selected | no |
| 15 | Still minimised after `SW_RESTORE` | no |
| 20 | Foreground refused (window not in front after `SetForegroundWindow`) | no |
| 21 | Modifier or Enter held (user typing) | no |
| 22 | Window or tab lost the foreground / selection right before the paste | no |
| 30 | Clipboard busy | no |
| 31 | `SendInput` blocked | no |
| 32 | Pasted, but the clipboard restore failed | yes |
| 33 | Multi-line text refused for conhost's menu paste (line breaks could act as Enter) | no |
| 40 | `ShellExecuteW` refused the `vscode://` link | no |

What the exit code can't tell: whether the terminal actually read the clipboard within the restore delay (X-3), and whether the text went to Claude's input or elsewhere inside the right tab. Both are checked by eye in the matrix.

### VS Code recipe as implemented (`spikes/paste/src/vscode.rs`)

`vscode://anthropic.claude-code/open?session=<id>&prompt=<text>`; without `--session`, `?prompt=<text>` (new conversation). Both values are percent-encoded byte by byte except RFC 3986 unreserved characters (space → `%20`, line break → `%0A`). Opened with `ShellExecuteW(NULL, "open", url, NULL, NULL, SW_SHOWNORMAL)`; a result ≤ 32 is exit 40. The shell starts VS Code's registered URL handler (`Code.exe --open-url -- "<url>"`), which hands the URI to one VS Code window, then to the extension's `registerUriHandler` (`/open`: reads `session` and `prompt` with `URLSearchParams`, drops invalid session ids silently).

## list-terminals output

One read-only run by the implementing agent on 2026-10-31 (`list-terminals --title-chars 24`), titles redacted to a short prefix:

```text
window hwnd=0x500dc2 pid=8252 exe=WindowsTerminal.exe class=CASCADIA_HOSTING_WINDOW_CLASS state=normal foreground=yes monitor=primary z=18
  title="◐ Coordinator mode miles…"
  tab runtime-id=42.165152658.4.9 selected=yes name="◐ Coordinator mode miles…"
    bind: paste --hwnd 0x500dc2 --pid 8252 --tab 42.165152658.4.9 --text <file>
1 terminal window(s); z = position in the top-level Z order (0 = front)
```

- One Windows Terminal window (the one running the owner's Claude session), one tab; no visible conhost window. Hidden windows (`--all`) were not listed.
- UIA works from a plain console process: `ElementFromHandle` + `FindAll(TabItem)` found the tab, `GetRuntimeId` and `SelectionItemPattern.CurrentIsSelected` answered.
- The tab name equals the window title: Claude's task summary with a spinner glyph that changes while Claude works. Not usable as an id.
- RuntimeId `42.165152658.4.9`: the second part is `0x9D80792`, which looks like a child window handle (the XAML island) rather than the top-level HWND; the last parts index the element. Whether it survives tab reordering, dragging a tab to another window, or a WT restart is part of H-5 / R-3.
During the automated matrix run (after H-7; the agent's windows A and B plus the coordinator's terminal, titles cut):

```text
window hwnd=0xb060b26 pid=8252 exe=WindowsTerminal.exe class=CASCADIA_HOSTING_WINDOW_CLASS state=normal foreground=yes monitor=primary z=7
  tab runtime-id=42.32443704.4.106 selected=yes name="✳ Claude Code"
  tab runtime-id=42.32443704.4.111 selected=no name="powershell"
window hwnd=0x2f90468 pid=8252 exe=WindowsTerminal.exe class=CASCADIA_HOSTING_WINDOW_CLASS state=normal foreground=no monitor=primary z=9
  tab runtime-id=42.33096486.4.148 selected=yes name="powershell"
window hwnd=0x500dc2 pid=8252 exe=WindowsTerminal.exe class=CASCADIA_HOSTING_WINDOW_CLASS state=normal foreground=no monitor=primary z=12
  tab runtime-id=42.165152658.4.9 selected=yes name="◑ Coordinator curren…"
```

- Three WT windows, **one pid** (8252): the pid check only proves "still Windows Terminal"; the HWND identifies the window. A's tab RuntimeId was the same before and after B opened, and after A's second tab closed.
- The idle Claude tab is titled `✳ Claude Code`; the conhost window with Claude reports the pid of its **client** (`powershell.exe`), not `conhost.exe`, through `GetWindowThreadProcessId`.
- `z=18` in the first run counted every top-level window, hidden ones included. The CLI was changed right after to count **visible** windows only (always-on-top windows such as the taskbar still come first), so `z` is comparable between runs. `foreground=yes` is the reliable "in front" signal.

## How the app can learn session ids

| Source | What it gives | VS Code sessions? |
|---|---|---|
| `claude agents --json` | Running interactive sessions: `pid`, `sessionId`, `cwd`, `name`, `kind`, `status` (see `claude-isolation.md`, "Session listing") | **Yes** (V-0): the VS Code extension's sessions are listed too (`kind: "interactive"`), not only CLI ones. No `entrypoint` field there. |
| `~/.claude/sessions/<pid>.json` | The registry behind `claude agents`: `pid`, `sessionId`, `cwd`, `entrypoint`, `kind`, `name`, `status`, `startedAt`, `updatedAt`, `version`, `messagingSocketPath`, … One file per running session, removed at a clean exit (a killed process leaves its file behind). | **Yes** (V-0): one file per open VS Code conversation, `entrypoint: "claude-vscode"`, `cwd` = the workspace folder. The extension starts the session process as soon as a conversation tab opens, before any message, so the id exists before there is a transcript. |
| `~/.claude/projects/<project>/<sessionId>.jsonl` | Every transcript line carries `sessionId` (= the file name in all 3,339 lines checked), `cwd`, `version`, `gitBranch` and `entrypoint`. Values on this machine: `cli`, `claude-vscode`, `sdk-cli`, `claude-desktop`. `ai-title` / `custom-title` lines hold the session's title. | **Yes**: `entrypoint: "claude-vscode"` (8 transcripts here). The file's modified time shows recent activity, but not whether the session is open in VS Code. `paste-spike sessions --entrypoint claude-vscode` lists them. |
| The extension's own state (`listedSessions`, `sessionPanels`) | Which sessions are open in which panel | Not reachable from outside VS Code |

Candidate for T38–T40: `~/.claude/sessions` / `claude agents --json` for the **running** sessions (VS Code ones included, `entrypoint` from the registry file), with titles from the transcripts' `ai-title`/`custom-title` lines when a transcript exists. Only metadata is read, never message content. Note: a VS Code session id only helps with the link if the extension then puts the text into that session, which this run could not show (see the V cells).

## Test matrix

Baseline for every row unless the row says otherwise: Windows Terminal window **A**, bound tab = A's tab 1 running a throwaway Claude session in a scratch folder, Claude **idle**, text **X-1** (one line), trigger `--own-window` with `--focus plain`, A in front. Each row changes one thing. Record the exit code, what appeared in Claude's input, whether the clipboard came back (copy a canary text before each run), and anything odd.

**How the automated run judged the cells.** Trigger: the driver clicked the spike's topmost window with a synthesized mouse click (after checking with `WindowFromPoint` that the click lands on it); no Enter. Claude's input was read from the terminal buffer with UI Automation (`TextPattern` of WT's `TermControl` and of conhost's text area), and cleared between runs with Esc Esc (never Enter). The clipboard got a random `CANARY-…` text before each run (hidden from history) and was compared after it. "Notepad" was a WinForms window of the agent's own with a text box. VS Code: the Claude input was read through UI Automation (the webview's `messageInput` element) and screenshots of the agent's window; VS Code's URI confirmation was accepted with UIA `Invoke` on its "Open" button. Run logs stayed in the agent's scratch folder.

### Terminal hosts (T-H)

| ID | Case | Result (text in the right input?) | Exit | Notes |
|---|---|---|---|---|
| H-1 | WT window A in front, bound tab selected | Yes: `❯ Please summarise the README in two sentences.` | 0 | `SetForegroundWindow` true at once; whole run ~800 ms (100 ms settle + 200 ms restore delay) |
| H-2 | A behind other windows (a browser / Notepad maximised over it) | Yes; A came to the front over the occluder | 0 | Occluder: the agent's WinForms window covering A, clicked last |
| H-3 | A minimised | Yes | 0 | `SW_RESTORE` took 14 ms; paste after the 100 ms settle arrived complete |
| H-4 | A on another monitor | **Not checkable** | – | The machine has one monitor (`SM_CMONITORS` = 1) |
| H-5 | A with 2 tabs, bound tab **not** selected (tab 2 selected) | Yes: tab 1 selected by UIA, got the text; tab 2 (PowerShell) untouched (buffer read afterwards) | 0 | `Select()` + `IsSelected` poll: 29 ms |
| H-6 | conhost window (`conhost.exe powershell`), no `--tab` | **Ctrl+V: no** (nothing in Claude's input, also with `--restore-delay 1500`). **conhost Edit > Paste (`--via menu`, now the default for conhost): yes**, also when the console was in the background (tried with a direct `WM_COMMAND`). Multi-line: refused (exit 33), not tried | 0 / 0 / 33 | Ctrl+V reaches Claude Code as a key in conhost; the menu command makes conhost paste itself. `GetWindowThreadProcessId` gives the client pid (`powershell.exe`) |
| H-7 | Several WT windows open (A and B), B in front and last active | Yes: only A's tab got it; B's buffer unchanged | 0 | A, B and the coordinator's window all pid 8252; A's tab RuntimeId unchanged |

### Claude state (T-C)

| ID | Case | Result | Exit | Notes |
|---|---|---|---|---|
| C-1 | Idle | Yes (= H-1) | 0 | |
| C-2 | Busy (generating): does the text wait in the input while Claude keeps working? | **Not checkable** | – | Making Claude busy needs a submitted prompt (no submit by rule). The owner's own hand check (ticket): the text waits in the input and Claude keeps working |
| C-3 | A draft already in the input: appended at the cursor, or replaced? | **Inserted at the cursor, draft kept**: cursor at the end → `draft-Please summarise…`; cursor moved to the start (Home) → `Please summarise … sentences.draft-` | 0 | The draft was put there by a first paste of `draft-` (the agent can't type into a background window) |

### Text (T-X)

| ID | Case | Result | Exit | Notes |
|---|---|---|---|---|
| X-1 | One line | Inserted as text (= H-1) | 0 | |
| X-2 | Multi-line (5 lines): "[Pasted text]" placeholder? Then the owner presses Enter in Claude: is all of it sent as one prompt? | One placeholder `❯ [Pasted text #1 +4 lines]`, hint "paste again to expand". Pasting the same text again expanded it to all 5 lines in **one** input box. **Enter: not checkable** (no Enter by rule) | 0 | One input = one prompt is the expected outcome; the owner's hand check in the ticket says one paste is enough |
| X-3 | 4,000 characters (single paragraph): all of it arrives with the default 200 ms restore delay? | Yes: placeholder `[Pasted text #3]`; expanded (second paste) it ends with the file's last words `… END-4000.` | 0 | 200 ms restore delay is enough; WT reads the clipboard at Ctrl+V |
| X-4 | Starts with `!` (Claude's bash mode prefix) | **Interpreted**: the input switched to shell mode (`! echo spike-bang`, hint `! for shell mode`); Enter would run the command | 0 | D35's guard is needed. Esc Esc + Backspace leave shell mode |
| X-5 | Starts with `/` (slash command) | Plain text in the input `❯ /help spike-slash`; no command menu shown; Enter would run it as a slash command (not tried) | 0 | D35's guard is needed |
| X-6 | Starts with `#` (memory shortcut in older versions) | Plain text `❯ # spike-hash note`, no mode hint | 0 | No special handling visible in 2.1.316 |

### Focus rules (T-F)

| ID | Case | Result (`SetForegroundWindow` returned / window in front?) | Exit | Notes |
|---|---|---|---|---|
| F-1 | Right after the spike's own window had the foreground (`--own-window`, `plain`): the app's Enter case | true / in front, in every click-triggered run (H-*, X-*, C-*) | 0 | Trigger was a click, not Enter (same foreground rights: the process got the last input) |
| F-2 | Without it: `--delay 4000`, the owner clicks a non-terminal window (Notepad) during the delay, `plain` | **false** / not in front (the occluder kept the foreground); nothing pasted, clipboard untouched | 20 | Run with `--delay 3000`, the driver clicked the occluder during the delay |
| F-3 | Only if F-2 fails: F-2 with `--focus alt` | true / in front, pasted | 0 | The Alt-tap workaround passes the foreground lock |
| F-4 | Only if F-2 fails: F-2 with `--focus attach` | false / not in front | 20 | `AttachThreadInput` does not help here |

### Races and failures (T-R)

| ID | Case | Result | Exit | Notes |
|---|---|---|---|---|
| R-1a | The user holds Shift during the paste (`--delay 3000`, hold Shift until the run ends) | Nothing pasted; `keys still held: Shift` | 21 | Simulated: synthesized Shift down in the occluder, released after the run; `--focus alt` so focus wasn't the reason |
| R-1b | The user types letters while the paste runs (`--delay 3000`, keep typing into Notepad) | Paste went to Claude (`--focus alt`), **but the letters typed after the switch landed in Claude's input around it**: `bcPlease summarise the README in two sentences.defghi`; the rest stayed in the occluder | 0 | Simulated: synthesized letters every 40 ms, only while the foreground was one of the agent's windows. Plain keys are not in the held-keys check |
| R-2 | Terminal window closed between binding and paste | Nothing pasted anywhere (A's input unchanged) | 10 | Window B closed during the `--delay`, caught by the re-check after the trigger |
| R-3 | Bound tab closed (window still open), or dragged to another window | Closed tab: nothing pasted | 13 | Caught by the tab check at the start. **Dragging** a tab to another window was not done (needs a mouse drag): not checked |
| R-4 | Clipboard after a run: canary text back? Pasted text absent from Win+V history? | Canary back after **every** run (also the failed ones). Clipboard history on (`EnableClipboardHistory = 1`); of its 25 items none held a pasted text or a canary | – | History read with WinRT `Clipboard.GetHistoryItemsAsync`, matched against markers only (no content printed) |

### VS Code (V)

| ID | Case | Result (which window, which session, input filled?) | Exit | Notes |
|---|---|---|---|---|
| V-0 | While a VS Code session is open: does it appear in `claude agents --json` and `~/.claude/sessions/`? | **Both**: `~/.claude/sessions/<pid>.json` with `entrypoint: "claude-vscode"`, `cwd` = workspace folder, and the same `pid`/`sessionId` in `claude agents --json` | – | The session process starts when the conversation tab opens, before any message |
| V-1 | Session id of a session **open in an editor tab** | Saved session: **not checkable** (see below). Open session without a saved transcript (id from `~/.claude/sessions`): a **new** tab opened with the text and a **new** session id; the open session's tab and its input were unchanged | 0 | Code says for a saved, open session: tab revealed, prompt **not** applied, info message |
| V-2 | Session id of a session **open in the sidebar** | **Not checkable** | – | Needs a saved session; code says: `activate_session` with `initialPrompt`, plus the same "not applied" message |
| V-3 | Session id of a **closed** session (in `sessions` list, no tab open) | **Not checkable** | – | Needs a saved session (a transcript in `~/.claude/projects`): creating one needs a submit; writing a synthetic one was refused by the agent's permission classifier. Code says: new tab, prompt in input |
| V-4 | No session (new conversation): baseline from the ticket | New "Claude Code" editor tab with `Please summarise the README in two sentences.` in its input, not sent | 0 | Only after VS Code's "Allow 'Claude Code for VS Code' extension to open this URI?" dialog (accepted with Open); it came back for **every** link ("Do not ask me again" left unticked, it would change the owner's profile) |
| V-5 | Several VS Code windows open (different folders): which window gets the link? With a session id from the other window's folder? | The **last active** window (the second one, other folder) got the link and opened a new tab with the text, in both cases: no session, and a session id of a conversation open in the first window | 0 | The first window stayed unchanged |
| V-6 | A draft already in the input of the target: replaced, appended, or kept? | Kept: every link opened a **new** tab; an earlier conversation tab still had its own text afterwards. Into an existing input: not checkable (no path that fills an existing input could be produced) | 0 | |
| V-7 | Long text: 4,000 chars (`--print-only` shows the URL length), then larger files (16,000 and 30,000 chars) to find the limit | 4,000 chars (URL 5,639): complete (`… END-4000.`). 16,000 (URL 22,439): complete (`… END-16000`). 30,000 (URL 42,041): `ShellExecuteW` failed (2), nothing opened | 0 / 0 / 40 | Limit between 22,439 and 42,041 URL chars, consistent with the 32,767-char command line of `Code.exe --open-url -- "%1"`. Percent-encoding makes the URL ~1.4× the text for English, about 6× for Cyrillic (`%D0%BF` per letter) |
| V-8 | Multi-line text: line breaks kept in the input? | Yes: all 81 chars, 5 lines in the input | 0 | |
| V-9 | Text starting with `/` or `!` through the link | Shown as plain text in the input (`/help spike-slash`, `!echo spike-bang`); not sent | 0 | What Enter would do: not tried |

## Recipe for the app (T38–T40)

What the matrix supports, as input for the binding and bound-Translate-tab tasks:

1. **Bind** (Translate tab ↔ terminal): store HWND + pid + window class, and for Windows Terminal the tab's UIA RuntimeId (`list-terminals` logic). Show the tab's title only as a label; it changes while Claude works. pid alone can't tell WT windows apart.
2. **Deliver only from the user's own action in the app** (Enter / button in the bound tab), with the app's window still in front: then plain `SetForegroundWindow` works (F-1). From the popup, a timer or the tray, expect exit 20 (F-2); fall back to copy + notification. The Alt-tap trick (F-3) works but is a foreground-lock workaround; use it only if the owner accepts it.
3. **Before focusing:** window exists, same pid, still a terminal (10/11/12), tab RuntimeId found (13); refuse text starting with `!` or `/` (X-4, X-5: D35), or strip/escape per D35.
4. **Focus:** restore if minimised, `SetForegroundWindow`, poll `GetForegroundWindow` (≤ 500 ms), select the tab with `SelectionItemPattern.Select()` and poll `IsSelected` (≤ 500 ms), settle 100 ms.
5. **Last checks:** foreground and tab still right (22); no modifier or Enter held (21); **new:** no input at all in the last ~300 ms (`GetLastInputInfo`), so typing in flight doesn't mix into Claude's input (R-1b).
6. **Paste:** save clipboard text, write the text with the history/monitor exclusion formats, one Ctrl+V (Windows Terminal) or `WM_COMMAND 0xFFF1` (conhost, single line only), wait 200 ms, restore if the sequence number is unchanged (R-4). Never Enter.
7. **Tell the user** the text is waiting in Claude's input; a second delivery of the same multi-line text expands the placeholder rather than duplicating it (X-2), but a second single-line delivery appends (C-3).
8. **VS Code:** not a delivery route into an open session (see the verdict). At most an "Open in a new VS Code conversation" action: `vscode://anthropic.claude-code/open?prompt=…` without `session`, text ≤ ~16,000 chars (≤ ~5,000 for Cyrillic, which percent-encodes to 6 chars a letter; not measured), after VS Code's one-time-per-link confirmation, in the last active VS Code window.

## Owner runbook

The automated run of 2026-10-31 followed this runbook with these differences: the `--own-window` trigger was a synthesized click on the spike window (never Enter); "Notepad" was a WinForms window of the agent's own; X-2's Enter, C-2 and H-4 were skipped (marked not checkable); the conhost multi-line case was not run; VS Code ran on scratch folders with the owner's profile, and the V cells needing a saved session were skipped.

Run everything from a **control console** that is not one of the test windows: a classic console (`Win+R` → `conhost.exe powershell`) works; keep it on the side. It shows up in `list-terminals` too (a `ConsoleWindowClass` window); never bind it. Order: all Windows Terminal cases first (one window, then two), then conhost, then VS Code, so windows are opened once.

### 0. Prepare (once)

```powershell
cd C:\Users\User\Documents\Projects\prompt_translator      # the repo (or the worktree with this branch)
cargo build --manifest-path spikes/paste/Cargo.toml --locked
$spike = (Resolve-Path spikes\paste\target\debug\paste-spike.exe).Path
$t = "$HOME\paste-spike"; mkdir "$t\work" -Force | Out-Null
Set-Content "$t\x1.txt" -Encoding utf8 "Please summarise the README in two sentences."
Set-Content "$t\x2.txt" -Encoding utf8 @("Line one of a multi-line prompt.","Line two.","Line three.","Line four.","Line five: end.")
Set-Content "$t\x3.txt" -Encoding utf8 (("The quick brown fox jumps over the lazy dog. " * 89).Substring(0,4000))
Set-Content "$t\x4.txt" -Encoding utf8 "!echo spike-bang"
Set-Content "$t\x5.txt" -Encoding utf8 "/help spike-slash"
Set-Content "$t\x6.txt" -Encoding utf8 "# spike-hash note"
Set-Content "$t\v16k.txt" -Encoding utf8 (("The quick brown fox jumps over the lazy dog. " * 360).Substring(0,16000))
Set-Content "$t\v30k.txt" -Encoding utf8 (("The quick brown fox jumps over the lazy dog. " * 670).Substring(0,30000))
function canary { Set-Clipboard "CANARY-$(Get-Random)"; Get-Clipboard }   # before each paste
```

Open **WT window A** (`wt -w new -d "$HOME\paste-spike\work"`), start `claude` in its first tab (a throwaway session in the scratch folder), and open a second tab with plain PowerShell (`Ctrl+Shift+T`). Then:

```powershell
& $spike list-terminals
```

Copy A's tab 1 `bind:` line; below, `$bind` stands for its arguments, e.g. `$bind = @("--hwnd","0x500dc2","--pid","8252","--tab","42.165152658.4.9")`. Check it once with `& $spike paste @bind --text "$t\x1.txt" --dry-run` (read-only, exit 0). Record the `list-terminals` output (titles may be cut with `--title-chars 20`).

Each paste run below: `canary`, then the command; with `--own-window`, click the small "Paste spike" window and press Enter there. Look at Claude's input in A, then press **Esc** in Claude (or `Ctrl+U`) to clear it unless the cell says to send. Run `Get-Clipboard` afterwards (R-4: the canary should be back). Record exit code and the step log.

### 1. Window A, tab 1 selected, A in front (H-1, C-1, X-*, F-1, C-3, C-2)

| Cell | Do |
|---|---|
| H-1 / C-1 / X-1 / F-1 | `& $spike paste @bind --text "$t\x1.txt" --own-window` |
| X-2 | `... --text "$t\x2.txt" --own-window`; check "[Pasted text]", then press **Enter in Claude** (throwaway session) and check the whole text went as one prompt |
| X-3 | `... --text "$t\x3.txt" --own-window`; check the end of the text arrived (scroll / expand). If cut, retry with `--restore-delay 1000` and record both |
| X-4, X-5, X-6 | `... --text "$t\x4.txt"` (then x5, x6) `--own-window`; record how Claude's input shows it (mode switch? command menu?); **don't press Enter**, clear with Esc |
| C-3 | type `draft-` into Claude's input, leave the cursor at its end, then run X-1; appended or replaced? |
| C-2 | ask Claude in A for something long ("write a 500-word story"), and while it streams run X-1; does the text wait in the input and Claude keep working? Optionally press Enter after it finishes |
| R-4 | after any run: `Get-Clipboard` shows the canary; open Win+V: the pasted text must not be listed |

### 2. Window A in other states (H-2, H-3, H-4, H-5, F-2…F-4, R-1)

| Cell | Do |
|---|---|
| H-2 | maximise Notepad (or a browser) over A, click the spike window, Enter |
| H-3 | minimise A, then run with `--own-window` |
| H-4 | if there is a second monitor: drag A there, run with `--own-window` |
| H-5 | in A select tab 2 (PowerShell), run with `--own-window`; tab 1 must be selected and get the text; tab 2 must stay untouched |
| F-2 | `& $spike paste @bind --text "$t\x1.txt" --delay 4000`, then click Notepad within 4 s; record `SetForegroundWindow returned …` and the exit code (20 = refused; a taskbar flash is the usual sign) |
| F-3, F-4 | only if F-2 gave 20: same with `--focus alt`, then `--focus attach` |
| R-1a | `... --delay 3000`, click Notepad, hold **Shift** until the run ends: expected 21 |
| R-1b | `... --delay 3000`, click Notepad and keep typing letters there until the run ends: where did the letters and the paste go? |

### 3. Two WT windows (H-7, R-3)

| Cell | Do |
|---|---|
| H-7 | open WT window B (`wt -w new`), click into B so it's last active, then run with `--own-window` bound to A's tab 1: only A must get the text. `list-terminals` again: are A and B the same `pid` (one WT process)? Is A's tab RuntimeId unchanged? |
| R-3 | drag A's tab 1 into B (or close it), run the old `$bind`: expected 13; record the new RuntimeId from `list-terminals` |

### 4. Closed window (R-2)

Close window B (or A) entirely, then run the `$bind` of the closed window: expected exit 10, nothing pasted anywhere.

### 5. conhost (H-6)

```powershell
conhost.exe powershell -NoExit -Command "cd $HOME\paste-spike\work; claude"
& $spike list-terminals        # take the ConsoleWindowClass window's bind: line (no --tab)
```

Run X-1 with `--own-window`. Then X-2 **only in this throwaway session**: if conhost has no bracketed paste, each line may be submitted on its own; record exactly what happened.

### 6. VS Code (V-*)

```powershell
& $spike sessions --entrypoint claude-vscode       # pick ids; newest first
& $spike vscode --session <id> --text "$t\x1.txt" --print-only   # check the URL first
```

| Cell | Do |
|---|---|
| V-0 | with a VS Code Claude session open: `claude agents --json` and `Get-ChildItem $HOME\.claude\sessions` (look for `entrypoint: "claude-vscode"`) |
| V-4 | `& $spike vscode --text "$t\x1.txt"` (new conversation) |
| V-1 | open a session in an editor tab, then `& $spike vscode --session <its id> --text "$t\x1.txt"`: input filled, or the "Your prompt was not applied" message? |
| V-2 | same with the session in the sidebar |
| V-3 | close the session's tab (it stays in the session list), run again: a new tab with the text in the input? |
| V-6 | type a draft in the target's input first (only possible where V-1/V-2/V-3 fill it), then run |
| V-5 | open a second VS Code window on another folder, make it the last active, run V-3 with a session of the first window's folder: which window opens it? |
| V-7 | `x3.txt` (4,000), `v16k.txt`, `v30k.txt` with `--print-only` first (URL length), then for real: does it open, is the text complete? |
| V-8 | `x2.txt`: line breaks in the input? |
| V-9 | `x5.txt`, `x4.txt`: shown as text, or interpreted? Don't send. |

### 7. Record

Fill every result cell above, then the verdicts in [Result in short](#result-in-short), and add any change to the recipe (delays, focus method) to the recipe tables. Remove `$HOME\paste-spike` when done.

## Limits and open questions

- **Foreground lock:** the app can bring a window to the front only while it owns the foreground (the user just pressed Enter in it, F-1) or with the Alt-tap workaround (F-3; `AttachThreadInput`, F-4, failed). Delivery from the popup or after a delay is refused (F-2, exit 20); T38–T40 need a fallback (copy + notification).
- **Clipboard:** only text is saved and restored (as in the layout fix); an image or file list on the clipboard is lost. The 200 ms restore delay was enough for 4,000 characters in Windows Terminal (X-3).
- **Typing in flight** goes into Claude's input around the paste (R-1b) unless the app waits for a quiet keyboard (`GetLastInputInfo`).
- **Still open for the owner** (not checkable unattended): that Enter sends a multi-line paste as one prompt (X-2), pasting while Claude is busy (C-2; the owner's earlier hand check says it waits), a second monitor (H-4), a tab dragged to another window (R-3), and the VS Code cells that need a saved session (V-1 proper, V-2, V-3, V-6).
- **No paste verification:** after Ctrl+V nothing confirms the text arrived; the last checks before it (foreground, tab selected, no keys held) are the guard against pasting into the wrong window.
- **Elevated terminals:** `SendInput` into an elevated window is blocked by UIPI (exit 31); `SetForegroundWindow` may still succeed.
- **Windows Terminal runs all its windows in one process by default:** the pid check can't tell two WT windows apart; the HWND does.
- **VS Code link:** undocumented and version-specific (2.1.314); see the open-session finding above. Every link asks "Allow 'Claude Code for VS Code' extension to open this URI?" until the user ticks "Do not ask me again". URL length: 22,439 chars worked, 42,041 failed in `ShellExecuteW` (V-7), so the Windows command line (32,767 chars) of `Code.exe --open-url -- "%1"` is the limit. The window is VS Code's last active one, whatever the session id (V-5).
