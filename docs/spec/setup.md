# Setup

The commands behind the first-run setup: checking the machine, recommending a local model, installing and starting Ollama, downloading and benchmarking a model. The setup wizard screen (SETUP-008 to SETUP-013) drives them; the "Start Ollama" button on the Ollama-unreachable error banner (SETUP-002) is the other visible part. The webview reaches the commands through the wrappers in `src/lib/api.ts` (`environmentCheck`, `ollamaInstall`, `ollamaStart`, `ollamaPull`, `ollamaBenchmark`, `cancelSetupTask`, `completeSetup`, and `onSetupProgress` for the progress event).

> Code: `src-tauri/src/setup/`, `src/screens/Setup/`, `src/components/ErrorBanner.tsx`, `src/lib/api.ts`

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
`complete_setup` sets `setupCompleted` to true, saves the settings and emits `settings://changed`. The main window reads the flag (SETUP-008); the wizard's Finish button calls it.
Since: #70 · Changed: #71

### SETUP-006 Model benchmark
`ollama_benchmark(model)` times the model with the Ollama provider, using the settings' endpoint and timeouts but the given model, and with history off for these runs. The preset is `general`, or the first preset if it was deleted (`InvalidInput` "There is no preset to run the test with" when there is none). It runs one untimed Translate to load the model, then a timed Translate (a fixed Russian sentence) and a timed Correct (a fixed English sentence with mistakes), and reads `GET <endpoint>/api/ps` (2 s). It returns `{ translateMs, correctMs, gpuOffloadPercent }`; `gpuOffloadPercent` is the share of the loaded model in GPU memory (0 to 100), 0 when the model is not listed or the call fails. A failing run returns its translation error (TRANS-008, PROV-003) as it is.
Since: #70

### SETUP-007 Progress events and one step at a time
`ollama_install`, `ollama_start` and `ollama_pull` share one slot: starting one while another runs fails with `InvalidInput` "Another setup step is still running". `cancel_setup_task` cancels the one that runs (nothing happens when none does), which then fails with `Cancelled`. The benchmark does not use the slot.
The event `setup://progress` carries `{ phase, message?, completedBytes?, totalBytes?, status? }`. Phase `install`: `message` is one output line of winget (blank lines and single spinner characters are dropped). Phase `download`: `status` is Ollama's status text (e.g. "pulling manifest") and the byte counts are summed over all layers; sent when the status changes and at most every 150 ms otherwise; a last event with `status` "success" has `completedBytes` equal to `totalBytes`.
Since: #70

## Setup wizard

### SETUP-008 When the wizard shows
The wizard fills the main window (a "Setup" header with the numbered steps, a "Leave setup" button, and the step below), in place of the tabs, while the loaded settings have `setupCompleted` false (until the user leaves it, SETUP-009; it shows again at the next start), and when the user opens it on purpose (SETUP-009). It does not show before the settings are loaded. The steps are 1 Welcome, 2 Local model, 3 Claude Code, 4 Defaults, 5 Done; the current one is highlighted. Every step but the first has Back; Continue/Skip/Finish go forward. Progress (step, chosen model size, benchmark result) is kept while the app runs, so leaving and coming back resumes it; it is lost when the app restarts (the wizard then starts at Welcome).
Since: #71

### SETUP-009 Leaving and resuming
"Leave setup" closes the wizard without changing `setupCompleted`. While `setupCompleted` is false the shell then shows an info banner under the tab bar, "Setup isn't finished. Translating needs a working provider." with a "Resume setup" button that reopens the wizard at the step where it was left. Settings, About has a "Run setup again" button that opens the wizard even when setup is completed. Finishing (SETUP-013) closes it and selects the Translate tab.
Since: #71

### SETUP-010 Welcome and Local model steps
Welcome explains the app and its privacy (text goes only to the chosen provider; only accepted English text is copied to the clipboard) and has "Get started".
Local model runs the environment check (SETUP-001) on entry ("Checking this computer...") and describes the machine: the graphics card and memory, or "No supported graphics card (...)" with an info banner and a "Set up anyway" button that reveals the local-model controls. A failed check shows the error with Retry and a Skip button. The controls (shown for a usable GPU or after "Set up anyway"):
- a "Model size" select: Small `qwen3.5:4b` (3.3 GB), Medium `qwen3.5:9b` (6.6 GB), Large `qwen3.5:9b` (6.6 GB), the machine's recommended tier marked "(recommended)" and preselected; a tier above the recommendation shows a warning that it may be slow or run out of memory;
- one primary action that follows the state: Ollama not installed gives "Install Ollama" (SETUP-004; without winget "Open the Ollama download page" and a note to install it and choose "Check again"), plus "Check again"; installed but not running gives "Start Ollama" (SETUP-002); the model missing gives "Download <model> (<size>)" (SETUP-003) which then runs the benchmark by itself; otherwise "Measure speed" ("Measure again" after a result) (SETUP-006);
- while a step runs, its label ("Installing Ollama...", "Starting Ollama...", "Downloading the model...", "Measuring speed (...)"), a progress bar with percent and sizes for download and install (from SETUP-007 events), and a Cancel button (not for the benchmark); a cancelled step shows no error, another failure shows its error banner;
- a benchmark result "Translate: <s> · Correct: <s> · On the graphics card: <n>%". It is flagged slow when Translate takes over 8 s, Correct over 20 s, or, on a machine with a recommended tier, under 50% of the model is on the graphics card; a slow result shows a warning with the reasons and says Claude Code will be preselected as default provider, otherwise "The local model is fast enough on this computer." Choosing another model size clears the result.
The forward button reads "Continue" (primary) once Ollama runs, the model is installed and a benchmark exists, else "Skip"; it is disabled while a step runs.
Since: #71

### SETUP-011 Claude Code step
On entry it re-checks the provider status (PROV-013) and shows "Claude Code found (version <v>)" with "Signed in.", a warning "Not signed in. Run claude once in a terminal and sign in, then choose "Check again".", or an info "Couldn't tell if you're signed in..."; when not found, a warning with an "Install instructions" button that opens Claude Code's setup page. "Check again" repeats the check. The forward button is "Continue" when Claude Code is found and not known to be signed out, else "Skip".
Since: #71

### SETUP-012 Defaults step
Provider: a select listing only the working providers (Ollama reachable with the selected model installed; Claude Code found and not signed out). It preselects Claude Code when it works and either the benchmark was slow or Ollama does not work, else Ollama when it works; with no working provider a warning says so and the default is left as it is. A "Languages" section has two searchable selects, "My language" (`nativeLanguage`) and "Language I write prompts in" (`targetLanguage`), preselected from the settings (Russian / English by default), listing the supported languages by English name (TRANS-015). Choosing the same language twice shows "Choose two different languages" and disables Continue. Also editable: the global hotkey (as in SETTINGS-010), "Start with Windows" and "Save translations to history", all initialised from the current settings. Continue saves the settings (with the current `ollama.model`, which a model download may have changed, and the language pair) and moves on; a refused save shows its error and stays.
Since: #71 · Changed: #151

### SETUP-013 Done step
It re-checks the provider status and shows a success banner "You're all set. Ollama is ready to translate.", "... Claude Code is ready ..." or "... Ollama and Claude Code are ready ...", or, with no working provider, a warning that translating will fail until one is set up (the user can go Back or finish). It adds "You can run this setup again any time from Settings, About." "Finish" calls `complete_setup` (SETUP-005), clears the wizard's progress and opens the Translate tab; a failure shows its error.
Since: #71

## Known issues

None.

Retired IDs: none
