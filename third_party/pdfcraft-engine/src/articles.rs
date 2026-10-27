//! Sheaf: article threads (Acrobat's Article tool): boxes drawn on pages and chained into one
//! reading order across columns and pages (ISO 32000-2 §12.4.3). The catalog's `/Threads` lists
//! the threads; each is a ring of beads (`/F` its first; a bead's `/T` its thread, `/N` the next,
//! `/V` the previous, `/P` its page, `/R` its box), and each page lists its beads in `/B`. Readers
//! that follow articles (Acrobat, Preview) go from one box to the next.

use pdfcraft_cos::{Dict, Document, ObjRef, Object, PdfString};

use crate::EditError;

/// The most beads read in one thread (a broken ring is not followed for ever).
const MAX_BEADS: usize = 10_000;

/// A thread as read: its title and its boxes in reading order (page, 0-based; box in user space).
#[derive(Clone, Debug, PartialEq)]
pub struct Article {
    pub title: String,
    pub beads: Vec<(usize, [f64; 4])>,
}

fn write(e: impl std::fmt::Display) -> EditError {
    EditError::Write(e.to_string())
}

fn rect_of(o: Option<&Object>) -> Option<[f64; 4]> {
    let v: Vec<f64> = o?.as_array()?.iter().filter_map(Object::as_f64).collect();
    let r = <[f64; 4]>::try_from(v).ok()?;
    Some([r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])])
}

/// The catalog's `/Threads` (references to the threads), and the catalog.
fn threads(doc: &Document) -> (Option<ObjRef>, Vec<Object>) {
    let Some(root) = doc.root() else { return (None, Vec::new()) };
    let list = doc.get(root).as_dict().and_then(|d| d.get(b"Threads").map(|t| doc.resolve(t).as_array().cloned().unwrap_or_default())).unwrap_or_default();
    (Some(root), list)
}

/// A thread's beads in order, from its first.
fn beads(doc: &Document, thread: &Object) -> Vec<ObjRef> {
    let mut out = Vec::new();
    let first = doc.resolve(thread).as_dict().and_then(|t| t.reference(b"F"));
    let mut at = first;
    while let Some(b) = at {
        if out.contains(&b) || out.len() >= MAX_BEADS {
            break;
        }
        out.push(b);
        at = doc.get(b).as_dict().and_then(|d| d.reference(b"N"));
    }
    out
}

/// Every article thread of the document, in the catalog's order.
pub fn articles(doc: &Document) -> Vec<Article> {
    let pages = pdfcraft_annot::page_refs(doc).unwrap_or_default();
    threads(doc)
        .1
        .iter()
        .map(|t| {
            let info = doc.resolve(t).as_dict().and_then(|d| d.get(b"I").map(|i| doc.resolve(i).as_dict().cloned().unwrap_or_default())).unwrap_or_default();
            let title = info.get(b"Title").and_then(Object::as_string).map(PdfString::to_text).unwrap_or_default();
            let beads = beads(doc, t)
                .into_iter()
                .filter_map(|b| {
                    let bead = doc.get(b);
                    let d = bead.as_dict()?;
                    let page = pages.iter().position(|p| Some(*p) == d.reference(b"P"))?;
                    Some((page, rect_of(d.get(b"R"))?))
                })
                .collect();
            Article { title, beads }
        })
        .collect()
}

/// Put `list` as `owner`'s `key`, into the array object when it is indirect; none left: removed.
fn set_list(doc: &mut Document, owner: ObjRef, key: &[u8], list: Vec<Object>) -> Result<(), EditError> {
    let existing = doc.get(owner).as_dict().and_then(|d| d.reference(key));
    match existing {
        Some(r) if doc.get(r).as_array().is_some() && !list.is_empty() => doc.set(r, Object::Array(list)),
        _ => doc
            .update_dict(owner, |d| {
                if list.is_empty() {
                    d.remove(key);
                } else {
                    d.set(key.to_vec(), Object::Array(list));
                }
            })
            .map_err(write)?,
    }
    Ok(())
}

fn list_of(doc: &Document, owner: ObjRef, key: &[u8]) -> Vec<Object> {
    doc.get(owner).as_dict().and_then(|d| d.get(key).map(|l| doc.resolve(l).as_array().cloned().unwrap_or_default())).unwrap_or_default()
}

/// A box `rect` (user space) on `page` (0-based) at the end of article `thread` (an index of
/// [`articles`]), or the first of a new article titled `title` (`None`), as Acrobat's Article tool
/// draws them: the box after the last one, in reading order.
pub fn add_article_box(doc: &mut Document, thread: Option<usize>, title: &str, page: usize, rect: [f64; 4]) -> Result<(), EditError> {
    let rect = [rect[0].min(rect[2]), rect[1].min(rect[3]), rect[0].max(rect[2]), rect[1].max(rect[3])];
    if !rect.iter().all(|v| v.is_finite()) || rect[2] - rect[0] < 4.0 || rect[3] - rect[1] < 4.0 {
        return Err(EditError::Invalid("an article box must be at least 4 points wide and high".into()));
    }
    let pages = pdfcraft_annot::page_refs(doc).map_err(|e| EditError::Invalid(e.to_string()))?;
    let p = *pages.get(page).ok_or_else(|| EditError::Invalid(format!("page {} does not exist", page + 1)))?;
    let (root, mut list) = threads(doc);
    let root = root.ok_or_else(|| EditError::Invalid("the document has no catalog".into()))?;
    let mut bead = Dict::new();
    bead.set(b"Type".to_vec(), Object::name("Bead"));
    bead.set(b"P".to_vec(), Object::Ref(p));
    bead.set(b"R".to_vec(), Object::Array(rect.iter().map(|v| Object::Real(*v)).collect()));
    let b = doc.add(Object::Dict(bead));
    match thread {
        None => {
            let mut info = Dict::new();
            info.set(b"Title".to_vec(), PdfString::text(title));
            let mut t = Dict::new();
            t.set(b"Type".to_vec(), Object::name("Thread"));
            t.set(b"F".to_vec(), Object::Ref(b));
            t.set(b"I".to_vec(), Object::Dict(info));
            let t = doc.add(Object::Dict(t));
            // A ring of one.
            doc.update_dict(b, |d| {
                d.set(b"T".to_vec(), Object::Ref(t));
                d.set(b"N".to_vec(), Object::Ref(b));
                d.set(b"V".to_vec(), Object::Ref(b));
            })
            .map_err(write)?;
            list.push(Object::Ref(t));
            set_list(doc, root, b"Threads", list)?;
        }
        Some(i) => {
            let t = list.get(i).and_then(Object::as_ref).ok_or_else(|| EditError::Invalid(format!("article {} does not exist", i + 1)))?;
            let ring = beads(doc, &Object::Ref(t));
            let (Some(&first), Some(&last)) = (ring.first(), ring.last()) else {
                return Err(EditError::Invalid(format!("article {} has no boxes", i + 1)));
            };
            // After the last box, before the first (the ring closes on it).
            doc.update_dict(b, |d| {
                d.set(b"T".to_vec(), Object::Ref(t));
                d.set(b"N".to_vec(), Object::Ref(first));
                d.set(b"V".to_vec(), Object::Ref(last));
            })
            .map_err(write)?;
            doc.update_dict(last, |d| d.set(b"N".to_vec(), Object::Ref(b))).map_err(write)?;
            doc.update_dict(first, |d| d.set(b"V".to_vec(), Object::Ref(b))).map_err(write)?;
        }
    }
    let mut on_page = list_of(doc, p, b"B");
    on_page.push(Object::Ref(b));
    set_list(doc, p, b"B", on_page)
}

/// Article `thread` (an index of [`articles`]) deleted: its boxes leave their pages.
pub fn delete_article(doc: &mut Document, thread: usize) -> Result<(), EditError> {
    let (root, mut list) = threads(doc);
    let root = root.ok_or_else(|| EditError::Invalid("the document has no catalog".into()))?;
    if thread >= list.len() {
        return Err(EditError::Invalid(format!("article {} does not exist", thread + 1)));
    }
    let t = list.remove(thread);
    let ring = beads(doc, &t);
    let mut pages: Vec<ObjRef> = Vec::new();
    for p in ring.iter().filter_map(|b| doc.get(*b).as_dict().and_then(|d| d.reference(b"P"))) {
        if !pages.contains(&p) {
            pages.push(p);
        }
    }
    for p in pages {
        let left: Vec<Object> = list_of(doc, p, b"B").into_iter().filter(|o| o.as_ref().is_none_or(|r| !ring.contains(&r))).collect();
        set_list(doc, p, b"B", left)?;
    }
    set_list(doc, root, b"Threads", list)
}

/// Bead `bead` of article `thread` (indexes of [`articles`]).
fn bead_at(doc: &Document, thread: usize, bead: usize) -> Result<(ObjRef, Vec<ObjRef>), EditError> {
    let (_, list) = threads(doc);
    let t = list.get(thread).ok_or_else(|| EditError::Invalid(format!("article {} does not exist", thread + 1)))?;
    let ring = beads(doc, t);
    let b = *ring.get(bead).ok_or_else(|| EditError::Invalid(format!("article {} has no box {}", thread + 1, bead + 1)))?;
    Ok((b, ring))
}

/// Box `bead` of article `thread` moved or resized to `rect` (user space, same page), as
/// Acrobat's Article tool drags a box or its corner.
pub fn set_article_box(doc: &mut Document, thread: usize, bead: usize, rect: [f64; 4]) -> Result<(), EditError> {
    let rect = [rect[0].min(rect[2]), rect[1].min(rect[3]), rect[0].max(rect[2]), rect[1].max(rect[3])];
    if !rect.iter().all(|v| v.is_finite()) || rect[2] - rect[0] < 4.0 || rect[3] - rect[1] < 4.0 {
        return Err(EditError::Invalid("an article box must be at least 4 points wide and high".into()));
    }
    let (b, _) = bead_at(doc, thread, bead)?;
    doc.update_dict(b, |d| d.set(b"R".to_vec(), Object::Array(rect.iter().map(|v| Object::Real(*v)).collect()))).map_err(write)
}

/// Box `bead` of article `thread` deleted, the boxes before and after it joined (Acrobat's
/// Delete on a box: the article goes on without it); its last box deletes the article.
pub fn delete_article_box(doc: &mut Document, thread: usize, bead: usize) -> Result<(), EditError> {
    let (b, ring) = bead_at(doc, thread, bead)?;
    if ring.len() == 1 {
        return delete_article(doc, thread);
    }
    let k = ring.iter().position(|r| *r == b).unwrap_or(0);
    let (prev, next) = (ring[(k + ring.len() - 1) % ring.len()], ring[(k + 1) % ring.len()]);
    doc.update_dict(prev, |d| d.set(b"N".to_vec(), Object::Ref(next))).map_err(write)?;
    doc.update_dict(next, |d| d.set(b"V".to_vec(), Object::Ref(prev))).map_err(write)?;
    let (_, list) = threads(doc);
    if let Some(t) = list.get(thread).and_then(Object::as_ref) {
        // The first box gone: the thread starts at the next (which carries its thread).
        if k == 0 {
            doc.update_dict(t, |d| d.set(b"F".to_vec(), Object::Ref(next))).map_err(write)?;
            doc.update_dict(next, |d| d.set(b"T".to_vec(), Object::Ref(t))).map_err(write)?;
        }
    }
    if let Some(p) = doc.get(b).as_dict().and_then(|d| d.reference(b"P")) {
        let left: Vec<Object> = list_of(doc, p, b"B").into_iter().filter(|o| o.as_ref() != Some(b)).collect();
        set_list(doc, p, b"B", left)?;
    }
    Ok(())
}

impl crate::Document {
    /// The document's article threads ([`articles`]).
    pub fn articles(&self) -> Vec<Article> {
        self.editor.as_ref().map(|e| articles(&e.cos)).unwrap_or_default()
    }
}
