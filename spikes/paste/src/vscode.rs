//! `vscode`: the VS Code route. Builds
//! `vscode://anthropic.claude-code/open?session=<id>&prompt=<text>` (extension 2.1.314's URI
//! handler: opens the session and fills its input with `setInputText`, not sent) and hands it to
//! the shell like `Start-Process` does.

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

use crate::{Exit, Flags, text, usage};

const BASE: &str = "vscode://anthropic.claude-code/open";

pub fn run(args: &[String]) -> Result<(), Exit> {
    let flags = Flags::parse(args, &["session", "text"], &["print-only"]).map_err(usage)?;
    let text_path = flags
        .value("text")
        .ok_or_else(|| usage("`--text` is required"))?;
    let session = flags.value("session");
    if let Some(id) = session
        && !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        eprintln!("warning: `{id}` doesn't look like a session id (UUID)");
    }
    let text = text::read(text_path)?;
    let url = link(session, &text);
    println!("url: {} chars", url.len());
    if flags.has("print-only") {
        println!("{url}");
        return Ok(());
    }
    // SAFETY: plain call with valid NUL-terminated strings; the result is an error code <= 32 or
    // a fake HINSTANCE > 32 on success.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(url.as_str()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;
    if code <= 32 {
        eprintln!("ShellExecuteW failed with {code}");
        return Err(Exit::LinkFailed);
    }
    println!("link opened (ShellExecuteW {code}); check which VS Code window and session got it");
    Ok(())
}

/// The link; without a session the extension opens a new conversation.
pub fn link(session: Option<&str>, text: &str) -> String {
    match session {
        Some(id) => format!("{BASE}?session={}&prompt={}", encode(id), encode(text)),
        None => format!("{BASE}?prompt={}", encode(text)),
    }
}

/// Percent-encodes every byte except RFC 3986 unreserved characters (spaces become `%20`, line
/// breaks `%0A`).
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
