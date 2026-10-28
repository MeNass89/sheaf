//! Sheaf: a text box's drawn text recoloured in place. A rich text box another program drew
//! (bold words, larger runs, other faces) keeps its appearance; only the fill colour of its text
//! changes, as Acrobat's colour on a selected text box does (Sol, 27/10: the box was redrawn from
//! its plain text in one font and size).

/// A token of a content stream: its byte range, and whether it is an operator (else an operand:
/// a number, name, string, array or dictionary part).
fn tokens(c: &[u8]) -> Vec<(usize, usize, bool)> {
    let delim = |b: u8| b.is_ascii_whitespace() || b"()<>[]{}/%".contains(&b);
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let b = c[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        match b {
            b'%' => {
                while i < c.len() && c[i] != b'\n' && c[i] != b'\r' {
                    i += 1;
                }
                continue;
            }
            b'(' => {
                let mut depth = 0usize;
                while i < c.len() {
                    match c[i] {
                        b'\\' => i += 1,
                        b'(' => depth += 1,
                        b')' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            b'<' if c.get(i + 1) == Some(&b'<') => i += 2,
            b'>' if c.get(i + 1) == Some(&b'>') => i += 2,
            b'<' => {
                while i < c.len() && c[i] != b'>' {
                    i += 1;
                }
                i += 1;
            }
            b'[' | b']' | b'{' | b'}' => i += 1,
            b'/' => {
                i += 1;
                while i < c.len() && !delim(c[i]) {
                    i += 1;
                }
            }
            _ => {
                while i < c.len() && !delim(c[i]) {
                    i += 1;
                }
                let word = &c[start..i];
                let operand = word.iter().all(|b| b.is_ascii_digit() || b"+-.".contains(b)) || word == b"true" || word == b"false" || word == b"null";
                out.push((start, i.min(c.len()), !operand));
                continue;
            }
        }
        out.push((start, i.min(c.len()), false));
    }
    out
}

/// `content` with the text it shows filled in `rgb`: inside each text object (`BT … ET`) its
/// fill colour operators (with their operands) are replaced by `r g b rg`, and one is set right
/// after `BT` for text in the default colour. Everything else (backgrounds, borders, the runs'
/// fonts and sizes) is kept byte for byte. `None` when the content shows no text.
pub(crate) fn recolour_text(content: &[u8], rgb: [f64; 3]) -> Option<Vec<u8>> {
    let colour = format!("{} {} {} rg", crate::n(rgb[0]), crate::n(rgb[1]), crate::n(rgb[2]));
    let toks = tokens(content);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut in_text = false;
    let mut found = false;
    // The first operand of the operator being read: where a replaced operator's operands start.
    let mut operands_from: Option<usize> = None;
    for &(s, e, op) in &toks {
        if !op {
            operands_from.get_or_insert(s);
            continue;
        }
        let from = operands_from.take().unwrap_or(s);
        match &content[s..e] {
            b"BT" => {
                in_text = true;
                found = true;
                edits.push((e, e, format!(" {colour}")));
            }
            b"ET" => in_text = false,
            b"rg" | b"g" | b"k" | b"sc" | b"scn" | b"cs" if in_text => edits.push((from, e, colour.clone())),
            _ => {}
        }
    }
    if !found {
        return None;
    }
    let mut out = Vec::with_capacity(content.len() + edits.len() * 16);
    let mut at = 0;
    for (s, e, with) in edits {
        out.extend_from_slice(&content[at..s]);
        out.extend_from_slice(with.as_bytes());
        at = e;
    }
    out.extend_from_slice(&content[at..]);
    Some(out)
}
