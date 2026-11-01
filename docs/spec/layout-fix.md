# Layout fix

Converting text typed with the wrong keyboard layout (US QWERTY vs Russian ЙЦУКЕН). A global hotkey replaces the selected text, or the word before the cursor, in the foreground app with the same keys typed on the other layout, and switches the input language. It uses no model, no popup and no history. The settings block `layoutFix` (SETTINGS-001) holds its options; the Settings screen edits it in the Keyboard layout fix section (SETTINGS-008).

> Code: `src-tauri/src/layout_fix/convert.rs`, `src-tauri/src/layout_fix/hotkey.rs`, `src-tauri/src/system/hotkey.rs`, `src-tauri/src/system/input.rs`

## Behaviour

### LAYOUT-001 Conversion
Text is converted character by character through a US/Russian key table, in one of two directions: US to Russian (`ghbdtn` becomes `привет`) or Russian to US (`руддщ` becomes `hello`). Case is kept (`Q` gives `Й`). Punctuation follows the physical key: `.` gives `ю`, `/` gives `.`, `?` gives `,`, `,` gives `б`, `;` gives `ж`, `'` gives `э`, `` ` `` gives `ё`, `[` gives `х`, `]` gives `ъ`, and on the shifted number row `@ # $ ^ & |` give `" № ; : ? /`. Characters identical on both layouts (digits, space, newlines, `! % * ( ) - _ = + \`) and any character not in the table pass through unchanged.
Since: #77

### LAYOUT-002 Direction detection
The direction is guessed by counting ASCII letters against Cyrillic letters (U+0400-U+04FF) and ignoring all other characters: more Latin means US to Russian, more Cyrillic means Russian to US, and a tie (including no letters) gives no direction.
Since: #77

### LAYOUT-003 Hotkey
The layout-fix hotkey is `layoutFix.hotkey` (default ``Ctrl+Shift+` ``; settings files saved earlier keep their stored hotkey). It is registered at startup and after every settings change, only while `layoutFix.enabled` is on (off unregisters it); a failure to register is reported with the popup hotkey's error (POPUP-002, "The layout fix hotkey is off. Choose another combination in Settings."). It runs on key press. A press while an earlier run is still going is ignored. Saving settings is refused when the layout fix is enabled and its hotkey is the same combination as the popup hotkey, in any modifier order or case (SETTINGS-004).
Since: #78 · Changed: #79, #146

### LAYOUT-004 Replacing the selection
When the foreground window is not a terminal (POPUP-007) and `autoCopySelection` is on:
1. If Alt or Win is still held from the hotkey, a dummy key is tapped so that releasing it does not open the app's menu bar or the Start menu. It waits up to 500 ms for all modifier keys to be released; if one is still down, nothing happens.
2. The clipboard text is saved and Ctrl+C is sent (it waits up to 300 ms for the clipboard to change). If the window lost focus or a modifier was pressed meanwhile, or Windows blocks the input, nothing is changed.
3. The copied text is converted in its detected direction (LAYOUT-002), put on the clipboard (kept out of Windows clipboard history) and pasted with Ctrl+V; 200 ms later the saved clipboard text is put back (the clipboard is emptied if it held no text), but only if the clipboard changed since it was saved.
4. If nothing converts (no direction, or the text is unchanged by the conversion) nothing is pasted and no message is shown.
Clipboard use is exclusive with the popup's auto-copy (POPUP-007): only one runs at a time.
Since: #78

### LAYOUT-005 No selection: wrong-layout words before the cursor
When Ctrl+C copies nothing (the clipboard did not change, the copy was empty, or the app is VS Code or another Monaco editor that copied the whole current line because nothing was selected), Ctrl+Shift+Left is sent to select the word before the cursor and it is copied again. The word before the cursor is always converted. The selection then grows to the left, one word per Ctrl+Shift+Left press, while the words look wrong-layout (LAYOUT-008) in the same direction. Undecided words (LAYOUT-008), and a single right-layout word that is only weakly right (its letter pairs average less than 1.5 bits per pair more likely as written than converted), join only if a wrong-layout word lies further left, so one misjudged word does not cut a sentence. It stops at a word in the other script (Latin before a Cyrillic tail or the other way round), at a clearly right-layout word, at two right-layout words in a row, at a line break, at the start of the line or field (the selection stops growing) and when the selected text holds 60 whitespace-separated words. Words are counted in the selected text, not by key presses (an editor that stops at `,` or `.` inside a word needs several presses for it); as a guard against an editor that misbehaves, at most 180 Ctrl+Shift+Left presses are made. Then the selection shrinks back with Ctrl+Shift+Right, copying after each press, to the first converted word; whatever cannot be dropped that way is pasted back unchanged. The words to convert are replaced as in LAYOUT-004.
If the first word copies nothing, nothing changes and the saved clipboard is restored. If a later copy fails or the selection changes unexpectedly while growing or shrinking, the run ends without pasting and the text stays selected (the saved clipboard is restored).
Since: #78 · Changed: #147, #188, #199

### LAYOUT-008 Wrong-layout word test
A word is judged by comparing how likely its letter pairs (word start and end included, averaged per pair) are in its own language with those of its conversion (LAYOUT-001) in the other language, English and Russian: the word is wrong-layout when the conversion is the likelier one and right-layout when the word is. A word with fewer than 3 letters, with as many Latin as Cyrillic letters (including none), or whose two averages differ by at most a quarter of a bit per pair (`ршь`, "him", is such a near-tie), is undecided. Brackets, quotes and punctuation that are letters on the other layout may open or close a word without counting against it (`[link`, `word,`); an apostrophe inside a word only splits it.
Since: #147 · Changed: #199

### LAYOUT-006 Terminals and auto-copy off
When the foreground window is a terminal, there is no foreground window, or `autoCopySelection` is off, no keys are sent. The current clipboard text is converted and written back to the clipboard (not hidden from clipboard history) and a Windows notification titled "Prompt Translator" says "Layout fixed: paste with Ctrl+V"; the user pastes it. If the clipboard holds no text, no direction, or text the conversion does not change, the notification says "Copy some text first" and the clipboard is left as it is.
Since: #78

### LAYOUT-007 Input language switch
After a replacement (LAYOUT-004/005), when `layoutFix.switchInputLanguage` is on (default), the foreground window's input language is switched to the layout the text was converted to: Russian after US to Russian, English after Russian to US. Nothing happens if the window already uses that language or no keyboard layout of that language is installed. There is no switch after the terminal path (LAYOUT-006) or when nothing was replaced.
Since: #78

## Known issues

None.

Retired IDs: none
