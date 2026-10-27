mod common;

use sheaf_core::{pages::Input, pdfcraft::Doc, press::{self, Options}};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn pdfx3_uses_the_chosen_press_profile_and_preserves_the_working_document() {
    let Some(gs) = press::find_ghostscript() else { eprintln!("skip: no Ghostscript"); return };
    let profile = std::env::var_os("SHEAF_TEST_PRESS_PROFILE").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/share/color/icc/colord/FOGRA39L_coated.icc"));
    if !profile.is_file() { eprintln!("skip: set SHEAF_TEST_PRESS_PROFILE to a real CMYK press ICC"); return; }
    let t = tempfile::tempdir().unwrap();
    let source = common::fixture_pdf(t.path(), "source.pdf", 1);
    let doc = Doc::open(&Input::new(&source)).unwrap();
    let before = doc.full_bytes().unwrap();
    let options = Options { level: press::Level::X3, profile, condition: "FOGRA39 coated test condition".into() };
    let out = t.path().join("press.pdf");
    std::fs::write(&out, "existing press copy").unwrap();
    let written = press::to_pdfx3_with(&doc, &options, &out, &gs, Duration::from_secs(120)).unwrap();
    assert_ne!(written, out);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "existing press copy");
    let pdf = lopdf::Document::load(&written).unwrap();
    assert_eq!(pdf.version, "1.3");
    let info = pdf.get_dictionary(pdf.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();
    assert_eq!(info.get(b"GTS_PDFXVersion").unwrap().as_str().unwrap(), b"PDF/X-3:2002");
    assert_eq!(common::shown_texts(&written), ["Page 1"]);
    // The working document is as it was: the same objects (a full write's dates may differ).
    let (was, now) = (lopdf::Document::load_mem(&before).unwrap(), lopdf::Document::load_mem(&doc.full_bytes().unwrap()).unwrap());
    assert_eq!(was.get_pages().len(), now.get_pages().len());
    assert_eq!(was.extract_text(&[1]).unwrap(), now.extract_text(&[1]).unwrap());
    assert_eq!(was.objects.len(), now.objects.len());
    assert!(Doc::open(&Input::new(&written)).unwrap().document().print_readiness().fonts_not_embedded.is_empty());
    common::qpdf_clean(&written);
}

#[cfg(unix)]
#[test]
fn a_converter_that_claims_success_without_pdfx_and_a_timeout_publish_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempfile::tempdir().unwrap();
    let source = common::fixture_pdf(t.path(), "source.pdf", 1);
    let doc = Doc::open(&Input::new(&source)).unwrap();
    let mut header = vec![0; 128];
    header[..4].copy_from_slice(&128u32.to_be_bytes());
    header[12..16].copy_from_slice(b"prtr");
    header[16..20].copy_from_slice(b"CMYK");
    header[36..40].copy_from_slice(b"acsp");
    let profile = t.path().join("fake.icc");
    std::fs::write(&profile, header).unwrap();
    let options = Options { level: press::Level::X3, profile, condition: "Test only".into() };
    let fake = t.path().join("fake-gs");
    // Produces an ordinary PDF with exit 0: a successful process alone is not a press export.
    std::fs::write(&fake, r#"#!/bin/sh
for arg do
  case "$arg" in -sOutputFile=*) out=${arg#*=} ;; esac
  last=$arg
done
cp "$last" "$out"
"#).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = t.path().join("out.pdf");
    assert!(press::to_pdfx3_with(&doc, &options, &out, &fake, Duration::from_secs(5)).is_err());
    assert!(!out.exists());
    std::fs::write(&fake, "#!/bin/sh\nexec sleep 30\n").unwrap();
    let start = std::time::Instant::now();
    let error = press::to_pdfx3_with(&doc, &options, &out, &fake, Duration::from_millis(250)).unwrap_err();
    assert!(error.to_string().contains("timed out"));
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(!out.exists());
}
