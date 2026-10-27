//! Sheaf: Acrobat's barcode form fields. A barcode field is a read-only text field carrying
//! Acrobat's paper metadata (`/PMD`: symbology, error correction, field names or not); its value
//! is the other fields' data, tab-delimited, and its appearance is that value drawn as a QR Code.
//! Both are written again after every change to the form, as Acrobat recalculates its barcodes.

use pdfcraft_cos::{Dict, Document, Object, PdfString, Stream};
use qrcode::{Color, EcLevel, QrCode};

/// The symbology and error correction of a barcode field (Acrobat's Barcode Field Properties ▸
/// Options). QR Code is the symbology Sheaf draws; `ecc` is Low, Medium, Quartile or High.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarcodeSettings {
    /// 0 Low (7 %), 1 Medium (15 %), 2 Quartile (25 %), 3 High (30 %).
    pub ecc: u8,
    /// A first line with the field names (Acrobat's "Include field names").
    pub names: bool,
}

impl Default for BarcodeSettings {
    fn default() -> Self {
        Self { ecc: 1, names: true }
    }
}

fn level(ecc: u8) -> EcLevel {
    match ecc {
        0 => EcLevel::L,
        2 => EcLevel::Q,
        3 => EcLevel::H,
        _ => EcLevel::M,
    }
}

/// The `/PMD` dictionary Acrobat writes on a barcode field.
fn metadata(s: BarcodeSettings) -> Dict {
    let mut d = Dict::new();
    d.set(b"Symbology".to_vec(), Object::name("QRCode"));
    d.set(b"ECC".to_vec(), Object::Int(i64::from(s.ecc.min(3))));
    d.set(b"Names".to_vec(), Object::Bool(s.names));
    d.set(b"DataPrep".to_vec(), Object::Int(0));
    d
}

fn settings_of(doc: &Document, field: &Dict) -> Option<BarcodeSettings> {
    let pmd = doc.resolve(field.get(b"PMD")?);
    let pmd = pmd.as_dict()?;
    let ecc = pmd.int(b"ECC").unwrap_or(1).clamp(0, 3) as u8;
    let names = !matches!(pmd.get(b"Names"), Some(Object::Bool(false)));
    Some(BarcodeSettings { ecc, names })
}

/// The barcode fields of `doc`: (name, settings).
pub fn barcode_fields(doc: &Document) -> Vec<(String, BarcodeSettings)> {
    pdfcraft_forms::fields(doc)
        .into_iter()
        .filter_map(|f| {
            let d = doc.get(f.obj);
            let s = settings_of(doc, d.as_dict()?)?;
            Some((f.name, s))
        })
        .collect()
}

/// The data a barcode encodes: every other field that holds a value (text, choices, check boxes,
/// radio buttons), tab-delimited, after a line of their names when asked.
pub fn data(doc: &Document, settings: BarcodeSettings) -> String {
    let fields = pdfcraft_forms::fields(doc);
    let encoded: Vec<_> = fields
        .iter()
        .filter(|f| !matches!(f.kind, pdfcraft_forms::FieldKind::PushButton | pdfcraft_forms::FieldKind::Signature))
        .filter(|f| doc.get(f.obj).as_dict().is_none_or(|d| !d.contains(b"PMD")))
        .collect();
    let clean = |s: &str| s.replace(['\t', '\r', '\n'], " ");
    let values = encoded.iter().map(|f| clean(&f.value.join(","))).collect::<Vec<_>>().join("\t");
    if settings.names {
        format!("{}\n{values}", encoded.iter().map(|f| clean(&f.name)).collect::<Vec<_>>().join("\t"))
    } else {
        values
    }
}

/// The QR Code modules of `data` (dark = true), row by row, and the side in modules.
pub fn modules(data: &str, settings: BarcodeSettings) -> Result<(Vec<bool>, usize), String> {
    let code = QrCode::with_error_correction_level(data.as_bytes(), level(settings.ecc))
        .map_err(|e| format!("The form's data does not fit in a QR Code ({e}); choose lower error correction or fewer fields"))?;
    let side = code.width();
    Ok((code.to_colors().into_iter().map(|c| c == Color::Dark).collect(), side))
}

/// The appearance of a `w` × `h` widget: black modules centred on white, with the four-module
/// quiet zone readers need.
pub fn appearance(cells: &[bool], side: usize, w: f64, h: f64) -> Vec<u8> {
    let module = (w.min(h) / (side + 8) as f64).max(0.01);
    let (x0, y0) = ((w - module * side as f64) / 2.0, (h - module * side as f64) / 2.0);
    let mut c = format!("1 g 0 0 {w:.3} {h:.3} re f 0 g\n");
    for row in 0..side {
        for col in 0..side {
            if cells[row * side + col] {
                // Row 0 is the top of the symbol; PDF's y grows upwards.
                let y = y0 + (side - 1 - row) as f64 * module;
                c.push_str(&format!("{:.3} {:.3} {m:.3} {m:.3} re\n", x0 + col as f64 * module, y, m = module));
            }
        }
    }
    c.push_str("f\n");
    c.into_bytes()
}

/// Write every barcode field's value and appearance again from the form as it is now.
pub fn refresh(doc: &mut Document) -> Result<(), String> {
    let barcodes = barcode_fields(doc);
    if barcodes.is_empty() {
        return Ok(());
    }
    let fields = pdfcraft_forms::fields(doc);
    for (name, settings) in barcodes {
        let Some(field) = fields.iter().find(|f| f.name == name) else { continue };
        let text = data(doc, settings);
        let (cells, side) = modules(&text, settings)?;
        doc.update_dict(field.obj, |d| d.set(b"V".to_vec(), Object::String(PdfString::text(&text)))).map_err(|e| e.to_string())?;
        for w in &field.widgets {
            let (ww, hh) = (w.rect[2] - w.rect[0], w.rect[3] - w.rect[1]);
            let mut sd = Dict::new();
            sd.set(b"Type".to_vec(), Object::name("XObject"));
            sd.set(b"Subtype".to_vec(), Object::name("Form"));
            sd.set(b"BBox".to_vec(), Object::Array(vec![Object::Real(0.0), Object::Real(0.0), Object::Real(ww), Object::Real(hh)]));
            sd.set(b"Resources".to_vec(), Object::Dict(Dict::new()));
            let ap = doc.add(Object::Stream(Stream::flate(sd, &appearance(&cells, side, ww, hh))));
            let mut n = Dict::new();
            n.set(b"N".to_vec(), Object::Ref(ap));
            doc.update_dict(w.obj, |d| d.set(b"AP".to_vec(), Object::Dict(n))).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Make field `name` a barcode field with `settings`: read only (its data comes from the form),
/// with Acrobat's paper metadata, drawn at once.
pub fn make(doc: &mut Document, name: &str, settings: BarcodeSettings) -> Result<(), String> {
    let field = pdfcraft_forms::fields(doc).into_iter().find(|f| f.name == name).ok_or_else(|| format!("No field named {name}"))?;
    if field.kind != pdfcraft_forms::FieldKind::Text {
        return Err(format!("{name} is not a text field: a barcode field is one"));
    }
    doc.update_dict(field.obj, |d| {
        d.set(b"PMD".to_vec(), Object::Dict(metadata(settings)));
        let ff = d.int(b"Ff").unwrap_or(0);
        d.set(b"Ff".to_vec(), Object::Int(ff | 1));
    })
    .map_err(|e| e.to_string())?;
    refresh(doc)
}
