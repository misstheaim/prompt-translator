//! OS integration: tray, hotkeys, popup, clipboard, synthetic input and app paths.

pub mod autostart;
pub mod clipboard;
pub mod commands;
pub mod hotkey;
pub mod input;
pub mod paths;
pub mod popup;
pub mod process;
pub mod tray;
#[cfg(windows)]
pub mod uia;
pub mod uninstall;
