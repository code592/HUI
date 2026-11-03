//! UTF-8 word selection shared by native reading and editing surfaces.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

pub fn word_at(text: &str, byte: usize) -> Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let byte = byte.min(text.len().saturating_sub(1));
    // Unicode word boundaries cover CJK, Latin, Cyrillic and Korean. For punctuation
    // and emoji select a complete grapheme, never half of a combining/ZWJ sequence.
    text.unicode_word_indices()
        .find(|(start, word)| *start <= byte && byte < *start + word.len())
        .map(|(start, word)| start..start + word.len())
        .or_else(|| {
            text.grapheme_indices(true)
                .find(|(start, s)| *start <= byte && byte < *start + s.len())
                .map(|(start, s)| start..start + s.len())
        })
        .unwrap_or(0..0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multilingual_words_and_graphemes() {
        for text in ["hello", "Русский", "한국어", "cafe\u{301}", "👨‍👩‍👧‍👦"]
        {
            let range = word_at(text, 1);
            assert_eq!(&text[range], text);
        }
        assert_eq!(&"打开"[word_at("打开", 0)], "打");
        assert_eq!(word_at("", 0), 0..0);
    }
}
