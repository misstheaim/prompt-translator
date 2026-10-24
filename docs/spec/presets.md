# Presets

A preset is a named set of domain rules plus a default mode, kept as one JSON file per preset. Four built-in presets are seeded on first run. The core can list, save, delete and reset them; no screen or `api.ts` wrapper uses them yet.

> Code: `src-tauri/src/presets/`, `src-tauri/resources/presets/`, `src/bindings/Preset.ts`

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

## Known issues

None.

Retired IDs: none
