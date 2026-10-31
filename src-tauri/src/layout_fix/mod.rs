//! Keyboard layout fix (D36–D38): `convert` is the pure conversion and the wrong-layout word test
//! (with its letter-pair tables in `bigrams`), `hotkey` the stand-alone hotkey that replaces text
//! in the foreground app.

mod bigrams;
pub mod commands;
pub mod convert;
pub mod hotkey;
