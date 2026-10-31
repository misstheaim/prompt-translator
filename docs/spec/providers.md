# Providers

The two real translation providers, Ollama (a local model over HTTP) and Claude Code (an isolated, short-lived `claude -p` process), and the status check that reports on both. Both are equal choices in every mode: nothing picks a provider per mode. The request's `provider` field selects one (TRANS-008); the Translate screen lets the user choose (TRANS-013) and the Settings screen shows the status (SETTINGS-009). The mock provider is in TRANS-011.

> Code: `src-tauri/src/translation/providers/` (`ollama.rs`, `claude_code/`), `src-tauri/src/translation/commands.rs`, `docs/spikes/claude-invocation.json` (the Claude Code contract)

## Behaviour

## Ollama

### PROV-001 Ollama request
A run sends `POST <ollama.endpoint>/api/chat` (trailing `/` of the endpoint removed) with the model `ollama.model` (default `qwen3.5:9b`), the system prompt and the user message as chat messages, `stream: false`, `think: false`, `format` set to the output JSON Schema of the mode (TRANS-012), `keep_alive` from `ollama.keepAlive` (default `10m`), `options.temperature` 0.2 and `options.num_ctx` from `ollama.numCtx` (default 8192). The answer is the `message.content` text, which is then parsed (TRANS-007); the result's model is the configured model. A response that is not JSON or has no `message.content` fails with `MalformedOutput` (the first 2000 characters as detail). The run is limited by `ollama.timeoutSecs` (default 60 s, TRANS-009).
Since: #46 · Changed: #146

### PROV-002 Too-long prompts are refused
Before anything is sent, the prompt size is estimated in tokens: system characters / 4, plus user-message characters / 3, plus the expected answer (user-message characters times 0.3 for Translate, 0.6 for Improve, 1.25 for Correct, rounded up). If the estimate is above `ollama.numCtx`, the run fails with `InvalidInput` "The text is too long for the local model in this mode. Shorten it, split it into parts, switch to Translate mode, or use Claude Code." and nothing is sent.
Since: #46

### PROV-003 Ollama errors
- Cannot connect: `OllamaUnreachable`.
- HTTP 404, or an error text that contains both "model" and "not found": `ModelMissing` with the configured model name.
- Any other non-2xx status: `Io` "Ollama returned HTTP <status>: <first 300 characters of the error text>" (the `error` field of a JSON body, else the body).
- Any other send or read failure: `Io` "Ollama request failed: <text>".
- Cancel: `Cancelled`.
Since: #46

### PROV-004 Ollama status
`GET <endpoint>/api/tags` with a 2 s timeout gives `{ reachable, models, selectedModelInstalled }`: `models` lists the installed model names, and `selectedModelInstalled` is true when `ollama.model` is one of them (a name without a tag means `:latest`). Any failure (not running, error status, timeout) gives `reachable` false, no models and `selectedModelInstalled` false; the check never fails.
Since: #46

## Claude Code

### PROV-005 Finding Claude Code
Searched in this order, the first hit wins: the path in `claude.executablePath` (if not blank and the file exists; a configured path that does not exist is logged and the search continues), `%USERPROFILE%\.local\bin\claude.exe`, `claude.exe` in a `PATH` folder, and only then `claude.cmd` in a `PATH` folder. A file with the extension `.cmd` or `.bat` is a shim, which cannot be given arguments with newlines or quotes (PROV-007). The search runs again for every status check and every run, so a replaced file is picked up.
Since: #49

### PROV-006 Claude Code status
When Claude Code is not found the status is `found` false with no path, version or sign-in state. Otherwise `path` is the found file, `version` is the trimmed output of `claude --version`, and `signedIn` comes from `claude auth status`: exit code 0 is true, 1 is false, anything else is unknown (null). The two checks run side by side, each limited to 5 s (null on timeout or failure), without a console window; the output of `auth status` (it holds the account e-mail) is discarded, never read or logged.
Since: #49

### PROV-007 Isolated run
Each translation starts its own `claude -p` process from the fixed empty folder `claude-cwd/` with no console window, and the app never reads or writes the user's own Claude folders. The process gets: no settings sources, no tools, no MCP servers, no slash commands, safe mode, no session persistence, the session name `prompt-translator`, `--output-format json`, the model (PROV-010), the system prompt as a UTF-8 file in `tmp/` (`system-prompt-<uuid>.txt`, deleted when the run ends) and the output JSON Schema of the mode inline (TRANS-012). The user message goes to stdin, which is then closed. The environment has `DISABLE_AUTOUPDATER=1`, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` and `TEMP`/`TMP` set to `tmp/`, and these session markers removed: `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_SESSION_ATTENDED`, `CLAUDE_CODE_MESSAGING_SOCKET`, `CLAUDE_CODE_MESSAGING_TOKEN`, `CLAUDE_CODE_EXECPATH`, `CLAUDE_PID`, `CLAUDE_EFFORT`. If Claude Code is not found the run fails with `ClaudeNotFound`; an empty user message fails with `InvalidInput` "Enter some text first". The run is limited by `claude.timeoutSecs` (default 90 s, TRANS-009).
Since: #50

### PROV-008 Process tree is killed
The process starts suspended, is put into a Windows Job Object that kills its members when closed, and is then resumed, so everything it starts belongs to the job. When the run finishes, is cancelled or times out, the whole tree is killed (also if the app itself dies). If the job cannot be made, `taskkill /T /F` is used instead. Cancel fails the request with `Cancelled`.
Since: #50

### PROV-009 Claude Code result and errors
With exit code 0, the answer is the `structured_output` of the JSON that Claude prints; it is then parsed (TRANS-007). The free-text `result` is never used as an answer.
- No `structured_output` (the model answered in free text), or output that is not a JSON object: `MalformedOutput` (first 2000 characters as detail).
- `ClaudeAuth` only when the run failed (the JSON has `is_error` true, or the exit code is not 0) and its `result` text contains one of (case-insensitive): "not logged in", "/login", "invalid api key", "authentication", "oauth token".
- A JSON with `is_error` true otherwise: `MalformedOutput` if its `subtype` mentions `structured_output`, else `ProcessCrashed` with the `result` text (or the subtype), cut to 500 characters.
- A non-zero exit without such a JSON: `ProcessCrashed` "claude exited with code <n>: <first 500 characters of stderr, or of stdout when stderr is empty>" ("claude was terminated" when there is no exit code).
- The process cannot be started: file not found is `ClaudeNotFound`; with a `.cmd` shim that cannot take the arguments, `ProcessCrashed` "claude.cmd can't take the translator's arguments (<reason>). Install the native claude.exe or set its path in Settings."; anything else is `ProcessCrashed` "Couldn't start claude: <reason>".
Since: #50

### PROV-010 Claude model
The model is `claude.model` (trimmed); when it is blank, `haiku` is used. The result reports the model used.
Since: #50

### PROV-011 Translator-run registry
While a Claude Code translation runs, its process ID is kept in a shared registry, and removed when the run ends. A session counts as the app's own translator run if its name is exactly `prompt-translator` (never by prefix: `prompt-translator-d2` is a user's session) or its PID is in the registry; either check is enough. The session targets (SESSION-001) skip windows whose process is in the registry or an ancestor of one.
Since: #49 · Changed: #50

### PROV-012 Folder housekeeping
At startup `claude-cwd/` and `tmp/` are created and everything inside them is deleted. After every 50th finished Claude Code run, `claude-cwd/` is emptied again and files in `tmp/` older than 24 hours are deleted. Only the contents of these two folders are touched; failures are logged and never fail a run.
Since: #49 · Changed: #50

## Status command

### PROV-013 Provider status command
`provider_status` (wrapper `providerStatus`) returns `{ ollama, claude }` for the current settings: the Ollama status (PROV-004) and the Claude Code status (PROV-006). It never fails because of an unreachable provider.
Since: #46 · Changed: #49

## Text generation only

### PROV-014 No tools
The providers only generate text. The Ollama `/api/chat` request never includes `tools`, only `message.content` of the reply is read and any tool calls in it are ignored, and no setting enables tools. The Claude Code provider runs with `--tools ""` (PROV-007), so it has no tools either.
Since: #46

## Unloading the model

### PROV-015 Unloading the model
`unload_model` (wrapper `unloadModel`) asks Ollama to unload the configured model `ollama.model`: it lists the loaded models (`GET /api/ps`) and, if the model is among them, sends `POST /api/generate` with `keep_alive: 0`. Each request waits at most 3 s. It returns true when the model was unloaded and false when it was not loaded or Ollama is not running (neither is an error); another failure (for example an HTTP error status) fails with `Io`.
Since: #146

## Known issues

None.

Retired IDs: none
