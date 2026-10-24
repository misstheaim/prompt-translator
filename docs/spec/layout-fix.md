# Layout fix

Converting text typed with the wrong keyboard layout (US QWERTY vs Russian ЙЦУКЕН). Only the pure conversion exists; no hotkey, command or UI uses it yet, although the settings hold a `layoutFix` block (see SETTINGS-001).

> Code: `src-tauri/src/layout_fix/convert.rs`

## Behaviour

### LAYOUT-001 Conversion
Text is converted character by character through a US/Russian key table, in one of two directions: US to Russian (`ghbdtn` becomes `привет`) or Russian to US (`руддщ` becomes `hello`). Case is kept (`Q` gives `Й`). Punctuation follows the physical key: `.` gives `ю`, `/` gives `.`, `?` gives `,`, `,` gives `б`, `;` gives `ж`, `'` gives `э`, `` ` `` gives `ё`, `[` gives `х`, `]` gives `ъ`, and on the shifted number row `@ # $ ^ & |` give `" № ; : ? /`. Characters identical on both layouts (digits, space, newlines, `! % * ( ) - _ = + \`) and any character not in the table pass through unchanged.
Since: #77

### LAYOUT-002 Direction detection
The direction is guessed by counting ASCII letters against Cyrillic letters (U+0400-U+04FF) and ignoring all other characters: more Latin means US to Russian, more Cyrillic means Russian to US, and a tie (including no letters) gives no direction.
Since: #77

## Known issues

None.

Retired IDs: none
