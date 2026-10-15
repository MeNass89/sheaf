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
