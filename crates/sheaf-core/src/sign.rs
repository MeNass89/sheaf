//! Sign PDF: a signature drawn, typed in a script font or taken from an image, placed in a box on
//! a page, optionally sealed with a digital certificate (a `.p12` / `.pfx` digital ID).
//!
//! Without a certificate the signature becomes page content (PdfCraft's Fill & Sign shapes,
//! flattened), upright as the page is displayed even on a page with `/Rotate`. With one,
//! PdfCraft signs the document (CAdES detached, an incremental update) and its bytes are written
//! as they are: re-saving them would break the signature. Earlier signatures stay valid because
//! an already-signed input is only ever updated incrementally.
//!
//! A password-protected input is refused: signing must neither keep a protection the user did
//! not choose to keep nor strip it silently. The user unlocks the PDF first.

use crate::pdfcraft::engine::{self, Edit, MarkFile, NewAnnotation, Shape, SignOptions, SignatureImage, Style};
use crate::pdfcraft::{render::PageInfo, Doc};
use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the signature looks like.
#[derive(Debug, Clone, PartialEq)]
pub enum Graphic {
    /// The name, typed in a handwriting font.
    Typed(String, crate::script::ScriptFont),
    /// Strokes drawn on a pad, each a list of points, y up, in units where the pad is 1 wide.
    Drawn(Vec<Vec<[f64; 2]>>),
    /// A PNG or JPEG image of a signature.
    Image(PathBuf),
}

/// A digital ID to sign with.
#[derive(Debug, Clone, PartialEq)]
pub struct DigitalSpec {
    pub p12: PathBuf,
    pub password: String,
    pub reason: Option<String>,
    pub location: Option<String>,
}

/// Where and how to sign.
#[derive(Debug, Clone, PartialEq)]
pub struct SignSpec {
    /// 0-based page.
    pub page: usize,
    /// The signature box `[x0, y0, x1, y1]` in PDF user space. The graphic is fitted inside it,
    /// centred and upright as the page is displayed.
    pub rect: [f64; 4],
    pub graphic: Graphic,
    pub digital: Option<DigitalSpec>,
}

/// What [`sign`] wrote.
#[derive(Debug, Clone)]
pub struct SignReport {
    pub path: PathBuf,
    /// The certificate's name (its common name, else organisation) when signed digitally.
    pub signer: Option<String>,
}

impl SignReport {
    /// The result line, in Sheaf's CLI and app alike.
    pub fn summary(&self) -> String {
        match &self.signer {
            Some(cn) => format!("Signed digitally (certificate: {cn})"),
            None => "Signed".into(),
        }
    }
}

/// The default width of a placed signature, in points (PdfCraft's Fill & Sign uses the same).
pub const DEFAULT_WIDTH: f64 = 150.0;

/// The script font's limit on a typed signature.
pub const MAX_TYPED_CHARS: usize = engine::MAX_SIGNATURE_CHARS;

/// The graphic, decoded and checked, ready to place.
enum Ready {
    Typed(engine::ScriptOutline),
    Drawn(Vec<Vec<[f64; 2]>>, [f64; 4]),
    Image(SignatureImage),
}

impl Ready {
    fn new(g: &Graphic) -> Result<Ready> {
        match g {
            Graphic::Typed(t, font) => {
                let t = t.trim();
                if t.is_empty() {
                    return Err(OpdfError::Unsupported("type a name to sign with".into()));
                }
                if t.chars().count() > MAX_TYPED_CHARS {
                    return Err(OpdfError::Unsupported(format!("a typed signature can have at most {MAX_TYPED_CHARS} characters")));
                }
                let o = crate::script::outline(*font, t);
                if !o.contours.iter().any(|c| c.len() > 2) {
                    return Err(OpdfError::Unsupported("the signature font has no letters for that name".into()));
                }
                Ok(Ready::Typed(o))
            }
            Graphic::Drawn(strokes) => {
                let strokes: Vec<Vec<[f64; 2]>> = strokes.iter().filter(|s| !s.is_empty()).cloned().collect();
                if strokes.iter().flatten().any(|p| !p.iter().all(|v| v.is_finite())) {
                    return Err(OpdfError::Unsupported("the drawn signature has a point that isn't a number".into()));
                }
                // Ink is drawn as smooth curves, which stay within their points and control
                // points: the bounds the signer fits by too, so both paths place it alike.
                let controls: Vec<[f64; 2]> = strokes
                    .iter()
                    .flat_map(|s| engine::annot_text::smooth_segments(&s.iter().map(|p| (p[0], p[1])).collect::<Vec<_>>()))
                    .flat_map(|[a, c, _]| [[a.0, a.1], [c.0, c.1]])
                    .collect();
                let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
                for p in strokes.iter().flatten().chain(&controls) {
                    b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
                }
                if !b.iter().all(|v| v.is_finite()) {
                    return Err(OpdfError::Unsupported("draw a signature first".into()));
                }
                Ok(Ready::Drawn(strokes, b))
            }
            Graphic::Image(path) => {
                let file = std::fs::File::open(path).map_err(|e| OpdfError::Unsupported(format!("{}: {e}", path.display())))?;
                SignatureImage::read(file).map(Ready::Image).map_err(|e| OpdfError::Unsupported(e.to_string()))
            }
        }
    }

    /// The graphic's own width and height (any unit): what it is fitted by.
    fn extent(&self) -> (f64, f64) {
        match self {
            Ready::Typed(o) => {
                let [l, b, r, t] = o.bounds();
                (r - l, t - b)
            }
            // A dot or straight line still gets a box (pen width).
            Ready::Drawn(_, b) => ((b[2] - b[0]).max(0.02), (b[3] - b[1]).max(0.02)),
            Ready::Image(i) => (i.size()[0] as f64, i.size()[1] as f64),
        }
    }

    /// As the graphic of a visible digital signature.
    fn digital(&self) -> Option<engine::sign::Graphic> {
        match self {
            Ready::Typed(o) => engine::typed_signature_graphic_from(o.clone()),
            Ready::Drawn(s, _) => Some(engine::sign::Graphic::Strokes(s.clone())),
            Ready::Image(i) => Some(i.graphic()),
        }
    }
}

/// The height-to-width ratio of `graphic` as it will be placed, so a box can be sized for it.
pub fn aspect(graphic: &Graphic) -> Result<f64> {
    let (w, h) = Ready::new(graphic)?.extent();
    Ok(h / w.max(1e-9))
}

/// The user-space box for a signature `width` points wide and `width * aspect` tall as the page
/// is displayed, centred on `center` (view space: points from the displayed page's top-left,
/// y down, as the page viewer and text layer use).
pub fn box_at(page: &PageInfo, center: [f64; 2], width: f64, aspect: f64) -> [f64; 4] {
    let h = width * aspect;
    view_rect_to_user(page, [center[0] - width / 2.0, center[1] - h / 2.0, center[0] + width / 2.0, center[1] + h / 2.0])
}

/// A view-space rectangle (`[x0, y0, x1, y1]`, y down) → the same area in user space.
pub fn view_rect_to_user(page: &PageInfo, r: [f64; 4]) -> [f64; 4] {
    let a = page.view_to_user(r[0] as f32, r[1] as f32);
    let b = page.view_to_user(r[2] as f32, r[3] as f32);
    let (a, b) = (a.map(f64::from), b.map(f64::from));
    [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
}

/// A user-space rectangle → view space (`[x0, y0, x1, y1]`, y down).
pub fn user_rect_to_view(page: &PageInfo, r: [f64; 4]) -> [f64; 4] {
    let a = page.user_to_view(r[0] as f32, r[1] as f32);
    let b = page.user_to_view(r[2] as f32, r[3] as f32);
    let (a, b) = (a.map(f64::from), b.map(f64::from));
    [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
}

/// A signature photographed or scanned on paper: the paper turns transparent and the ink keeps its
/// colour, edges softened (PDF Expert's background removal). Written as a PNG among the system's
/// temporary files and returned; a picture that already has transparency is returned as it is.
pub fn clear_background(path: &Path) -> Result<PathBuf> {
    let img = image::open(path).map_err(|e| OpdfError::Unsupported(format!("{}: {e}", path.display())))?.to_rgba8();
    if img.pixels().any(|p| p[3] < 250) {
        return Ok(path.to_path_buf());
    }
    let lum = |p: &image::Rgba<u8>| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
    // The paper: the brightness most of the picture has (its 80th percentile).
    let mut l: Vec<u8> = img.pixels().map(|p| lum(p) as u8).collect();
    l.sort_unstable();
    let paper = (l[l.len() * 8 / 10] as f64).max(1.0);
    let mut out = img.clone();
    for p in out.pixels_mut() {
        // Darker than the paper by a third or more: ink, opaque; between: a soft edge.
        let a = ((paper - lum(p)) / (paper * 0.33)).clamp(0.0, 1.0);
        p[3] = (a * 255.0).round() as u8;
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    img.as_raw().hash(&mut h);
    let target = std::env::temp_dir().join(format!("sheaf-signature-{:016x}.png", h.finish()));
    out.save(&target).map_err(|e| OpdfError::Unsupported(format!("{}: {e}", target.display())))?;
    Ok(target)
}

/// Page `page` (0-based) of `input`, as displayed: what [`box_at`] needs.
pub fn page_info(input: &Input, page: usize) -> Result<PageInfo> {
    let doc = open(input)?;
    let pages = &doc.document().info.pages;
    pages.get(page).cloned().ok_or_else(|| no_page(page, pages.len()))
}

fn no_page(page: usize, n: usize) -> OpdfError {
    OpdfError::Unsupported(format!("page {} does not exist (the document has {n} pages)", page + 1))
}

/// Opens `input` with its password. A protected file stays protected once signed (the save keeps
/// its encryption, as Acrobat's does); one whose owner password forbids filling in forms and
/// adding comments, which a signature is, is refused (Acrobat's "Signing: Not Allowed").
fn open(input: &Input) -> Result<Doc> {
    let doc = Doc::open(input)?;
    if doc.was_encrypted() {
        let restricted = crate::facts::read(&std::fs::read(&input.path)?, input.password.as_deref()).restricted;
        if restricted.contains(&"Filling in forms") && restricted.contains(&"Adding comments") {
            return Err(OpdfError::Unsupported(format!("{}'s owner password does not allow signing: unlock the PDF first, then sign it", input.path.display())));
        }
    }
    Ok(doc)
}

/// Signs `input` as `spec` says and writes the result to `out` (never overwriting). Nothing is
/// written when anything fails: a wrong certificate password is
/// [`OpdfError::CertificatePassword`], checked before the document is touched.
pub fn sign(input: &Input, spec: &SignSpec, out: &Path) -> Result<SignReport> {
    let ready = Ready::new(&spec.graphic)?;
    let id = match &spec.digital {
        Some(d) => Some(digital_id(d)?),
        None => None,
    };
    let mut doc = open(input)?;
    let info = doc.document().info.pages.get(spec.page).cloned().ok_or_else(|| no_page(spec.page, doc.document().info.pages.len()))?;
    let r = spec.rect;
    if !(r.iter().all(|v| v.is_finite()) && r[2] - r[0] >= 1.0 && r[3] - r[1] >= 1.0) {
        return Err(OpdfError::Unsupported("the signature box is empty".into()));
    }
    let view = user_rect_to_view(&info, r);
    let already_signed = doc.document().is_signed();
    let Some((id, d)) = id.as_ref().zip(spec.digital.as_ref()) else {
        // Visual only: page content. A signed input keeps its signatures: an incremental update
        // that adds the signature as a Fill & Sign annotation (an allowed change), unflattened.
        doc.apply(place(spec.page, &info, view, &ready)?)?;
        let path = if already_signed {
            doc.save_incremental(out)?
        } else {
            doc.apply(Edit::FlattenFillSign)?;
            doc.save(out)?
        };
        return Ok(SignReport { path, signer: None });
    };
    // On an unturned page the graphic goes in the signature's own appearance. PdfCraft draws that
    // appearance unturned, so on a page shown turned it would read sideways: there the graphic
    // is drawn upright onto the page first and the certificate signs invisibly.
    let upright = info.rotation % 360 == 0;
    let graphic = if upright {
        Some(ready.digital().ok_or_else(|| OpdfError::Unsupported("the signature has nothing to draw".into()))?)
    } else {
        doc.apply(place(spec.page, &info, view, &ready)?)?;
        if !already_signed {
            doc.apply(Edit::FlattenFillSign)?;
        }
        None
    };
    let some = |s: &Option<String>| s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let (reason, location) = (some(&d.reason), some(&d.location));
    // No label in the box: the graphic alone, drawn as the preview and the visual-only path draw
    // it. PdfCraft insets the graphic by a pad, so the widget is grown by that pad to keep the
    // ink in the box asked for. Reason and location stay in the signature dictionary.
    let opts = SignOptions {
        page: spec.page,
        rect: upright.then(|| padded(r)),
        reason,
        location,
        appearance: engine::sign::Appearance { name: false, date: false, reason: false, location: false, distinguished_name: false, labels: false },
        graphic,
        ..SignOptions::default()
    };
    let bytes = doc.session().sign(doc.id(), id, opts).map_err(|e| OpdfError::Engine(format!("signing failed: {e}")))?;
    // Written exactly as PdfCraft produced them: any re-save would invalidate the signature.
    let path = atomic_write(out, |part| Ok(std::fs::write(part, bytes.as_slice())?))?;
    Ok(SignReport { path, signer: Some(id.certificate.display_name()) })
}

/// The widget rectangle whose inner area, once PdfCraft insets its graphic by its pad
/// (`clamp(0.06 × height, 1, 6)` on every side), is `r`.
fn padded(r: [f64; 4]) -> [f64; 4] {
    let h = r[3] - r[1];
    let full = h / 0.88;
    let pad = (0.06 * full).clamp(1.0, 6.0);
    [r[0] - pad, r[1] - pad, r[2] + pad, r[3] + pad]
}

/// Reads and unlocks a digital ID.
fn digital_id(d: &DigitalSpec) -> Result<engine::sign::DigitalId> {
    let bytes = std::fs::read(&d.p12).map_err(|e| OpdfError::Unsupported(format!("{}: {e}", d.p12.display())))?;
    engine::sign::pkcs12::open(&bytes, &d.password).map_err(|e| match e {
        engine::sign::SignError::WrongPassword => OpdfError::CertificatePassword(d.p12.clone()),
        e => OpdfError::Unsupported(format!("{}: not a usable digital ID ({e})", d.p12.display())),
    })
}

/// The edit placing a visible signature (no certificate) `width` points wide, centred on `center`
/// of page `page` (view space), as a Fill & Sign annotation: what the reader's Signature tool
/// adds. Undoable, and saved like any annotation.
pub fn signature_edit(info: &PageInfo, page: usize, center: [f64; 2], width: f64, graphic: &Graphic) -> Result<Edit> {
    let ready = Ready::new(graphic)?;
    let (w, h) = ready.extent();
    let aspect = h / w.max(1e-9);
    // Drawn ink is inset by half its pen width (see `place`): size the box so the ink still spans
    // the whole width, else a thin signature is fitted by its height and comes out narrower.
    let height = match ready {
        Ready::Drawn(..) => {
            let pen = Style::default_for(&Shape::Signature { strokes: Vec::new() }).width;
            (width - pen).max(0.0) * aspect + pen
        }
        _ => width * aspect,
    };
    let user = box_at(info, center, width, height / width.max(1e-9));
    place(page, info, user_rect_to_view(info, user), &ready)
}

/// The edit placing `ready` centred in `view` (view space of page `page`), upright as displayed.
fn place(page: usize, info: &PageInfo, view: [f64; 4], ready: &Ready) -> Result<Edit> {
    // Drawn ink spreads half its pen width beyond the stroke: keep it inside the box, as the
    // signer does.
    let view = match ready {
        Ready::Drawn(..) => {
            let half = Style::default_for(&Shape::Signature { strokes: Vec::new() }).width / 2.0;
            [view[0] + half, view[1] + half, view[2] - half, view[3] - half]
        }
        _ => view,
    };
    let (gw, gh) = ready.extent();
    let (vw, vh) = (view[2] - view[0], view[3] - view[1]);
    let k = (vw / gw).min(vh / gh);
    // The graphic's displayed box, centred in the signature box.
    let (w, h) = (gw * k, gh * k);
    let (x0, y0) = (view[0] + (vw - w) / 2.0, view[1] + (vh - h) / 2.0);
    let to_user = |x: f64, y: f64| info.view_to_user(x as f32, y as f32).map(f64::from);
    let annot = |shape: Shape| {
        let style = Style::default_for(&shape);
        Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: String::new(), author: "Sheaf".into() })
    };
    let bad = || OpdfError::Unsupported("the signature can't be placed there".into());
    Ok(match ready {
        Ready::Typed(o) => {
            let at = to_user(x0, y0 + h / 2.0);
            annot(engine::typed_signature_shape_from(at, o, h, i64::from(info.rotation)).ok_or_else(bad)?)
        }
        Ready::Drawn(strokes, b) => {
            let strokes = strokes.iter().map(|s| s.iter().map(|p| to_user(x0 + (p[0] - b[0]) * k, y0 + (b[3] - p[1]) * k)).collect()).collect();
            annot(Shape::Signature { strokes })
        }
        Ready::Image(img) => Edit::AddCustomStamp {
            page,
            rect: view_rect_to_user(info, [x0, y0, x0 + w, y0 + h]),
            name: "Signature".into(),
            file: MarkFile { name: "signature.png".into(), bytes: Arc::clone(img.bytes()), page: 0 },
            author: "Sheaf".into(),
        },
    })
}
