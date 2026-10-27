//! Acrobat's Optimizer final passes: flatten before arranging bytes for Fast Web View.
//! Local executables are optional; a requested pass must succeed before anything is published.

use super::Advanced;
use crate::{OpdfError, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn run(command: &mut Command, progress: &mut dyn FnMut(f32) -> bool) -> Result<()> {
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
        .map_err(|e| OpdfError::Unsupported(format!("Install the requested local optimizer (Ghostscript or qpdf): {e}")))?;
    let started = Instant::now();
    loop {
        if !progress(0.92) || started.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if started.elapsed() > Duration::from_secs(120) {
                OpdfError::Conversion("The local optimizer timed out after 120 seconds; no output saved".into())
            } else { OpdfError::Cancelled });
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(OpdfError::Conversion(format!("The local optimizer failed ({status}); no output saved"))),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
    }
}

fn no_interactive_content(pdf: &lopdf::Document) -> Result<()> {
    let forbidden = pdf.catalog().is_ok_and(|c| c.has(b"AcroForm") || c.has(b"OCProperties") || c.has(b"StructTreeRoot") || c.has(b"AF") || c.has(b"Collection")
        || c.get(b"Names").ok().and_then(|n| pdf.dereference(n).ok()).and_then(|(_, n)| n.as_dict().ok()).is_some_and(|n| n.has(b"EmbeddedFiles")))
        || pdf.objects.values().any(|v| v.as_dict().is_ok_and(|d| d.get(b"Type").is_ok_and(|t| t.as_name().is_ok_and(|n| n == b"Sig"))));
    if forbidden {
        return Err(OpdfError::Unsupported("Flatten transparency needs a copy without interactive forms, layers, signatures, structure tags or attachments; no output saved".into()));
    }
    Ok(())
}

pub(crate) fn finish(bytes: Vec<u8>, options: &Advanced, progress: &mut dyn FnMut(f32) -> bool) -> Result<Vec<u8>> {
    finish_pages(bytes, options, None, progress)
}

fn finish_pages(bytes: Vec<u8>, options: &Advanced, page: Option<usize>, progress: &mut dyn FnMut(f32) -> bool) -> Result<Vec<u8>> {
    if !options.flatten_transparency && !options.linearize { return Ok(bytes); }
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("input.pdf");
    std::fs::write(&source, &bytes)?;
    let mut current = source;
    if options.flatten_transparency {
        let pdf = lopdf::Document::load_mem(&bytes)?;
        no_interactive_content(&pdf)?;
        let output = dir.path().join("flat.pdf");
        let gs = crate::press::find_ghostscript().ok_or_else(|| OpdfError::Unsupported("Flatten transparency needs a local Ghostscript installation".into()))?;
        let mut command = Command::new(gs);
        command.args(["-dSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=pdfwrite", "-dCompatibilityLevel=1.3", "-dAutoRotatePages=/None"]);
        if let Some(page) = page {
            if page >= pdf.get_pages().len() { return Err(OpdfError::Unsupported("The flattener preview page does not exist".into())); }
            command.arg(format!("-dFirstPage={}", page + 1)).arg(format!("-dLastPage={}", page + 1));
        }
        run(command.arg(format!("-sOutputFile={}", output.display())).arg(&current), progress)?;
        validate_flattened(&output, if page.is_some() { 1 } else { pdf.get_pages().len() })?;
        current = output;
        // Ghostscript can generate fresh metadata; explicit removal must still be honoured.
        if options.discard_metadata || options.discard_tags || options.discard_thumbnails || options.unembed_standard {
            let mut flat = lopdf::Document::load(&current)?;
            super::discard(&mut flat, options);
            flat.prune_objects();
            flat.save(&current)?;
        }
    }
    if options.linearize {
        let output = dir.path().join("web.pdf");
        run(Command::new("qpdf").arg("--linearize").arg(&current).arg(&output), progress)?;
        run(Command::new("qpdf").arg("--check-linearization").arg(&output), progress)?;
        current = output;
    }
    Ok(std::fs::read(current)?)
}

fn validate_flattened(path: &Path, expected_pages: usize) -> Result<()> {
    let pdf = lopdf::Document::load(path)?;
    if pdf.version != "1.3" || pdf.get_pages().len() != expected_pages || pdf.is_encrypted() {
        return Err(OpdfError::Conversion("Flattening did not produce PDF 1.3 with the original page count and no protection; no output saved".into()));
    }
    let doc = crate::pdfcraft::Doc::open(&crate::pages::Input::new(path))?;
    let readiness = doc.document().print_readiness();
    if readiness.pages.len() != expected_pages || readiness.findings.iter().any(|f| f.id == "transparency" || f.id == "resource-inventory-incomplete") {
        return Err(OpdfError::Conversion("The flattened file still has transparency or unreadable resources; no output saved".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object, Stream};

    fn transparent_pdf() -> Vec<u8> {
        let mut pdf = lopdf::Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let content = pdf.add_object(Stream::new(dictionary! {}, b"1 0 0 rg 10 10 80 80 re f /Alpha gs 0 0 1 rg 40 40 50 50 re f".to_vec()));
        let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()], "Contents" => content,
            "Resources" => dictionary! { "ExtGState" => dictionary! { "Alpha" => dictionary! { "ca" => 0.5 } } } });
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()] }.into());
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        pdf.trailer.set("Root", root);
        let mut bytes = Vec::new(); pdf.save_to(&mut bytes).unwrap(); bytes
    }

    #[test]
    fn optimizer_linearizes_and_flattens_before_publication() {
        let original = transparent_pdf();
        let options = Advanced { flatten_transparency: true, linearize: true, ..Default::default() };
        let bytes = finish(original, &options, &mut |_| true).unwrap();
        assert!(bytes.windows(11).any(|v| v == b"/Linearized"));
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out.pdf"); std::fs::write(&output, bytes).unwrap();
        validate_flattened(&output, 1).unwrap();
    }

    #[test]
    fn optimizer_cancellation_and_interactive_content_never_publish() {
        let bytes = transparent_pdf();
        let options = Advanced { linearize: true, ..Default::default() };
        assert!(matches!(finish(bytes.clone(), &options, &mut |_| false), Err(OpdfError::Cancelled)));
        for key in [b"AcroForm".as_slice(), b"OCProperties", b"StructTreeRoot", b"AF", b"Collection"] {
            let mut pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
            pdf.get_object_mut(root).unwrap().as_dict_mut().unwrap().set(key, Object::Dictionary(dictionary! {}));
            assert!(no_interactive_content(&pdf).is_err(), "{} must not be silently discarded", String::from_utf8_lossy(key));
            if key == b"StructTreeRoot" {
                super::super::discard(&mut pdf, &Advanced { discard_tags: true, ..Default::default() });
                no_interactive_content(&pdf).unwrap();
            }
        }
        let mut pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
        pdf.get_object_mut(root).unwrap().as_dict_mut().unwrap().set("Names", dictionary! { "EmbeddedFiles" => dictionary! {} });
        assert!(no_interactive_content(&pdf).is_err());
    }
}
