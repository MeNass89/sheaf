mod common;
use calamine::{open_workbook, Data, Reader, Xlsx};
use pdfium_render::prelude::*;
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::excel::{self, SheetLayout};
use sheaf_core::export::ExportOptions;
use sheaf_core::pages::Input;
use std::path::{Path, PathBuf};

/// Pages of text objects placed at fixed x/y (points, y up). Each page is a list of
/// (x, y, text); Helvetica 11 pt.
fn placed_pdf(dir: &Path, name: &str, pages: &[Vec<(f32, f32, &str)>]) -> PathBuf {
    let _g = pdfium_lock();
    let mut doc = new_document();
    let font = doc.fonts_mut().helvetica();
    for items in pages {
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        for (x, y, text) in items {
            page.objects_mut().create_text_object(PdfPoints::new(*x), PdfPoints::new(*y), *text, font, PdfPoints::new(11.0)).unwrap();
        }
    }
    let path = dir.join(name);
    doc.save_to_file(&path).unwrap();
    path
}

/// A 3×3 grid: a header row and two rows of numbers, columns at x = 72, 220, 368.
fn grid(top: f32) -> Vec<(f32, f32, &'static str)> {
    let rows = [["Item", "Qty", "Price"], ["Apples", "3", "1,234.5"], ["Pears", "12", "7 890,25"]];
    let mut v = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            v.push((72.0 + 148.0 * c as f32, top - 22.0 * r as f32, *text));
        }
    }
    v
}

fn read(path: &Path) -> Vec<(String, Vec<Vec<Data>>)> {
    let mut wb: Xlsx<_> = open_workbook(path).unwrap();
    wb.sheet_names()
        .into_iter()
        .map(|name| {
            let range = wb.worksheet_range(&name).unwrap();
            let rows = range.rows().map(|r| r.to_vec()).collect();
            (name, rows)
        })
        .collect()
}

fn s(v: &str) -> Data {
    Data::String(v.into())
}

#[test]
fn a_grid_of_text_becomes_cells_with_numbers() {
    let t = tempfile::tempdir().unwrap();
    let a = placed_pdf(t.path(), "grid.pdf", &[grid(700.0)]);
    let r = excel::to_excel(&Input::new(&a), SheetLayout::OneSheet, &ExportOptions::default(), &t.path().join("grid.xlsx"), &mut |_| true).unwrap();
    assert_eq!(r.tables, 1);
    let sheets = read(&r.path);
    assert_eq!(sheets.len(), 1);
    let rows = &sheets[0].1;
    assert_eq!(rows[0], vec![s("Item"), s("Qty"), s("Price")]);
    assert_eq!(rows[1], vec![s("Apples"), Data::Float(3.0), Data::Float(1234.5)]);
    assert_eq!(rows[2], vec![s("Pears"), Data::Float(12.0), Data::Float(7890.25)]);
}

#[test]
fn multiple_sheets_puts_each_table_on_its_own_sheet() {
    let t = tempfile::tempdir().unwrap();
    let a = placed_pdf(t.path(), "two.pdf", &[grid(700.0), grid(700.0)]);
    let r = excel::to_excel(&Input::new(&a), SheetLayout::SheetPerTable, &ExportOptions::default(), &t.path().join("two.xlsx"), &mut |_| true).unwrap();
    let sheets = read(&r.path);
    let names: Vec<&str> = sheets.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Page 1 table 1", "Page 2 table 1"]);
    assert_eq!(sheets[1].1[2][0], s("Pears"));
    // One sheet: both tables stacked with a blank row between.
    let r = excel::to_excel(&Input::new(&a), SheetLayout::OneSheet, &ExportOptions::default(), &t.path().join("one.xlsx"), &mut |_| true).unwrap();
    let sheets = read(&r.path);
    assert_eq!(sheets.len(), 1);
    let rows = &sheets[0].1;
    assert_eq!(rows.len(), 7, "{rows:?}");
    assert!(rows[3].iter().all(|c| *c == Data::Empty));
    assert_eq!(rows[4][0], s("Item"));
}

#[test]
fn a_page_without_a_table_keeps_its_text_one_paragraph_per_row() {
    // PdfCraft joins consecutive lines into paragraphs; separate paragraphs get their own row.
    let t = tempfile::tempdir().unwrap();
    let text = vec![(72.0, 760.0, "Quarterly report"), (72.0, 600.0, "Prepared in October")];
    let a = placed_pdf(t.path(), "mixed.pdf", &[text, grid(700.0)]);
    let r = excel::to_excel(&Input::new(&a), SheetLayout::SheetPerTable, &ExportOptions::default(), &t.path().join("m.xlsx"), &mut |_| true).unwrap();
    let sheets = read(&r.path);
    let names: Vec<&str> = sheets.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Page 1 text", "Page 2 table 1"]);
    assert_eq!(sheets[0].1, vec![vec![s("Quarterly report")], vec![s("Prepared in October")]]);
}

#[test]
fn numbers_in_both_conventions_parse() {
    assert_eq!(excel::parse_number("1,234.5"), Some(1234.5));
    assert_eq!(excel::parse_number("1 234,5"), Some(1234.5));
    assert_eq!(excel::parse_number("-12"), Some(-12.0));
    assert_eq!(excel::parse_number("3,5"), Some(3.5));
    assert_eq!(excel::parse_number("1.234.567"), Some(1234567.0));
    assert_eq!(excel::parse_number("12a"), None);
    assert_eq!(excel::parse_number("2026-10-15"), None);
    assert_eq!(excel::parse_number(""), None);
    assert_eq!(excel::parse_number("inf"), None);
}
