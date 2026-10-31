# Spike T58: paste into a bound terminal window, open VS Code sessions by link

Task T58 (#148), story S9.1 (#30), epic E9 (#9), decision D47. Goal: a GO / NO-GO and a working recipe for the two delivery routes of D47, without writing into another process's console:

1. **Terminal:** bring the bound terminal window to the front, select the bound Windows Terminal tab (UI Automation), paste once with Ctrl+V through the clipboard, restore the clipboard. **Never send Enter.**
2. **VS Code extension:** open `vscode://anthropic.claude-code/open?session=<id>&prompt=<text>`.

- **Status:** the spike CLI is built and its read-only parts have run. **The matrix is pending the owner** (it switches windows and pastes into live sessions, `docs/development.md` §8). Every result cell below is empty until then; both verdicts are **TBD**.
- **Date:** 2026-10-31. **Machine:** Windows 11 Pro 10.0.26200, Windows Terminal, Claude Code VS Code extension `anthropic.claude-code` 2.1.314.
- **Tool:** `spikes/paste/` (throwaway Rust CLI, its own Cargo project, not part of the app or its checks).

## Result in short

| Route | Verdict | Basis |
|---|---|---|
| Terminal focus-and-paste | **TBD** (owner run) | Needs every T-* cell. GO requires AC2: bound window + tab get the text in front, behind and minimised; multi-line arrives as one prompt after the user's Enter; failures are detectable (exit codes, no paste into the wrong window). |
| VS Code link | **TBD** (owner run) | Needs every V-* cell. GO requires AC3: the text lands in the right session's input in the owner's setup; window choice, URL length and draft replace documented. **Warning from the code (below): for a session that is already open in an editor tab, 2.1.314 does not apply the prompt.** |

### Found before the owner run (read-only)

- **Windows Terminal exposes tabs through UIA**, confirmed by `list-terminals` (see [list-terminals output](#list-terminals-output)): each tab is a `TabItem` with a name, a RuntimeId and a working `SelectionItemPattern.IsSelected`. The window title and the tab name are Claude's task summary with a spinner glyph (`◐ …`), so neither identifies a session; binding has to be by HWND + pid + tab RuntimeId, as the ticket says.
- **VS Code link, open sessions (static read of `extension.js` 2.1.314, to be confirmed in V-1/V-2):** the `/open` handler validates `session` (a UUID; an invalid id makes the link do nothing at all) and calls `claude-vscode.primaryEditor.open(session, prompt)` → `createPanel`. In `createPanel`:
  - session **already open in an editor tab** (or a remembered tab being revived): the tab is revealed and, if a prompt was given, VS Code shows the info message *"Session is already open. Your prompt was not applied — enter it manually."* **The prompt is dropped.**
  - session **held by the sidebar**: the sidebar is revealed and gets `activate_session` with `initialPrompt` = the prompt, **and** the same "not applied" message is shown. Whether the sidebar webview fills the input then is unknown (V-2).
  - session **not open anywhere** (closed): a new editor tab is created with the session and the prompt (`setupPanel(..., prompt, ...)`), so the text should land in its input (V-3).
  - no `session`: a new conversation with the prompt (the owner's check from the ticket, V-4).

  If V-1 confirms this, the VS Code route only works for sessions that are **not** open, which is the opposite of the T38–T40 use case (deliver into the session the user is working in). That would be a NO-GO for "open session" delivery unless the sidebar case (V-2) works.

## The spike CLI

Build (from the repo root; any `CARGO_TARGET_DIR` works, default is `spikes/paste/target/`, ignored by git):

```powershell
cargo build --manifest-path spikes/paste/Cargo.toml --locked
$spike = "spikes\paste\target\debug\paste-spike.exe"   # or <CARGO_TARGET_DIR>\debug\paste-spike.exe
```

| Subcommand | What it does | Touches |
|---|---|---|
| `list-terminals [--all] [--title-chars <n>]` | Top-level terminal windows (the app's terminal classifier, POPUP-007: classes `CASCADIA_HOSTING_WINDOW_CLASS`, `ConsoleWindowClass`; exes `WindowsTerminal.exe`, `wezterm-gui.exe`, `alacritty.exe`, `mintty.exe`) with HWND, pid, exe, class, state (normal / minimised / hidden), foreground, monitor (`primary` or `other@x,y`), Z position among visible windows, title; for Windows Terminal every tab (UIA `TabItem` name, RuntimeId, selected). Prints a ready `bind:` command line per window or tab. `--all` adds hidden windows; `--title-chars` cuts titles. | Nothing (read-only) |
| `paste --hwnd <h> --text <file> [--pid <p>] [--tab <runtime-id>] [--own-window \| --delay <ms>] [--focus plain\|alt\|attach] [--restore-delay <ms>] [--dry-run]` | The terminal recipe below. Prints a timestamped line per step. `--dry-run` runs only the read-only checks (window, pid, terminal, tab found). | Focus, tab selection, clipboard, one Ctrl+V |
| `vscode --text <file> [--session <id>] [--print-only]` | Builds the link and opens it with `ShellExecuteW("open")` (what `Start-Process` does). `--print-only` prints the link and its length without opening it. | Opens a `vscode://` link |
| `sessions [--limit <n>] [--entrypoint <name>]` | Newest session transcripts in `~/.claude/projects/<project>/<id>.jsonl` with their `entrypoint` (`claude-vscode`, `cli`, …). Never prints message content. | Nothing (read-only) |

`--hwnd` takes hex (`0x500dc2`) or decimal. The text file is UTF-8; a BOM is dropped, line breaks are normalised, **trailing line breaks are removed** (a paste ending in a line break could act as Enter in a host without bracketed paste), and the text goes on the clipboard with CRLF line breaks (what a copy from Notepad gives). The CLI prints the text's length and line count, and a note when it starts with `!`, `/` or `#` (D35; the spike does not guard, the app will).

### Terminal recipe as implemented (`spikes/paste/src/paste.rs`)

| # | Step | API / check | Timing | Failure → exit |
|---|---|---|---|---|
| 1 | Read the text file | UTF-8, BOM, trim trailing line breaks | – | 3 |
| 2 | Bound window still valid | `IsWindow(hwnd)`; `GetWindowThreadProcessId` = `--pid`; class/exe is a terminal | – | 10 / 11 / 12 |
| 3 | Bound tab found (Windows Terminal, `--tab`) | `CoInitializeEx(MTA)`, `CUIAutomation.ElementFromHandle(hwnd)`, `FindAll(Descendants, ControlType = TabItem)`, match `GetRuntimeId()` | – | 13 (1 on UIA failure) |
| 4 | Trigger | `--own-window`: the spike's own top-level window, triggers on Enter **key-up** after an Enter key-down in that window (so the key-up of the Enter that started the command doesn't count); the process then owns the foreground and got the last input, like the app after the user's Enter. The window stays open until the end. Otherwise wait `--delay` ms. | – | 4 (window closed) |
| 5 | No key held | `GetAsyncKeyState` for Shift, Ctrl, Alt, LWin, RWin **and Enter** (an Enter still down when the terminal takes focus would auto-repeat into Claude and send) | poll 10 ms, up to 500 ms | 21 |
| 6 | Re-check the window (closed since binding?) | step 2 again | – | 10 / 11 / 12 |
| 7 | Restore if minimised | `IsIconic` → `ShowWindow(SW_RESTORE)`, poll `IsIconic` | up to 1000 ms | 15 |
| 8 | Bring to front | `--focus plain` (default): `SetForegroundWindow`. `alt`: `SendInput` Alt down, unassigned key 0xE8, Alt up, then `SetForegroundWindow`. `attach`: `AttachThreadInput` to the foreground thread, `SetForegroundWindow` + `BringWindowToTop`, detach. Then poll `GetForegroundWindow() == hwnd`. | up to 500 ms | 20 |
| 9 | Select the bound tab | `SelectionItemPattern.Select()` if `CurrentIsSelected` is false, poll `CurrentIsSelected` | up to 500 ms | 14 |
| 10 | Settle | let the terminal route input to the tab | 100 ms | – |
| 11 | Last checks | step 5 again; `GetForegroundWindow() == hwnd`; tab still selected | – | 21 / 22 |
| 12 | Save the clipboard | `OpenClipboard` (10 tries, 10 ms apart), read `CF_UNICODETEXT` (text only, like the app's layout fix) | – | 30 |
| 13 | Put the text there, hidden | `EmptyClipboard`, `SetClipboardData(CF_UNICODETEXT)` + `ExcludeClipboardContentFromMonitorProcessing` and `CanIncludeInClipboardHistory` = DWORD 0 (not in Win+V history, not seen by monitors) | – | 30 (saved text put back) |
| 14 | Paste | one `SendInput`: Ctrl down, V down, V up, Ctrl up. **No Enter, ever.** | – | 31 (UIPI: elevated target; saved text put back) |
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
- `z=18` in this run counted every top-level window, hidden ones included. The CLI was changed right after to count **visible** windows only (always-on-top windows such as the taskbar still come first), so `z` is comparable between runs. `foreground=yes` is the reliable "in front" signal.

## How the app can learn session ids

| Source | What it gives | VS Code sessions? |
|---|---|---|
| `claude agents --json` | Running interactive sessions: `pid`, `sessionId`, `cwd`, `name`, `kind`, `status` (see `claude-isolation.md`, "Session listing") | Per the ticket, **CLI sessions only** |
| `~/.claude/sessions/<pid>.json` | The registry behind `claude agents`: `pid`, `sessionId`, `cwd`, `entrypoint`, `kind`, `name`, `status`, `startedAt`, `updatedAt`, `version`, `messagingSocketPath`, … One file per running session, removed at exit. Seen during this spike: one file, `entrypoint: "cli"`, `kind: "interactive"`. | **Unknown**: no VS Code session was running. Check while one is open (runbook step V-0). |
| `~/.claude/projects/<project>/<sessionId>.jsonl` | Every transcript line carries `sessionId` (= the file name in all 3,339 lines checked), `cwd`, `version`, `gitBranch` and `entrypoint`. Values on this machine: `cli`, `claude-vscode`, `sdk-cli`, `claude-desktop`. `ai-title` / `custom-title` lines hold the session's title. | **Yes**: `entrypoint: "claude-vscode"` (8 transcripts here). The file's modified time shows recent activity, but not whether the session is open in VS Code. `paste-spike sessions --entrypoint claude-vscode` lists them. |
| The extension's own state (`listedSessions`, `sessionPanels`) | Which sessions are open in which panel | Not reachable from outside VS Code |

Candidate for T38–T40: list `claude-vscode` transcripts from `~/.claude/projects` (newest first, title from `ai-title`/`custom-title`, project from `cwd`), filtered to the last few hours; plus `~/.claude/sessions` if V-0 shows VS Code sessions there (that would also tell which are running). Only metadata is read, never message content.

## Test matrix

Baseline for every row unless the row says otherwise: Windows Terminal window **A**, bound tab = A's tab 1 running a throwaway Claude session in a scratch folder, Claude **idle**, text **X-1** (one line), trigger `--own-window` with `--focus plain`, A in front. Each row changes one thing. Record the exit code, what appeared in Claude's input, whether the clipboard came back (copy a canary text before each run), and anything odd.

### Terminal hosts (T-H)

| ID | Case | Result (text in the right input?) | Exit | Notes |
|---|---|---|---|---|
| H-1 | WT window A in front, bound tab selected | | | |
| H-2 | A behind other windows (a browser / Notepad maximised over it) | | | |
| H-3 | A minimised | | | |
| H-4 | A on another monitor | | | |
| H-5 | A with 2 tabs, bound tab **not** selected (tab 2 selected) | | | |
| H-6 | conhost window (`conhost.exe powershell`), no `--tab` | | | |
| H-7 | Several WT windows open (A and B), B in front and last active | | | |

### Claude state (T-C)

| ID | Case | Result | Exit | Notes |
|---|---|---|---|---|
| C-1 | Idle | | | |
| C-2 | Busy (generating): does the text wait in the input while Claude keeps working? | | | |
| C-3 | A draft already in the input: appended at the cursor, or replaced? | | | |

### Text (T-X)

| ID | Case | Result | Exit | Notes |
|---|---|---|---|---|
| X-1 | One line | | | |
| X-2 | Multi-line (5 lines): "[Pasted text]" placeholder? Then the owner presses Enter in Claude: is all of it sent as one prompt? | | | |
| X-3 | 4,000 characters (single paragraph): all of it arrives with the default 200 ms restore delay? | | | |
| X-4 | Starts with `!` (Claude's bash mode prefix) | | | |
| X-5 | Starts with `/` (slash command) | | | |
| X-6 | Starts with `#` (memory shortcut in older versions) | | | |

### Focus rules (T-F)

| ID | Case | Result (`SetForegroundWindow` returned / window in front?) | Exit | Notes |
|---|---|---|---|---|
| F-1 | Right after the spike's own window had the foreground (`--own-window`, `plain`): the app's Enter case | | | |
| F-2 | Without it: `--delay 4000`, the owner clicks a non-terminal window (Notepad) during the delay, `plain` | | | |
| F-3 | Only if F-2 fails: F-2 with `--focus alt` | | | |
| F-4 | Only if F-2 fails: F-2 with `--focus attach` | | | |

### Races and failures (T-R)

| ID | Case | Result | Exit | Notes |
|---|---|---|---|---|
| R-1a | The user holds Shift during the paste (`--delay 3000`, hold Shift until the run ends) | | | expected 21, nothing pasted |
| R-1b | The user types letters while the paste runs (`--delay 3000`, keep typing into Notepad) | | | do letters end up in Claude's input, or the paste in Notepad? |
| R-2 | Terminal window closed between binding and paste | | | expected 10 |
| R-3 | Bound tab closed (window still open), or dragged to another window | | | expected 13 |
| R-4 | Clipboard after a run: canary text back? Pasted text absent from Win+V history? | | | |

### VS Code (V)

| ID | Case | Result (which window, which session, input filled?) | Exit | Notes |
|---|---|---|---|---|
| V-0 | While a VS Code session is open: does it appear in `claude agents --json` and `~/.claude/sessions/`? | | – | |
| V-1 | Session id of a session **open in an editor tab** | | | code says: revealed, prompt **not** applied, info message |
| V-2 | Session id of a session **open in the sidebar** | | | code says: `activate_session` with `initialPrompt`, plus the same message |
| V-3 | Session id of a **closed** session (in `sessions` list, no tab open) | | | code says: new tab, prompt in input |
| V-4 | No session (new conversation): baseline from the ticket | | | |
| V-5 | Several VS Code windows open (different folders): which window gets the link? With a session id from the other window's folder? | | | |
| V-6 | A draft already in the input of the target: replaced, appended, or kept? | | | |
| V-7 | Long text: 4,000 chars (`--print-only` shows the URL length), then larger files (16,000 and 30,000 chars) to find the limit | | | |
| V-8 | Multi-line text: line breaks kept in the input? | | | |
| V-9 | Text starting with `/` or `!` through the link | | | |

## Owner runbook

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

## Limits and open questions (to settle in the matrix)

- **Foreground lock:** the app can bring a window to the front only while it owns the foreground (the user just pressed Enter in it) or with a workaround (F-3/F-4). Delivery from the popup or after a delay may be refused (exit 20); T38–T40 then need a fallback (copy + notification).
- **Clipboard:** only text is saved and restored (as in the layout fix); an image or file list on the clipboard is lost. The restore waits a fixed delay; a terminal that reads the clipboard later would paste the restored text instead (X-3).
- **No paste verification:** after Ctrl+V nothing confirms the text arrived; the last checks before it (foreground, tab selected, no keys held) are the guard against pasting into the wrong window.
- **Elevated terminals:** `SendInput` into an elevated window is blocked by UIPI (exit 31); `SetForegroundWindow` may still succeed.
- **Windows Terminal runs all its windows in one process by default:** the pid check can't tell two WT windows apart; the HWND does.
- **VS Code link:** undocumented and version-specific (2.1.314); see the open-session finding above. URL length limits: Windows command line (32,767 chars) for `Code.exe --open-url`, and whatever VS Code / the extension accept (V-7).
