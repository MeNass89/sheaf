//! PDF to Excel: tables found in the page text, written as worksheets.
//!
//! Tables are found from PdfCraft's words (`Document::words`, one box per word), not from its
//! text blocks: a block can join neighbouring cells ("Qty Unit price", "Widget B 10 1,234.00"),
//! which is also why PdfCraft's own detector (private, visible through its HTML export, and
//! working on blocks) loses the header row and merges columns of a plain invoice.
//!
//! On each page, words make lines (they overlap vertically), and a line splits into segments
//! wherever two words are further apart than half the text height. A table is a run of at least
//! three such lines with two or more short segments each (six words at most, or a number); a
//! line with one short segment between two of them stays in (a row with missing cells). The
//! columns are the x ranges the segments cover; their cells must line up (same left edge, right
//! edge or centre) in three rows for one column and two for every other, and one must hold
//! mostly short cells (three words at most, or numbers). Two columns of prose fail, even in
//! short lines; tilted text (a picture of a screen) is never a table.
//!
//! The rest of the text (PdfCraft's blocks, less those inside a table: headings, paragraphs)
//! goes in column A, one line per row, with the tables in reading order: the page is cut at its
//! widest empty bands, at a column gutter first, then between rows.

use crate::export::{open_for_export, ExportOptions};
use crate::paths::atomic_write;
use crate::pdfcraft::render::PageInfo;
use crate::{pages::Input, OpdfError, Result};
use pdfcraft_export::Block;
use rust_xlsxwriter::{Format, Workbook, Worksheet};
use std::path::{Path, PathBuf};

/// Where the tables go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetLayout {
    /// Every page on one sheet: its text in column A and its tables, a blank row between parts.
    #[default]
    OneSheet,
    /// One sheet per table ("Page 2 table 1", with the page's text above and below it), and one
    /// per page without a table ("Page 3 text").
    SheetPerTable,
}

/// What [`to_excel`] wrote.
#[derive(Debug, Clone)]
pub struct ExcelReport {
    pub path: PathBuf,
    /// Tables found.
    pub tables: usize,
}

/// How far two edges may drift (points) and still line up.
const COL_TOL: f64 = 4.0;
/// A table cell has at most this many words (or is a number).
const MAX_CELL_WORDS: usize = 6;
/// A table has at least this many rows, and two columns that line up in this many rows.
const MIN_ROWS: usize = 3;

/// A box in displayed points, y up: [x0, y0, x1, y1].
type Rect = [f64; 4];

fn union(a: Rect, b: Rect) -> Rect {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

/// Words of a line that sit close together: one table cell, or a run of prose.
#[derive(Debug, Clone)]
struct Segment {
    text: String,
    rect: Rect,
    words: usize,
}

impl Segment {
    fn is_cell_like(&self) -> bool {
        self.words <= MAX_CELL_WORDS || parse_number(&self.text).is_some()
    }
}

/// A table: its box and its rows, every row as wide as the table.
#[derive(Debug, Clone, PartialEq)]
struct Table {
    rect: Rect,
    rows: Vec<Vec<String>>,
}

/// What a page gives, in reading order.
#[derive(Debug, PartialEq)]
enum Part {
    Text(Vec<String>),
    Table(Vec<Vec<String>>),
}

/// A word box in user space → displayed points, y up (as PdfCraft's `export_pages` does).
fn visible(info: &PageInfo, r: [f64; 4]) -> Rect {
    let a = info.user_to_view(r[0] as f32, r[1] as f32);
    let b = info.user_to_view(r[2] as f32, r[3] as f32);
    let h = f64::from(info.height);
    [f64::from(a[0].min(b[0])), h - f64::from(a[1].max(b[1])), f64::from(a[0].max(b[0])), h - f64::from(a[1].min(b[1]))]
}

/// The words of a page as lines, top to bottom, each split into segments left to right.
fn lines(words: &[(String, Rect)]) -> Vec<Vec<Segment>> {
    let mut order: Vec<usize> = (0..words.len()).filter(|&i| !words[i].0.trim().is_empty()).collect();
    order.sort_by(|&a, &b| words[b].1[3].total_cmp(&words[a].1[3]).then(words[a].1[0].total_cmp(&words[b].1[0])));
    // A word joins the line whose first word it overlaps by half the smaller height.
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for i in order {
        let r = words[i].1;
        let joins = rows.last().and_then(|l| l.first()).is_some_and(|&f| {
            let a = words[f].1;
            let overlap = a[3].min(r[3]) - a[1].max(r[1]);
            overlap > 0.5 * (a[3] - a[1]).min(r[3] - r[1])
        });
        match rows.last_mut() {
            Some(l) if joins => l.push(i),
            _ => rows.push(vec![i]),
        }
    }
    rows.into_iter()
        .map(|mut l| {
            l.sort_by(|&a, &b| words[a].1[0].total_cmp(&words[b].1[0]));
            let mut segs: Vec<Segment> = Vec::new();
            for i in l {
                let (text, r) = (&words[i].0, words[i].1);
                let near = segs.last().is_some_and(|s| r[0] - s.rect[2] <= 0.5 * (s.rect[3] - s.rect[1]).max(r[3] - r[1]));
                match segs.last_mut() {
                    Some(s) if near => {
                        s.text.push(' ');
                        s.text.push_str(text.trim());
                        s.rect = union(s.rect, r);
                        s.words += 1;
                    }
                    _ => segs.push(Segment { text: text.trim().to_string(), rect: r, words: 1 }),
                }
            }
            segs
        })
        .collect()
}

/// How many of `xs` lie within [`COL_TOL`] of one another, at most.
fn aligned(xs: &[f64]) -> usize {
    xs.iter().map(|x| xs.iter().filter(|y| (*y - x).abs() <= COL_TOL).count()).max().unwrap_or(0)
}

/// The table made of `run` (lines), if it is one.
fn table(run: &[Vec<Segment>]) -> Option<Table> {
    if run.len() < MIN_ROWS {
        return None;
    }
    // Columns: the x ranges the segments of the full rows cover.
    let mut spans: Vec<(f64, f64)> = run.iter().filter(|l| l.len() >= 2).flatten().map(|s| (s.rect[0], s.rect[2])).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut cols: Vec<(f64, f64)> = Vec::new();
    for (x0, x1) in spans {
        match cols.last_mut() {
            Some(c) if x0 <= c.1 + 1.0 => c.1 = c.1.max(x1),
            _ => cols.push((x0, x1)),
        }
    }
    if cols.len() < 2 {
        return None;
    }
    let column = |s: &Segment| {
        (0..cols.len()).max_by(|&a, &b| {
            let ov = |c: (f64, f64)| s.rect[2].min(c.1) - s.rect[0].max(c.0);
            ov(cols[a]).total_cmp(&ov(cols[b]))
        })
    };
    let mut rows = Vec::with_capacity(run.len());
    // Per column, the edges of its cells (left, right, centre), to check they line up.
    let mut edges: Vec<[Vec<f64>; 3]> = vec![Default::default(); cols.len()];
    let mut rect = run[0][0].rect;
    for line in run {
        let mut cells = vec![String::new(); cols.len()];
        let mut seen = vec![false; cols.len()];
        for s in line {
            let c = column(s)?;
            rect = union(rect, s.rect);
            if !cells[c].is_empty() {
                cells[c].push(' ');
            }
            cells[c].push_str(&s.text);
            if !seen[c] {
                seen[c] = true;
                edges[c][0].push(s.rect[0]);
                edges[c][1].push(s.rect[2]);
                edges[c][2].push((s.rect[0] + s.rect[2]) / 2.0);
            }
        }
        rows.push(cells);
    }
    // One column lines up in three rows, and every column in two (cells may be missing).
    let line_up: Vec<usize> = edges.iter().map(|e| e.iter().map(|xs| aligned(xs)).max().unwrap_or(0)).collect();
    let lined_up = line_up.iter().any(|n| *n >= MIN_ROWS) && line_up.iter().all(|n| *n >= 2);
    // Some column holds short cells (three words at most, or numbers) in two rows out of three:
    // two columns of wrapped prose, even in short lines, don't.
    let short = (0..cols.len()).any(|c| {
        let cells: Vec<&String> = rows.iter().map(|r: &Vec<String>| &r[c]).filter(|t| !t.is_empty()).collect();
        let n = cells.iter().filter(|t| t.split_whitespace().count() <= 3 || parse_number(t).is_some()).count();
        cells.len() >= MIN_ROWS && 3 * n >= 2 * cells.len()
    });
    (lined_up && short).then_some(Table { rect, rows })
}

/// The tables of a page's lines, top to bottom.
fn tables(lines: &[Vec<Segment>]) -> Vec<Table> {
    let full = |l: &Vec<Segment>| l.len() >= 2 && l.iter().all(Segment::is_cell_like);
    let height = |l: &Vec<Segment>| l.iter().map(|s| s.rect[3] - s.rect[1]).fold(0.0, f64::max);
    let mut member: Vec<bool> = lines.iter().map(full).collect();
    for k in 1..lines.len().saturating_sub(1) {
        if !member[k] && lines[k].len() == 1 && lines[k][0].is_cell_like() && full(&lines[k - 1]) && full(&lines[k + 1]) {
            member[k] = true;
        }
    }
    let mut out = Vec::new();
    let mut k = 0;
    while k < lines.len() {
        if !member[k] {
            k += 1;
            continue;
        }
        let start = k;
        k += 1;
        // A run ends at a non-member line, or at a gap of more than three line heights.
        while k < lines.len() && member[k] {
            let (above, here) = (&lines[k - 1], &lines[k]);
            let gap = above.iter().map(|s| s.rect[1]).fold(f64::MAX, f64::min) - here.iter().map(|s| s.rect[3]).fold(f64::MIN, f64::max);
            if gap > 3.0 * height(above).max(height(here)) {
                break;
            }
            k += 1;
        }
        out.extend(table(&lines[start..k]));
    }
    out
}

/// Reading order of `rects`: cut at the widest vertical gutter (left part first) when there is
/// one at least 1.5 text heights wide with a column of two or more stacked boxes on each side,
/// else at the widest horizontal gap (top part first), and recurse; boxes with no gap between
/// them go top to bottom, then left to right.
fn reading_order(rects: &[Rect], size: f64) -> Vec<usize> {
    fn widest_gap(mut spans: Vec<(f64, f64)>) -> Option<(f64, f64)> {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut end = spans.first()?.1;
        let mut best: Option<(f64, f64)> = None;
        for (a, b) in spans.into_iter().skip(1) {
            if a > end && best.is_none_or(|(w, _)| a - end > w) {
                best = Some((a - end, (a + end) / 2.0));
            }
            end = end.max(b);
        }
        best
    }
    fn cut(rects: &[Rect], idx: Vec<usize>, size: f64, out: &mut Vec<usize>) {
        if idx.len() <= 1 {
            out.extend(idx);
            return;
        }
        let v = widest_gap(idx.iter().map(|&i| (rects[i][0], rects[i][2])).collect());
        let h = widest_gap(idx.iter().map(|&i| (rects[i][1], rects[i][3])).collect());
        let (first, second): (Vec<usize>, Vec<usize>) = match (v, h) {
            (Some((w, at)), _) if w >= 1.5 * size && column_gutter(rects, &idx, at) => idx.iter().partition(|&&i| rects[i][2] <= at),
            (_, Some((_, at))) => idx.iter().partition(|&&i| rects[i][1] >= at),
            _ => {
                // No gap either way: group into lines (overlapping vertically), each left to right.
                let mut idx = idx;
                idx.sort_by(|&a, &b| rects[b][3].total_cmp(&rects[a][3]));
                let mut lines: Vec<Vec<usize>> = Vec::new();
                for i in idx {
                    let r = rects[i];
                    let joins = lines.last().and_then(|l| l.first()).is_some_and(|&f| {
                        let a = rects[f];
                        a[3].min(r[3]) - a[1].max(r[1]) > 0.5 * (a[3] - a[1]).min(r[3] - r[1])
                    });
                    match lines.last_mut() {
                        Some(l) if joins => l.push(i),
                        _ => lines.push(vec![i]),
                    }
                }
                for mut l in lines {
                    l.sort_by(|&a, &b| rects[a][0].total_cmp(&rects[b][0]));
                    out.extend(l);
                }
                return;
            }
        };
        cut(rects, first, size, out);
        cut(rects, second, size, out);
    }
    // Both sides of a gutter hold at least two boxes, each side stacked (a column, not one word).
    fn column_gutter(rects: &[Rect], idx: &[usize], at: f64) -> bool {
        let stacked = |side: Vec<usize>| {
            side.len() >= 2 && side.iter().any(|&a| side.iter().any(|&b| rects[b][3] <= rects[a][1]))
        };
        let (l, r): (Vec<usize>, Vec<usize>) = idx.iter().partition(|&&i| rects[i][2] <= at);
        stacked(l) && stacked(r)
    }
    let mut out = Vec::with_capacity(rects.len());
    cut(rects, (0..rects.len()).collect(), size, &mut out);
    out
}

/// The parts of one page: its tables, and its other text one line per row, in reading order.
fn page_parts(blocks: &[Block], words: &[(String, Rect)]) -> Vec<Part> {
    // Words of tilted text (a picture of a screen, a stamp) are not table cells.
    let tilted: Vec<&Block> = blocks.iter().filter(|b| b.rotation.rem_euclid(360.0).min(360.0 - b.rotation.rem_euclid(360.0)) > 1.0).collect();
    let level: Vec<(String, Rect)> = words
        .iter()
        .filter(|(_, r)| {
            let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
            !tilted.iter().any(|b| cx >= b.rect[0] && cx <= b.rect[2] && cy >= b.rect[1] && cy <= b.rect[3])
        })
        .cloned()
        .collect();
    let tables = tables(&lines(&level));
    let inside = |b: &Block| {
        let (cx, cy) = ((b.rect[0] + b.rect[2]) / 2.0, (b.rect[1] + b.rect[3]) / 2.0);
        tables.iter().any(|t| cx >= t.rect[0] - COL_TOL && cx <= t.rect[2] + COL_TOL && cy >= t.rect[1] - COL_TOL && cy <= t.rect[3] + COL_TOL)
    };
    let text: Vec<&Block> = blocks.iter().filter(|b| !b.text.trim().is_empty() && !inside(b)).collect();
    let mut sizes: Vec<f64> = text.iter().map(|b| b.size).collect();
    sizes.sort_by(f64::total_cmp);
    let size = sizes.get(sizes.len() / 2).copied().unwrap_or(11.0);
    let rects: Vec<Rect> = text.iter().map(|b| b.rect).chain(tables.iter().map(|t| t.rect)).collect();
    let mut tables: Vec<Option<Table>> = tables.into_iter().map(Some).collect();
    let mut out: Vec<Part> = Vec::new();
    // The block written last: the next one joins its row when it continues the same line to
    // the right (OCR gives one block per word).
    let mut last: Option<Rect> = None;
    for i in reading_order(&rects, size) {
        if let Some(b) = text.get(i) {
            let same_line = last.is_some_and(|r| {
                let overlap = r[3].min(b.rect[3]) - r[1].max(b.rect[1]);
                b.rect[0] >= r[2] - COL_TOL && overlap > 0.5 * (r[3] - r[1]).min(b.rect[3] - b.rect[1])
            });
            let mut lines = b.text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from);
            match out.last_mut() {
                Some(Part::Text(rows)) => {
                    if same_line && let Some(row) = rows.last_mut() && let Some(first) = lines.next() {
                        row.push(' ');
                        row.push_str(&first);
                    }
                    rows.extend(lines);
                }
                _ => out.push(Part::Text(lines.collect())),
            }
            last = Some(b.rect);
        } else if let Some(t) = tables.get_mut(i - text.len()).and_then(Option::take) {
            last = None;
            out.push(Part::Table(t.rows));
        }
    }
    out.retain(|p| !matches!(p, Part::Text(rows) if rows.is_empty()));
    out
}

/// A cell's text as a number, or `None` to keep it as text.
///
/// Accepts "1,234.5", "1.234,5", "1 234,5" (groups of three digits after a space or no-break
/// space), "3,5", "-12", accounting negatives "(42)" (-42) and percentages "12%", "-12,5 %"
/// (0.12, -0.125). One separator before exactly three digits is a thousands separator when it
/// is a comma ("1,234" is 1234) and a decimal point when it is a dot ("1.234" is 1.234).
///
/// Text that only looks like a number stays text: a leading zero before a digit ("00042"; "0",
/// "0.5" and "0,5" are numbers), a leading "+" (a phone number), space-separated groups that are
/// not three digits ("12 34"), more than 15 significant digits (an account or card number, which
/// a spreadsheet would round), dates and words.
pub fn parse_number(s: &str) -> Option<f64> {
    parse(s).map(|(v, _)| v)
}

/// [`parse_number`], with the number of decimals written when the text is a percentage.
fn parse(s: &str) -> Option<(f64, Option<usize>)> {
    const SPACES: [char; 3] = [' ', '\u{a0}', '\u{202f}'];
    let mut t = s.trim();
    let percent = t.ends_with('%');
    if percent {
        t = t[..t.len() - 1].trim_end_matches(SPACES);
    }
    let mut negative = false;
    if let Some(inner) = t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        t = inner.trim();
        negative = true;
    }
    if let Some(rest) = t.strip_prefix('-') {
        if negative {
            return None;
        }
        t = rest;
        negative = true;
    }
    if !t.starts_with(|c: char| c.is_ascii_digit()) || !t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',') || SPACES.contains(&c)) {
        return None;
    }
    // Space-separated groups: 1 to 3 digits, then exactly 3 (the last may carry the decimals).
    let groups: Vec<&str> = t.split(SPACES).collect();
    let spaced = groups.len() > 1;
    if spaced {
        let digits = |g: &str| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit());
        let last = groups[groups.len() - 1];
        let last_int = last.split(['.', ',']).next().unwrap_or("");
        if !digits(groups[0]) || groups[0].len() > 3 || groups[1..groups.len() - 1].iter().any(|g| g.len() != 3 || !digits(g)) || last_int.len() != 3 || !digits(last_int) {
            return None;
        }
    }
    let t: String = groups.concat();
    if t.len() > 1 && t.starts_with('0') && t.as_bytes()[1].is_ascii_digit() {
        return None;
    }
    let (dots, commas) = (t.matches('.').count(), t.matches(',').count());
    let normal = match (dots, commas) {
        (0, 0) => t,
        // "1,234.5" / "1.234,5": the last separator is the decimal one.
        (_, _) if dots > 0 && commas > 0 => {
            let (thousands, decimal) = if t.rfind('.') > t.rfind(',') { (',', '.') } else { ('.', ',') };
            if t.matches(decimal).count() > 1 {
                return None;
            }
            t.replace(thousands, "").replace(decimal, ".")
        }
        // Several of one kind: thousands separators ("1.234.567").
        (d, 0) if d > 1 && !spaced => t.replace('.', ""),
        (0, c) if c > 1 && !spaced => t.replace(',', ""),
        (_, 0) | (0, _) if dots + commas > 1 => return None,
        // One comma: a decimal comma ("3,5"), unless it groups exactly three digits after a
        // number that has some before it ("1,234"), and there are no space groups ("1 234,567").
        (0, 1) => {
            let (a, b) = t.split_once(',')?;
            if b.len() == 3 && a != "0" && !spaced { format!("{a}{b}") } else { format!("{a}.{b}") }
        }
        _ => t,
    };
    if normal.chars().filter(char::is_ascii_digit).skip_while(|c| *c == '0').count() > 15 {
        return None;
    }
    let v = normal.parse::<f64>().ok().filter(|v| v.is_finite())?;
    let v = if negative { -v } else { v };
    if percent {
        let decimals = normal.split_once('.').map_or(0, |(_, d)| d.len());
        Some((v / 100.0, Some(decimals)))
    } else {
        Some((v, None))
    }
}

fn xlsx_err(e: rust_xlsxwriter::XlsxError) -> OpdfError {
    OpdfError::Conversion(format!("spreadsheet: {e}"))
}

fn write_cell(ws: &mut Worksheet, row: u32, col: u16, text: &str, header: Option<&Format>) -> Result<()> {
    let r = match (parse(text), header) {
        (Some((v, None)), _) => ws.write_number(row, col, v).map(|_| ()),
        (Some((v, Some(d))), _) => {
            let pattern = if d == 0 { "0%".to_string() } else { format!("0.{}%", "0".repeat(d)) };
            ws.write_number_with_format(row, col, v, &Format::new().set_num_format(pattern)).map(|_| ())
        }
        (None, _) if text.is_empty() => Ok(()),
        (None, Some(bold)) => ws.write_string_with_format(row, col, text, bold).map(|_| ()),
        (None, None) => ws.write_string(row, col, text).map(|_| ()),
    };
    r.map_err(xlsx_err)
}

/// Writes `parts` from row `top`, a blank row between parts; returns the next free row.
fn write_parts(ws: &mut Worksheet, parts: &[&Part], mut top: u32, bold: &Format) -> Result<u32> {
    for (k, part) in parts.iter().enumerate() {
        if k > 0 {
            top += 1;
        }
        match part {
            Part::Table(rows) => {
                for (r, cells) in rows.iter().enumerate() {
                    for (c, text) in cells.iter().enumerate() {
                        write_cell(ws, top + r as u32, c as u16, text, (r == 0).then_some(bold))?;
                    }
                }
                top += rows.len() as u32;
            }
            Part::Text(lines) => {
                for (r, l) in lines.iter().enumerate() {
                    ws.write_string(top + r as u32, 0, l).map_err(xlsx_err)?;
                }
                top += lines.len() as u32;
            }
        }
    }
    Ok(top)
}

/// Sheets for [`SheetLayout::SheetPerTable`]: (name, parts). Each table takes the text above
/// it; the text after a page's last table goes below that table.
fn sheets(pages: &[Vec<Part>]) -> Vec<(String, Vec<&Part>)> {
    let mut out = Vec::new();
    for (p, parts) in pages.iter().enumerate() {
        let n = parts.iter().filter(|x| matches!(x, Part::Table(_))).count();
        if n == 0 {
            if !parts.is_empty() {
                out.push((format!("Page {} text", p + 1), parts.iter().collect()));
            }
            continue;
        }
        let mut current: Vec<&Part> = Vec::new();
        let mut done = 0;
        for part in parts {
            current.push(part);
            if matches!(part, Part::Table(_)) {
                done += 1;
                if done < n {
                    out.push((format!("Page {} table {done}", p + 1), std::mem::take(&mut current)));
                }
            }
        }
        out.push((format!("Page {} table {n}", p + 1), current));
    }
    out
}

/// Exports the tables of `input` to an Excel workbook at `out` (never overwriting).
/// `progress` works as for [`crate::export::to_office`].
pub fn to_excel(input: &Input, layout: SheetLayout, opts: &ExportOptions, out: &Path, progress: &mut dyn FnMut(f32) -> bool) -> Result<ExcelReport> {
    let doc = open_for_export(input, opts, progress)?;
    let d = doc.document();
    let mut words: Vec<Vec<(String, Rect)>> = vec![Vec::new(); d.info.pages.len()];
    for w in d.words() {
        if let (Some(info), Some(list)) = (d.info.pages.get(w.page), words.get_mut(w.page)) {
            list.push((w.text, visible(info, w.rect)));
        }
    }
    let pages: Vec<Vec<Part>> = d.export_pages().iter().zip(&words).map(|(p, w)| page_parts(&p.blocks, w)).collect();
    let tables = pages.iter().flatten().filter(|p| matches!(p, Part::Table(_))).count();
    let mut wb = Workbook::new();
    let bold = Format::new().set_bold();
    match layout {
        SheetLayout::OneSheet => {
            let ws = wb.add_worksheet();
            ws.set_name("Tables").map_err(xlsx_err)?;
            let all: Vec<&Part> = pages.iter().flatten().collect();
            write_parts(ws, &all, 0, &bold)?;
        }
        SheetLayout::SheetPerTable => {
            let sheets = sheets(&pages);
            for (name, parts) in &sheets {
                let ws = wb.add_worksheet();
                ws.set_name(name).map_err(xlsx_err)?;
                write_parts(ws, parts, 0, &bold)?;
            }
            if sheets.is_empty() {
                wb.add_worksheet();
            }
        }
    }
    let bytes = wb.save_to_buffer().map_err(xlsx_err)?;
    let path = atomic_write(out, |part| Ok(std::fs::write(part, &bytes)?))?;
    progress(1.0);
    Ok(ExcelReport { path, tables })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Words at (x, y) left edges, 11 pt high, each word 6 pt per character, spaces 3 pt.
    fn words(items: &[(f64, f64, &str)]) -> Vec<(String, Rect)> {
        let mut out = Vec::new();
        for &(x, y, text) in items {
            let mut x = x;
            for w in text.split(' ') {
                let width = 6.0 * w.chars().count() as f64;
                out.push((w.to_string(), [x, y, x + width, y + 11.0]));
                x += width + 3.0;
            }
        }
        out
    }

    #[test]
    fn a_paragraph_is_not_a_table() {
        let w = words(&[(72.0, 700.0, "One line"), (72.0, 680.0, "Another line"), (72.0, 660.0, "A third one")]);
        assert!(tables(&lines(&w)).is_empty());
    }

    #[test]
    fn missing_cells_stay_empty() {
        let w = words(&[(72.0, 700.0, "A"), (200.0, 700.0, "B"), (72.0, 680.0, "C"), (72.0, 660.0, "D"), (200.0, 660.0, "E")]);
        let t = tables(&lines(&w));
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].rows, vec![vec!["A".to_string(), "B".into()], vec!["C".into(), "".into()], vec!["D".into(), "E".into()]]);
    }

    #[test]
    fn two_columns_of_prose_are_not_a_table() {
        let left = "The quick brown fox jumps over the lazy dog again";
        let right = "Pack my box with five dozen liquor jugs right now";
        let items: Vec<(f64, f64, &str)> = (0..6).flat_map(|i| [(72.0, 700.0 - 14.0 * i as f64, left), (320.0, 700.0 - 14.0 * i as f64, right)]).collect();
        assert!(tables(&lines(&words(&items))).is_empty());
    }

    #[test]
    fn reading_order_takes_columns_before_rows() {
        // Two columns of three lines; a title across the top.
        let mut rects = vec![[72.0, 760.0, 500.0, 780.0]];
        for i in 0..3 {
            rects.push([72.0, 700.0 - 14.0 * i as f64, 260.0, 711.0 - 14.0 * i as f64]);
        }
        for i in 0..3 {
            rects.push([320.0, 700.0 - 14.0 * i as f64, 500.0, 711.0 - 14.0 * i as f64]);
        }
        assert_eq!(reading_order(&rects, 11.0), vec![0, 1, 2, 3, 4, 5, 6]);
    }
}
