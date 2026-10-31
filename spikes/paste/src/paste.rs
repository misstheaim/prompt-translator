//! `paste`: the terminal route's recipe. Every step prints a timestamped line, so the owner can
//! record where a case failed. Only Ctrl+V is ever sent: **never Enter**.

use std::thread::sleep;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;

use crate::uia::{Tab, Uia};
use crate::win::{self, FocusMethod, WINDOWS_TERMINAL_CLASS};
use crate::{Exit, Flags, clipboard, own_window, text, usage};

/// How long the keys that triggered delivery (the app's Enter, a modifier) may stay down.
const KEY_RELEASE_TIMEOUT: Duration = Duration::from_millis(500);
/// How long a restored window may take to leave the minimised state.
const RESTORE_TIMEOUT: Duration = Duration::from_millis(1000);
/// How long `SetForegroundWindow`'s effect may take to show in `GetForegroundWindow`.
const FOREGROUND_TIMEOUT: Duration = Duration::from_millis(500);
/// How long the selected tab may take to report `IsSelected`.
const SELECT_TIMEOUT: Duration = Duration::from_millis(500);
/// Pause after focus + tab switch so the terminal routes keyboard input to the new tab.
const SETTLE: Duration = Duration::from_millis(100);
/// The app's layout-fix `PASTE_DELAY`: time for the target to read the clipboard after Ctrl+V.
const DEFAULT_RESTORE_DELAY_MS: u64 = 200;

pub fn run(args: &[String]) -> Result<(), Exit> {
    let flags = Flags::parse(
        args,
        &[
            "hwnd",
            "pid",
            "tab",
            "text",
            "focus",
            "delay",
            "restore-delay",
            "via",
        ],
        &["own-window", "dry-run"],
    )
    .map_err(usage)?;
    let hwnd = flags
        .value("hwnd")
        .ok_or_else(|| usage("`--hwnd` is required"))
        .and_then(|v| parse_hwnd(v).ok_or_else(|| usage(format!("bad `--hwnd` `{v}`"))))?;
    let expected_pid: Option<u32> = match flags.value("pid") {
        None => None,
        Some(v) => Some(v.parse().map_err(|_| usage(format!("bad `--pid` `{v}`")))?),
    };
    let tab_id = flags.value("tab").map(str::to_string);
    let text_path = flags
        .value("text")
        .ok_or_else(|| usage("`--text` is required"))?;
    let focus = match flags.value("focus") {
        None => FocusMethod::Plain,
        Some(v) => FocusMethod::parse(v).ok_or_else(|| usage(format!("bad `--focus` `{v}`")))?,
    };
    let delay: u64 = flags.number("delay", 0).map_err(usage)?;
    let restore_delay: u64 = flags
        .number("restore-delay", DEFAULT_RESTORE_DELAY_MS)
        .map_err(usage)?;
    let own = flags.has("own-window");
    if own && delay > 0 {
        return Err(usage("use either `--own-window` or `--delay`"));
    }

    let log = Log::new();
    let text = text::read(text_path)?;

    // 1. The bound window: still there, same process, a terminal.
    check_window(&log, hwnd, expected_pid)?;
    let class = win::class_name(hwnd);

    // How the text goes in: Ctrl+V, or conhost's own Edit > Paste (Ctrl+V doesn't paste into
    // Claude Code in conhost, matrix H-6). `auto` picks the menu for conhost windows.
    let menu = match flags.value("via").unwrap_or("auto") {
        "auto" => class == win::CONHOST_CLASS,
        "ctrl-v" => false,
        "menu" => true,
        other => return Err(usage(format!("bad `--via` `{other}`"))),
    };
    if menu && class != win::CONHOST_CLASS {
        return Err(usage("`--via menu` works only for conhost windows"));
    }
    // conhost's paste turns line breaks into Enter presses: that would submit the lines one by
    // one, so multi-line text is refused on that route.
    if menu && text.contains('\n') {
        log.say("multi-line text refused for conhost menu paste (line breaks would act as Enter)");
        return Err(Exit::MultiLineRefused);
    }

    // 2. The bound tab (Windows Terminal only), found by RuntimeId.
    let tab = match &tab_id {
        None => None,
        Some(id) => {
            let uia = Uia::new().map_err(|e| {
                log.say(format!("UIA init failed: {e}"));
                Exit::Internal
            })?;
            Some(find_tab(&log, &uia, hwnd, &class, id)?)
        }
    };

    if flags.has("dry-run") {
        log.say("dry run: checks passed; nothing focused, selected, pasted or copied");
        return Ok(());
    }

    // 3. The trigger: Enter released in the spike's own window, or a plain delay.
    let _own_window = if own {
        log.say("waiting for Enter or a click in the spike window");
        match own_window::wait_for_enter() {
            Ok(Some(window)) => {
                log.say("Enter or click released in the spike window");
                Some(window)
            }
            Ok(None) => {
                log.say("spike window closed: cancelled");
                return Err(Exit::Cancelled);
            }
            Err(e) => {
                log.say(format!("spike window failed: {e}"));
                return Err(Exit::Internal);
            }
        }
    } else {
        if delay > 0 {
            log.say(format!("waiting {delay} ms"));
            sleep(Duration::from_millis(delay));
        }
        None
    };
    let front = win::foreground();
    log.say(format!(
        "foreground before: {:#x} class={}",
        win::hwnd_value(front),
        win::class_name(front)
    ));

    // 4. No modifier or Enter down (the trigger's Enter must not auto-repeat into Claude).
    if let Err(held) = win::wait_for_keys_released(KEY_RELEASE_TIMEOUT) {
        log.say(format!("keys still held: {}", held.join("+")));
        return Err(Exit::KeysHeld);
    }

    // 5. Closed between binding and paste?
    check_window(&log, hwnd, expected_pid)?;

    // 6. Restore if minimised.
    if win::is_minimised(hwnd) {
        log.say("window is minimised: ShowWindow(SW_RESTORE)");
        win::restore(hwnd);
        if !win::wait_until(RESTORE_TIMEOUT, || !win::is_minimised(hwnd)) {
            log.say("still minimised");
            return Err(Exit::StillMinimised);
        }
        log.say("restored");
    }

    // 7. Bring it to the front.
    let returned = win::set_foreground(hwnd, focus);
    log.say(format!(
        "SetForegroundWindow ({focus:?}) returned {returned}"
    ));
    if !win::wait_until(FOREGROUND_TIMEOUT, || win::foreground() == hwnd) {
        let now = win::foreground();
        log.say(format!(
            "not in front; foreground is {:#x} class={}",
            win::hwnd_value(now),
            win::class_name(now)
        ));
        return Err(Exit::ForegroundRefused);
    }
    log.say("window is in front");

    // 8. Select the bound tab.
    if let Some(tab) = &tab {
        if tab.is_selected() != Some(true) {
            if let Err(e) = tab.select() {
                log.say(format!("Select() failed: {e}"));
                return Err(Exit::TabSelectFailed);
            }
            if !win::wait_until(SELECT_TIMEOUT, || tab.is_selected() == Some(true)) {
                log.say("tab did not become selected");
                return Err(Exit::TabSelectFailed);
            }
            log.say("tab selected");
        } else {
            log.say("tab already selected");
        }
    }
    sleep(SETTLE);

    // 9. Last checks right before the paste: still in front, tab still selected, no key held.
    if let Err(held) = win::wait_for_keys_released(KEY_RELEASE_TIMEOUT) {
        log.say(format!("keys held before paste: {}", held.join("+")));
        return Err(Exit::KeysHeld);
    }
    if win::foreground() != hwnd {
        log.say("window lost the foreground before the paste");
        return Err(Exit::TargetLostFocus);
    }
    if let Some(tab) = &tab
        && tab.is_selected() != Some(true)
    {
        log.say("bound tab no longer selected before the paste");
        return Err(Exit::TargetLostFocus);
    }

    // 10. Save the clipboard text, put ours there (hidden from history), Ctrl+V, restore.
    let saved = clipboard::read_text().map_err(|_| {
        log.say("clipboard busy (save)");
        Exit::ClipboardBusy
    })?;
    if !clipboard::write_text_hidden(Some(&text::to_crlf(&text))) {
        log.say("clipboard busy (write)");
        clipboard::write_text_hidden(saved.as_deref());
        return Err(Exit::ClipboardBusy);
    }
    let written = clipboard::sequence_number();
    if menu {
        win::conhost_menu_paste(hwnd);
        log.say("conhost Edit > Paste (WM_COMMAND 0xFFF1) sent (no Enter)");
    } else {
        if !win::send_ctrl_v() {
            log.say("SendInput blocked (elevated target?)");
            clipboard::write_text_hidden(saved.as_deref());
            return Err(Exit::InputBlocked);
        }
        log.say("Ctrl+V sent (no Enter)");
    }
    sleep(Duration::from_millis(restore_delay));
    if clipboard::sequence_number() != written {
        log.say("clipboard changed since the paste (user copied?): not restored");
        return Ok(());
    }
    if !clipboard::write_text_hidden(saved.as_deref()) {
        log.say("clipboard restore failed");
        return Err(Exit::RestoreFailed);
    }
    log.say(format!(
        "clipboard restored after {restore_delay} ms ({})",
        if saved.is_some() {
            "text"
        } else {
            "was empty or non-text: emptied"
        }
    ));
    Ok(())
}

fn parse_hwnd(value: &str) -> Option<HWND> {
    let number = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => usize::from_str_radix(hex, 16).ok()?,
        None => value.parse().ok()?,
    };
    (number != 0).then(|| win::hwnd_from(number))
}

fn check_window(log: &Log, hwnd: HWND, expected_pid: Option<u32>) -> Result<(), Exit> {
    if !win::exists(hwnd) {
        log.say("IsWindow: false (window closed)");
        return Err(Exit::WindowGone);
    }
    let (pid, _) = win::pid_and_thread(hwnd);
    if let Some(expected) = expected_pid
        && pid != expected
    {
        log.say(format!("pid is {pid}, expected {expected} (HWND reused)"));
        return Err(Exit::PidMismatch);
    }
    let class = win::class_name(hwnd);
    let exe = win::exe(pid);
    if !win::is_terminal(&class, &exe) {
        log.say(format!(
            "not a terminal: class={class} exe={}",
            win::file_name(&exe)
        ));
        return Err(Exit::NotTerminal);
    }
    log.say(format!(
        "window ok: pid={pid} class={class} exe={} minimised={}",
        win::file_name(&exe),
        win::is_minimised(hwnd)
    ));
    Ok(())
}

fn find_tab(log: &Log, uia: &Uia, hwnd: HWND, class: &str, id: &str) -> Result<Tab, Exit> {
    if class != WINDOWS_TERMINAL_CLASS {
        log.say(format!(
            "`--tab` given but the window is {class}, not Windows Terminal"
        ));
        return Err(Exit::TabNotFound);
    }
    let tabs = uia.tabs(hwnd).map_err(|e| {
        log.say(format!("UIA tabs failed: {e}"));
        Exit::Internal
    })?;
    let count = tabs.len();
    match tabs.into_iter().find(|t| t.runtime_id == id) {
        Some(tab) => {
            log.say(format!("tab found, selected={:?}", tab.is_selected()));
            Ok(tab)
        }
        None => {
            log.say(format!("tab {id} not found among {count} tab(s)"));
            Err(Exit::TabNotFound)
        }
    }
}

/// Step lines with milliseconds since the start.
struct Log(Instant);

impl Log {
    fn new() -> Self {
        Self(Instant::now())
    }

    fn say(&self, message: impl std::fmt::Display) {
        println!("[{:>5} ms] {message}", self.0.elapsed().as_millis());
    }
}
