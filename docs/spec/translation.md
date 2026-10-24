# Translation helpers

The text checks that surround a translation: the technical-token check (does the output keep the identifiers, paths and links of the source) and mode detection for the popup (Translate or Correct from the text). The translation itself, the providers and the popup are not implemented yet.

> Code: `src-tauri/src/translation/token_check.rs`, `src-tauri/src/translation/mode_detect.rs`, `src-tauri/src/translation/commands.rs`

## Behaviour

### TRANS-001 Technical tokens
`check_tokens(source, output)` finds these tokens in the source, in this order of kinds:
- `codeSpan`: text in single backticks on one line, backticks included;
- `url`: `http://` or `https://` up to whitespace, `<`, `>` or a quote, with trailing `. , ; : ! ? ) ]` removed;
- `path`: a Windows path (`C:\dir\file`, same trailing trim), or a Unix-style path of 2+ segments: kept when it starts with `/`, `~` or `.`, or has 3+ segments, or its last segment has an extension; paths without a letter (dates like `12/05/2024`) are not tokens;
- `fileName`: a name ending in one of a fixed list of extensions (json, rs, ts, tsx, js, py, md, toml, yaml, cs, csproj, sql, sh, log, exe, png, pdf, csv, and others; case-insensitive), e.g. `config.json`;
- `ticketId`: `PROJ-123` (capital letter, then capitals or digits, hyphen, digits) and `#42`;
- `identifier`: snake_case (`user_id`) and camelCase or PascalCase with an inner capital (`getUserName`, `HttpClient`); all-capital words and their plurals (`API`, `APIs`) are not tokens.
A match is blanked out before later kinds are tried, so a file name inside a URL is not reported twice. Plain words, misspellings, non-English words and bare numbers are never tokens (`number` exists as a kind but is never produced).
Since: #42

### TRANS-002 Missing-token warnings
`check_tokens` returns a warning (`token`, `kind`) for each token that does not appear verbatim (case-sensitive substring) in the output, without duplicates, in order of the kinds above. An empty list means the output kept every token. No command or screen calls it yet.
Since: #42

### TRANS-003 Mode detection
`detect_mode(text, last)` returns `translate` or `correct` (never `improve`), or `last` when the text is too short:
- technical tokens (TRANS-001) are removed first and the rest is split into words at every non-letter (so `don't` counts as two words);
- fewer than 3 words: `last` is returned;
- share of words starting with a Latin letter (Basic Latin to Latin Extended-B): 0.6 or more gives `correct`, 0.4 or less gives `translate`;
- in between, the text is run through language detection: English gives `correct`, anything else (or no result) gives `translate`.
Text in other Latin-script languages (German, Spanish) therefore resolves to `correct`.
Since: #60

### TRANS-004 Detect-mode command
The `detect_mode(text, last)` command exposes TRANS-003 to the webview as `detectMode(text, last)` in `api.ts`. Nothing calls it yet.
Since: #60

## Known issues

None.

Retired IDs: none
