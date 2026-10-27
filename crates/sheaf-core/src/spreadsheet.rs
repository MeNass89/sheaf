//! Offline data-only spreadsheet pages. iLovePDF's faithful layout still uses LibreOffice;
//! this explicit fallback keeps cell addresses and cached values without any outside program.

use crate::{paths::atomic_write, ranges::parse_ranges, OpdfError, Result};
use calamine::{open_workbook_auto, Reader, SheetVisible};
use lopdf::{content::{Content, Operation}, dictionary, Document, Object, Stream};
use std::path::{Path, PathBuf};

const WIDTH: usize = 116;
const LINES: usize = 43;

/// Export visible sheets, or explicitly numbered sheets in the requested order. Hidden sheets
/// are never included implicitly. Formulas use their saved values; no calculation is attempted.
/// No original formatting, charts, print areas or automatic date/number formats are reproduced.
pub fn to_pdf(input: &Path, sheets: &str, out: &Path) -> Result<PathBuf> {
    let mut book = open_workbook_auto(input).map_err(|e| OpdfError::Unsupported(format!("Cannot read this workbook for a data report: {e}. Use Saved print layout with LibreOffice for other formats")))?;
    let metadata = book.sheets_metadata().to_vec();
    let chosen: Vec<usize> = if sheets.trim().is_empty() {
        metadata.iter().enumerate().filter_map(|(i, sheet)| (sheet.visible == SheetVisible::Visible).then_some(i)).collect()
    } else {
        parse_ranges(sheets, metadata.len() as u32).map_err(|_| OpdfError::Unsupported(format!("Choose sheet numbers from 1 to {} (for example, 1,3-5)", metadata.len())))?
            .into_iter().flatten().map(|n| n as usize - 1).collect()
    };
    if chosen.is_empty() { return Err(OpdfError::Unsupported("No visible sheets. Choose sheet numbers explicitly to include a hidden sheet".into())); }
    let mut pdf = Document::with_version("1.4");
    let pages = pdf.new_object_id();
    let font = pdf.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier", "Encoding" => "WinAnsiEncoding" });
    let resources = pdf.add_object(dictionary! { "Font" => dictionary! { "F" => font } });
    let mut kids = Vec::new();
    for index in chosen {
        let name = &metadata[index].name;
        let range = book.worksheet_range(name).map_err(|e| OpdfError::Unsupported(format!("Cannot read sheet {name}: {e}")))?;
        let (rows, cols) = range.get_size();
        if rows.saturating_mul(cols) > 1_000_000 {
            return Err(OpdfError::Unsupported(format!("Sheet {name} is too large for the built-in data report; use Saved print layout")));
        }
        let origin = range.start().unwrap_or_default();
        let mut lines = Vec::new();
        for (row, col, cell) in range.used_cells() {
            let address = format!("{}{}: ", column(origin.1 as usize + col), origin.0 as usize + row + 1);
            let value = cell.to_string().replace('\t', "    ");
            for (n, line) in value.split(['\r', '\n']).enumerate() {
                let text = format!("{}{line}", if n == 0 { address.as_str() } else { "    " });
                let bytes = encode(&text)?;
                lines.extend(bytes.chunks(WIDTH).map(<[u8]>::to_vec));
            }
            if lines.len() > 100_000 {
                return Err(OpdfError::Unsupported(format!("Sheet {name} is too large for the built-in data report; use Saved print layout")));
            }
        }
        if lines.is_empty() { lines.push(b"(Empty sheet)".to_vec()); }
        for (part, body) in lines.chunks(LINES).enumerate() {
            let header = encode(&format!("Sheet {}: {} - data report, page {}", index + 1, name, part + 1))?;
            let mut shown = header.chunks(WIDTH).take(2).map(<[u8]>::to_vec).collect::<Vec<_>>();
            if header.len() > WIDTH * 2 { return Err(OpdfError::Unsupported("Sheet title is too long for the data report".into())); }
            shown.push(b"Saved values only; no styles, charts or formula recalculation.".to_vec());
            shown.push(Vec::new());
            shown.extend_from_slice(body);
            let mut ops = vec![Operation::new("BT", vec![]), Operation::new("Tf", vec![Object::Name(b"F".to_vec()), 10.into()]),
                Operation::new("Td", vec![36.into(), 556.into()]), Operation::new("TL", vec![11.into()])];
            for line in shown {
                ops.push(Operation::new("Tj", vec![Object::String(line, lopdf::StringFormat::Literal)]));
                ops.push(Operation::new("T*", vec![]));
            }
            ops.push(Operation::new("ET", vec![]));
            let stream = pdf.add_object(Stream::new(dictionary! {}, Content { operations: ops }.encode()?));
            let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 792.into(), 612.into()], "Resources" => resources, "Contents" => stream });
            kids.push(page.into());
        }
    }
    pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => kids.len() as i64, "Kids" => kids }.into());
    let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    pdf.trailer.set("Root", root);
    atomic_write(out, |part| { pdf.save(part)?; Ok(()) })
}

fn encode(text: &str) -> Result<Vec<u8>> {
    let (bytes, _, replaced) = encoding_rs::WINDOWS_1252.encode(text);
    if replaced || text.chars().any(|c| c.is_control()) {
        return Err(OpdfError::Unsupported("The data report supports Western European text only. Use Saved print layout with LibreOffice for this workbook's characters".into()));
    }
    Ok(bytes.into_owned())
}

fn column(mut n: usize) -> String {
    let mut chars = Vec::new();
    loop {
        chars.push((b'A' + (n % 26) as u8) as char);
        if n < 26 { break; }
        n = n / 26 - 1;
    }
    chars.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_data_report_selects_sheets_preserves_cells_and_needs_no_office_runtime() {
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("data.xlsx");
        let mut book = rust_xlsxwriter::Workbook::new();
        book.add_worksheet().set_name("Sales").unwrap().write_string(2, 27, "Café (Q1)").unwrap();
        book.add_worksheet().set_name("Secret").unwrap().set_hidden(true).write_string(0, 0, "HiddenValue").unwrap();
        book.save(&input).unwrap();
        let original = std::fs::read(&input).unwrap();
        let out = t.path().join("data.pdf");
        let visible = to_pdf(&input, "", &out).unwrap();
        let pdf = Document::load(&visible).unwrap();
        let text = pdf.extract_text(&pdf.get_pages().keys().copied().collect::<Vec<_>>()).unwrap();
        assert!(text.contains("AB3: Café (Q1)"), "{text}");
        assert!(!text.contains("HiddenValue"));
        let selected = to_pdf(&input, "2,1", &out).unwrap();
        assert_ne!(selected, visible);
        let pdf = Document::load(selected).unwrap();
        assert!(pdf.extract_text(&[1]).unwrap().contains("HiddenValue"));
        assert!(pdf.extract_text(&[2]).unwrap().contains("AB3:"));
        assert!(to_pdf(&input, "3", &t.path().join("bad.pdf")).is_err());
        assert!(!t.path().join("bad.pdf").exists());
        assert_eq!(std::fs::read(input).unwrap(), original);
    }

    #[test]
    fn native_data_report_wraps_long_values_and_refuses_missing_glyphs_atomically() {
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("data.xlsx");
        let mut book = rust_xlsxwriter::Workbook::new();
        book.add_worksheet().write_string(0, 0, "LongValue".repeat(1000)).unwrap();
        book.save(&input).unwrap();
        let out = to_pdf(&input, "", &t.path().join("long.pdf")).unwrap();
        let pdf = Document::load(out).unwrap();
        assert!(pdf.get_pages().len() > 1);
        assert_eq!(column(0), "A");
        assert_eq!(column(701), "ZZ");
        let mut book = rust_xlsxwriter::Workbook::new();
        book.add_worksheet().write_string(0, 0, "東京").unwrap();
        book.save(&input).unwrap();
        let out = t.path().join("unsupported.pdf");
        assert!(to_pdf(&input, "", &out).unwrap_err().to_string().contains("Saved print layout"));
        assert!(!out.exists());
    }
}
