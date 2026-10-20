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

/// A LibreOffice invoice: a title and a client line, a 4-column table (header, 4 rows), and a
/// closing paragraph. PdfCraft's blocks join some neighbouring cells; the table must not.
#[test]
fn an_invoice_table_has_its_header_and_four_columns() {
    let t = tempfile::tempdir().unwrap();
    let a = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf");
    let r = excel::to_excel(&Input::new(&a), SheetLayout::OneSheet, &ExportOptions::default(), &t.path().join("inv.xlsx"), &mut |_| true).unwrap();
    assert_eq!(r.tables, 1);
    let rows = &read(&r.path)[0].1;
    let f = Data::Float;
    assert_eq!(rows[0][0], s("Invoice 2026-114"));
    assert_eq!(rows[1][0], s("Client: Acme SARL, 12 rue des Lilas, 75011 Paris"));
    let table: Vec<Vec<Data>> = rows[3..8].to_vec();
    assert_eq!(
        table,
        vec![
            vec![s("Item"), s("Qty"), s("Unit price"), s("Total")],
            vec![s("Widget A"), f(3.0), f(12.5), f(37.5)],
            vec![s("Widget B"), f(10.0), f(1234.0), f(12340.0)],
            vec![s("Service"), f(1.0), f(99.99), f(99.99)],
            vec![s("Ref 00042"), f(2.0), f(-5.0), f(-10.0)],
        ]
    );
    assert!(rows.iter().all(|r| r.len() == 4), "the table is exactly four columns wide");
    // The paragraph after the table stays, below it, in column A.
    assert!(matches!(&rows[9][0], Data::String(t) if t.starts_with("Thank you for your business.")), "{:?}", rows[9]);
}

/// Two columns of eight sentences each, like the pages of a brochure: prose, not a table.
#[test]
fn two_columns_of_prose_are_text_in_reading_order() {
    let t = tempfile::tempdir().unwrap();
    let left: Vec<String> = (1..=8).map(|i| format!("Left sentence {i} runs across the column.")).collect();
    let right: Vec<String> = (1..=8).map(|i| format!("Right sentence {i} runs across too.")).collect();
    let mut page: Vec<(f32, f32, &str)> = vec![(72.0, 780.0, "Welcome")];
    for i in 0..8 {
        page.push((72.0, 700.0 - 40.0 * i as f32, &left[i]));
        page.push((320.0, 700.0 - 40.0 * i as f32, &right[i]));
    }
    let a = placed_pdf(t.path(), "prose.pdf", &[page]);
    let r = excel::to_excel(&Input::new(&a), SheetLayout::SheetPerTable, &ExportOptions::default(), &t.path().join("p.xlsx"), &mut |_| true).unwrap();
    assert_eq!(r.tables, 0);
    let sheets = read(&r.path);
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].0, "Page 1 text");
    let col_a: Vec<String> = sheets[0].1.iter().map(|r| r[0].to_string()).collect();
    let mut expected = vec!["Welcome".to_string()];
    expected.extend(left);
    expected.extend(right);
    assert_eq!(col_a, expected);
    assert!(sheets[0].1.iter().all(|r| r.len() == 1));
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
    // "1.234" is read as 1.234 (one dot is a decimal point); "1,234" as 1234.
    assert_eq!(excel::parse_number("1.234"), Some(1.234));
    assert_eq!(excel::parse_number("1,234"), Some(1234.0));
    assert_eq!(excel::parse_number("0"), Some(0.0));
    assert_eq!(excel::parse_number("0.5"), Some(0.5));
    assert_eq!(excel::parse_number("0,5"), Some(0.5));
    assert_eq!(excel::parse_number("(42)"), Some(-42.0));
    assert_eq!(excel::parse_number("(1,234.50)"), Some(-1234.5));
    assert_eq!(excel::parse_number("12%"), Some(0.12));
    assert_eq!(excel::parse_number("-12,5 %"), Some(-0.125));
    assert_eq!(excel::parse_number("1 234 567,89"), Some(1234567.89));
    // Text that only looks like a number stays text.
    for text in ["00042", "007", "+33 6 12 34 56 78", "+12", "12 34", "1 23", "1234 567", "1234567890123456", "12345678901234567890", "(-5)", "--5"] {
        assert_eq!(excel::parse_number(text), None, "{text}");
    }
    assert_eq!(excel::parse_number("123456789012345"), Some(123456789012345.0), "15 digits are a number");
}

/// Percentages are stored as fractions with a percent format, so Excel shows "12%".
#[test]
fn percentages_keep_a_percent_format() {
    let t = tempfile::tempdir().unwrap();
    let page = vec![(72.0, 700.0, "Region"), (220.0, 700.0, "Share"), (72.0, 678.0, "North"), (220.0, 678.0, "12%"), (72.0, 656.0, "South"), (220.0, 656.0, "-12,5 %")];
    let a = placed_pdf(t.path(), "pct.pdf", &[page]);
    let r = excel::to_excel(&Input::new(&a), SheetLayout::OneSheet, &ExportOptions::default(), &t.path().join("pct.xlsx"), &mut |_| true).unwrap();
    let rows = &read(&r.path)[0].1;
    assert_eq!(rows[1][1], Data::Float(0.12));
    assert_eq!(rows[2][1], Data::Float(-0.125));
    let styles = export_unzip(&r.path, "xl/styles.xml");
    assert!(styles.contains("formatCode=\"0%\"") && styles.contains("formatCode=\"0.0%\""), "{styles}");
}

fn export_unzip(file: &Path, member: &str) -> String {
    let o = std::process::Command::new("unzip").arg("-p").arg(file).arg(member).output().unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Headings and paragraphs above and below a table stay, in column A, in reading order.
#[test]
fn text_around_a_table_is_kept_in_order() {
    let t = tempfile::tempdir().unwrap();
    let mut page = vec![(72.0, 780.0, "Quarterly report"), (72.0, 750.0, "Sales by product for the third quarter.")];
    page.extend(grid(700.0));
    page.push((72.0, 600.0, "Prices include tax."));
    let a = placed_pdf(t.path(), "around.pdf", &[page]);
    for layout in [SheetLayout::OneSheet, SheetLayout::SheetPerTable] {
        let r = excel::to_excel(&Input::new(&a), layout, &ExportOptions::default(), &t.path().join("around.xlsx"), &mut |_| true).unwrap();
        let sheets = read(&r.path);
        assert_eq!(sheets.len(), 1, "{layout:?}");
        let rows = &sheets[0].1;
        let col_a: Vec<String> = rows.iter().map(|r| r[0].to_string()).collect();
        assert_eq!(col_a, ["Quarterly report", "Sales by product for the third quarter.", "", "Item", "Apples", "Pears", "", "Prices include tax."], "{layout:?}");
        assert_eq!(rows[4], vec![s("Apples"), Data::Float(3.0), Data::Float(1234.5)]);
    }
}
