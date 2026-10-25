use crate::{OpdfError, Result};
use std::ops::RangeInclusive;

/// Parses `"3, 1-2, 5-"` into 1-based inclusive ranges, preserving order.
/// `"n-"` means n to the last page; an empty start (`"-3"`) is invalid.
pub fn parse_ranges(spec: &str, pages: u32) -> Result<Vec<RangeInclusive<u32>>> {
    let bad = || OpdfError::InvalidRange { range: spec.to_string(), pages };
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim) {
        if part.is_empty() { return Err(bad()); }
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let a: u32 = a.parse().map_err(|_| bad())?;
        let b: u32 = if b.is_empty() { pages } else { b.parse().map_err(|_| bad())? };
        if a == 0 || a > b || b > pages { return Err(bad()); }
        out.push(a..=b);
    }
    Ok(out)
}

/// Ranges written back as a spec ("1-3,4,5-9").
pub fn spec_of(ranges: &[RangeInclusive<u32>]) -> String {
    ranges.iter().map(|r| if r.start() == r.end() { r.start().to_string() } else { format!("{}-{}", r.start(), r.end()) }).collect::<Vec<_>>().join(",")
}

/// Split's page grid (PDF Expert's split points): a click between pages `after` and `after + 1`
/// cuts the range holding both in two, or joins the range ending at `after` with the one starting
/// right after it. An empty or unreadable spec counts as the whole document in one range.
pub fn toggle_cut(spec: &str, after: u32, pages: u32) -> String {
    let mut r = match parse_ranges(spec, pages) {
        Ok(r) if !spec.trim().is_empty() => r,
        _ => std::iter::once(1..=pages.max(1)).collect(),
    };
    if let Some(i) = r.iter().position(|x| *x.start() <= after && after < *x.end()) {
        let (a, b) = (*r[i].start(), *r[i].end());
        r.splice(i..=i, [a..=after, after + 1..=b]);
    } else if let Some(i) = r.windows(2).position(|w| *w[0].end() == after && *w[1].start() == after + 1) {
        let (a, b) = (*r[i].start(), *r[i + 1].end());
        r.splice(i..=i + 1, std::iter::once(a..=b));
    }
    spec_of(&r)
}

/// Which range (0-based) of `spec` page `page` falls in first, for colouring the grid; None when no
/// range takes it (or the spec does not read).
pub fn range_of(spec: &str, page: u32, pages: u32) -> Option<usize> {
    parse_ranges(spec, pages).ok()?.iter().position(|r| r.contains(&page))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_between_pages_cuts_a_range_and_a_second_click_joins_it_back() {
        assert_eq!(toggle_cut("", 3, 10), "1-3,4-10");
        assert_eq!(toggle_cut("1-3,4-10", 7, 10), "1-3,4-7,8-10");
        assert_eq!(toggle_cut("1-3,4-7,8-10", 3, 10), "1-7,8-10");
        assert_eq!(toggle_cut("1-2,3", 2, 3), "1-3");
        // Between two pages not in one run of ranges: nothing to cut or join.
        assert_eq!(toggle_cut("1-2,5-6", 3, 6), "1-2,5-6");
        assert_eq!(toggle_cut("1,2", 1, 2), "1-2");
        assert_eq!(range_of("1-3,4-10", 4, 10), Some(1));
        assert_eq!(range_of("1-3,8", 5, 10), None);
        assert_eq!(range_of("zz", 1, 10), None);
    }
}
