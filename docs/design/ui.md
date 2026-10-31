# UI design spec

> Task T18 (#51). Source of truth for look and behaviour of every screen so all UI tickets stay consistent. Follows plan §4.8. Wireframes are ASCII and show layout and content, not pixel sizes.

## 1. Principles

- Compact, keyboard-first, calm. A utility app, not a dashboard: one accent colour, colour elsewhere only carries meaning (status, issue category).
- Light and dark follow the OS (`prefers-color-scheme`); there is no manual theme switch in v1.
- Every async view has the same five states: **idle, loading, success, error, empty** (§4).
- Text is English UI copy; the user's input and output are shown verbatim in a text font and never altered.
- Colour is never the only signal: dots and chips always have a text label or tooltip.

## 2. Layout

Main window about 720x640, resizable, minimum 560x480.

```
+--------------------------------------------------------------+
| Translate | History | Mistakes | Settings      Target: [None v]|  <- top bar (tabs + session picker)
+--------------------------------------------------------------+
| [Banner area: provider missing, update available, errors]    |  <- optional, full width
+--------------------------------------------------------------+
|                                                              |
|                    screen content                            |
|                                                              |
+--------------------------------------------------------------+
```

- Top tabs (not a left nav): the window is narrow. Order: Translate, History, Mistakes, Settings. Presets lives under Settings.
- The session picker is always visible at the right of the top bar (hidden when Claude Code is not installed).
- Content padding `p-4` (16 px); sections separated by `gap-4`.
- Setup wizard replaces the whole main UI while `setupCompleted` is false (section 3.7).

## 3. Wireframes

### 3.1 Session picker (top bar)

```
Target: [ None (copy to clipboard)      v ]
        +----------------------------------------+
        | None (copy to clipboard)               |
        | (g) api — C:\work\api — idle           |
        | (a) web — C:\work\web — busy           |
        +----------------------------------------+
```

| State | Display |
|---|---|
| Claude Code not installed | Picker hidden |
| No sessions running | List shows "No open Claude sessions" (disabled row) plus None |
| Bound, idle | Green dot (`success`) before the name |
| Bound, busy | Amber dot (`warning`), hint "prompt will be queued" under the picker |
| Bound session closed | Red text (`danger`) "Session ended"; Accept disabled until another session or None is picked |

### 3.2 Translate

Idle (empty input):

```
 Mode: [ Translate | Improve | Correct ]   Provider: [Ollama v]   Preset: [General v]
+--------------------------------------------------------------+
| Type or paste your prompt in any language...                 |
|                                                              |
|                                                              |
+--------------------------------------------------------------+
 1,204 chars                                  [ Run  Ctrl+Enter ]
+--------------------------------------------------------------+
| Result will appear here.                                     |   <- muted placeholder
+--------------------------------------------------------------+
```

Loading:

```
 ...input (read-only)...
 (spinner) Translating with qwen2.5:7b...                [ Cancel  Esc ]
+--------------------------------------------------------------+
| (skeleton lines)                                             |
+--------------------------------------------------------------+
```

Success, Translate mode:

```
+--------------------------------------------------------------+
| Please add a retry to the upload function.   (editable)      |
+--------------------------------------------------------------+
 1.8 s · qwen2.5:7b                 [ Accept (Copy)  Ctrl+Shift+Enter ]
```

Accept label: "Copy" when Target is None, "Send to <session>" when bound.

Success, Improve and Correct (adds diff and issues):

```
+--------------------------------------------------------------+
| Output (editable)                                            |
+--------------------------------------------------------------+
 Changes
+--------------------------------------------------------------+
| Please [-added-][+add] a retry to the [-upload-][+upload] ...|   <- DiffView, word level
+--------------------------------------------------------------+
 Issues (3)
 [Grammar] "added" -> "add"
           Use the base verb after "please".
 [Article] "a upload" -> "an upload"
           Use "an" before a vowel sound.
 [Vocabulary] "загрузка" -> "upload"
           Russian word translated to English.
 1.8 s · qwen2.5:7b                 [ Accept (Copy)  Ctrl+Shift+Enter ]
```

Token warning (yellow banner, not the main action):

```
+--------------------------------------------------------------+
| (!) The result may have changed a file name or code token:   |
|     `uploadFile`.                       [ Ignore ] [ Retry ] |
+--------------------------------------------------------------+
```

Error:

```
+--------------------------------------------------------------+
| (x) Could not reach Ollama.                                  |
|     Start Ollama or pick another provider.                   |
|                              [ Retry ] [ Open settings ]     |
+--------------------------------------------------------------+
```

No provider working: yellow-less info banner above the form: "Set up a provider to start translating." with [ Run setup ].

Empty input: Run is disabled; no error is shown.

### 3.3 History

List and detail side by side when the window is wide (>= 680 px), otherwise detail replaces the list with a Back button.

```
 [ Search history...                         ]
+---------------------------+----------------------------------+
| 24 Oct 14:02  [Correct]   | Source                           |
| Ollama                    | сделай ретрай для upload         |
| "сделай ретрай..." ->     | Output                           |
| "Add a retry..."          | Add a retry to upload.           |
|---------------------------| Issues (2)  [Grammar] ...        |
| 24 Oct 13:40  [Translate] | Token warnings: none             |
| ...                       | [ Copy ] [ Open in Translate ]   |
|                           | [ Delete ]                       |
| [ Load more ]             |                                  |
+---------------------------+----------------------------------+
```

States:

| State | Display |
|---|---|
| Idle / loading | Search box + spinner row; list keeps the previous page while reloading |
| Success | List as above, 50 per page, "Load more" at the bottom |
| Empty (no entries) | Centered "No history yet" + one line "Translations you accept or run will appear here." |
| Empty (history disabled) | "History is turned off." + link "Open settings" |
| Empty (no search match) | "No entries match "<query>"." + [ Clear search ] |
| Error | Banner with message + [ Retry ] |

Delete uses ConfirmDialog ("Delete this entry? This cannot be undone.").

### 3.4 Mistakes

```
 Period: [ 7 days | 30 days | All ]
 By category
 Grammar      ██████████████  14
 Article      ██████████       9
 Vocabulary   ██████           6
 ...
 Recurring mistakes
 "a upload" -> "an upload"   ×4      (click: opens History filtered)
 "added" -> "add"            ×3
 Vocabulary
 загрузка -> upload   ×3   last seen 24 Oct
 ретрай   -> retry    ×2   last seen 23 Oct
```

Bars use the category colour tokens. Empty state: "No mistakes recorded yet." and "This list is built from Correct and Improve runs." Loading shows a Spinner; error shows a Banner with [ Retry ].

### 3.5 Settings

Sub-tabs on the left (or a segmented row in narrow windows): **General, Providers, Hotkeys, System, Presets, About**.

```
 [ General | Providers | Hotkeys | System | Presets | About ]
+--------------------------------------------------------------+
| Default mode           [ Translate v ]                       |
| Default provider       [ Ollama v ]                          |
| History                [x] Save history     [ Clear all ]    |
| Close button hides to tray   [x]                             |
+--------------------------------------------------------------+
```

- Settings apply immediately; a Toast "Saved" confirms. Destructive buttons ("Clear all", "Reset built-ins") use ConfirmDialog.
- **System** holds autostart and the "Keyboard layout fix" group (enabled, hotkey, switch input language).
- Error saving: Banner at the top of the section with the backend message.

**About**

```
 Prompt Translator 0.1.0
 [ Check for updates ]
 +----------------------------------------------------------+
 | Update 0.1.1 available                                   |
 | (release notes, scrollable)       [ Install and restart ]|
 +----------------------------------------------------------+
 [ Run setup again ]   [ Open data folder ]
```

States: idle ("Check for updates"), loading (spinner "Checking..."), up to date ("You're on the latest version." as Toast), update available (card above), error (Banner with [ Retry ]).

### 3.6 Presets (Settings > Presets)

```
+---------------------+----------------------------------------+
| General  [built-in] | Name         [ General              ]  |
| Git messages [b-i]  | Id           general (read-only)       |
| My preset           | Description  [ ...                  ]  |
|                     | Default mode [ Translate v ]           |
| [ + New ]           | Rules (one per line)                   |
| [ Reset built-ins ] | +------------------------------------+ |
|                     | | Keep file names unchanged          | |
|                     | +------------------------------------+ |
|                     | [ Save ]  [ Delete ]                   |
+---------------------+----------------------------------------+
```

States: empty list ("No presets. Reset built-ins to restore the defaults."), validation error (inline, `danger` text under the field with the backend message), delete and reset use ConfirmDialog.

### 3.7 Setup wizard

Step indicator at the top, content in the middle, [ Back ] and [ Next ] / [ Skip ] in the footer.

```
 (1 Welcome) - (2 Local model) - (3 Claude Code) - (4 Defaults) - (5 Done)
+--------------------------------------------------------------+
| Local model                                                  |
| Ollama: installed, running                                   |
| Recommended for this PC: qwen2.5:7b   [ Other models v ]     |
| [ Download ]   ████████░░░░ 62%   [ Cancel ]                 |
| [ Run benchmark ]   Result: 2.4 s                            |
+--------------------------------------------------------------+
                                        [ Skip ]  [ Next ]
```

Done step: quick tour of the hotkey flow; if no provider works it shows a `warning` Banner and Done stays enabled.

### 3.8 Popup

Frameless, always on top, about 520x360, centered on the monitor under the cursor.

```
+--------------------------------------------------------+
| Source: "сделай ретрай для upload функции..."     [x]  |
| Mode: ( Translate ) ( Improve ) ( Correct )            |
|--------------------------------------------------------|
| Add a retry to the upload function.                    |
|                                                        |
| 2 issues  [v]                                          |
|   [Grammar] "..." -> "..."  Explanation                |
|--------------------------------------------------------|
| Enter: send to api    Ctrl+R retry   Open in main window|
+--------------------------------------------------------+
```

| State | Display |
|---|---|
| Loading (auto-run on open) | Spinner + "Translating..." in the result area, mode chips still clickable (re-run) |
| Success | Result text, issues count (expandable list), footer shows target |
| Error | Message + hint in the result area, footer shows `Ctrl+R retry`; Enter does nothing |
| Bound session ended | Error shown in the result area, popup stays open; footer "Session ended. Esc to close" |
| No selection / empty source | "Nothing selected." + `Esc` to close |

Footer target text: "Enter: send to <session>" or "Enter: copy".

## 4. States matrix

| State | Pattern |
|---|---|
| Idle | Muted placeholder text; primary action enabled only when it can succeed |
| Loading | Spinner plus a verb ("Translating with <model>..."), inputs read-only, Cancel visible; no layout jump (skeleton keeps the height) |
| Success | Result, metadata (duration, model), primary action |
| Error | `Banner` (danger): human message, one-line hint, actions [ Retry ] and, for config problems, [ Open settings ]. Messages follow plan §4.6 |
| Empty | Centered title + one explanatory line + (optional) a link/button to fix it |

## 5. Keyboard map

| Key | Where | Action |
|---|---|---|
| Ctrl+Enter | Translate | Run |
| Esc | Translate while loading | Cancel |
| Ctrl+Shift+Enter | Translate | Accept (copy or send) |
| Ctrl+1 / 2 / 3 | Translate | Mode Translate / Improve / Correct |
| Ctrl+Tab / Ctrl+Shift+Tab | Main window | Next / previous top tab |
| Ctrl+, | Main window | Open Settings |
| / or Ctrl+F | History | Focus search |
| Up / Down | History list | Move selection |
| Delete | History list | Delete selected (opens ConfirmDialog) |
| Enter | Popup | Accept (send or copy) and close |
| Esc | Popup | Close |
| Ctrl+R | Popup | Retry |
| 1 / 2 / 3 | Popup (focus not in a text field) | Switch mode and re-run |
| Esc | ConfirmDialog | Cancel |
| Enter | ConfirmDialog | Confirm (focus starts on the safe button for destructive actions) |

Global hotkeys (selection translate, layout fix) are configured in Settings > Hotkeys. Tab order follows reading order; every control has a visible focus ring (`ring-2 ring-accent`).

## 6. Component inventory

All components live in `src/components/`. Variants listed are the full set for v1.

| Component | Variants / props | Notes |
|---|---|---|
| **Button** | `primary` (accent), `secondary` (surface + border), `danger`, `ghost`; sizes `sm`, `md`; `loading`, `disabled` | Height 32 px (`md`) / 28 px (`sm`). Shows the shortcut as a muted suffix when given. |
| **Select** | native-styled `<select>` with label | Same height as Button. Keeps the OS dropdown for accessibility. |
| **SegmentedControl** | options, value, onChange | Used for mode, period filter. Arrow keys move, selected option has `surface` raised style plus accent text. |
| **TextArea** | `readOnly`, `mono`, auto-grow with max height | Input and output both use the sans font (`mono` only when a caller asks for it). Min 4 rows. |
| **Banner** | `info`, `success`, `warning`, `danger`; optional actions | Left coloured border, tinted background, icon plus text. `role="status"` (info/success) or `role="alert"` (warning/danger). |
| **IssueItem** | `issue` | `CategoryChip`, then `original -> corrected` (original struck through in `danger`, corrected in `success`), explanation below in `muted`. |
| **CategoryChip** | `category` | Small rounded label, tinted background and text from the category token (section 7). Label is the human name ("Word choice"). |
| **DiffView** | `original`, `corrected` | Word-level. Removed: `danger` tint plus strikethrough. Added: `success` tint plus underline. Unchanged: normal text. Wraps, selectable. |
| **Spinner** | `sm`, `md` | 16 / 24 px ring in `accent`; respects `prefers-reduced-motion` (static dots). |
| **ConfirmDialog** | `title`, `message`, `confirmLabel`, `destructive` | Centered modal over a scrim, focus trap, Esc cancels. Destructive confirm uses the `danger` Button. |
| **Toast** | `info`, `success`, `error` | Bottom-center, auto-dismiss after 3 s (errors 6 s), max 3 stacked, `role="status"`. |

## 7. Colour per issue category

The 12 categories come from the `IssueCategory` enum (plan §4.3; `idiom` and `phrasing` are used only by Improve). Colours are distinct hues so bars and chips can be told apart; each token has a light and a dark value (section 9).

| Category | Label | Hue |
|---|---|---|
| `spelling` | Spelling | Red |
| `grammar` | Grammar | Orange |
| `article` | Article | Amber |
| `tense` | Tense | Lime/green |
| `preposition` | Preposition | Teal |
| `word_choice` | Word choice | Cyan/blue |
| `vocabulary` | Vocabulary | Violet |
| `punctuation` | Punctuation | Pink |
| `style` | Style | Indigo |
| `idiom` | Idiom | Fuchsia |
| `phrasing` | Phrasing | Green |
| `other` | Other | Gray |

Unknown categories fall back to `other`. Chips use the category colour as text and a 15% tint as background (`color-mix`), which keeps contrast in both themes.

## 8. Spacing, type and shape

**Spacing** (Tailwind default 4 px grid): `1` 4, `2` 8, `3` 12, `4` 16, `6` 24, `8` 32. Use `gap-2` inside controls, `gap-3` between related items, `gap-4` between sections, `p-4` for screens, `p-3` for cards.

**Type scale** (font stack `system-ui, "Segoe UI Variable", "Segoe UI", sans-serif`; mono `ui-monospace, "Cascadia Mono", Consolas, monospace`):

| Role | Size / line | Weight |
|---|---|---|
| Screen title | 18 / 24 (`text-lg`) | 600 |
| Section heading | 14 / 20 (`text-sm`) | 600 |
| Body, inputs, output | 14 / 20 (`text-sm`) | 400 |
| Secondary, metadata, chips | 12 / 16 (`text-xs`) | 400/500 |
| Popup result text | 15 / 22 | 400 |

**Shape:** radius `md` (6 px) for controls, `lg` (8 px) for cards and dialogs, `full` for chips and dots. Borders 1 px `border` token. Shadows only on Toast, popup and dialogs (`shadow-lg`). Motion: 120-150 ms ease-out for hover/focus, none for layout; disabled when `prefers-reduced-motion`.

## 9. Theme tokens (ready to paste into `src/index.css`)

Tailwind CSS v4: tokens are CSS variables on `:root`, switched by `prefers-color-scheme`, and exposed as utilities (`bg-bg`, `bg-surface`, `text-text`, `text-muted`, `bg-accent`, `text-cat-grammar`, ...) through `@theme inline`.

```css
@import "tailwindcss";

:root {
  color-scheme: light dark;

  /* Base */
  --bg: #f6f7f9;
  --surface: #ffffff;
  --surface-raised: #eef0f4;
  --border: #d9dde4;
  --text: #1b1f27;
  --muted: #5d6675;

  /* Semantic */
  --accent: #2f6fed;
  --accent-contrast: #ffffff;
  --danger: #c62828;
  --warning: #b26a00;
  --success: #1b7f3b;

  /* Issue categories */
  --cat-spelling: #c62828;
  --cat-grammar: #c25100;
  --cat-article: #9a6b00;
  --cat-tense: #4d7c0f;
  --cat-preposition: #0f766e;
  --cat-word-choice: #0369a1;
  --cat-vocabulary: #7c3aed;
  --cat-punctuation: #be185d;
  --cat-style: #4338ca;
  --cat-idiom: #a21caf;
  --cat-phrasing: #15803d;
  --cat-other: #5d6675;
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #14171c;
    --surface: #1d2128;
    --surface-raised: #272c35;
    --border: #343a45;
    --text: #e8eaef;
    --muted: #9aa3b2;

    --accent: #6c9bff;
    --accent-contrast: #0d1117;
    --danger: #ff7b72;
    --warning: #f0b429;
    --success: #4cc27a;

    --cat-spelling: #ff7b72;
    --cat-grammar: #ff9f5a;
    --cat-article: #f0b429;
    --cat-tense: #a3d95b;
    --cat-preposition: #2dd4bf;
    --cat-word-choice: #4fb3ff;
    --cat-vocabulary: #b794f6;
    --cat-punctuation: #f472b6;
    --cat-style: #8b9cff;
    --cat-idiom: #e879f9;
    --cat-phrasing: #4ade80;
    --cat-other: #9aa3b2;
  }
}

@theme inline {
  --color-bg: var(--bg);
  --color-surface: var(--surface);
  --color-surface-raised: var(--surface-raised);
  --color-border: var(--border);
  --color-text: var(--text);
  --color-muted: var(--muted);
  --color-accent: var(--accent);
  --color-accent-contrast: var(--accent-contrast);
  --color-danger: var(--danger);
  --color-warning: var(--warning);
  --color-success: var(--success);

  --color-cat-spelling: var(--cat-spelling);
  --color-cat-grammar: var(--cat-grammar);
  --color-cat-article: var(--cat-article);
  --color-cat-tense: var(--cat-tense);
  --color-cat-preposition: var(--cat-preposition);
  --color-cat-word-choice: var(--cat-word-choice);
  --color-cat-vocabulary: var(--cat-vocabulary);
  --color-cat-punctuation: var(--cat-punctuation);
  --color-cat-style: var(--cat-style);
  --color-cat-idiom: var(--cat-idiom);
  --color-cat-phrasing: var(--cat-phrasing);
  --color-cat-other: var(--cat-other);

  --font-sans: system-ui, "Segoe UI Variable", "Segoe UI", sans-serif;
  --font-mono: ui-monospace, "Cascadia Mono", Consolas, monospace;
}

html, body, #root {
  height: 100%;
}

body {
  background: var(--bg);
  color: var(--text);
  font-family: var(--font-sans);
  font-size: 14px;
  line-height: 20px;
}
```

Usage notes:

- Tinted backgrounds (chips, banners, diff): `bg-[color-mix(in_srgb,var(--cat-grammar)_15%,transparent)]`, or a small helper that takes the token name.
- Light values for text-on-surface were chosen to pass roughly 4.5:1 on `--surface`; re-check contrast in the app after T19 and adjust the token, not the components.
- Do not hard-code hex values in components; always use the token utilities.
