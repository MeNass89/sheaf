//! Sheaf: a calculation in simplified field notation ("Price * Quantity") written as the script
//! Acrobat writes for it: the notation kept in its `BVCALC … EVCALC` comment, then the same sum
//! over the fields' values (upstream wrote `event.value = AFMakeNumber(0)`, which Acrobat runs as
//! 0: Sol, 26/10).

use pdfcraft_cos::{Document, Object, PdfString};

/// The JavaScript for notation `expr`: each field name read as a number.
pub fn notation_js(expr: &str) -> String {
    let mut js = String::new();
    let mut chars = expr.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_alphabetic() || c == '_' {
            let mut name = String::new();
            // A name may hold dots (parent.child) and backslash-escaped characters (spaces).
            while let Some(&c) = chars.peek() {
                if c == '\\' {
                    chars.next();
                    if let Some(e) = chars.next() {
                        name.push(e);
                    }
                } else if c.is_alphanumeric() || matches!(c, '_' | '.') {
                    name.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            js.push_str(&format!("AFMakeNumber(getField(\"{}\").value)", name.replace('\\', "\\\\").replace('"', "\\\"")));
        } else {
            js.push(c);
            chars.next();
        }
    }
    format!("/** BVCALC {expr} EVCALC **/ event.value = {js};")
}

/// Field `name`'s calculation script set to [`notation_js`] of `expr`.
pub fn write_notation(doc: &mut Document, name: &str, expr: &str) -> Result<(), pdfcraft_forms::FormError> {
    let all = pdfcraft_forms::fields(doc);
    let f = all.iter().find(|f| f.name == name).ok_or_else(|| pdfcraft_forms::FormError::NoSuchField(name.into()))?;
    let aa = doc.get(f.obj).as_dict().and_then(|d| d.get(b"AA").cloned());
    let Some(aa) = aa else { return Ok(()) };
    let aa_dict = doc.resolve(&aa).as_dict().cloned();
    let Some(mut aa_dict) = aa_dict else { return Ok(()) };
    let Some(c) = aa_dict.get(b"C").cloned() else { return Ok(()) };
    let js = Object::String(PdfString::text(&notation_js(expr)));
    match c {
        Object::Ref(r) => {
            doc.update_dict(r, |d| d.set(b"JS".to_vec(), js)).map_err(|e| pdfcraft_forms::FormError::Invalid(e.to_string()))?;
        }
        Object::Dict(mut d) => {
            d.set(b"JS".to_vec(), js);
            aa_dict.set(b"C".to_vec(), Object::Dict(d));
            match aa {
                Object::Ref(r) => doc.set(r, Object::Dict(aa_dict)),
                _ => doc.update_dict(f.obj, |d| d.set(b"AA".to_vec(), Object::Dict(aa_dict))).map_err(|e| pdfcraft_forms::FormError::Invalid(e.to_string()))?,
            }
        }
        _ => {}
    }
    Ok(())
}

