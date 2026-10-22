//! How a searched text ([`Needle`]) is found in a string, outside the page's text layer: any
//! run of white space matching any other, compared in Unicode form C, with the search's case
//! and whole-word options; and the string with the text removed.

use super::Needle;

/// `s` in Unicode normalisation form C ("E" + combining acute becomes "É").
pub(super) fn nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
}

/// Where `needle` occurs in `hay` (byte range of `hay`): any run of white space in the text
/// matching any run in `hay`, both compared in Unicode form C; any case unless
/// `match_case`, and only between non-letters when `whole_word`. `hay` must already be in
/// form C (see [`nfc`]) for the range to be meaningful.
fn find_loose(hay: &str, needle: &Needle) -> Option<(usize, usize)> {
    let text = nfc(&needle.text);
    let words: Vec<Vec<char>> = text.split_whitespace().map(|w| w.chars().collect()).collect();
    if words.is_empty() {
        return None;
    }
    let same = |a: char, b: char| a == b || (!needle.match_case && a.to_lowercase().eq(b.to_lowercase()));
    let chars: Vec<(usize, char)> = hay.char_indices().collect();
    let wordy = |i: usize| chars.get(i).is_some_and(|c| c.1.is_alphanumeric());
    'start: for s in 0..chars.len() {
        if needle.whole_word && s > 0 && wordy(s - 1) {
            continue;
        }
        let mut i = s;
        for (k, w) in words.iter().enumerate() {
            if k > 0 {
                let ws = i;
                while i < chars.len() && chars[i].1.is_whitespace() {
                    i += 1;
                }
                if i == ws {
                    continue 'start;
                }
            }
            for c in w {
                match chars.get(i) {
                    Some(&(_, h)) if same(h, *c) => i += 1,
                    _ => continue 'start,
                }
            }
        }
        if needle.whole_word && wordy(i) {
            continue;
        }
        let end = chars.get(i).map_or(hay.len(), |c| c.0);
        return Some((chars[s].0, end));
    }
    None
}

pub(super) fn contains_any(hay: &str, needles: &[Needle]) -> bool {
    let hay = nfc(hay);
    needles.iter().any(|n| find_loose(&hay, n).is_some())
}

/// `text` (in form C) without any of `needles`, or `None` when none occurs.
pub(super) fn scrub(text: &str, needles: &[Needle]) -> Option<String> {
    let mut t = nfc(text);
    let mut changed = false;
    while let Some((a, b)) = needles.iter().find_map(|n| find_loose(&t, n)) {
        t.replace_range(a..b, "");
        changed = true;
    }
    changed.then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_ignores_case_spacing_and_unicode_form() {
        let hay = nfc("Mme E\u{301}LODIE  Martin");
        assert!(find_loose(&hay, &"élodie martin".into()).is_some());
        assert_eq!(scrub("Doc secretword", &["secretword".into()]).as_deref(), Some("Doc "));
        assert_eq!(scrub("nothing", &["secretword".into()]), None);
        // The search's options hold outside the page too: "Ann" as a whole word, case kept.
        let ann = Needle { text: "Ann".into(), match_case: true, whole_word: true };
        assert_eq!(scrub("Annual planning", std::slice::from_ref(&ann)), None);
        assert_eq!(scrub("ann notes", std::slice::from_ref(&ann)), None);
        assert_eq!(scrub("By Ann, 2026", std::slice::from_ref(&ann)).as_deref(), Some("By , 2026"));
    }
}
