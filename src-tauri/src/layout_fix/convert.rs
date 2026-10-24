//! Pure keyboard-layout conversion (D36): turns text typed with the wrong layout
//! (Windows US QWERTY vs Russian ЙЦУКЕН) into what the user meant.
//!
//! Characters that are identical on both layouts (digits, space, newlines,
//! `! % * ( ) - _ = + \`) and anything not in the source column pass through unchanged.

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
    let mut latin = 0usize;
    let mut cyrillic = 0usize;
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            latin += 1;
        } else if ('\u{0400}'..='\u{04FF}').contains(&c) && c.is_alphabetic() {
            cyrillic += 1;
        }
    }
    match latin.cmp(&cyrillic) {
        std::cmp::Ordering::Greater => Some(Direction::ToRussian),
        std::cmp::Ordering::Less => Some(Direction::ToEnglish),
        std::cmp::Ordering::Equal => None,
    }
}
