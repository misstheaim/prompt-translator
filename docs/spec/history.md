# History

The SQLite database that holds every translation with its issues, and the "my mistakes" aggregates over it. Each successful translation is saved when history is on, and commands list, read, delete and summarise entries. The History tab lists and shows the entries (HISTORY-010) and the Mistakes tab shows the summary (HISTORY-011).

> Code: `src-tauri/src/history/`, `src/bindings/HistoryEntry.ts` and related, `src/screens/History/`, `src/screens/Mistakes/`

## Behaviour

### HISTORY-001 Database opening
At startup the app opens `history.db` (see APP-006), creating the file if needed, turns on foreign keys and WAL journal mode, and applies pending migrations. If this fails the error is logged ("History database unavailable: ...") and the app starts anyway, without a history database: nothing is saved (HISTORY-007) and the history commands fail (HISTORY-008).
Since: #62 · Changed: #63

### HISTORY-002 Schema
Migration 1 creates `translations` (id, `created_at`, mode, provider, model, preset id, source text, output text, duration in ms, token warnings as JSON text defaulting to `[]`, and nullable accepted target, session id and session name) and `issues` (id, translation id, category, original, corrected, explanation), with indexes on `created_at`, issue category and issue translation id. Deleting a translation deletes its issues. Migration 2 adds the nullable columns `source_language` and `target_language` (ISO 639-1 codes of the language pair the request used); entries saved before it read as `ru` and `en`.
Since: #62 · Changed: #150

### HISTORY-003 Stored values
`created_at` is set by the database as ISO 8601 UTC with millisecond precision (e.g. `2026-10-24T09:30:00.123Z`). Mode, provider and issue category are stored as their wire strings (`translate`, `claudeCode`, `word_choice`). A result and its issues are inserted in one transaction. The accepted target is `clipboard` or `session` (with session id and name); marking an unknown id is not an error. Each new entry stores its language pair, and `sourceLanguage` and `targetLanguage` (codes) are part of both the list rows and the full entry.
Since: #62 · Changed: #150

### HISTORY-004 Listing and search
A list is newest first (ties by higher id), paged by limit and offset, with each row carrying the first 120 characters of source and output, the issue count and the accepted target. An optional text query (trimmed; empty means none) keeps entries whose source or output contains it; `%`, `_` and `\` in the query match literally. An optional issue filter keeps entries having an issue with that exact original and corrected pair. Optional filters `mode`, `category` (entries with at least one issue of that category) and `sinceDays` (entries of the last N days) narrow further; all given filters, the text query and the pair filter must match together. Matching ignores case for any script, including Cyrillic.
Since: #62 · Changed: #143

### HISTORY-005 Single entry, delete, clear
Reading an entry returns it with its issues in stored order, or nothing for an unknown id. Deleting one entry and clearing all entries are available; deleting an unknown id is not an error.
Since: #62

### HISTORY-006 Mistakes summary
Over the entries of the last N days (all entries when no limit is given), the summary has the three parts below. Every count is per distinct source text: an issue pair (case-insensitive) found in several entries with the same source text (trimmed, whitespace collapsed, case-insensitive) counts once.
- issue counts by category, most frequent first (ties by category name);
- recurring mistakes: original/corrected pairs (case-insensitive) that occur for at least two distinct source texts, most frequent first then most recent, at most 50, shown with the spelling and category of the latest occurrence;
- vocabulary: pairs from `vocabulary` issues, most frequent first then most recent, at most 100, with count and the time of the latest entry containing them.
Since: #62 · Changed: #143

### HISTORY-007 Saving translations
Every successful `translate` result, from any provider including the mock, is saved with its issues and token warnings before it is returned, when `historyEnabled` is on and the database is open; the result's `historyId` is the new entry's id. When `historyEnabled` is off, or the database is unavailable, or saving fails, nothing is saved, `historyId` is null and the translation still succeeds (a failure is only logged). Failed or cancelled requests are never saved. Turning history off does not delete existing entries.
Since: #63

### HISTORY-008 History commands
- `history_list(query, issue, mode, category, sinceDays, limit, offset)` (wrapper `historyList`): HISTORY-004; `limit` is clamped to 1-500.
- `history_get(id)` (`historyGet`): the entry with its issues; an unknown id fails with `InvalidInput` "This history entry doesn't exist".
- `history_delete(id)` (`historyDelete`) and `history_clear()` (`historyClear`): HISTORY-005; only `history_clear` removes everything.
- `mistakes_summary(sinceDays)` (`mistakesSummary`): HISTORY-006; null means all entries.
When the database could not be opened at startup, each of them fails with `Db` "The history database is unavailable".
Since: #63 · Changed: #143

### HISTORY-009 Accepted marking
Copying a result with its `historyId` (TRANS-010) sets that entry's accepted target to `clipboard`, so the list and the entry show it as accepted. Delivering a result to a bound terminal session with its `historyId` (SESSION-005) sets the accepted target to `session`, with the id `terminal:0x<window handle>` and the session's title as its name; this also happens when the delivery fell back to the clipboard (SESSION-006). An id that no longer exists changes nothing.
Since: #63 · Changed: #74

### HISTORY-010 History screen
The History tab has a search box ("Search source and output"; the list updates 300 ms after typing stops, trimmed text, HISTORY-004), a mode filter (All / Translate / Improve / Correct), the entries newest first in pages of 50 with a "Load more" button while a page came back full, and a detail panel.
- Each row shows the mode badge, the provider, the date and time, and the source and output previews on one line each.
- Empty states: "No history yet", or "No matching entries" while a text search, the mode filter or a mistake filter is active. A list error shows an error banner with Retry.
- When `historyEnabled` is off an info banner says "History is turned off, so new translations aren't saved. Existing entries stay here." with an "Open settings" button.
- Clicking a row loads the entry into the panel: mode, provider and model, date, full source and output, the issues (the shared issue list, APP-031), and, when there are token warnings, a banner "Missing from the output: <tokens>". "Select an entry to see it in full." shows until then.
- Panel buttons: "Copy" copies the output and marks the entry accepted (TRANS-010, toast "Copied to clipboard"); "Open in Translate" fills the Translate screen with the entry's source, mode, provider and result and switches to it (TRANS-013); "Delete" asks "Delete this entry?" and then removes it from the list and shows "Entry deleted".
- A filter set from the Mistakes tab (HISTORY-011) keeps only entries having that mistake pair or an issue of that category, within the period, and shows "Entries with the mistake <original> → <corrected>" or "Entries with <category> mistakes", plus "· last <n> days" when a period is set, with a "Clear filter" button. It stays set, also after leaving the tab, until cleared.Since: #64 · Changed: #65, #54, #143

### HISTORY-011 Mistakes screen
The Mistakes tab has a period switch, 7 days, 30 days (selected at first) or All time, and loads the summary (HISTORY-006) for it again on every switch; a failure shows an error banner with Retry. Sections:
- **By category**: one bar per category, the longest scaled to the most frequent, with the count and the category's colour. Clicking a bar sets the History filter to that category and opens the History tab.
- **Recurring mistakes**: each pair as `original → corrected`, its category and "×<count>"; "Nothing has repeated yet." when empty. Clicking a pair sets the History filter to it and opens the History tab.
- **Vocabulary**: `original → corrected`, "last seen <date>" and "×<count>"; "No new words yet." when empty. Clicking an item sets the History filter to that pair.
Every such filter carries the selected period (7 or 30 days, or none for All time).
When all three are empty the screen shows "No mistakes recorded for this period. They appear here after Correct and Improve runs, which explain what you wrote wrong."
Since: #65 · Changed: #143

## Known issues

None.

Retired IDs: none
