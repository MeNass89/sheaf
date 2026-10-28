//! Sheaf: the stacking order of objects picked with the Edit tool (Acrobat's Arrange: Bring to
//! Front, Bring Forward, Send Backward, Send to Back). An object's painting operators leave their
//! place and are drawn again elsewhere in the page's content, inside a `q … Q` of their own that
//! sets the graphics state they were drawn with (clip, matrix, colours, line style, text state).
//! A paragraph's line moves stay where they were, so the text after it keeps its place. Added
//! items (each its own content stream) move as whole streams.
//!
//! To the front, the page's content is wrapped in `q … Q` and the objects follow it; to the back,
//! they come first. One step forward (backward) puts the object just above (below) the nearest
//! object it overlaps; when the content there is clipped in a way the object was not, that object
//! is moved below (above) it instead.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use pdfcraft_content::{Matrix, Op, Pieces, num, serialize_ops};
use pdfcraft_cos::{Dict, Document, ObjRef, Object, Stream};

use crate::EditError;
use crate::objects::{ObjectKind, ObjectTarget, editable_objects};

/// Where Arrange puts the objects picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restack {
    /// Above everything on the page.
    Front,
    /// Just above the nearest object above that it overlaps.
    Forward,
    /// Just below the nearest object below that it overlaps.
    Backward,
    /// Below everything on the page.
    Back,
}

const PAINT: [&[u8]; 9] = [b"S", b"s", b"f", b"F", b"f*", b"B", b"B*", b"b", b"b*"];
const SHOW: [&[u8]; 4] = [b"Tj", b"TJ", b"'", b"\""];

/// The graphics state parameters set by one operator each, with the value an object drawn
/// elsewhere starts from when its own content never set them (ISO 32000-2 Table 51, 103).
const PARAMS: [&str; 13] = ["w", "J", "j", "M", "d", "ri", "i", "Tc", "Tw", "Tz", "TL", "Tr", "Ts"];

fn default_param(key: &str) -> Op {
    match key {
        "w" => Op::new("w", vec![num(1.0)]),
        "M" => Op::new("M", vec![num(10.0)]),
        "d" => Op::new("d", vec![Object::Array(Vec::new()), num(0.0)]),
        "ri" => Op::new("ri", vec![Object::name("RelativeColorimetric")]),
        "Tz" => Op::new("Tz", vec![num(100.0)]),
        k => Op::new(k, vec![num(0.0)]),
    }
}

/// The graphics state at one point of the content.
/// Shared (`Rc`) so the state kept before every operator costs a pointer, not a copy.
#[derive(Clone, Debug, Default, PartialEq)]
struct Gs {
    ctm: Matrix,
    /// Clipping paths in force: the matrix each was set in, and its operators (path, `W`, `n`).
    clips: Rc<Vec<(Matrix, Vec<Op>)>>,
    /// The last operator that set each of [`PARAMS`], and `Tf`, with its place in the content (so
    /// it is set again in the order it was set among the `gs` operators, which set parameters too).
    params: Rc<BTreeMap<&'static str, (usize, Op)>>,
    /// Stroke and fill colour: a colour space operator and the colour, or one operator for both.
    stroke: Rc<Vec<Op>>,
    fill: Rc<Vec<Op>>,
    /// `gs` operators in force, in order (each sets some parameters of its own), with their places.
    ext: Rc<Vec<(usize, Op)>>,
}

impl Gs {
    /// The text font size times the horizontal scaling: what a `TJ` number is a thousandth of.
    fn text_unit(&self) -> f64 {
        let size = self.params.get("Tf").and_then(|o| o.1.num(1)).unwrap_or(0.0);
        let scale = self.params.get("Tz").and_then(|o| o.1.num(0)).unwrap_or(100.0) / 100.0;
        size * scale
    }
}

/// The page's content read once: the state before each operator, and where text objects are.
struct Walk {
    before: Vec<Gs>,
    end: Gs,
    /// Whether each operator is inside a text object (`BT … ET`).
    in_text: Vec<bool>,
    /// `q` left open at the end, and whether a `Q` ever had no `q` to close.
    depth: usize,
    underflow: bool,
}

fn walk(ops: &[Op]) -> Walk {
    let mut gs = Gs::default();
    let mut stack: Vec<Gs> = Vec::new();
    let mut path: Vec<Op> = Vec::new();
    let mut clip: Option<Op> = None;
    let mut text = false;
    let mut underflow = false;
    let mut before = Vec::with_capacity(ops.len());
    let mut in_text = Vec::with_capacity(ops.len());
    for (at, op) in ops.iter().enumerate() {
        before.push(gs.clone());
        in_text.push(text);
        let name = op.op.as_slice();
        match name {
            b"q" => stack.push(gs.clone()),
            b"Q" => match stack.pop() {
                Some(g) => gs = g,
                None => underflow = true,
            },
            b"cm" => {
                if let Some(m) = op.nums::<6>() {
                    gs.ctm = Matrix(m).then(&gs.ctm);
                }
            }
            b"BT" => text = true,
            b"ET" => text = false,
            b"m" | b"l" | b"c" | b"v" | b"y" | b"re" | b"h" => path.push(op.clone()),
            b"W" | b"W*" => clip = Some(op.clone()),
            _ if name == b"n" || PAINT.contains(&name) => {
                let built = std::mem::take(&mut path);
                if let Some(w) = clip.take() {
                    let mut c = built;
                    c.push(w);
                    c.push(Op::new("n", vec![]));
                    let ctm = gs.ctm;
                    Rc::make_mut(&mut gs.clips).push((ctm, c));
                }
            }
            b"G" | b"RG" | b"K" | b"CS" => gs.stroke = Rc::new(vec![op.clone()]),
            b"SC" | b"SCN" => {
                let stroke = Rc::make_mut(&mut gs.stroke);
                let keep = usize::from(stroke.first().is_some_and(|o| o.is("CS")));
                stroke.truncate(keep);
                stroke.push(op.clone());
            }
            b"g" | b"rg" | b"k" | b"cs" => gs.fill = Rc::new(vec![op.clone()]),
            b"sc" | b"scn" => {
                let fill = Rc::make_mut(&mut gs.fill);
                let keep = usize::from(fill.first().is_some_and(|o| o.is("cs")));
                fill.truncate(keep);
                fill.push(op.clone());
            }
            b"gs" => Rc::make_mut(&mut gs.ext).push((at, op.clone())),
            b"Tf" => {
                Rc::make_mut(&mut gs.params).insert("Tf", (at, op.clone()));
            }
            b"\"" => {
                if let (Some(aw), Some(ac)) = (op.operands.first(), op.operands.get(1)) {
                    let params = Rc::make_mut(&mut gs.params);
                    params.insert("Tw", (at, Op::new("Tw", vec![aw.clone()])));
                    params.insert("Tc", (at, Op::new("Tc", vec![ac.clone()])));
                }
            }
            _ => {
                if let Some(key) = PARAMS.iter().find(|k| k.as_bytes() == name) {
                    Rc::make_mut(&mut gs.params).insert(*key, (at, op.clone()));
                }
            }
        }
    }
    Walk { before, end: gs, in_text, depth: stack.len(), underflow }
}

fn cm(m: Matrix) -> Option<Op> {
    let identity = m.0.iter().zip(Matrix::IDENTITY.0).all(|(a, b)| (a - b).abs() < 1e-9);
    (!identity).then(|| Op::new("cm", m.0.iter().map(|v| num(*v)).collect()))
}

/// The operators that take the state `at` (where the object is drawn again) to `target` (the
/// state it was drawn in), opening a `q`; `None` when `at` is clipped (or has a `gs`) that the
/// object was not drawn under, which no operator can undo.
fn setup(target: &Gs, at: &Gs) -> Option<Vec<Op>> {
    if !target.clips.starts_with(&at.clips) || !target.ext.starts_with(&at.ext) {
        return None;
    }
    let mut out = vec![Op::new("q", vec![])];
    let mut cur = at.ctm;
    for (m, ops) in &target.clips[at.clips.len()..] {
        out.extend(cm(m.then(&cur.invert()?)));
        out.extend(ops.iter().cloned());
        cur = *m;
    }
    out.extend(cm(target.ctm.then(&cur.invert()?)));
    if target.stroke.is_empty() { out.push(Op::new("G", vec![num(0.0)])) } else { out.extend(target.stroke.iter().cloned()) }
    if target.fill.is_empty() { out.push(Op::new("g", vec![num(0.0)])) } else { out.extend(target.fill.iter().cloned()) }
    // The parameters and the `gs` operators in the order the content set them: a `gs` setting the
    // line width before an explicit `5 w` must not override it (Sol, 27/10). Parameters never
    // set come first, at their defaults. A `gs` already in force is set again (the same values).
    let mut set: Vec<(usize, Op)> = PARAMS.iter().map(|k| target.params.get(k).cloned().unwrap_or_else(|| (0, default_param(k)))).collect();
    set.extend(target.params.get("Tf").cloned());
    set.extend(target.ext.iter().cloned());
    set.sort_by_key(|(at, _)| *at);
    out.extend(set.into_iter().map(|(_, op)| op));
    Some(out)
}

/// One run of an object's operators drawn again: the state before operator `at`, and the body.
#[derive(Clone, Debug)]
struct Part {
    at: usize,
    body: Vec<Op>,
}

/// An object of the page as Arrange sees it.
#[derive(Clone, Debug)]
struct Item {
    target: ObjectTarget,
    rect: [f64; 4],
    /// Its first and last operator (joined content).
    first: usize,
    last: usize,
    /// What it draws, or the added item's content stream (an index into the page's streams).
    parts: Vec<Part>,
    stream: Option<usize>,
    /// Operators taken out of their place, and those put there instead (a `'` leaves its `T*`).
    remove: BTreeSet<usize>,
    replace: BTreeMap<usize, Vec<Op>>,
}

/// The page content and every object Arrange can move.
struct Page {
    obj: ObjRef,
    streams: Vec<(Object, Vec<u8>)>,
    pieces: Pieces,
    ops: Vec<Op>,
    /// The piece each operator is in.
    piece: Vec<usize>,
    added: BTreeSet<usize>,
    items: Vec<Item>,
    walk: Walk,
}

fn invalid(s: &str) -> EditError {
    EditError::Invalid(s.into())
}

fn read_page(doc: &Document, page: usize) -> Result<Page, EditError> {
    let p = pdfcraft_model::pages(doc).into_iter().nth(page).ok_or(EditError::NoSuchPage(page))?;
    let streams = crate::images::page_streams(doc, &p.dict);
    let pieces = Pieces::join(&streams.iter().map(|(_, d)| d.as_slice()).collect::<Vec<_>>());
    let ops = pieces.parse().ops;
    // Drawings and images are found stream by stream: their operator `i` of stream `s` is the
    // joined operator `offset[s] + i`, as long as no operator runs across two streams.
    let mut piece = Vec::with_capacity(ops.len());
    let mut offset = vec![0usize; streams.len() + 1];
    for op in &ops {
        let (a, b) = pieces.pieces_of(op);
        if a != b {
            return Err(invalid("this page's content can't be rearranged (an operator runs across two streams)"));
        }
        piece.push(a);
    }
    for (s, (_, data)) in streams.iter().enumerate() {
        let count = pdfcraft_content::parse(data).ops.len();
        if piece.iter().filter(|p| **p == s).count() != count {
            return Err(invalid("this page's content can't be rearranged (its streams don't split between operators)"));
        }
        offset[s + 1] = offset[s] + count;
    }
    let added_items: Vec<_> = crate::list_added(doc).into_iter().filter(|a| a.page == page).collect();
    let excluded: BTreeSet<ObjRef> = added_items.iter().map(|a| a.obj).collect();
    let stream_of = |r: ObjRef| streams.iter().position(|(o, _)| o.as_ref() == Some(r));
    let added: BTreeSet<usize> = added_items.iter().filter_map(|a| stream_of(a.obj)).collect();
    let inventory = editable_objects(doc, page)?;
    let paths = crate::paths::page_paths(doc, page, &excluded)?;
    let images: BTreeMap<usize, crate::images::PageImage> = crate::images::selectable_images(doc, page, &excluded)?.into_iter().collect();
    let blocks: BTreeMap<usize, crate::text::TextBlock> = crate::text::selectable_blocks(doc, page, &excluded)?.into_iter().collect();
    let lines = if blocks.is_empty() { Vec::new() } else { crate::text::text_lines(doc, page)? };
    let walk = walk(&ops);
    let mut items = Vec::new();
    for o in inventory {
        let t = o.target;
        let mut item = Item { target: t, rect: o.rect, first: 0, last: 0, parts: Vec::new(), stream: None, remove: BTreeSet::new(), replace: BTreeMap::new() };
        match t.kind {
            ObjectKind::Path => {
                let Some(p) = paths.get(t.index) else { continue };
                let (a, b) = (offset[p.stream] + p.start, offset[p.stream] + p.end);
                item.parts.push(Part { at: a, body: ops[a..=b].to_vec() });
                item.remove.extend(a..=b);
                (item.first, item.last) = (a, b);
            }
            ObjectKind::Image => {
                let Some(image) = images.get(&t.index) else { continue };
                let (s, i) = image.location();
                let a = offset[s] + i;
                item.parts.push(Part { at: a, body: vec![ops[a].clone()] });
                item.remove.insert(a);
                (item.first, item.last) = (a, a);
            }
            ObjectKind::Text => {
                let Some(block) = blocks.get(&t.index) else { continue };
                let mut shown = BTreeSet::new();
                for line in block.lines.iter().filter_map(|i| lines.get(*i)) {
                    let mine: BTreeSet<usize> = line.op_indexes().iter().copied().filter(|i| ops.get(*i).is_some_and(|o| SHOW.contains(&o.op.as_slice()))).collect();
                    let (Some(&a), Some(&b)) = (mine.first(), mine.last()) else { continue };
                    let unit = walk.before[a].text_unit();
                    item.parts.push(Part { at: a, body: text_body(&ops, (a, b), &mine, (line.line_matrix(), line.text_matrix()), unit)? });
                    shown.extend(mine);
                }
                let (Some(&a), Some(&b)) = (shown.first(), shown.last()) else { continue };
                for &i in &shown {
                    // `'` and `"` move to the next line before showing: the move stays.
                    match ops[i].op.as_slice() {
                        b"'" => {
                            item.replace.insert(i, vec![Op::new("T*", vec![])]);
                        }
                        b"\"" => {
                            let o = &ops[i];
                            let (aw, ac) = (o.operands.first().cloned().unwrap_or(num(0.0)), o.operands.get(1).cloned().unwrap_or(num(0.0)));
                            item.replace.insert(i, vec![Op::new("Tw", vec![aw]), Op::new("Tc", vec![ac]), Op::new("T*", vec![])]);
                        }
                        _ => {}
                    }
                }
                item.remove = shown;
                (item.first, item.last) = (a, b);
            }
            ObjectKind::Added => {
                let Some(s) = added_items.get(t.index).and_then(|a| stream_of(a.obj)) else { continue };
                if offset[s] == offset[s + 1] {
                    continue;
                }
                item.stream = Some(s);
                (item.first, item.last) = (offset[s], offset[s + 1] - 1);
            }
        }
        items.push(item);
    }
    Ok(Page { obj: p.obj, streams, pieces, ops, piece, added, items, walk })
}

/// A line of text drawn again: its own text object, starting where the line started, with the
/// operators from its first run to its last that show it, move the line or set the text state.
/// Runs of other paragraphs in between are left out (their line moves kept). `(tlm, tm)`: the line
/// and text matrices at its first run; `unit`: what a `TJ` number is a thousandth of there.
fn text_body(ops: &[Op], (a, b): (usize, usize), mine: &BTreeSet<usize>, (tlm, tm): (Matrix, Matrix), unit: f64) -> Result<Vec<Op>, EditError> {
    let mut body = vec![Op::new("BT", vec![])];
    // The run starts `dx` along the line from where the line starts (after earlier runs): the
    // line's start is set and a `TJ` number moves along it, so later `Td`s start from the line.
    let rel = tlm.invert().map(|inv| tm.then(&inv)).ok_or_else(|| invalid("a line of text has no area"))?;
    let (dx, dy) = (rel.0[4], rel.0[5]);
    let along = dx.abs() > 1e-6;
    let shift = along && dy.abs() < 1e-6 && unit.abs() > 1e-9;
    body.push(Op::new("Tm", if along && !shift { tm } else { tlm }.0.iter().map(|v| num(*v)).collect()));
    let mut shift_left = shift.then_some(-dx * 1000.0 / unit);
    for (i, op) in ops.iter().enumerate().take(b + 1).skip(a) {
        let name = op.op.as_slice();
        let keep = match name {
            _ if SHOW.contains(&name) => mine.contains(&i),
            b"Td" | b"TD" | b"T*" | b"Tm" | b"Tc" | b"Tw" | b"Tz" | b"TL" | b"Tf" | b"Tr" | b"Ts" | b"gs" => true,
            b"g" | b"rg" | b"k" | b"cs" | b"sc" | b"scn" | b"G" | b"RG" | b"K" | b"CS" | b"SC" | b"SCN" => true,
            _ => false,
        };
        if !keep {
            if name == b"'" {
                body.push(Op::new("T*", vec![]));
            } else if name == b"\"" {
                body.push(Op::new("Tw", vec![op.operands.first().cloned().unwrap_or(num(0.0))]));
                body.push(Op::new("Tc", vec![op.operands.get(1).cloned().unwrap_or(num(0.0))]));
                body.push(Op::new("T*", vec![]));
            }
            continue;
        }
        if i == a {
            // The first run: shown where it was, after the line's start (a number in `TJ` moves
            // along the line without moving the line's start, which later `Td`s are from).
            if let Some(n) = shift_left.take() {
                body.push(Op::new("TJ", vec![Object::Array(vec![Object::Real(n)])]));
            }
            match name {
                b"'" => body.push(Op::new("Tj", op.operands.last().cloned().into_iter().collect())),
                b"\"" => {
                    body.push(Op::new("Tw", vec![op.operands.first().cloned().unwrap_or(num(0.0))]));
                    body.push(Op::new("Tc", vec![op.operands.get(1).cloned().unwrap_or(num(0.0))]));
                    body.push(Op::new("Tj", op.operands.last().cloned().into_iter().collect()));
                }
                _ => body.push(op.clone()),
            }
            continue;
        }
        body.push(op.clone());
    }
    body.push(Op::new("ET", vec![]));
    Ok(body)
}

/// Where objects go: before joined operator `k` (outside any text object), or after everything.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    Before(usize),
    End,
}

/// The page's content rebuilt: operators removed and inserted, streams added and moved.
#[derive(Default)]
struct Plan {
    remove: BTreeSet<usize>,
    replace: BTreeMap<usize, Vec<Op>>,
    /// Operators inserted before a joined operator (in a stream that is not an added item's).
    insert: BTreeMap<usize, Vec<Op>>,
    /// Streams put before piece `i` (`usize::MAX`: after the last), in order: new content, or a
    /// piece of the page moved there.
    streams_before: BTreeMap<usize, Vec<Chunk>>,
    moved: BTreeSet<usize>,
}

enum Chunk {
    New(Vec<Op>),
    Piece(usize),
}

impl Page {
    /// Put `chunk` at `place`: inserted operators, or a stream of its own where an added item's
    /// stream starts (its stream must stay whole) or at the end. `None` when a moved stream would
    /// have to split a stream.
    fn put(&self, plan: &mut Plan, place: Place, chunk: Chunk) -> Option<()> {
        let boundary = |k: usize| k == 0 || self.piece[k - 1] != self.piece[k];
        match place {
            Place::End => plan.streams_before.entry(usize::MAX).or_default().push(chunk),
            Place::Before(k) if self.added.contains(&self.piece[k]) || matches!(chunk, Chunk::Piece(_)) => {
                if !boundary(k) {
                    return None;
                }
                plan.streams_before.entry(self.piece[k]).or_default().push(chunk);
            }
            Place::Before(k) => {
                let Chunk::New(ops) = chunk else { return None };
                plan.insert.entry(k).or_default().extend(ops);
            }
        }
        Some(())
    }

    /// `item` drawn again where the state is `at`: its parts, each in a `q … Q` of its own.
    fn unit(&self, item: &Item, at: &Gs) -> Option<Chunk> {
        if let Some(s) = item.stream {
            return Some(Chunk::Piece(s));
        }
        let mut out = Vec::new();
        for part in &item.parts {
            out.extend(setup(self.walk.before.get(part.at)?, at)?);
            out.extend(part.body.iter().cloned());
            out.push(Op::new("Q", vec![]));
        }
        Some(Chunk::New(out))
    }

    fn take_out(&self, plan: &mut Plan, item: &Item) {
        plan.remove.extend(item.remove.iter().copied());
        plan.replace.extend(item.replace.iter().map(|(k, v)| (*k, v.clone())));
        if let Some(s) = item.stream {
            plan.moved.insert(s);
        }
    }

    fn write(&self, doc: &mut Document, mut plan: Plan) -> Result<(), EditError> {
        let data = crate::text::splice(&self.pieces, &self.ops, |i| {
            let mut insert = plan.insert.remove(&i).unwrap_or_default();
            if let Some(r) = plan.replace.get(&i) {
                insert.extend(r.iter().cloned());
            }
            (insert, !plan.remove.contains(&i))
        });
        let mut written: Vec<Object> = Vec::with_capacity(self.streams.len());
        for ((obj, old), new) in self.streams.iter().zip(&data) {
            if old == new {
                written.push(obj.clone());
                continue;
            }
            let mut dict = match &*doc.resolve(obj) {
                Object::Stream(s) => s.dict.clone(),
                _ => Dict::new(),
            };
            dict.remove(b"Length");
            written.push(Object::Ref(doc.add(Object::Stream(Stream::flate(dict, new)))));
        }
        let chunk = |doc: &mut Document, c: &Chunk| match c {
            Chunk::New(ops) => Object::Ref(doc.add(Object::Stream(Stream::flate(Dict::new(), &serialize_ops(ops))))),
            Chunk::Piece(s) => written[*s].clone(),
        };
        let mut contents = Vec::new();
        for i in 0..self.streams.len() {
            for c in plan.streams_before.get(&i).into_iter().flatten() {
                contents.push(chunk(doc, c));
            }
            if !plan.moved.contains(&i) {
                contents.push(written[i].clone());
            }
        }
        for c in plan.streams_before.get(&usize::MAX).into_iter().flatten() {
            contents.push(chunk(doc, c));
        }
        doc.update_dict(self.obj, |d| d.set(b"Contents".to_vec(), Object::Array(contents)))?;
        Ok(())
    }

    /// The place before operator `k`, moved out of a text object: back to its `BT` (`back`) or on
    /// past its `ET`.
    fn outside_text(&self, k: usize, back: bool) -> Place {
        let inside = |k: usize| self.walk.in_text.get(k).copied().unwrap_or(false);
        let mut k = k;
        if back {
            while k > 0 && inside(k) {
                k -= 1;
            }
        } else {
            while k < self.ops.len() && inside(k) {
                k += 1;
            }
        }
        if k >= self.ops.len() { Place::End } else { Place::Before(k) }
    }

    /// The graphics state at `place`.
    fn state(&self, place: Place) -> &Gs {
        match place {
            Place::Before(k) => &self.walk.before[k],
            Place::End => &self.walk.end,
        }
    }
}

fn overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0].max(b[0]) < a[2].min(b[2]) && a[1].max(b[1]) < a[3].min(b[3])
}

/// Sheaf: Arrange the objects picked with the Edit tool (`targets`, from [`editable_objects`]):
/// to the front or back of the page, or one step forward or backward past the nearest object
/// they overlap (Acrobat's Arrange). One step; a refusal leaves the page as it was.
pub fn restack_objects(doc: &mut Document, page: usize, targets: &[ObjectTarget], how: Restack) -> Result<(), EditError> {
    if targets.is_empty() || targets.len() > crate::objects::MAX_MOVE_OBJECTS {
        return Err(invalid("select between 1 and 1000 objects to arrange"));
    }
    let gone = || invalid("a selected object no longer exists; select the objects again");
    let mut work = doc.clone();
    match how {
        Restack::Front | Restack::Back => {
            let pg = read_page(&work, page)?;
            let mut chosen: Vec<&Item> = targets.iter().map(|t| pg.items.iter().find(|i| i.target == *t).ok_or_else(gone)).collect::<Result<_, _>>()?;
            chosen.sort_by_key(|i| i.first);
            chosen.dedup_by_key(|i| i.target);
            let start = Gs::default();
            let mut plan = Plan::default();
            let mut units = Vec::new();
            for item in &chosen {
                pg.take_out(&mut plan, item);
                units.push(pg.unit(item, &start).ok_or_else(|| invalid("an object is drawn in a space that can't be undone"))?);
            }
            if how == Restack::Front {
                if pg.walk.underflow {
                    return Err(invalid("this page's content closes more states than it opens; it can't be rearranged"));
                }
                // The page's content in a state of its own, closed with what it left open.
                plan.streams_before.entry(0).or_default().push(Chunk::New(vec![Op::new("q", vec![])]));
                plan.streams_before.entry(usize::MAX).or_default().push(Chunk::New(vec![Op::new("Q", vec![]); pg.walk.depth + 1]));
                plan.streams_before.entry(usize::MAX).or_default().extend(units);
            } else {
                plan.streams_before.entry(0).or_default().extend(units);
            }
            pg.write(&mut work, plan)?;
        }
        Restack::Forward | Restack::Backward => {
            // The objects picked, as they are (no finding again by place: two objects of the same
            // kind and size at the same place are told apart; Sol, 27/10), move together past the
            // nearest object one of them overlaps, keeping their own order.
            let pg = read_page(&work, page)?;
            let mut chosen: Vec<&Item> = targets.iter().map(|t| pg.items.iter().find(|i| i.target == *t).ok_or_else(gone)).collect::<Result<_, _>>()?;
            chosen.sort_by_key(|i| i.first);
            chosen.dedup_by_key(|i| i.target);
            let picked = |o: &Item| chosen.iter().any(|c| c.target == o.target);
            let forward = how == Restack::Forward;
            // The nearest object above (forward) or below one picked object that it overlaps.
            let next = pg
                .items
                .iter()
                .filter(|o| !picked(o))
                .filter(|o| chosen.iter().any(|c| overlap(o.rect, c.rect) && if forward { o.first > c.last } else { o.last < c.first }));
            let next = if forward { next.min_by_key(|o| o.first) } else { next.max_by_key(|o| o.last) };
            let Some(next) = next else {
                return Err(invalid(if forward { "nothing above the selection overlaps it" } else { "nothing below the selection overlaps it" }));
            };
            // The picked objects on the other side of it: those that move.
            let moving: Vec<&Item> = chosen.iter().copied().filter(|c| if forward { c.last < next.first } else { c.first > next.last }).collect();
            let (low, high) = (moving.iter().map(|c| c.first).min().unwrap_or(next.first), moving.iter().map(|c| c.last).max().unwrap_or(next.last));
            // They go past their neighbour, or else the neighbour goes past them.
            let tries: [(Vec<&Item>, Place); 2] = if forward {
                [(moving, pg.outside_text(next.last + 1, false)), (vec![next], pg.outside_text(low, true))]
            } else {
                [(moving, pg.outside_text(next.first, true)), (vec![next], pg.outside_text(high + 1, false))]
            };
            let mut done = false;
            'tries: for (who, place) in tries {
                let mut plan = Plan::default();
                for item in &who {
                    pg.take_out(&mut plan, item);
                    let Some(unit) = pg.unit(item, pg.state(place)) else { continue 'tries };
                    if pg.put(&mut plan, place, unit).is_none() {
                        continue 'tries;
                    }
                }
                pg.write(&mut work, plan)?;
                done = true;
                break;
            }
            if !done {
                return Err(invalid("these objects and the one they overlap are drawn in different clipped groups; they can't change places"));
            }
        }
    }
    // The page must still read: the objects found again.
    editable_objects(&work, page)?;
    *doc = work;
    Ok(())
}
