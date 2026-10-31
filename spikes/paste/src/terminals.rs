//! `list-terminals`: read-only. Never focuses, selects, types or touches the clipboard.

use windows::Win32::Foundation::HWND;

use crate::uia::Uia;
use crate::win::{self, WINDOWS_TERMINAL_CLASS};
use crate::{Exit, Flags, usage};

pub fn run(args: &[String]) -> Result<(), Exit> {
    let flags = Flags::parse(args, &["title-chars"], &["all"]).map_err(usage)?;
    let title_chars: usize = flags.number("title-chars", usize::MAX).map_err(usage)?;
    let include_hidden = flags.has("all");

    let uia = match Uia::new() {
        Ok(uia) => Some(uia),
        Err(e) => {
            eprintln!("warning: UI Automation unavailable ({e}); tabs not listed");
            None
        }
    };
    let front = win::foreground();
    let windows = win::top_level_windows();
    let mut shown = 0;
    // Position among visible top-level windows; hidden ones don't count.
    let mut z = 0usize;
    for &hwnd in &windows {
        let visible = win::is_visible(hwnd);
        let position = z;
        if visible {
            z += 1;
        }
        let class = win::class_name(hwnd);
        let (pid, _) = win::pid_and_thread(hwnd);
        let exe = win::exe(pid);
        if !win::is_terminal(&class, &exe) {
            continue;
        }
        if !visible && !include_hidden {
            continue;
        }
        let z = position;
        shown += 1;
        print_window(
            hwnd,
            z,
            pid,
            &exe,
            &class,
            visible,
            hwnd == front,
            title_chars,
        );
        if class == WINDOWS_TERMINAL_CLASS
            && let Some(uia) = &uia
        {
            print_tabs(uia, hwnd, pid, title_chars);
        }
    }
    println!(
        "{shown} terminal window(s); z = position among visible top-level windows \
         (0 = front; always-on-top windows such as the taskbar come first)"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn print_window(
    hwnd: HWND,
    z: usize,
    pid: u32,
    exe: &str,
    class: &str,
    visible: bool,
    foreground: bool,
    title_chars: usize,
) {
    let state = if !visible {
        "hidden"
    } else if win::is_minimised(hwnd) {
        "minimised"
    } else {
        "normal"
    };
    let value = win::hwnd_value(hwnd);
    println!(
        "window hwnd={value:#x} pid={pid} exe={} class={class} state={state} foreground={} monitor={} z={z}",
        win::file_name(exe),
        if foreground { "yes" } else { "no" },
        win::monitor(hwnd),
    );
    println!("  title=\"{}\"", clip(&win::title(hwnd), title_chars));
    if class != WINDOWS_TERMINAL_CLASS {
        println!("  bind: paste --hwnd {value:#x} --pid {pid} --text <file>");
    }
}

fn print_tabs(uia: &Uia, hwnd: HWND, pid: u32, title_chars: usize) {
    let value = win::hwnd_value(hwnd);
    match uia.tabs(hwnd) {
        Ok(tabs) if tabs.is_empty() => {
            println!("  (no TabItem elements)");
            println!("  bind: paste --hwnd {value:#x} --pid {pid} --text <file>");
        }
        Ok(tabs) => {
            for tab in tabs {
                let selected = match tab.is_selected() {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "n/a",
                };
                println!(
                    "  tab runtime-id={} selected={selected} name=\"{}\"",
                    tab.runtime_id,
                    clip(&tab.name, title_chars)
                );
                println!(
                    "    bind: paste --hwnd {value:#x} --pid {pid} --tab {} --text <file>",
                    tab.runtime_id
                );
            }
        }
        Err(e) => println!("  tabs: UIA error {e}"),
    }
}

/// The first `max` chars, with "…" when cut (for redacting personal titles).
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut cut: String = text.chars().take(max).collect();
        cut.push('…');
        cut
    }
}
