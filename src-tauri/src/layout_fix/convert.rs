//! Pure keyboard-layout conversion (D36): turns text typed with the wrong layout
//! (Windows US QWERTY vs Russian ЙЦУКЕН) into what the user meant.
//!
//! Characters that are identical on both layouts (digits, space, newlines,
//! `! % * ( ) - _ = + \`) and anything not in the source column pass through unchanged.
//!
//! [`is_wrong_layout`] judges a single word (LAYOUT-008) for the hotkey's word-by-word selection.

use super::bigrams::{EN_COSTS, RU_COSTS};

/// Direction of a conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Text typed on the US layout, meant as Russian (`ghbdtn` -> `привет`).
    ToRussian,
    /// Text typed on the Russian layout, meant as English (`руддщ` -> `hello`).
    ToEnglish,
}

/// Physical keys as `(us_char, ru_char)`, unshifted first, then shifted.
/// Each column holds a character at most once; the two directions are separate
/// lookups (e.g. `.` is `ю` going to Russian and `/` going to English).
/// To support another layout pair, add another table and pick it in `convert`.
const KEYS: &[(char, char)] = &[
    // Unshifted letters, row by row.
    ('q', 'й'),
    ('w', 'ц'),
    ('e', 'у'),
    ('r', 'к'),
    ('t', 'е'),
    ('y', 'н'),
    ('u', 'г'),
    ('i', 'ш'),
    ('o', 'щ'),
    ('p', 'з'),
    ('[', 'х'),
    (']', 'ъ'),
    ('a', 'ф'),
    ('s', 'ы'),
    ('d', 'в'),
    ('f', 'а'),
    ('g', 'п'),
    ('h', 'р'),
    ('j', 'о'),
    ('k', 'л'),
    ('l', 'д'),
    (';', 'ж'),
    ('\'', 'э'),
    ('z', 'я'),
    ('x', 'ч'),
    ('c', 'с'),
    ('v', 'м'),
    ('b', 'и'),
    ('n', 'т'),
    ('m', 'ь'),
    (',', 'б'),
    ('.', 'ю'),
    ('/', '.'),
    ('`', 'ё'),
    // Shifted letters.
    ('Q', 'Й'),
    ('W', 'Ц'),
    ('E', 'У'),
    ('R', 'К'),
    ('T', 'Е'),
    ('Y', 'Н'),
    ('U', 'Г'),
    ('I', 'Ш'),
    ('O', 'Щ'),
    ('P', 'З'),
    ('{', 'Х'),
    ('}', 'Ъ'),
    ('A', 'Ф'),
    ('S', 'Ы'),
    ('D', 'В'),
    ('F', 'А'),
    ('G', 'П'),
    ('H', 'Р'),
    ('J', 'О'),
    ('K', 'Л'),
    ('L', 'Д'),
    (':', 'Ж'),
    ('"', 'Э'),
    ('Z', 'Я'),
    ('X', 'Ч'),
    ('C', 'С'),
    ('V', 'М'),
    ('B', 'И'),
    ('N', 'Т'),
    ('M', 'Ь'),
    ('<', 'Б'),
    ('>', 'Ю'),
    ('?', ','),
    ('~', 'Ё'),
    // Shifted digit row and backslash, where the layouts differ.
    ('@', '"'),
    ('#', '№'),
    ('$', ';'),
    ('^', ':'),
    ('&', '?'),
    ('|', '/'),
];

/// Maps one character through the column for `dir`; unknown characters stay.
fn map_char(c: char, dir: Direction) -> char {
    match dir {
        Direction::ToRussian => KEYS.iter().find(|(us, _)| *us == c).map(|(_, ru)| *ru),
        Direction::ToEnglish => KEYS.iter().find(|(_, ru)| *ru == c).map(|(us, _)| *us),
    }
    .unwrap_or(c)
}

/// Converts `text` by mapping every character through the column for `dir`.
pub fn convert(text: &str, dir: Direction) -> String {
    text.chars().map(|c| map_char(c, dir)).collect()
}

/// Guesses the conversion direction from the letters in `text`: more Latin
/// letters means `ToRussian`, more Cyrillic means `ToEnglish`; a tie (including
/// no letters at all) gives `None`. Everything that is not a letter is ignored.
pub fn detect_direction(text: &str) -> Option<Direction> {
    let (latin, cyrillic) = letter_counts(text);
    match latin.cmp(&cyrillic) {
        std::cmp::Ordering::Greater => Some(Direction::ToRussian),
        std::cmp::Ordering::Less => Some(Direction::ToEnglish),
        std::cmp::Ordering::Equal => None,
    }
}

/// The number of Latin and of Cyrillic letters in `text`.
fn letter_counts(text: &str) -> (usize, usize) {
    let mut latin = 0usize;
    let mut cyrillic = 0usize;
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            latin += 1;
        } else if ('\u{0400}'..='\u{04FF}').contains(&c) && c.is_alphabetic() {
            cyrillic += 1;
        }
    }
    (latin, cyrillic)
}

/// Words with fewer letters than this are too short to judge (`is_wrong_layout` gives `None`).
const MIN_LETTERS: usize = 3;

/// Cost of one step into or out of a character that is no letter in the word's language while
/// its counterpart on the other layout is a letter (`,` for `б`, `[` for `х`), inside a word:
/// 12 bits, in the units of the `bigrams` tables (16 per bit).
const FOREIGN_COST: u32 = 12 * 16;

/// Such characters may still open or close a word without counting against it (`[link`,
/// `word,`).
const OPENERS: &[char] = &['(', '[', '\'', '"', '`', '<', '{'];
const CLOSERS: &[char] = &[
    ')', ']', ',', '.', ';', ':', '\'', '"', '`', '>', '}', '?', '!',
];

/// Apostrophes inside a word (`don't`) only split it, although `'` is `э` on the Russian layout.
const APOSTROPHES: &[char] = &['\'', '\u{2019}'];

/// Whether `word` looks typed in the wrong keyboard layout (LAYOUT-008): `Some(true)` when its
/// conversion is a more likely word in the other language than the word itself is in its own,
/// `Some(false)` when the word is the more likely one. `None` for words with fewer than three
/// letters, or as many Latin as Cyrillic letters (including none): too short or mixed to tell.
///
/// "Likely" is the average cost per letter pair under the English and Russian letter-pair
/// tables in `bigrams` (word start and end included), so long and short words compare fairly.
pub fn is_wrong_layout(word: &str) -> Option<bool> {
    let (latin, cyrillic) = letter_counts(word);
    if latin + cyrillic < MIN_LETTERS {
        return None;
    }
    let direction = detect_direction(word)?;
    let converted = convert(word, direction);
    let (own, other) = match direction {
        Direction::ToRussian => (Language::English, Language::Russian),
        Direction::ToEnglish => (Language::Russian, Language::English),
    };
    match (
        word_cost(word, &converted, own, other),
        word_cost(&converted, word, other, own),
    ) {
        // Compares the averages `other_total / other_steps < own_total / own_steps`.
        (Some((own_total, own_steps)), Some((other_total, other_steps))) => {
            Some(u64::from(other_total) * own_steps < u64::from(own_total) * other_steps)
        }
        (None, Some(_)) => Some(true),
        (Some(_), None) => Some(false),
        (None, None) => None,
    }
}

/// The two languages of the letter-pair tables.
#[derive(Debug, Clone, Copy)]
enum Language {
    English,
    Russian,
}

impl Language {
    /// The letter's row/column in this language's table (1-based; 0 is the word boundary),
    /// ignoring case; `None` for anything that is not one of its letters.
    fn letter_index(self, c: char) -> Option<usize> {
        match self {
            Language::English => {
                let c = c.to_ascii_lowercase();
                c.is_ascii_lowercase()
                    .then(|| (u32::from(c) - u32::from('a')) as usize + 1)
            }
            Language::Russian => {
                let c = match c {
                    'ё' | 'Ё' => 'е',
                    // `А`–`Я` are `а`–`я` minus 0x20.
                    'А'..='Я' => char::from_u32(u32::from(c) + 0x20)?,
                    c => c,
                };
                ('а'..='я')
                    .contains(&c)
                    .then(|| (u32::from(c) - u32::from('а')) as usize + 1)
            }
        }
    }

    fn cost(self, from: usize, to: usize) -> u32 {
        u32::from(match self {
            Language::English => EN_COSTS[from][to],
            Language::Russian => RU_COSTS[from][to],
        })
    }
}

/// One character of a word as `word_cost` sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Symbol {
    /// Not part of the word in either language (space, digit, `-`, `'`): splits the word.
    Boundary,
    Letter(usize),
    /// No letter here, but a letter on the other layout (see [`FOREIGN_COST`]).
    Foreign,
}

impl Symbol {
    fn index(self) -> usize {
        match self {
            Symbol::Letter(index) => index,
            Symbol::Boundary | Symbol::Foreign => 0,
        }
    }
}

/// The cost of `word` as a `language` word, as `(total, steps)` over its letter pairs (word start
/// and end included); `None` when it has no letters of `language`. `partner` is the same keys on
/// the `other` layout (char for char), which tells a foreign symbol from plain punctuation.
fn word_cost(word: &str, partner: &str, language: Language, other: Language) -> Option<(u32, u64)> {
    let symbols: Vec<(char, Symbol)> = word
        .chars()
        .zip(partner.chars())
        .map(|(c, p)| {
            let symbol = if let Some(index) = language.letter_index(c) {
                Symbol::Letter(index)
            } else if other.letter_index(p).is_some() && !APOSTROPHES.contains(&c) {
                Symbol::Foreign
            } else {
                Symbol::Boundary
            };
            (c, symbol)
        })
        .collect();
    // Plain punctuation, and foreign symbols that commonly open or close a word, stay outside it.
    let first = symbols.iter().position(|&(c, symbol)| {
        !(symbol == Symbol::Boundary || (symbol == Symbol::Foreign && OPENERS.contains(&c)))
    })?;
    let last = symbols.iter().rposition(|&(c, symbol)| {
        !(symbol == Symbol::Boundary || (symbol == Symbol::Foreign && CLOSERS.contains(&c)))
    })?;
    let symbols = symbols.get(first..=last)?;
    if !symbols
        .iter()
        .any(|(_, symbol)| matches!(symbol, Symbol::Letter(_)))
    {
        return None;
    }

    let mut total = 0u32;
    let mut steps = 0u64;
    let mut prev = Symbol::Boundary;
    for symbol in symbols
        .iter()
        .map(|&(_, symbol)| symbol)
        .chain([Symbol::Boundary])
    {
        if symbol == Symbol::Boundary && prev == Symbol::Boundary {
            continue;
        }
        total += if symbol == Symbol::Foreign || prev == Symbol::Foreign {
            FOREIGN_COST
        } else {
            language.cost(prev.index(), symbol.index())
        };
        steps += 1;
        prev = symbol;
    }
    Some((total, steps))
}
