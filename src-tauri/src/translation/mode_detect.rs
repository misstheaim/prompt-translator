//! Mode detection for the hotkey popup (plan D20, T52): Correct when the text is in the target
//! language, Translate otherwise.
//!
//! When the target language is written in the Latin script and the native one isn't (the
//! default Russian → English), the share of Latin words decides, as before T52; Latin-script
//! text in a third language (German, Spanish) is then detected as Correct, and the user can
//! switch modes in the popup. For every other pair `whatlang` decides, limited to the two
//! languages.

use whatlang::Detector;

use super::language::LanguagePair;
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

/// Picks the mode for `text` with the language pair `pair`; `last` is returned when there is
/// too little text to tell.
pub fn detect_mode(text: &str, last: Mode, pair: LanguagePair) -> Mode {
    let stripped = strip_tokens(text);
    let words: Vec<&str> = stripped
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() < MIN_WORDS {
        return last;
    }
    let target = pair.target.lang;
    let detected = if pair.target.latin && !pair.native.latin {
        let latin = words
            .iter()
            .filter(|w| w.chars().next().is_some_and(is_latin))
            .count();
        let share = latin as f64 / words.len() as f64;
        if share >= LATIN_HIGH {
            return Mode::Correct;
        }
        if share <= LATIN_LOW {
            return Mode::Translate;
        }
        // A mix of scripts: unrestricted detection, as before T52.
        whatlang::detect_lang(&stripped)
    } else {
        Detector::with_allowlist(vec![pair.native.lang, target]).detect_lang(&stripped)
    };
    if detected == Some(target) {
        Mode::Correct
    } else {
        Mode::Translate
    }
}
