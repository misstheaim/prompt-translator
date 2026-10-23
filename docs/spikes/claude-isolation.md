# Spike T16: isolated headless `claude.exe` invocation

Task T16 (#48), story S4.1 (#19). Goal: find the `claude` invocation that translates with **no** user or project CLAUDE.md, hooks, plugins, skills, MCP servers, memory or tools, using the user's normal subscription, and that leaves no history.

- **Date:** 2026-10-24. **Claude Code:** 2.1.309 (native install, `%USERPROFILE%\.local\bin\claude.exe`). **Machine:** Windows 11 Pro, Claude Pro subscription (`authMethod: claude.ai`). No managed policy settings on this machine.
- **Machine-readable result:** [`claude-invocation.json`](claude-invocation.json). T17b builds its invocation from it.
- **Status:** candidate 1 is fully verified and recommended. Candidate 2 (dedicated config dir) is **pending one manual step by the owner** (a separate sign-in, see [Pending owner steps](#pending-owner-steps)); it isn't needed for the recommendation.

## Result in short

- **Recommended invocation:** `--safe-mode` plus `--setting-sources ""` with the user's normal login (candidate 1 + variant). **No separate sign-in is needed.**
- **Isolation:** neither the owner's global `~/.claude/CLAUDE.md`, the project and ancestor `CLAUDE.md` files, nor any hook, skill, plugin, MCP server or tool loads under it (canary evidence below). What still reaches the model is Claude Code's own built-in context: an "Environment" block (cwd, platform, model, date) and a block with the account's e-mail address. Neither is a user customization.
- **History:** no transcript, no `~/.claude/projects` entry, no `~/.claude.json` project entry, nothing in `claude --resume`. 50 runs left `~/.claude.json`, `claude.exe` and the cwd unchanged.
- **One visible side effect:** while a translation runs, it **appears in `claude agents --json` as `kind: "interactive"`** (for about 2–5 s), with the name given by `--name` (`prompt-translator`) and the translator's cwd. The session picker must filter these out (see [Session listing](#session-listing-claude-agents---json-and---resume)).

## Recommended invocation

Spawn the native exe directly (not through a shell), with `CREATE_NO_WINDOW`, from the fixed empty cwd:

```text
exe:  %USERPROFILE%\.local\bin\claude.exe
cwd:  %LOCALAPPDATA%\com.misstheaim.prompttranslator\claude-cwd      (empty; never write into it)
args: -p --safe-mode --setting-sources "" --no-session-persistence --tools "" --strict-mcp-config
      --disable-slash-commands --name prompt-translator --model haiku --output-format json
      --system-prompt-file <tmp>\system-prompt.txt --json-schema <schema as inline JSON>
env:  + DISABLE_AUTOUPDATER=1
      + CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
      + TEMP=TMP=%LOCALAPPDATA%\com.misstheaim.prompttranslator\tmp
      - CLAUDECODE, CLAUDE_CODE_ENTRYPOINT, CLAUDE_CODE_CHILD_SESSION, CLAUDE_CODE_SESSION_ID,
        CLAUDE_CODE_SESSION_ATTENDED, CLAUDE_CODE_MESSAGING_SOCKET, CLAUDE_CODE_MESSAGING_TOKEN,
        CLAUDE_CODE_EXECPATH, CLAUDE_PID, CLAUDE_EFFORT   (only present when the app itself was
        started from inside a Claude Code session, e.g. `npm run tauri dev` in a Claude terminal)
stdin: the user's text as UTF-8, then close stdin
```

`""` means an empty argument (`Command::arg("")` in Rust). Why each part is there:

| Part | Why |
|---|---|
| `--safe-mode` | Disables CLAUDE.md (user, project, ancestors), skills, installed plugins and their hooks, settings-file hooks, claude.ai MCP connectors, custom agents and output styles. Auth works normally. Debug log: `Safe mode: installed plugins are disabled…`, `[reduced mode] Skipping skill dir discovery`, `[claudeai-mcp] Disabled in safe mode`, `project memory is off`. |
| `--setting-sources ""` | Also ignores the user, project and local `settings.json` files (e.g. a user's `env`, `model`, `outputStyle` or `permissions`). Safe mode still reads them (the debug log watches `~/.claude/settings.json`); with both flags nothing from them applies. Managed (policy) settings still apply, by design. |
| `--no-session-persistence` | No transcript, nothing to resume. |
| `--tools ""`, `--strict-mcp-config`, `--disable-slash-commands` | No built-in tools, no MCP servers, no skills or commands. The model reports "Tools: NONE". |
| `--name prompt-translator` | The run's name in `claude agents --json`; lets the session picker recognise and hide translator runs. |
| `--system-prompt-file` | Replaces the whole default system prompt. Works on 2.1.309 even though `--help` lists only `--system-prompt` (it's mentioned in the `--bare` text). |
| `--json-schema` | Must be inline JSON; a file path fails (`Error: --json-schema is not valid JSON: JSON Parse error: Unexpected identifier "C"`). The plan's schema is about 600 characters, far below Windows command-line limits. |
| `DISABLE_AUTOUPDATER=1` | Debug log: `Plugin autoupdate: skipped (auto-updater disabled)`; `claude.exe` hash unchanged after all runs. The binary also knows the stronger `DISABLE_UPDATES` (also blocks `claude update`); not needed here. |
| `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` | Debug log: `[Bootstrap] Skipped: Nonessential traffic disabled`, `[servedCatalog] off (essential_traffic)`, no telemetry POST. **Without it, every run rewrote the user's real `~/.claude.json`** (feature-flag caches, plus a rotated `backups/.claude.json.backup.*`); with it, no run wrote `~/.claude.json`. |
| `TEMP`/`TMP` → app `tmp` dir | Claude Code puts its temp files in `%TEMP%\claude\` (an `fswatch-probe-*` folder per run, deleted on a normal exit but left behind when a run exits early on an error). Redirecting keeps them inside the app's data dir (D27). Verified: with `TEMP` redirected, a normal run left only an empty `claude\` folder there. |
| Remove the `CLAUDE_*` session markers | They belong to a parent Claude Code session. `CLAUDECODE`/`CLAUDE_CODE_CHILD_SESSION` switch the CLI into "nested" behaviour (e.g. skip prompt history). All spike runs removed them. With them kept, the recommended invocation behaved the same (run r22: same output, same `agents` entry, no new files), but a clean environment is what the desktop app has. Don't strip `CLAUDE_CODE_*` by prefix: user settings such as `CLAUDE_CODE_USE_BEDROCK` share it. |

## Results table

All runs: `--model haiku`, environment cleaned of the 10 Claude Code session variables above, spawned with `CREATE_NO_WINDOW`, from a cwd with canaries (see [Leak tests](#leak-tests)) unless noted. "Ancestor CLAUDE.md" = the repo CLAUDE.md files above the test cwd (the test cwd lived inside the repo's git worktree, so the real `CLAUDE.md` files of this repo acted as extra canaries).

| # | Candidate | Global `~/.claude/CLAUDE.md` | Project / ancestor CLAUDE.md | Cwd CLAUDE.md canary | Project hook canary | Config-dir canaries | claude.ai MCP | Tools | Auth | Result |
|---|---|---|---|---|---|---|---|---|---|---|
| r01 | Baseline: plain `-p --no-session-persistence` | **leaks** | **leaks** | **leaks** (PINEAPPLE-7) | **fires** (MANGO-3) | – | fetched | default set | ok | Control: the test detects leaks |
| r08 | Isolation flags **without** `--safe-mode` | **leaks** | **leaks** | **leaks** | **fires** | – | fetched | none | ok | Not isolated |
| r02, r03 | **1. `--safe-mode`** + isolation flags | no | no | no | no | – | disabled | none | ok (normal login) | **Isolated** |
| r04 | **2. `CLAUDE_CONFIG_DIR`** (isolated dir) + isolation flags, no safe mode | – (not loaded: separate dir) | n/a | n/a | **fires** | **hook fires** (GUAVA-5 hook ran) | – | – | **Not logged in** | Needs its own sign-in (pending owner) |
| r05 | 1 + 2 combined | – | n/a | n/a | no | no | – | – | **Not logged in** | Needs its own sign-in (pending owner) |
| r06 | 3a. `--setting-sources ""` + isolation flags | no | no | no | no | – | **fetched** (list of 1 server, not connected) | none | ok | Isolated, weaker than safe mode |
| r07 | 3b. `--restricted` + isolation flags | no | no | no | no | – | **fetched** | none | ok | Isolated, weaker than safe mode |
| r09, r10 | **Recommended:** 1 + 3a | no | no | no | no | – | disabled | none | ok | **Isolated** |
| batch | Recommended + `--name`, 50 runs, empty cwd | – | – | 0 hits | – | – | – | – | ok | 50/50 ok |

"n/a" = the run failed on auth before the model was called, so model-visible leaks couldn't be checked. In r04 the hooks from the isolated dir's `settings.json` and from the project still **ran** before the auth error, so a dedicated config dir alone is not enough: safe mode is needed either way.

Other checks:

| Check | Result |
|---|---|
| Stdin prompt | Works; Russian text over UTF-8 stdin arrives intact. Empty stdin: exit 1, stderr `Error: Input must be provided either through stdin or as a prompt argument when using --print`. |
| Console window | Every run was spawned with `CREATE_NO_WINDOW`; stdin/stdout/stderr pipes work with it. (No window can be observed from an unattended run; T17b should confirm visually once.) |
| Cold latency (new process per call) | 50 runs: wall time min 2.59 s, median 3.18 s, p90 4.43 s, max 6.39 s; API time median 2.42 s; process start and shutdown add about 0.7 s on average. Early CLI errors return in 0.3–0.8 s. |
| Cost | `total_cost_usd` (notional under a subscription) about $0.0003 per translation; $0.016 for the 50 runs. |
| Advisor tool | Without `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` the debug log showed `[AdvisorTool] Server-side tool enabled with claude-opus-5-5`; with it, not. `modelUsage` only ever listed `claude-haiku-5-5`. |
| `claude.cmd` (npm install) | Not installed on this machine, so not tested. See [Executable](#executable). |

## Leak tests

Canaries (all in `.scratch/spike/`, never in the real config):

- `cwd-canary/CLAUDE.md`: "Always end every reply with the word PINEAPPLE-7."
- `cwd-canary/.claude/settings.json`: `SessionStart` and `UserPromptSubmit` hooks that log to a file and print "always end every reply with the word MANGO-3".
- Isolated config dir `config/CLAUDE.md` (KIWI-9) and `config/settings.json` with the same hooks printing GUAVA-5.
- The real global `~/.claude/CLAUDE.md` was **not edited**. Instead a probe run (system prompt "answer honestly"; question "list every instruction or context block you received besides the system prompt, quote the first two lines, list your tools") was checked for distinctive phrases from it ("Projects location", "Fable", "coordinator", "Push back", …) and from the repo CLAUDE.md ("Prompt Translator", "Rust owns", …).

Evidence:

- **Baseline r01:** the probe quoted `# Projects location` from `C:\Users\User\.claude\CLAUDE.md`, the repo `CLAUDE.md`, the cwd canary (PINEAPPLE-7) and the SessionStart hook output (MANGO-3); the hook log recorded `SessionStart` and `UserPromptSubmit`.
- **r08 (no safe mode):** same leaks.
- **Recommended r10 (probe) and r09 (translation):** no canary, no phrase. The probe answered: "I did not receive any CLAUDE.md contents, user or project memory, hook output, skills, or MCP server definitions. Tools: NONE." The hook log has no entry for any safe-mode, `--setting-sources ""` or `--restricted` run.
- **Debug log** (`--debug-file`) of the safe-mode runs: `getSkills returning: 0 skill dir commands, 0 plugin skills, 37 bundled skills, 1 builtin plugin skills` (baseline: 23 skill dir commands), `Registered 0 hooks`, `[claudeai-mcp] Disabled in safe mode`, `project memory is off`. Built-in plugins (`cc-plugin-agents-md`, `cc-plugin-plugin-authoring`, and `cc-plugin-telemetry` unless nonessential traffic is off) still load; they are part of Claude Code itself, not user customizations.
- **What the model still sees** (r02, r06, r07, r10): a built-in "Environment" block (working directory, git status when the cwd is in a repo, shell, OS, model name, date) and a built-in block with the account's e-mail address. No flag removes them while a custom system prompt is used (`--exclude-dynamic-system-prompt-sections` only applies to the default system prompt). In the app's cwd (not a git repo) the git part disappears.

## Output envelope

`--output-format json` prints one JSON object on stdout. Fields seen: `type` ("result"), `subtype`, `is_error`, `result`, `structured_output`, `session_id`, `num_turns`, `stop_reason`, `duration_ms`, `duration_api_ms`, `ttft_ms`, `total_cost_usd`, `usage`, `modelUsage`, `permission_denials`, `uuid`, plus a few internal ones (`api_error_status`, `fast_mode_state`, `result_index`, …).

- **Structured result:** field **`structured_output`** holds the parsed object that matches `--json-schema`. `result` holds the same object as a JSON string. Claude Code implements the schema with an internal `StructuredOutput` tool, so `num_turns` is 2 and `stop_reason` is `"tool_use"` on success; that's normal.
- **Schema not satisfied** (r11, deliberately impossible schema): exit 0, `is_error: false`, `subtype: "success"`, **no `structured_output` key**, and `result` is the model's free-text explanation. The provider must treat a missing `structured_output` as `MalformedOutput` (keep `result` as the detail), not as success.
- **Malformed schema** (r12): exit 1, empty stdout, stderr `Error: --json-schema is not valid JSON: JSON Parse error: Property name must be a string literal`.
- **Missing system prompt file** (r14): exit 1, stderr `Error: System prompt file not found: <path>`.
- **Not signed in** (r04, r05, and `--bare` without a key in r16): exit 1, stdout envelope with `is_error: true`, `subtype: "success"`, `total_cost_usd: 0`, `result: "Not logged in · Please run /login"` (the dot is U+00B7).
- **Sign-in check without an API call:** `claude auth status` prints JSON (`loggedIn`, `authMethod`, `apiProvider`, `subscriptionType`, `email`, `orgName`, …) and exits 0 when signed in; when not signed in it prints `"loggedIn": false` and exits 1 (`--text`: `Not logged in. Run claude auth login to authenticate.`). Don't log its output: it contains the e-mail address.

## Session listing (`claude agents --json` and `--resume`)

- **`claude agents --json` during a run (r18, r19, r22):** the translator **is listed** for the whole run, `kind: "interactive"`, `status: "busy"`, `pid` = the spawned `claude.exe`, `cwd` = the translator cwd, `sessionId` = the envelope's `session_id`, `name` = the `--name` value (without `--name`, an auto name from the cwd, e.g. `cwd-empty-e2`). It's registered through a transient `~/.claude/sessions/<pid>.json` that's removed when the run ends.
  - Listed with the isolated config dir (`CLAUDE_CONFIG_DIR` set for the `agents` call): **not** listed (0 entries), because the registry lives in the config dir. A translator that itself ran with a dedicated config dir would therefore be invisible to the user's `claude agents --json`; that part is pending the owner's sign-in.
  - **Consequence for the session picker and injection (§4.9, D31, D35):** "reported as an interactive session" is not enough to tell a real session from a translator run. Exclude entries whose `name` is `prompt-translator`, whose `cwd` is the translator cwd, or whose `pid` is one of the app's own children.
- **`--resume`:** no translator run created a transcript. `claude -p --resume <translator session_id>` fails with `No conversation found with session ID: <id>` (r17). A search of every file name and file content in the real `~/.claude` and `~/.claude.json` for all 64 translator session IDs found none of the 50 batch sessions and none of the safe-mode sessions (the only hits were the spike agent's own conversation log, which printed them). No `~/.claude/projects` folder and no `~/.claude.json` project entry was created for the test cwds. So nothing appears in `claude --resume`.

## Clutter check (50 runs)

50 translations with the recommended invocation (`--name prompt-translator`, no `TEMP` redirect), fixed empty cwd, snapshots of everything before and after (hash, size and mtime of every file):

| Location | Change after 50 runs |
|---|---|
| Real `~/.claude` | **None from the translator.** Only the spike agent's own conversation log changed, plus `.last-cleanup` (one timestamp line). `.last-cleanup` is Claude Code's throttle marker for its periodic housekeeping (old-transcript retention per `cleanupPeriodDays`); it was most likely touched by one translator run, though it could also have been the parent Claude session. Any Claude Code start does this housekeeping; it only removes data older than the user's own retention setting. |
| Real `~/.claude.json` | **Unchanged** (same hash). |
| `claude.exe` | **Unchanged** (`sha256 5441781f…73dc53` before and after all spike runs). |
| Cwd | **Empty**, nothing created. |
| `%TEMP%` | Nothing from the 50 runs. (One early-exit error run, r17, left an empty `%TEMP%\claude\fswatch-probe-*` folder; the `TEMP` redirect keeps this out of `%TEMP%`.) |
| Transient during a run | `~/.claude/sessions/<pid>.json` (+ a `.key` file), removed at exit. If the process is killed, Claude Code's `sessionRegistry` sweep removes the stale entry on a later start. |
| Isolated config dir | Not used by the recommended invocation. From the not-signed-in runs (r04, r05) a dedicated dir gets: `.claude.json`, `backups/.claude.json.backup.*`, `daemon/pipe.key`, `session-env/<id>/`, `sessions/`. |

Not part of the recommended invocation, but caused by the spike's comparison runs on the real config (listed for completeness, see "Real config" below): runs without safe mode (r01, r08) left two **empty** folders `~/.claude/session-env/<session id>/`, and the two runs without `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` (r01, r02) rewrote feature-flag cache keys in `~/.claude.json`. The recommended invocation does neither.

## Cleanup recipe

- **Recommended invocation (no dedicated config dir):** nothing accumulates. Housekeeping for the app:
  - `claude-cwd\`: must stay empty; if anything appears, delete it.
  - `tmp\` (system prompt files and Claude's redirected temp files): delete everything older than a day, or all of it at startup when no translation is running. Safe to wipe completely: no sign-in data lives there.
  - Nothing to clean in `~/.claude`; the app must never touch it.
- **If a dedicated config dir is ever used** (`claude-config\`): keep `.credentials.json` (the OAuth sign-in) and `.claude.json` (account and onboarding state; deleting it forces onboarding again). Everything else can be deleted while no translation runs: `backups\`, `session-env\`, `sessions\`, `projects\`, `shell-snapshots\`, `todos\`, `file-history\`, `debug\`, `paste-cache\`, `cache\`, `telemetry\`, `daemon\`. Pending verification after the owner's sign-in (the list is based on what the not-signed-in runs created plus the layout of a normal `~/.claude`).

## Sign-in

- **Recommended invocation: no separate sign-in.** It uses the user's normal Claude Code login (`claude.ai` OAuth, subscription). If the user isn't signed in, the run returns the "Not logged in" envelope above; the app should show "Sign in to Claude Code first (run `claude` once)" and can check up front with `claude auth status`.
- **Dedicated config dir (candidate 2): needs its own sign-in.** With `CLAUDE_CONFIG_DIR` pointing to an empty dir, `claude auth status` says `"loggedIn": false` and translations fail with "Not logged in · Please run /login". The procedure would be: open a console with `CLAUDE_CONFIG_DIR=%LOCALAPPDATA%\com.misstheaim.prompttranslator\claude-config` and run `claude auth login` (or `claude` and `/login`), sign in with the same Claude account in the browser. Whether that login then works with the subscription, and whether anything from `~/.claude` still loads, is **pending the owner** (not attempted unattended).

## Executable

- Native install: `%USERPROFILE%\.local\bin\claude.exe` (confirmed; `Get-Command claude -All` finds only this one). Detection order for T17a: `settings.claude.executablePath`, then this path, then `claude.exe` on `PATH`.
- The auto-updater replaces this file in place and keeps the previous build as `claude.exe.old.<timestamp>` next to it (seen on this machine: the user's own interactive session updated it to 2.1.309 shortly before the spike). The translator must resolve the path per run, not cache a file handle.
- npm install (`%APPDATA%\npm\claude.cmd`): **not installed here, not tested.** A `.cmd` shim runs through `cmd.exe`; since Rust 1.77.2 (CVE-2024-24576) `std::process::Command` escapes batch-file arguments with cmd.exe rules and returns `InvalidInput` for arguments it can't escape safely, and the inline `--json-schema` JSON is full of quotes. Recommendation: don't spawn `claude.cmd`; resolve it to `node.exe <npm prefix>\node_modules\@anthropic-ai\claude-code\cli.js` and spawn node directly, or ask the user to install the native build. Test this in T17a if npm installs must be supported.

## Known limits

- **Translator runs show up in `claude agents --json`** as `kind: "interactive"` for 2–5 s each; the picker must filter them (above). A dedicated config dir would hide them but needs a separate sign-in.
- **Built-in context still reaches the model:** the Environment block (cwd, OS, model, date) and the account e-mail block. They don't carry user customizations and the e-mail already belongs to the account the request is billed to, but they are extra tokens and could in theory influence a translation.
- **Managed (policy) settings still apply** under both `--safe-mode` and `--setting-sources ""` (by design; an admin can force hooks or MCP servers). Not present on this machine, so untested.
- **Housekeeping** (`.last-cleanup`, retention of the user's own old transcripts) may run from a translator process, as from any Claude Code start.
- **Version-dependent:** `--system-prompt-file` is undocumented in `--help`; `--safe-mode` semantics may change. Re-run the spike after Claude Code upgrades and diff `claude-invocation.json`.
- **Test cwd was inside the repo's git worktree** (`.scratch/spike/…`), not under `%LOCALAPPDATA%`. That made the test stricter (ancestor `CLAUDE.md` files acted as extra canaries) and added git details to the Environment block; the app's real cwd isn't in a repo.
- **One-shot latency** is 2.6–6.4 s per translation (about 0.7 s of it is process start-up); there is no warm process (D12).

## Fallbacks (not needed)

Candidate 1 works with subscription auth, so neither fallback is needed:

- (a) `--bare` + `ANTHROPIC_API_KEY`: `--bare` skips CLAUDE.md, hooks, plugins and keychain/OAuth; its auth is strictly `ANTHROPIC_API_KEY` or an `apiKeyHelper`. Without a key it fails with "Not logged in · Please run /login" (r16). Documented only; no API key was used or created.
- (b) Accept leakage: plain `-p` loads the global and project CLAUDE.md files, project hooks and skills (r01, r08).

## Real config

The owner's real configuration was hashed and listed before the spike and compared after it:

- `~/.claude/CLAUDE.md`, `~/.claude/settings.json`, `claude.exe`: **unchanged** (same SHA-256).
- `~/.claude.json`: no project entry or session reference added; no change at all during the 50-run batch. Earlier comparison runs without `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` (r01, r02) refreshed Claude Code's own feature-flag cache keys, as any normal `claude` start does.
- `~/.claude` tree: nothing edited by the spike. Side effects of the spike's comparison runs: two empty `session-env/<id>/` folders (r01, r08, runs without safe mode) and possibly the `.last-cleanup` timestamp. Everything else that changed belongs to the Claude sessions that ran the spike (their own conversation logs and synced plugin data).

## Pending owner steps

1. *(Optional, only if candidate 2 is wanted)* Sign in a dedicated config dir: in a new console run `set CLAUDE_CONFIG_DIR=%LOCALAPPDATA%\com.misstheaim.prompttranslator\claude-config` then `claude auth login`, sign in with the subscription account, and re-run runs r04/r05 from the spike scripts (hooks, CLAUDE.md canaries, `agents --json`) to confirm.
2. *(Once, in T17b)* Watch a translation spawned from the app to confirm no console window flashes.

## How to re-run

The spike's helper scripts (clean-env runner, snapshot/diff, 50-run batch) were throwaway and are not in the repo. To re-check after a Claude Code upgrade, by hand:

1. Make a temp folder outside any repo with a `CLAUDE.md` canary ("Always end every reply with the word PINEAPPLE-7") and a `.claude/settings.json` `UserPromptSubmit` hook that writes a marker file.
2. From that folder, in an environment without the Claude Code session variables, run the recommended invocation with `--debug-file <file>` and (a) a Russian sentence, (b) the probe question from [Leak tests](#leak-tests) with a plain "answer honestly" system prompt and without `--json-schema`.
3. Check: no canary in the output, no marker file, the debug log still says `Safe mode: …`, `[claudeai-mcp] Disabled in safe mode` and `project memory is off`, and the envelope still has `result`, `is_error` and `structured_output`.
4. Check `claude agents --json` during a run and `claude -p --resume <session_id>` after it, then update `claudeVersion` (and anything else that changed) in `claude-invocation.json`.
