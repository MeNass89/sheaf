//! Acrobat's Output Preview: inspect actual process and spot ink plates of a working snapshot.
//! Ghostscript's local tiffsep device interprets painted content, including nested artwork.
//! These are plate masks, not a calibrated press soft proof or a PDF/X certificate.

use crate::{pages::Input, pdfcraft::Doc, OpdfError, Result};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Plate {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// The ink's colour as CMYK 0–1: the process inks their own, a spot ink its alternate (a
    /// Separation colour space's tint at full strength), for the composite of the inks shown.
    pub cmyk: [f32; 4],
}

/// Acrobat's Output Preview with several inks ticked: the page as the `shown` plates print together,
/// each plate's coverage (black = full ink) laid down in its ink's colour, as RGBA. Plates of another
/// size are left out.
pub fn composite(plates: &[Plate], shown: &[bool]) -> Option<(u32, u32, Vec<u8>)> {
    let first = plates.first()?;
    let (w, h) = (first.width, first.height);
    let mut cmyk = vec![[0f32; 4]; (w * h) as usize];
    for (plate, _) in plates.iter().zip(shown).filter(|(p, s)| **s && p.width == w && p.height == h) {
        for (px, out) in plate.rgba.as_chunks::<4>().0.iter().zip(cmyk.iter_mut()) {
            let coverage = 1.0 - f32::from(px[0]) / 255.0;
            for (o, ink) in out.iter_mut().zip(plate.cmyk) {
                *o = (*o + coverage * ink).min(1.0);
            }
        }
    }
    let rgba = cmyk.iter().flat_map(|[c, m, y, k]| {
        let ch = |v: f32| ((1.0 - v) * (1.0 - k) * 255.0).round() as u8;
        [ch(*c), ch(*m), ch(*y), 255]
    }).collect();
    Some((w, h, rgba))
}

/// The CMYK of ink `name`: a process ink's own, or the alternate of the Separation (or DeviceN)
/// colour space of that name in `bytes` when its tint function is exponential; mid grey otherwise.
fn ink_cmyk(name: &str, bytes: &[u8]) -> [f32; 4] {
    match name {
        "Cyan" => return [1.0, 0.0, 0.0, 0.0],
        "Magenta" => return [0.0, 1.0, 0.0, 0.0],
        "Yellow" => return [0.0, 0.0, 1.0, 0.0],
        "Black" => return [0.0, 0.0, 0.0, 1.0],
        _ => {}
    }
    let found = lopdf::Document::load_mem(bytes).ok().and_then(|pdf| {
        pdf.objects.values().find_map(|o| {
            let a = o.as_array().ok()?;
            (a.first()?.as_name().ok()? == b"Separation" && a.get(1)?.as_name().ok()? == name.as_bytes() && a.get(2)?.as_name().ok()? == b"DeviceCMYK").then_some(())?;
            let f = pdf.dereference(a.get(3)?).ok()?.1.as_dict().ok()?;
            let c1 = f.get(b"C1").ok()?.as_array().ok()?;
            let v: Vec<f32> = c1.iter().filter_map(|n| n.as_float().ok()).collect();
            (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
        })
    });
    found.unwrap_or([0.0, 0.0, 0.0, 0.5])
}

/// Capture bytes on the UI thread only after pending edits have completed, then run off-thread.
/// A bounded single-page square contains the complete crop at its original aspect ratio.
pub fn preview(bytes: &[u8], password: Option<&str>, page: usize, progress: &mut dyn FnMut() -> bool) -> Result<Vec<Plate>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("snapshot.pdf");
    std::fs::write(&source, bytes)?;
    let mut doc = Doc::open(&Input { path: source, password: password.map(str::to_string) })?;
    if page >= doc.document().info.pages.len() { return Err(OpdfError::Unsupported("The preview page no longer exists".into())); }
    let source = doc.save_unprotected(&dir.path().join("source.pdf"))?.0;
    drop(doc);
    let gs = crate::press::find_ghostscript().ok_or_else(|| OpdfError::Unsupported("Ink plate preview needs a local Ghostscript installation".into()))?;
    let output = dir.path().join("plate.tif");
    let mut child = Command::new(gs).args(["-q", "-dSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=tiffsep", "-dUseCropBox", "-dFIXEDMEDIA", "-dPDFFitPage", "-g1200x1200", "-r144"])
        .arg(format!("-dFirstPage={}", page + 1)).arg(format!("-dLastPage={}", page + 1))
        .arg(format!("-sOutputFile={}", output.display())).arg(source)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    let start = Instant::now();
    loop {
        let cancelled = !progress();
        if cancelled || start.elapsed() > Duration::from_secs(60) {
            let _ = child.kill(); let _ = child.wait();
            return Err(if cancelled { OpdfError::Cancelled } else { OpdfError::Conversion("Ink plate preview timed out after 60 seconds".into()) });
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(status)) => return Err(OpdfError::Conversion(format!("The local ink plate renderer failed ({status})"))),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => { let _ = child.kill(); let _ = child.wait(); return Err(e.into()); }
        }
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir.path())? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(ink) = name.strip_prefix("plate(").and_then(|n| n.strip_suffix(").tif")) {
            files.push((ink.to_string(), entry.path()));
        }
    }
    if files.len() < 4 || files.len() > 32 { return Err(OpdfError::Conversion("The preview needs four process plates and at most 28 spot plates".into())); }
    files.sort_by_key(|(name, _)| (match name.as_str() { "Cyan" => 0, "Magenta" => 1, "Yellow" => 2, "Black" => 3, _ => 4 }, name.clone()));
    let mut plates = Vec::new();
    for (name, path) in files {
        let reader = image::ImageReader::open(&path)?.with_guessed_format()?;
        let (width, height) = reader.into_dimensions()?;
        if width == 0 || height == 0 || width > 1200 || height > 1200 { return Err(OpdfError::Conversion("The ink plate renderer returned an oversized preview".into())); }
        let pixels = image::open(path)?.to_rgba8();
        let cmyk = ink_cmyk(&name, bytes);
        plates.push(Plate { name, width, height, rgba: pixels.into_raw(), cmyk });
    }
    Ok(plates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object, Stream};

    /// Several inks ticked (Acrobat's Output Preview): each plate's coverage in its ink's colour; a spot ink
    /// takes its Separation alternate. Cyan and Magenta together make blue; Black alone stays grey-black.
    #[test]
    fn inks_shown_together_make_their_colours() {
        let plate = |name: &str, cov: u8, cmyk| Plate { name: name.into(), width: 1, height: 1, rgba: vec![255 - cov, 255 - cov, 255 - cov, 255], cmyk };
        let plates = [plate("Cyan", 255, [1.0, 0.0, 0.0, 0.0]), plate("Magenta", 255, [0.0, 1.0, 0.0, 0.0]), plate("Black", 128, [0.0, 0.0, 0.0, 1.0])];
        let (_, _, both) = composite(&plates, &[true, true, false]).unwrap();
        assert_eq!(&both[..3], &[0, 0, 255], "cyan and magenta: blue");
        let (_, _, none) = composite(&plates, &[false, false, false]).unwrap();
        assert_eq!(&none[..3], &[255, 255, 255], "no ink: paper");
        let (_, _, black) = composite(&plates, &[false, false, true]).unwrap();
        assert!(black[0] == black[1] && black[1] == black[2] && (120..=135).contains(&black[0]), "{black:?}");
        // A spot ink's alternate from its Separation space.
        let mut pdf = lopdf::Document::with_version("1.7");
        let f = pdf.add_object(dictionary! { "FunctionType" => 2, "Domain" => vec![0.into(), 1.into()], "C0" => vec![0.into(), 0.into(), 0.into(), 0.into()], "C1" => vec![0.into(), 0.5.into(), 1.into(), 0.into()], "N" => 1 });
        pdf.add_object(Object::Array(vec![Object::Name(b"Separation".to_vec()), Object::Name(b"BrandOrange".to_vec()), Object::Name(b"DeviceCMYK".to_vec()), f.into()]));
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        assert_eq!(ink_cmyk("BrandOrange", &bytes), [0.0, 0.5, 1.0, 0.0]);
    }

    #[test]
    fn output_preview_reads_painted_process_and_spot_plates_without_editing() {
        let mut pdf = lopdf::Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let spot = Object::Array(vec![Object::Name(b"Separation".to_vec()), Object::Name(b"BrandOrange".to_vec()), Object::Name(b"DeviceCMYK".to_vec()),
            dictionary! { "FunctionType" => 2, "Domain" => vec![0.into(), 1.into()], "C0" => vec![0.into(),0.into(),0.into(),0.into()], "C1" => vec![0.into(),0.5.into(),1.into(),0.into()], "N" => 1 }.into()]);
        let content = pdf.add_object(Stream::new(dictionary! {}, b"1 0 0 0 k 0 0 40 100 re f /Brand cs 1 scn 60 0 40 100 re f".to_vec()));
        let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),100.into(),100.into()],
            "Contents" => content, "Resources" => dictionary! { "ColorSpace" => dictionary! { "Brand" => spot } } });
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()] }.into());
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages }); pdf.trailer.set("Root", root);
        let mut bytes = Vec::new(); pdf.save_to(&mut bytes).unwrap();
        let before = bytes.clone();
        let plates = preview(&bytes, None, 0, &mut || true).unwrap();
        assert_eq!(plates.iter().take(4).map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Cyan", "Magenta", "Yellow", "Black"]);
        let cyan = &plates[0];
        let spot = plates.iter().find(|p| p.name == "BrandOrange").expect("spot plate, not its CMYK alternate");
        let black = &plates[3];
        let ink = |p: &Plate| p.rgba.as_chunks::<4>().0.iter().filter(|px| px[0] < 128).count();
        assert!(ink(cyan) > 1000 && ink(spot) > 1000);
        assert_eq!(ink(black), 0);
        assert_eq!(bytes, before);
        assert!(matches!(preview(&bytes, None, 0, &mut || false), Err(OpdfError::Cancelled)));
        assert!(preview(&bytes, None, 1, &mut || true).is_err());
    }
}
