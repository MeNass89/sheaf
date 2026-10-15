//! PDF to Excel: tables found in the page text, written as worksheets.
//!
//! PdfCraft reads each page as text blocks with their boxes ([`Doc::document`]'s
//! `export_pages`). Its table detector is private, so tables are found here with the same idea:
//! columns are left edges shared by several short blocks, rows are blocks whose vertical spans
//! overlap, and a run of two or more rows spread over two or more columns is a table. A page
//! with no table keeps its text, one paragraph per row in column A, so nothing is lost.

use crate::export::{open_for_export, ExportOptions};
use crate::paths::atomic_write;
use crate::{pages::Input, OpdfError, Result};
use pdfcraft_export::{Block, Page};
use rust_xlsxwriter::{Format, Workbook, Worksheet};
use std::path::{Path, PathBuf};

/// Where the tables go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetLayout {
    /// Every table on one sheet, stacked, a blank row between them.
    #[default]
    OneSheet,
    /// One sheet per table ("Page 2 table 1"), and one per page without a table ("Page 3 text").
    SheetPerTable,
}

/// What [`to_excel`] wrote.
#[derive(Debug, Clone)]
pub struct ExcelReport {
    pub path: PathBuf,
    /// Tables found.
    pub tables: usize,
}

/// How far two left edges may drift (points) and still be one column.
const COL_TOL: f64 = 4.0;

/// One block of a table: its text.
type Grid = Vec<Vec<String>>;

/// What one page gives: its tables, or its text lines when it has none.
enum Part {
    Table { page: usize, index: usize, rows: Grid },
    Text { page: usize, lines: Vec<String> },
}

/// A short, single-line-ish block that can be a table cell.
fn is_cell_like(b: &Block) -> bool {
    (b.rect[3] - b.rect[1]) <= b.size.max(1.0) * 5.0 && !b.text.trim().is_empty()
}

/// Tables of one page, top to bottom, and which blocks they used.
fn tables(blocks: &[Block]) -> (Vec<Grid>, Vec<bool>) {
    let mut used = vec![false; blocks.len()];
    // Columns: cluster the left edges of cell-like blocks; a column needs two members.
    let mut xs: Vec<f64> = blocks.iter().filter(|b| is_cell_like(b)).map(|b| b.rect[0]).collect();
    xs.sort_by(f64::total_cmp);
    let mut cols: Vec<(f64, usize)> = Vec::new();
    for x in xs {
        match cols.last_mut() {
            Some((e, n)) if (x - *e).abs() <= COL_TOL => {
                *e = (*e * *n as f64 + x) / (*n + 1) as f64;
                *n += 1;
            }
            _ => cols.push((x, 1)),
        }
    }
    let edges: Vec<f64> = cols.into_iter().filter(|(_, n)| *n >= 2).map(|(e, _)| e).collect();
    if edges.len() < 2 {
        return (Vec::new(), used);
    }
    let column = |x: f64| edges.iter().position(|e| (e - x).abs() <= COL_TOL);
    // Rows: cell-like blocks in a column, top to bottom, grouped by vertical overlap.
    let mut order: Vec<usize> = (0..blocks.len()).filter(|&i| is_cell_like(&blocks[i]) && column(blocks[i].rect[0]).is_some()).collect();
    order.sort_by(|&a, &b| blocks[b].rect[3].total_cmp(&blocks[a].rect[3]).then(blocks[a].rect[0].total_cmp(&blocks[b].rect[0])));
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for i in order {
        let b = &blocks[i];
        let joins = rows.last().and_then(|r| r.first()).is_some_and(|&f| {
            let a = &blocks[f];
            let overlap = a.rect[3].min(b.rect[3]) - a.rect[1].max(b.rect[1]);
            overlap > 0.5 * (a.rect[3] - a.rect[1]).min(b.rect[3] - b.rect[1])
        });
        match rows.last_mut() {
            Some(r) if joins => r.push(i),
            _ => rows.push(vec![i]),
        }
    }
    // Runs of consecutive rows spanning two or more columns are tables.
    let spread = |r: &Vec<usize>| {
        let mut cs: Vec<usize> = r.iter().filter_map(|&i| column(blocks[i].rect[0])).collect();
        cs.sort_unstable();
        cs.dedup();
        cs.len() >= 2
    };
    // A row with a missing cell, between two full ones, stays in the table.
    let mut qual: Vec<bool> = rows.iter().map(spread).collect();
    for k in 1..rows.len().saturating_sub(1) {
        if !qual[k] && qual[k - 1] && qual[k + 1] {
            qual[k] = true;
        }
    }
    let mut out = Vec::new();
    let mut k = 0;
    while k < rows.len() {
        if !qual[k] {
            k += 1;
            continue;
        }
        let start = k;
        while k < rows.len() && qual[k] {
            k += 1;
        }
        let run = &rows[start..k];
        if run.len() < 2 {
            continue;
        }
        // The table's own columns, so a table does not get the page's other edges.
        let mut cs: Vec<usize> = run.iter().flatten().filter_map(|&i| column(blocks[i].rect[0])).collect();
        cs.sort_unstable();
        cs.dedup();
        let grid = run
            .iter()
            .map(|r| {
                let mut cells = vec![String::new(); cs.len()];
                for &i in r {
                    if let Some(c) = column(blocks[i].rect[0]).and_then(|c| cs.iter().position(|x| *x == c)) {
                        let cell = &mut cells[c];
                        if !cell.is_empty() {
                            cell.push(' ');
                        }
                        cell.push_str(blocks[i].text.trim());
                        used[i] = true;
                    }
                }
                cells
            })
            .collect();
        out.push(grid);
    }
    (out, used)
}

fn parts(pages: &[Page]) -> Vec<Part> {
    let mut out = Vec::new();
    for (p, page) in pages.iter().enumerate() {
        let (grids, _) = tables(&page.blocks);
        if grids.is_empty() {
            let mut blocks: Vec<&Block> = page.blocks.iter().filter(|b| !b.text.trim().is_empty()).collect();
            blocks.sort_by(|a, b| b.rect[3].total_cmp(&a.rect[3]).then(a.rect[0].total_cmp(&b.rect[0])));
            let lines: Vec<String> = blocks.iter().flat_map(|b| b.text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect::<Vec<_>>()).collect();
            if !lines.is_empty() {
                out.push(Part::Text { page: p, lines });
            }
        } else {
            out.extend(grids.into_iter().enumerate().map(|(i, rows)| Part::Table { page: p, index: i, rows }));
        }
    }
    out
}

/// A cell's text as a number, accepting "1,234.5" and "1 234,5" (and a no-break space). `None`
/// for anything else, including dates and words.
pub fn parse_number(s: &str) -> Option<f64> {
    let s: String = s.trim().chars().filter(|c| !matches!(c, ' ' | '\u{a0}' | '\u{202f}')).collect();
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+')) {
        return None;
    }
    if s[1..].contains(['-', '+']) {
        return None;
    }
    let (dots, commas) = (s.matches('.').count(), s.matches(',').count());
    let normal = match (dots, commas) {
        (0, 0) => s,
        // "1,234.5" / "1.234,5": the last separator is the decimal one.
        (_, _) if dots > 0 && commas > 0 => {
            let (thousands, decimal) = if s.rfind('.') > s.rfind(',') { (',', '.') } else { ('.', ',') };
            if s.matches(decimal).count() > 1 {
                return None;
            }
            s.replace(thousands, "").replace(decimal, ".")
        }
        // Several of one kind: thousands separators ("1.234.567").
        (d, 0) if d > 1 => s.replace('.', ""),
        (0, c) if c > 1 => s.replace(',', ""),
        // One comma: a decimal comma ("3,5"), unless it groups exactly three digits after a
        // number that has some before it ("1,234").
        (0, 1) => {
            let (a, b) = s.split_once(',')?;
            if b.len() == 3 && !a.trim_start_matches(['-', '+']).is_empty() && a.trim_start_matches(['-', '+']) != "0" { format!("{a}{b}") } else { format!("{a}.{b}") }
        }
        _ => s,
    };
    normal.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn write_cell(ws: &mut Worksheet, row: u32, col: u16, text: &str) -> Result<()> {
    let r = match parse_number(text) {
        Some(v) => ws.write_number(row, col, v).map(|_| ()),
        None if text.is_empty() => Ok(()),
        None => ws.write_string(row, col, text).map(|_| ()),
    };
    r.map_err(xlsx_err)
}

fn xlsx_err(e: rust_xlsxwriter::XlsxError) -> OpdfError {
    OpdfError::Conversion(format!("spreadsheet: {e}"))
}

fn write_part(ws: &mut Worksheet, part: &Part, top: u32, bold: &Format) -> Result<u32> {
    match part {
        Part::Table { rows, .. } => {
            for (r, cells) in rows.iter().enumerate() {
                for (c, text) in cells.iter().enumerate() {
                    if r == 0 && parse_number(text).is_none() && !text.is_empty() {
                        ws.write_string_with_format(top + r as u32, c as u16, text, bold).map_err(xlsx_err)?;
                    } else {
                        write_cell(ws, top + r as u32, c as u16, text)?;
                    }
                }
            }
            Ok(rows.len() as u32)
        }
        Part::Text { lines, .. } => {
            for (r, l) in lines.iter().enumerate() {
                ws.write_string(top + r as u32, 0, l).map_err(xlsx_err)?;
            }
            Ok(lines.len() as u32)
        }
    }
}

/// Exports the tables of `input` to an Excel workbook at `out` (never overwriting).
/// `progress` works as for [`crate::export::to_office`].
pub fn to_excel(input: &Input, layout: SheetLayout, opts: &ExportOptions, out: &Path, progress: &mut dyn FnMut(f32) -> bool) -> Result<ExcelReport> {
    let doc = open_for_export(input, opts, progress)?;
    let parts = parts(&doc.document().export_pages());
    let tables = parts.iter().filter(|p| matches!(p, Part::Table { .. })).count();
    let mut wb = Workbook::new();
    let bold = Format::new().set_bold();
    match layout {
        SheetLayout::OneSheet => {
            let ws = wb.add_worksheet();
            ws.set_name("Tables").map_err(xlsx_err)?;
            let mut row = 0;
            for p in &parts {
                row += write_part(ws, p, row, &bold)? + 1;
            }
        }
        SheetLayout::SheetPerTable => {
            for p in &parts {
                let name = match p {
                    Part::Table { page, index, .. } => format!("Page {} table {}", page + 1, index + 1),
                    Part::Text { page, .. } => format!("Page {} text", page + 1),
                };
                let ws = wb.add_worksheet();
                ws.set_name(&name).map_err(xlsx_err)?;
                write_part(ws, p, 0, &bold)?;
            }
            if parts.is_empty() {
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

    fn block(x: f64, y: f64, text: &str) -> Block {
        Block { text: text.into(), rect: [x, y, x + 40.0, y + 11.0], size: 11.0, bold: false, italic: false, color: [0.0; 3], rotation: 0.0 }
    }

    #[test]
    fn a_paragraph_is_not_a_table() {
        let blocks = vec![block(72.0, 700.0, "One line"), block(72.0, 680.0, "Another line")];
        assert!(tables(&blocks).0.is_empty());
    }

    #[test]
    fn missing_cells_stay_empty() {
        let blocks = vec![
            block(72.0, 700.0, "A"),
            block(200.0, 700.0, "B"),
            block(72.0, 680.0, "C"),
            block(72.0, 660.0, "D"),
            block(200.0, 660.0, "E"),
        ];
        let (t, used) = tables(&blocks);
        assert_eq!(t, vec![vec![vec!["A".to_string(), "B".into()], vec!["C".into(), "".into()], vec!["D".into(), "E".into()]]]);
        assert!(used.iter().all(|u| *u));
    }
}
