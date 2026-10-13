# Local Prompt Translator for Claude Code

## 1. Goal

Build a small desktop application that acts as an input layer in front of Claude Code.

The application allows the user to:

1. Enter a prompt in their native language.
2. Select how the prompt should be processed:
   - **Local LLM**: translate/rewrite locally.
   - **Claude Code session**: use a separate Claude Code process/session for translation.
3. Apply a configurable preset containing translation skills and rules.
4. Review the resulting English prompt.
5. Send only the English prompt to the main Claude Code session.

The most important architectural requirement is:

> The main Claude Code session must never receive the original native-language prompt or the translator's instructions/context.

The translator is therefore a separate layer, not another participant in the main Claude conversation.

---

# 2. Recommended Architecture

```text
                    ┌─────────────────────────┐
                    │       Desktop UI        │
                    │                         │
                    │ Native prompt           │
                    │ Translation method      │
                    │ Preset                   │
                    │ Rules                    │
                    │ English preview          │
                    └────────────┬────────────┘
                                 │
                                 ▼
                    ┌─────────────────────────┐
                    │   Translation Service    │
                    │                         │
                    │ Common translation      │
                    │ contract/interface      │
                    └──────┬──────────┬───────┘
                           │          │
                 Local LLM │          │ Claude Code
                           │          │
                           ▼          ▼
                    ┌───────────┐  ┌──────────────┐
                    │ Local LLM │  │ Separate     │
                    │ Backend   │  │ Claude Code  │
                    └───────────┘  │ process      │
                                   └──────────────┘
                           │          │
                           └────┬─────┘
                                ▼
                     ┌─────────────────────┐
                     │ English Prompt      │
                     │ + optional metadata │
                     └──────────┬──────────┘
                                │
                                ▼
                     ┌─────────────────────┐
                     │ Main Claude Code    │
                     │ session             │
                     └─────────────────────┘
```

---

# 3. Technology Recommendation

For this particular tool, the simplest practical choice is:

## Desktop framework

**Tauri 2 + TypeScript + React**

Why:

- lightweight compared with Electron;
- easy to create a modern desktop UI;
- cross-platform if the project later needs macOS/Linux support;
- React is mature and well suited for this small UI;
- Tauri can execute local processes such as a local LLM runtime or Claude Code CLI;
- easy access to clipboard, keyboard shortcuts, and desktop integration;
- small application footprint.

Recommended stack:

```text
UI:
React
TypeScript
Vite

Desktop:
Tauri 2

State:
Zustand or simple React state

Styling:
Tailwind CSS

Local storage:
SQLite or a small JSON configuration file

Local LLM:
Ollama initially

Claude integration:
Claude Code CLI as a separate process

Packaging:
Tauri bundler
```

### Why not Electron?

Electron would also work, but it is unnecessary for this application.

The application mostly needs:

- a text box;
- settings;
- process execution;
- clipboard;
- keyboard shortcuts;
- a small amount of local state.

Tauri is a better fit for this footprint.

---

# 4. Translation Backend Abstraction

The most important implementation decision is to hide the translation mechanism behind one interface.

Conceptually:

```typescript
interface TranslationProvider {
    translate(request: TranslationRequest): Promise<TranslationResult>;
}
```

Implement:

```text
TranslationProvider
       │
       ├── LocalLlmProvider
       │
       └── ClaudeCodeProvider
```

This means the UI does not care how translation happens.

For example:

```typescript
type TranslationMode =
    | "local-llm"
    | "claude-code";
```

The application simply chooses the provider.

This also makes it easy to add another provider later.

---

# 5. Translation Request

Use a structured request instead of passing random strings between components.

```typescript
interface TranslationRequest {
    sourceText: string;

    sourceLanguage?: string;

    targetLanguage: "en";

    preset: TranslationPreset;

    rules: TranslationRule[];

    preserveTechnicalTerms: boolean;

    improveNaturalness: boolean;

    preserveIntent: boolean;
}
```

The result:

```typescript
interface TranslationResult {
    translatedText: string;

    provider: "local-llm" | "claude-code";

    durationMs: number;

    success: boolean;

    error?: string;
}
```

The UI should only display `translatedText`.

---

# 6. Presets

Presets are one of the most valuable parts of the application.

Instead of creating a completely different prompt every time, define reusable translation profiles.

Example presets:

```text
General
Technical / Development
Code Review
Bug Investigation
Jira / Work Communication
Architecture Discussion
Documentation
```

The user can select:

```text
Preset: Technical / Development
```

and the application automatically supplies the appropriate rules.

---

# 7. Translation Rules

Rules should be editable by the user.

Example:

```text
Preserve all code identifiers.

Never translate:
- class names
- method names
- variable names
- database names
- table names
- column names
- filenames
- URLs
- ticket numbers
- product names
- technology names

Do not invent information.

Preserve the original intent.

Improve English grammar and naturalness.

Use concise professional technical English.

Do not answer the user's question.

Return only the resulting English prompt.
```

These rules should be stored outside the code.

For example:

```text
presets/
    general.json
    technical.json
    code-review.json
    jira.json
```

---

# 8. Important Distinction: Translation vs Prompt Improvement

The application should support two concepts.

## Translation

Stay close to the original meaning.

Example:

```text
Native language:
"check why this method returns null"
```

Output:

```text
"Check why this method returns null."
```

## Prompt improvement

Preserve the intent but make the request more natural and useful to Claude.

Input:

```text
"check why this method returns null,
I think maybe because database doesn't have record"
```

Output:

```text
"Investigate why this method returns null.
I suspect the issue may be that the corresponding record
does not exist in the database. Please verify this assumption."
```

For development work, **Prompt improvement** should be the default.

However, it must never change the technical intent.

---

# 9. Local LLM Backend

The recommended initial local runtime is **Ollama**.

The application communicates with Ollama through its local HTTP API.

Architecture:

```text
Tauri
  │
  │ localhost HTTP
  ▼
Ollama
  │
  ▼
Local LLM
```

The application should not implement model inference itself.

Ollama provides the runtime and model management.

This keeps the application simple.

## Ollama Lifecycle

Ollama should remain a separate server process, not be embedded into the desktop application. The desktop application manages its lifecycle.

### Application startup

```text
Application starts
       │
       ▼
Check whether Ollama is already running
       │
       ├── Yes → use existing Ollama
       │
       └── No
            │
            ▼
       Start Ollama server
            │
            ▼
       Wait until ready
            │
            ▼
       Enable Local LLM
```

### Application shutdown

The application must track whether it started Ollama.

```text
Application closes
       │
       ▼
Did this application start Ollama?
       │
       ├── No → leave Ollama running
       │
       └── Yes → stop Ollama
```

If Ollama was already running before the translator started, the translator must not terminate it when it exits.

The application should expose an `OllamaManager` responsible for:

```text
isRunning()
start()
waitUntilReady()
stop()
wasStartedByApplication()
```

### Model lifetime

For the initial implementation, keep the selected model available while the translator application is running. This avoids repeatedly loading the model for every prompt.

If memory optimization becomes important later, add an idle timeout that unloads the model after several minutes without translation while keeping the application UI open.

---

# 10. Local Model

Do not hard-code a single model permanently.

Make the model configurable.

Example configuration:

```json
{
    "provider": "local-llm",
    "model": "qwen",
    "endpoint": "http://localhost:11434"
}
```

The exact model should be selected based on the user's hardware.

A practical strategy is:

```text
Low-end machine
    -> smaller model

Normal development PC
    -> medium model

Powerful workstation
    -> larger model
```

Translation does not require an enormous model, especially if the application provides a strong system prompt and explicit preservation rules.

---

# 11. Local LLM Prompt

Use a strict system prompt.

Conceptually:

```text
You are a technical English prompt translator.

Your task is to convert the user's native-language request
into natural, precise English suitable for Claude Code.

Rules:

1. Preserve the user's original intent.
2. Do not answer the request.
3. Return only the translated/improved English prompt.
4. Preserve code identifiers exactly.
5. Preserve filenames exactly.
6. Preserve URLs exactly.
7. Preserve ticket numbers exactly.
8. Preserve database, table, column, class, method and variable names.
9. Do not invent technical details.
10. Improve grammar and naturalness.
11. Use concise professional technical English.
12. If the original request is ambiguous, preserve the ambiguity
    rather than inventing an interpretation.
```

Then provide:

```text
USER PROMPT:

{sourceText}
```

The output should contain only the English prompt.

---

# 12. Claude Code Backend

The second provider should use a **separate Claude Code process/session**.

Conceptually:

```text
Translator application
        │
        │ spawn
        ▼
Separate Claude Code process
        │
        │ translation only
        ▼
English text
        │
        ▼
Translator application
```

This process must not be the same process/session as the user's actual Claude Code session.

The translation session should receive:

1. the translator system instructions;
2. the selected preset;
3. the user’s native-language text.

It should not receive:

- the main Claude conversation;
- repository conversation history;
- previous Claude messages;
- unrelated project context.

This preserves the separation you want.

---

# 13. Claude Code Translation Session Lifecycle

There are two possible approaches.

## Approach A: One process per translation

```text
User presses Translate
        │
        ▼
Start Claude Code
        │
        ▼
Send translation prompt
        │
        ▼
Read result
        │
        ▼
Terminate process
```

Advantages:

- strongest isolation;
- no accumulated translation context;
- simple mental model;
- no session pollution.

This should be the MVP.

## Approach B: Persistent translation session

```text
Application starts
        │
        ▼
Start translator Claude session
        │
        ├── Translation 1
        ├── Translation 2
        ├── Translation 3
        └── ...
```

This is potentially faster, but it creates context accumulation.

For this application, **Approach A is preferable initially**.

---

# 14. Claude Code Authentication

Do not attempt to extract or reuse private Claude Code authentication tokens manually.

The application should rely on the normal Claude Code authentication mechanism available on the machine.

The application launches the CLI as a normal subprocess.

Before implementing this backend, verify which Claude Code CLI invocation mode and authentication flow are currently supported by the installed Claude Code version.

The integration should treat Claude Code as an external executable rather than attempting to reproduce its authentication protocol.

---

# 15. Sending the Final Prompt to Claude Code

This should also be abstracted.

Create:

```typescript
interface ClaudeTarget {
    sendPrompt(prompt: string): Promise<void>;
}
```

Potential implementations:

```text
ClaudeTarget
     │
     ├── ClipboardTarget
     │
     ├── TerminalTarget
     │
     └── FutureClaudeIntegrationTarget
```

## MVP

Use clipboard.

Flow:

```text
Translate
   ↓
Preview English
   ↓
Send
   ↓
Copy English text
   ↓
User pastes into Claude Code
```

This avoids building a fragile UI automation mechanism.

---

# 16. Later Claude Code Integration

After the MVP works, investigate whether the installed Claude Code version provides a supported mechanism for programmatic input.

If it does, add:

```text
Send to Claude Code
```

without changing the translation architecture.

The UI should not know whether the target is:

```text
clipboard
```

or:

```text
Claude Code CLI
```

That decision belongs to `ClaudeTarget`.

---

# 17. UI Design

Keep the UI intentionally small.

```text
┌──────────────────────────────────────────────────────┐
│ Prompt Translator                                    │
├──────────────────────────────────────────────────────┤
│                                                      │
│ Translation method                                   │
│ [ Local LLM ▼ ]                                      │
│                                                      │
│ Preset                                               │
│ [ Technical Development ▼ ]                          │
│                                                      │
│ Native-language prompt                               │
│ ┌──────────────────────────────────────────────────┐ │
│ │                                                  │ │
│ │                                                  │ │
│ │                                                  │ │
│ └──────────────────────────────────────────────────┘ │
│                                                      │
│ [ Translate ]                                        │
│                                                      │
│ English prompt                                       │
│ ┌──────────────────────────────────────────────────┐ │
│ │                                                  │ │
│ │                                                  │ │
│ │                                                  │ │
│ └──────────────────────────────────────────────────┘ │
│                                                      │
│ [ Copy ]                         [ Send to Claude ]   │
│                                                      │
└──────────────────────────────────────────────────────┘
```

---

# 18. Advanced Settings

Settings should contain:

## Local LLM

```text
Provider:
    Ollama

Endpoint:
    http://localhost:11434

Model:
    [ configurable ]
```

## Claude Code

```text
Claude Code executable:
    [ auto-detected ]

Working directory:
    [ optional ]

Timeout:
    [ configurable ]
```

## Translation

```text
Source language:
    Auto

Target language:
    English

Improve naturalness:
    ON

Preserve technical terms:
    ON

Preserve code:
    ON
```

---

# 19. Keyboard Shortcuts

A global shortcut would make this tool particularly useful.

For example:

```text
Ctrl + Shift + Space
```

opens the translator.

Then:

```text
Ctrl + Enter
```

translates.

And:

```text
Ctrl + Shift + Enter
```

could send/copy the result.

The exact shortcuts can be configurable.

---

# 20. Clipboard Workflow

A very useful feature is:

```text
Copy selected text
       ↓
Open translator
       ↓
Native text automatically appears
```

The application could optionally read the clipboard when opened.

Example:

```text
Claude / browser / IDE
        │
        │ Ctrl+C
        ▼
Clipboard
        │
        ▼
Prompt Translator
```

This allows the user to copy a native-language prompt from anywhere and immediately translate it.

---

# 21. History

Keep a small local history.

Example:

```text
History
──────────────────────────────────────────
Today 18:31
"check why migration fails..."
→
"Investigate why the migration fails..."

Today 17:54
"можешь проверить..."
→
"Could you check..."
```

History should be optional.

Because prompts may contain sensitive source-code information, the application should make it possible to disable history completely.

---

# 22. Privacy Model

The application should explicitly distinguish three modes.

## Local LLM

```text
Native prompt
     │
     ▼
Local machine
     │
     ▼
Local model
```

Nothing is sent to an external translation provider.

## Claude Code translation

```text
Native prompt
     │
     ▼
Claude Code
```

The native-language prompt is sent to Claude through the separate translation session.

## Main Claude session

```text
ONLY English prompt
```

The original native-language prompt is never forwarded.

---

# 23. Error Handling

The application should handle:

```text
Local LLM unavailable
Claude Code unavailable
Model not installed
Claude authentication failure
Translation timeout
Empty translation
Malformed output
Process crash
```

The UI should show actionable messages.

Example:

```text
Local LLM is unavailable.

Make sure Ollama is running and the selected model
is installed.

[ Retry ] [ Open Settings ]
```

---

# 24. Translation Output Validation

Before displaying the result, perform lightweight validation.

Check:

```text
translatedText is not empty
```

and optionally verify preservation of important tokens.

For example, if the input contains:

```text
PD-3693
MigrationQueue
GetStudentData
appsettings.json
```

the output should still contain them.

The application can detect accidental modifications:

```text
Warning:

The translation changed the following technical tokens:

MigrationQueue → Migration Queue
```

Then allow:

```text
[ Accept ] [ Retry Translation ]
```

This is especially valuable for programming prompts.

---

# 25. Prompt Preset Format

A preset could look like:

```json
{
    "id": "technical-development",
    "name": "Technical Development",
    "description": "For programming and software development prompts",

    "rules": [
        "Preserve all code identifiers.",
        "Preserve filenames.",
        "Preserve URLs.",
        "Preserve ticket numbers.",
        "Preserve database terminology.",
        "Use professional technical English.",
        "Do not answer the request.",
        "Do not invent missing information."
    ],

    "settings": {
        "improveNaturalness": true,
        "preserveTechnicalTerms": true,
        "preserveIntent": true
    }
}
```

---

# 26. Suggested Project Structure

```text
prompt-translator/
│
├── src/
│   ├── components/
│   │   ├── PromptInput.tsx
│   │   ├── PromptOutput.tsx
│   │   ├── ProviderSelector.tsx
│   │   ├── PresetSelector.tsx
│   │   └── SettingsPanel.tsx
│   │
│   ├── services/
│   │   ├── translation/
│   │   │   ├── TranslationProvider.ts
│   │   │   ├── LocalLlmProvider.ts
│   │   │   └── ClaudeCodeProvider.ts
│   │   │
│   │   ├── claude/
│   │   │   └── ClaudeTarget.ts
│   │   │
│   │   ├── clipboard/
│   │   │   └── ClipboardService.ts
│   │   │
│   │   └── presets/
│   │       └── PresetService.ts
│   │
│   ├── models/
│   │   ├── TranslationRequest.ts
│   │   ├── TranslationResult.ts
│   │   └── TranslationPreset.ts
│   │
│   └── App.tsx
│
├── src-tauri/
│   ├── src/
│   │   └── main.rs
│   ├── capabilities/
│   └── tauri.conf.json
│
├── presets/
│   ├── general.json
│   ├── technical.json
│   ├── code-review.json
│   └── jira.json
│
└── README.md
```

The exact structure can be simplified during implementation.

---

# 27. MVP Scope

Do not implement everything at once.

Version 1 should contain only:

```text
1. Desktop UI
2. Native-language text box
3. Translation provider selector
4. Preset selector
5. Local LLM provider
6. Claude Code provider
7. English preview
8. Copy button
9. Basic settings
```

Do NOT initially implement:

```text
- automatic repository detection
- persistent Claude translation sessions
- complex history
- advanced analytics
- automatic context extraction
- UI automation
- complicated plugin architecture
```

---

# 28. Development Phases

## Phase 1 - UI

Build:

```text
Input
Provider selector
Preset selector
Output
Copy
Settings
```

Use mocked translation initially.

---

## Phase 2 - Local LLM

Integrate Ollama.

Test:

```text
Native language
      ↓
Ollama
      ↓
English
```

Measure:

- latency;
- translation quality;
- CPU/GPU usage;
- memory usage.

---

## Phase 3 - Claude Code Provider

Implement:

```text
Tauri
   ↓
spawn Claude Code
   ↓
translation request
   ↓
capture output
   ↓
terminate process
```

Ensure the translator has an isolated context.

---

## Phase 4 - Presets

Implement editable presets.

Start with:

```text
General
Technical Development
Code Review
Work Communication
```

---

## Phase 5 - Claude Target

Initially:

```text
Send to Claude
       ↓
Copy English prompt
```

Then investigate direct Claude Code integration.

---

## Phase 6 - Quality Improvements

Add:

```text
technical token validation
retry
history
global shortcut
clipboard auto-import
model selection
translation statistics
```

---

# 29. Recommended Default Workflow

The ideal daily workflow should become:

```text
1. Think in native language.

2. Open Prompt Translator.

3. Enter prompt.

4. Select:
       Local LLM
       Technical Development

5. Press Ctrl+Enter.

6. Review English prompt.

7. Press Send to Claude.

8. Claude receives ONLY the English prompt.
```

The translator should disappear into the workflow rather than become another application you have to manage.

---

# 30. Long-Term Architecture

If the application becomes more sophisticated, the architecture can evolve into:

```text
                     Prompt Translator
                            │
             ┌──────────────┴──────────────┐
             │                             │
       Translation                    Prompt Processing
             │                             │
      ┌──────┴──────┐                ┌─────┴─────┐
      │             │                │           │
   Local LLM    Claude Code       Presets    Rules
      │             │                │           │
      └─────────────┴────────────────┴───────────┘
                            │
                            ▼
                    English Prompt
                            │
                            ▼
                     Claude Target
                            │
                 ┌──────────┴──────────┐
                 │                     │
             Clipboard             Claude Code
```

The important principle is to keep these responsibilities independent:

```text
Translation
    ≠
Prompt improvement
    ≠
Claude Code communication
    ≠
UI
```

This makes the project easy to change later.

---

# 31. Final Recommendation

For the first implementation, use:

```text
Tauri 2
React
TypeScript
Tailwind
Ollama
Local LLM
Claude Code CLI
JSON presets
Clipboard integration
```

The central abstraction should be:

```text
TranslationProvider
```

with:

```text
LocalLlmProvider
ClaudeCodeProvider
```

And the application should treat Claude Code as two completely separate roles:

```text
                 Claude Code
                     │
          ┌──────────┴──────────┐
          │                     │
   Translation session     Main user session
          │                     │
   Native language          English only
          │                     │
          ▼                     ▼
     Translation            Development
```

This gives you the flexibility to use a free/private local model for most prompts while retaining Claude-based translation when you want higher quality, without contaminating the actual Claude Code development context.
