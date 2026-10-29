# Presets

A preset is a named set of domain rules plus a default mode, kept as one JSON file per preset. Four built-in presets are seeded on first run. The core can list, save, delete and reset them, and the Presets sub-tab of Settings edits them (PRESETS-009).

> Code: `src-tauri/src/presets/`, `src-tauri/resources/presets/`, `src/bindings/Preset.ts`, `src/screens/Settings/PresetsSettings.tsx`

## Behaviour

### PRESETS-001 Preset shape
A preset has `id`, `name`, `description` (default empty), `defaultMode` (`translate`, `improve` or `correct`), `rules` (list of strings, default empty) and `builtIn` (default false). It is stored as pretty-printed JSON in `presets/<id>.json` in the presets folder (see APP-006).
Since: #41

### PRESETS-002 Built-in presets
Four presets are bundled with the app (`builtIn` true):
- `general` "General", default mode `translate`;
- `technical-development` "Technical Development", `improve`;
- `code-review` "Code Review", `improve`;
- `work-communication` "Work Communication", `correct`.
Each has 5-6 rules. `builtIn` stays true after the user edits one, and saving any preset sets `builtIn` from its id alone (true only for these four ids), whatever the caller sent.
Since: #41

### PRESETS-003 Seeding on first run
At startup, if the presets folder has no `*.json` file (first run, or all deleted by hand), the folder is created and the four built-ins are written into it. If the folder already has any `.json` file, nothing is written. A failure to seed is logged and the app still starts.
Since: #41

### PRESETS-004 Listing
`list_presets` reads the folder on every call (so edits made outside the app show up) and returns valid presets sorted by name, case-insensitive. A file is skipped with a logged warning when it cannot be read, is not valid JSON for a preset, fails the validation of PRESETS-005, or has an `id` different from its file name (without `.json`).
Since: #41

### PRESETS-005 Validation
`save_preset` refuses with `InvalidInput` (nothing written) when:
- the id is empty or has anything but lowercase ASCII letters, digits and hyphens: "The preset id may only contain lowercase letters, digits and hyphens";
- the name is empty or whitespace: "The preset name must not be empty";
- there are more than 30 rules: "A preset can have at most 30 rules";
- a rule is longer than 500 characters: "A rule can have at most 500 characters".
Since: #41

### PRESETS-006 Saving
`save_preset(preset)` validates, then writes `<id>.json` through a temporary file renamed over the target, creating the preset or replacing the one with the same id, and returns the full sorted list. Writes are applied one at a time.
Since: #41

### PRESETS-007 Deleting
`delete_preset(id)` removes the file and returns the full list. Built-ins can be deleted like any other. It is refused with `InvalidInput` "This preset does not exist" for an unknown or invalid id, and "You can't delete the last preset" when only one valid preset is left. Settings are not checked: `defaultPresetId` may point to a deleted preset.
Since: #41

### PRESETS-008 Resetting
`reset_presets` rewrites the four built-ins from the bundled versions (replacing any edited or missing built-in) and keeps every other preset, then returns the full list.
Since: #41

### PRESETS-009 Preset editor
Settings > Presets shows the presets in a list (name, with a "Built-in" badge) and an editor on the side; "Select a preset to edit, or create a new one." shows until one is opened.
- Fields: name, id (read-only), description, default mode and rules (one rule per line; blank lines dropped, lines trimmed; hint "At most 30 rules, 500 characters each" with a rule count).
- "New preset" opens an empty editor (mode Translate). The id is made from the name: lowercase, every run of other characters becomes `-`, ends trimmed, "preset" if nothing is left; if a preset already uses it, `-2`, `-3`... is appended. After the first save the id cannot change.
- Save is disabled while the name is blank (error "Enter a name") and, for an existing preset, without changes. On success the toast "Preset saved" shows and the editor reloads the saved preset. Revert restores the last saved values. A backend validation error (PRESETS-005) appears as an error banner in the editor and keeps the draft.
- Delete (existing presets only) asks "Delete preset?" and then shows "Preset deleted" and closes the editor; errors such as "You can't delete the last preset" show as a banner. "Reset built-ins" asks "Reset built-in presets?" (presets the user created are kept), then shows "Built-in presets restored" and reloads the open preset. Every change refreshes the preset list in the app, including the Translate screen's preset select.
Since: #56

## Known issues

None.

Retired IDs: none
