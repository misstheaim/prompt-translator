//! Mode detection for the hotkey popup (plan D20): picks Translate or Correct
//! from the text. Latin-script text that isn't English (German, Spanish) is
//! detected as Correct; the user can switch modes in the popup.

use super::model::Mode;
use super::token_check::strip_tokens;

/// Fewer words than this (after stripping technical tokens) keeps `last`.
const MIN_WORDS: usize = 3;
/// Latin share at or above this is Correct.
const LATIN_HIGH: f64 = 0.6;
/// Latin share at or below this is Translate.
const LATIN_LOW: f64 = 0.4;

fn is_latin(c: char) -> bool {
    // Basic Latin, Latin-1 Supplement letters and Latin Extended-A/B.
    c.is_alphabetic() && c <= '\u{024F}'
}

/// Picks the mode for `text`; `last` is returned when there is too little text to tell.
pub fn detect_mode(text: &str, last: Mode) -> Mode {
    let stripped = strip_tokens(text);
    let words: Vec<&str> = stripped
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() < MIN_WORDS {
        return last;
    }
    let latin = words
        .iter()
        .filter(|w| w.chars().next().is_some_and(is_latin))
        .count();
    let share = latin as f64 / words.len() as f64;
    if share >= LATIN_HIGH {
        Mode::Correct
    } else if share <= LATIN_LOW {
        Mode::Translate
    } else {
        match whatlang::detect_lang(&stripped) {
            Some(whatlang::Lang::Eng) => Mode::Correct,
            _ => Mode::Translate,
        }
    }
}
