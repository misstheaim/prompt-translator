# Setup

The commands behind the first-run setup: checking the machine, recommending a local model, installing and starting Ollama, downloading and benchmarking a model. There is no setup wizard screen yet; the only visible part is the "Start Ollama" button on the Ollama-unreachable error banner (SETUP-002). The webview reaches the commands through the wrappers in `src/lib/api.ts` (`environmentCheck`, `ollamaInstall`, `ollamaStart`, `ollamaPull`, `ollamaBenchmark`, `cancelSetupTask`, `completeSetup`, and `onSetupProgress` for the progress event).

> Code: `src-tauri/src/setup/`, `src/components/ErrorBanner.tsx`, `src/lib/api.ts`

## Behaviour

### SETUP-001 Environment check and model tier
`environment_check` never fails: each check that cannot run reports "not available". The checks run in parallel, each limited to 3 s. It returns `EnvironmentReport`: `os` (e.g. "Windows 11 (build 26200)"), `ramMb` and `ramAvailableMb` (0 when unknown), `gpus`, `usableGpu`, `recommendation`, `winget` (`winget --version` succeeds), `ollama` (`installed`, `running`, `version`, `models`) and `claude` (the Claude Code status of PROV-006).
- `gpus`: every non-software graphics adapter with `name`, dedicated `vramMb`, `integrated` and `usable`. Integrated: Intel, AMD with under 1024 MB dedicated memory, or a name containing "radeon graphics", "radeon(tm) graphics", "uhd" or "iris". Usable: not integrated, NVIDIA or AMD, with at least 3900 MB of VRAM. `usableGpu` is true when any adapter is usable.
- `recommendation` `{ tier, model, downloadMb }` follows the usable GPU with the most VRAM: under 5800 MB `gpu-small` (`qwen3.5:4b`, 3300 MB); 5800 to under 10500 MB `gpu-medium` and 10500 MB or more `gpu-large` (both `qwen3.5:9b`, 6600 MB); no usable GPU gives `none` with `model` and `downloadMb` null (Claude Code is the recommended provider; Ollama stays selectable).
- `ollama.installed` is true when `ollama.exe` is in `%LOCALAPPDATA%\Programs\Ollama` or on `PATH`, or when the server answers; `running` and `models` come from `GET <endpoint>/api/tags`, `version` from `/api/version`.
Since: #69

### SETUP-002 Starting Ollama
`ollama_start` returns at once if `GET <endpoint>/api/tags` answers (2 s timeout). Otherwise it launches Ollama detached (`ollama app.exe` from `%LOCALAPPDATA%\Programs\Ollama`, else `ollama serve` from there or from `PATH`) and checks every 500 ms for up to 30 s. The app never stops that Ollama. Errors: `InvalidInput` "Ollama isn't installed. Install it first."; `Io` "Couldn't start Ollama: <text>"; `Io` "Ollama didn't start within 30 seconds"; `Cancelled` (SETUP-007).
In the UI, an error banner for `OllamaUnreachable` (APP-022), in the main window and in the popup, has a "Start Ollama" button. While it works the button reads "Starting Ollama..." and is disabled; on success the screen's Retry action runs (if it has one); on failure the banner adds "Couldn't start Ollama: <message>".
Since: #70

### SETUP-003 Downloading a model
`ollama_pull(model)` downloads a model through `POST <endpoint>/api/pull` and then sets `ollama.model` to it (trimmed), saves the settings and emits `settings://changed`. An empty name or one with whitespace fails with `InvalidInput` "Enter a valid model name". Progress arrives as SETUP-007 events. Errors: `OllamaUnreachable` (cannot connect); `Io` "Ollama refused the download (HTTP <status>): <first 300 characters>"; `Io` "Couldn't download <model>: <Ollama's message>"; `Io` "The download stalled: Ollama sent nothing for two minutes" (120 s without data); `Io` "The download was interrupted: <text>"; `Io` "The download ended before Ollama reported success. Retry to resume it."; `Cancelled`. A failed or cancelled download leaves `ollama.model` as it was.
Since: #70

### SETUP-004 Installing Ollama
`ollama_install` runs `winget install -e --id Ollama.Ollama --accept-package-agreements --accept-source-agreements --silent` without a console window and reports each non-trivial output line as a progress event. Exit code 0 and winget's "already installed" and "no applicable upgrade" codes (0x8A150061, 0x8A15002B) count as success. Errors: `InvalidInput` "winget isn't available on this PC. Download Ollama from https://ollama.com/download and install it manually."; `Io` "Couldn't start winget: <text>"; `Io` "The Ollama installation failed (winget exit code <hex>): <last output line>"; `Cancelled` (winget is killed).
Since: #70

### SETUP-005 Completing the setup
`complete_setup` sets `setupCompleted` to true, saves the settings and emits `settings://changed`. Nothing reads the flag yet.
Since: #70

### SETUP-006 Model benchmark
`ollama_benchmark(model)` times the model with the Ollama provider, using the settings' endpoint and timeouts but the given model, and with history off for these runs. The preset is `general`, or the first preset if it was deleted (`InvalidInput` "There is no preset to run the test with" when there is none). It runs one untimed Translate to load the model, then a timed Translate (a fixed Russian sentence) and a timed Correct (a fixed English sentence with mistakes), and reads `GET <endpoint>/api/ps` (2 s). It returns `{ translateMs, correctMs, gpuOffloadPercent }`; `gpuOffloadPercent` is the share of the loaded model in GPU memory (0 to 100), 0 when the model is not listed or the call fails. A failing run returns its translation error (TRANS-008, PROV-003) as it is.
Since: #70

### SETUP-007 Progress events and one step at a time
`ollama_install`, `ollama_start` and `ollama_pull` share one slot: starting one while another runs fails with `InvalidInput` "Another setup step is still running". `cancel_setup_task` cancels the one that runs (nothing happens when none does), which then fails with `Cancelled`. The benchmark does not use the slot.
The event `setup://progress` carries `{ phase, message?, completedBytes?, totalBytes?, status? }`. Phase `install`: `message` is one output line of winget (blank lines and single spinner characters are dropped). Phase `download`: `status` is Ollama's status text (e.g. "pulling manifest") and the byte counts are summed over all layers; sent when the status changes and at most every 150 ms otherwise; a last event with `status` "success" has `completedBytes` equal to `totalBytes`.
Since: #70

## Known issues

None.

Retired IDs: none
