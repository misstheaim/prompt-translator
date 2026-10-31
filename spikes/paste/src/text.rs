//! The text to deliver, read from a UTF-8 file.

use crate::Exit;

/// The file's text with a BOM dropped, line breaks normalised to `\n` and **trailing line breaks
/// removed**: a paste that ends with a line break could act as Enter in a terminal without
/// bracketed paste and send the prompt.
pub fn read(path: &str) -> Result<String, Exit> {
    let bytes = std::fs::read(path).map_err(|e| {
        eprintln!("error: can't read `{path}`: {e}");
        Exit::TextFile
    })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        eprintln!("error: `{path}` is not UTF-8");
        Exit::TextFile
    })?;
    let text = text
        .strip_prefix('\u{feff}')
        .unwrap_or(&text)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let text = text.trim_end_matches('\n').to_string();
    if text.trim().is_empty() {
        eprintln!("error: `{path}` holds no text");
        return Err(Exit::TextFile);
    }
    describe(&text);
    Ok(text)
}

/// Prints the facts the matrix records: length, lines, and a leading `!` `/` `#` (D35).
fn describe(text: &str) {
    let chars = text.chars().count();
    let lines = text.lines().count();
    println!("text: {chars} chars, {lines} line(s)");
    if let Some(first) = text.trim_start().chars().next()
        && matches!(first, '!' | '/' | '#')
    {
        println!(
            "note: text starts with `{first}` (D35: the app guards this; the spike pastes it as is)"
        );
    }
}

/// `\n` to `\r\n`: the Windows clipboard convention, what a copy from Notepad puts there.
pub fn to_crlf(text: &str) -> String {
    text.replace('\n', "\r\n")
}
