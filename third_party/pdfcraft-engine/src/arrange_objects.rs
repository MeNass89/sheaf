//! Sheaf: objects picked with the Edit tool lined up or spaced evenly (Acrobat's Align Objects
//! and Distribute): each moved by its own offset, in one step. A move may number the page's
//! objects anew (paragraphs regroup), so each is found again by its kind and place before it moves.

use pdfcraft_cos::Document;
use pdfcraft_edit::{EditError, ObjectTarget, editable_objects, move_objects};

pub fn arrange_objects(doc: &mut Document, page: usize, moves: &[(ObjectTarget, [f64; 2])]) -> Result<(), EditError> {
    let gone = || EditError::Invalid("a selected object no longer exists; select the objects again".into());
    let start = editable_objects(doc, page)?;
    let todo: Vec<(ObjectTarget, [f64; 4], [f64; 2])> = moves
        .iter()
        .filter(|(_, d)| d[0].abs() > 1e-3 || d[1].abs() > 1e-3)
        .map(|(t, d)| start.iter().find(|o| o.target == *t).map(|o| (*t, o.rect, *d)).ok_or_else(gone))
        .collect::<Result<_, _>>()?;
    if todo.is_empty() {
        return Err(EditError::Invalid("the objects are arranged so already".into()));
    }
    let off = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    for (t, rect, d) in todo {
        let now = editable_objects(doc, page)?;
        let found = now.iter().filter(|o| o.target.kind == t.kind).min_by(|a, b| off(a.rect, rect).total_cmp(&off(b.rect, rect))).filter(|o| off(o.rect, rect) < 0.5).ok_or_else(gone)?;
        move_objects(doc, page, &[found.target], d)?;
    }
    Ok(())
}
