//! The language pair (plan D44, T52): a native language (default Russian) and a target
//! language (default English).
//!
//! The supported languages are the ones `whatlang` can detect, so mode detection works for
//! every pair the user can choose. Each has an ISO 639-1 code (what settings, requests and
//! history store) and an English name (what the prompt templates and the UI show).

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use whatlang::Lang;

use crate::error::{AppError, AppResult};
use crate::settings::Settings;

/// The default native language (`Settings::native_language`).
pub const DEFAULT_NATIVE: &str = "ru";
/// The default target language (`Settings::target_language`).
pub const DEFAULT_TARGET: &str = "en";

/// One supported language, as the UI lists it (`list_languages`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Language {
    /// ISO 639-1 code, such as `ru` or `en`.
    pub code: String,
    /// English name, such as `Russian`.
    pub name: String,
}

/// A supported language with what the core needs to know about it.
#[derive(Debug, PartialEq, Eq)]
pub struct LanguageInfo {
    pub code: &'static str,
    pub name: &'static str,
    pub lang: Lang,
    /// Written in the Latin script (used by mode detection's Latin-share shortcut).
    pub latin: bool,
}

const fn info(lang: Lang, code: &'static str, name: &'static str, latin: bool) -> LanguageInfo {
    LanguageInfo {
        code,
        name,
        lang,
        latin,
    }
}

/// Every language `whatlang` 0.18 detects, sorted by English name.
pub static LANGUAGES: [LanguageInfo; 70] = [
    info(Lang::Afr, "af", "Afrikaans", true),
    info(Lang::Aka, "ak", "Akan", true),
    info(Lang::Amh, "am", "Amharic", false),
    info(Lang::Ara, "ar", "Arabic", false),
    info(Lang::Hye, "hy", "Armenian", false),
    info(Lang::Aze, "az", "Azerbaijani", true),
    info(Lang::Bel, "be", "Belarusian", false),
    info(Lang::Ben, "bn", "Bengali", false),
    info(Lang::Bul, "bg", "Bulgarian", false),
    info(Lang::Mya, "my", "Burmese", false),
    info(Lang::Cat, "ca", "Catalan", true),
    info(Lang::Cmn, "zh", "Chinese", false),
    info(Lang::Hrv, "hr", "Croatian", true),
    info(Lang::Ces, "cs", "Czech", true),
    info(Lang::Dan, "da", "Danish", true),
    info(Lang::Nld, "nl", "Dutch", true),
    info(Lang::Eng, "en", "English", true),
    info(Lang::Epo, "eo", "Esperanto", true),
    info(Lang::Est, "et", "Estonian", true),
    info(Lang::Fin, "fi", "Finnish", true),
    info(Lang::Fra, "fr", "French", true),
    info(Lang::Kat, "ka", "Georgian", false),
    info(Lang::Deu, "de", "German", true),
    info(Lang::Ell, "el", "Greek", false),
    info(Lang::Guj, "gu", "Gujarati", false),
    info(Lang::Heb, "he", "Hebrew", false),
    info(Lang::Hin, "hi", "Hindi", false),
    info(Lang::Hun, "hu", "Hungarian", true),
    info(Lang::Ind, "id", "Indonesian", true),
    info(Lang::Ita, "it", "Italian", true),
    info(Lang::Jpn, "ja", "Japanese", false),
    info(Lang::Jav, "jv", "Javanese", true),
    info(Lang::Kan, "kn", "Kannada", false),
    info(Lang::Khm, "km", "Khmer", false),
    info(Lang::Kor, "ko", "Korean", false),
    info(Lang::Lat, "la", "Latin", true),
    info(Lang::Lav, "lv", "Latvian", true),
    info(Lang::Lit, "lt", "Lithuanian", true),
    info(Lang::Mkd, "mk", "Macedonian", false),
    info(Lang::Mal, "ml", "Malayalam", false),
    info(Lang::Mar, "mr", "Marathi", false),
    info(Lang::Nep, "ne", "Nepali", false),
    info(Lang::Nob, "nb", "Norwegian Bokmål", true),
    info(Lang::Ori, "or", "Odia", false),
    info(Lang::Pes, "fa", "Persian", false),
    info(Lang::Pol, "pl", "Polish", true),
    info(Lang::Por, "pt", "Portuguese", true),
    info(Lang::Pan, "pa", "Punjabi", false),
    info(Lang::Ron, "ro", "Romanian", true),
    info(Lang::Rus, "ru", "Russian", false),
    info(Lang::Srp, "sr", "Serbian", false),
    info(Lang::Sna, "sn", "Shona", true),
    info(Lang::Sin, "si", "Sinhala", false),
    info(Lang::Slk, "sk", "Slovak", true),
    info(Lang::Slv, "sl", "Slovenian", true),
    info(Lang::Spa, "es", "Spanish", true),
    info(Lang::Swe, "sv", "Swedish", true),
    info(Lang::Tgl, "tl", "Tagalog", true),
    info(Lang::Tam, "ta", "Tamil", false),
    info(Lang::Tel, "te", "Telugu", false),
    info(Lang::Tha, "th", "Thai", false),
    info(Lang::Tur, "tr", "Turkish", true),
    info(Lang::Tuk, "tk", "Turkmen", true),
    info(Lang::Ukr, "uk", "Ukrainian", false),
    info(Lang::Urd, "ur", "Urdu", false),
    info(Lang::Uzb, "uz", "Uzbek", true),
    info(Lang::Vie, "vi", "Vietnamese", true),
    info(Lang::Cym, "cy", "Welsh", true),
    info(Lang::Yid, "yi", "Yiddish", false),
    info(Lang::Zul, "zu", "Zulu", true),
];

/// The supported language with this ISO 639-1 code (trimmed, case-insensitive).
pub fn find(code: &str) -> Option<&'static LanguageInfo> {
    let code = code.trim();
    LANGUAGES
        .iter()
        .find(|language| language.code.eq_ignore_ascii_case(code))
}

/// Every supported language, for the UI.
pub fn list() -> Vec<Language> {
    LANGUAGES
        .iter()
        .map(|language| Language {
            code: language.code.to_owned(),
            name: language.name.to_owned(),
        })
        .collect()
}

/// The native and the target language of one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguagePair {
    /// The language the text is translated from (in Translate mode).
    pub native: &'static LanguageInfo,
    /// The language of the result.
    pub target: &'static LanguageInfo,
}

impl Default for LanguagePair {
    /// Russian → English.
    fn default() -> Self {
        Self {
            native: find(DEFAULT_NATIVE).expect("the default native language is supported"),
            target: find(DEFAULT_TARGET).expect("the default target language is supported"),
        }
    }
}

impl LanguagePair {
    /// The pair saved in the settings. A hand-edited `settings.json` with an unknown code or
    /// two equal languages falls back to the default pair (logged), like any other bad value.
    pub fn from_settings(settings: &Settings) -> Self {
        match (
            find(&settings.native_language),
            find(&settings.target_language),
        ) {
            (Some(native), Some(target)) if native != target => Self { native, target },
            _ => {
                log::warn!(
                    "Invalid language pair in the settings ({:?} → {:?}); using Russian → English",
                    settings.native_language,
                    settings.target_language
                );
                Self::default()
            }
        }
    }

    /// The pair for a request: the given codes, each missing one taken from the settings
    /// (TRANS-008). Unknown codes and equal languages are an `InvalidInput` error.
    pub fn resolve(
        native: Option<&str>,
        target: Option<&str>,
        settings: &Settings,
    ) -> AppResult<Self> {
        let saved = Self::from_settings(settings);
        let native = match native {
            Some(code) => known(code)?,
            None => saved.native,
        };
        let target = match target {
            Some(code) => known(code)?,
            None => saved.target,
        };
        if native == target {
            return Err(AppError::InvalidInput(
                "Native and target language must differ".into(),
            ));
        }
        Ok(Self { native, target })
    }
}

fn known(code: &str) -> AppResult<&'static LanguageInfo> {
    find(code).ok_or_else(|| {
        AppError::InvalidInput(format!("The language \"{}\" isn't supported", code.trim()))
    })
}

/// `name` with its indefinite article: "an English", "a German".
pub fn with_article(name: &str) -> String {
    // "Ukrainian" starts with a "you" sound; every other vowel-initial name takes "an".
    let vowel = name
        .chars()
        .next()
        .is_some_and(|c| "AEIOU".contains(c.to_ascii_uppercase()));
    let article = if vowel && !name.starts_with("Uk") {
        "an"
    } else {
        "a"
    };
    format!("{article} {name}")
}
