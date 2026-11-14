pub trait UnicodeWidthChar {
    fn width(&self) -> Option<usize>;
}

pub trait UnicodeWidthStr {
    fn width(&self) -> usize;
}

impl UnicodeWidthChar for char {
    fn width(&self) -> Option<usize> {
        let cp = *self as u32;
        if cp == 0 {
            return Some(0);
        }
        if cp < 0x20 || (0x7F..=0x9F).contains(&cp) {
            return None;
        }
        if is_east_asian_wide(cp) {
            Some(2)
        } else {
            Some(1)
        }
    }
}

impl UnicodeWidthStr for str {
    fn width(&self) -> usize {
        self.chars().filter_map(|c| c.width()).sum()
    }
}

fn is_east_asian_wide(cp: u32) -> bool {
    matches!(
        cp,
        0x1100..=0x11FF   // Hangul Jamo
        | 0x2E80..=0x2FFF // CJK Radicals / Kangxi
        | 0x3000..=0x303F // CJK Symbols and Punctuation
        | 0x3040..=0x309F // Hiragana
        | 0x30A0..=0x30FF // Katakana
        | 0x3130..=0x318F // Hangul Compatibility Jamo
        | 0x3200..=0x4DBF // Enclosed CJK / CJK Ext A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xA960..=0xA97F // Hangul Jamo Extended-A
        | 0xAC00..=0xD7A3 // Hangul Syllables
        | 0xD7B0..=0xD7FF // Hangul Jamo Extended-B
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
        | 0xFE30..=0xFE6F // CJK Compatibility Forms
        | 0xFF01..=0xFF60 // Fullwidth Forms
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x2FFFD
        | 0x30000..=0x3FFFD
    )
}
