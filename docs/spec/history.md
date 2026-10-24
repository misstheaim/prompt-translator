# History

The SQLite database that will hold every translation with its issues, and the "my mistakes" aggregates over it. Only the database and its repository exist: no command exposes them, nothing writes to it and no screen shows it yet.

> Code: `src-tauri/src/history/`, `src/bindings/HistoryEntry.ts` and related

## Behaviour

### HISTORY-001 Database opening
At startup the app opens `history.db` (see APP-006), creating the file if needed, turns on foreign keys and WAL journal mode, and applies pending migrations. If this fails the error is logged ("History database unavailable: ...") and the app starts anyway, without a history database.
Since: #62

### HISTORY-002 Schema
Migration 1 creates `translations` (id, `created_at`, mode, provider, model, preset id, source text, output text, duration in ms, token warnings as JSON text defaulting to `[]`, and nullable accepted target, session id and session name) and `issues` (id, translation id, category, original, corrected, explanation), with indexes on `created_at`, issue category and issue translation id. Deleting a translation deletes its issues.
Since: #62

### HISTORY-003 Stored values
`created_at` is set by the database as ISO 8601 UTC with millisecond precision (e.g. `2026-10-24T09:30:00.123Z`). Mode, provider and issue category are stored as their wire strings (`translate`, `claudeCode`, `word_choice`). A result and its issues are inserted in one transaction. The accepted target is `clipboard` or `session` (with session id and name); marking an unknown id is not an error.
Since: #62

### HISTORY-004 Listing and search
A list is newest first (ties by higher id), paged by limit and offset, with each row carrying the first 120 characters of source and output, the issue count and the accepted target. An optional text query (trimmed; empty means none) keeps entries whose source or output contains it; `%`, `_` and `\` in the query match literally. An optional issue filter keeps entries having an issue with that exact original and corrected pair. Matching ignores case for any script, including Cyrillic.
Since: #62

### HISTORY-005 Single entry, delete, clear
Reading an entry returns it with its issues in stored order, or nothing for an unknown id. Deleting one entry and clearing all entries are available; deleting an unknown id is not an error.
Since: #62

### HISTORY-006 Mistakes summary
Over the entries of the last N days (all entries when no limit is given), the summary has:
- issue counts by category, most frequent first (ties by category name);
- recurring mistakes: original/corrected pairs (case-insensitive) that occur at least twice, most frequent first then most recent, at most 50, shown with the spelling and category of the latest occurrence;
- vocabulary: pairs from `vocabulary` issues, most frequent first then most recent, at most 100, with count and the time of the latest entry containing them.
Since: #62

## Known issues

None.

Retired IDs: none
