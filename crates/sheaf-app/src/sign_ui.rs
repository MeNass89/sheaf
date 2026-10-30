//! Sign panel: the signature being made (typed, drawn or an image), its size, its preview, and
//! the job spec built from it and the place clicked in the page viewer.

use std::path::PathBuf;

use sheaf_app::sign_preview::{self, Picture};
use sheaf_app::{AppWindow, ToolOptions, ToolTab};
use sheaf_core::sign::{self, DigitalSpec, Graphic};
use sheaf_core::{OpdfError, Result};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::state::{refresh, with};

/// Signature width in points for Small, Medium and Large (Medium is PdfCraft's Fill & Sign size).
pub const WIDTHS: [f64; 3] = [100.0, sign::DEFAULT_WIDTH, 220.0];
/// Preview size in pixels (wide, as a signature is).
const PREVIEW: (u32, u32) = (600, 160);

#[derive(Default)]
pub struct SignState {
    /// 0 Type, 1 Draw, 2 Upload.
    pub mode: i32,
    pub name: String,
    /// The handwriting font of the typed name.
    pub font: sheaf_core::script::ScriptFont,
    pub strokes: Vec<Vec<[f64; 2]>>,
    pub image: Option<PathBuf>,
    /// 0 small, 1 medium, 2 large.
    pub size: i32,
    /// The current graphic drawn, and its height-to-width ratio as placed.
    pub preview: Option<(Image, f64)>,
}

impl SignState {
    pub fn new() -> Self {
        Self { size: 1, ..Default::default() }
    }

    /// The graphic of the chosen mode, if one is made.
    pub fn graphic(&self) -> Option<Graphic> {
        match self.mode {
            0 => (!self.name.trim().is_empty()).then(|| Graphic::Typed(self.name.trim().to_string(), self.font)),
            1 => (!self.strokes.is_empty()).then(|| Graphic::Drawn(self.strokes.clone())),
            _ => self.image.clone().map(Graphic::Image),
        }
    }

    pub fn width(&self) -> f64 {
        WIDTHS[self.size.clamp(0, 2) as usize]
    }

    /// Redraws the preview of the current graphic.
    fn redraw(&mut self) {
        self.preview = self.graphic().and_then(|g| {
            let aspect = sign::aspect(&g).ok()?;
            // Drawn at the aspect it is placed with, so the overlay matches the output.
            let h = ((PREVIEW.0 as f64 * aspect).round() as u32).clamp(8, PREVIEW.0 * 4);
            let pic = sign_preview::render(&g, PREVIEW.0, h)?;
            Some((to_image(&pic), aspect))
        });
    }
}

fn to_image(p: &Picture) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&p.rgba, p.w, p.h))
}

/// The Sign fields of `tab` (the viewer's part is filled with the placement).
pub fn fill(tab: &mut ToolTab, s: &SignState) {
    tab.sign_mode = s.mode;
    tab.sign_font = s.font.index() as i32;
    tab.sign_size = s.size;
    tab.sign_has_graphic = s.preview.is_some();
    tab.sign_preview = s.preview.as_ref().map(|p| p.0.clone()).unwrap_or_default();
    tab.sign_image_name = s.image.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into();
}

fn edit(tab_id: i32, f: impl FnOnce(&mut SignState)) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            f(&mut t.sign);
            t.sign.redraw();
        }
        refresh(a, tab_id);
    });
}

pub fn mode(tab_id: i32, mode: i32) {
    edit(tab_id, |s| s.mode = mode.clamp(0, 2));
}

pub fn typed(tab_id: i32, name: &str) {
    // The script font takes a bounded name; longer text is cut, not refused, as it is typed.
    let name: String = name.chars().take(sign::MAX_TYPED_CHARS).collect();
    edit(tab_id, |s| s.name = name);
}

/// Pad commands ("M x y L x y …", pad pixels) as strokes in pad units.
pub fn pad_strokes(commands: &str, w: f32, h: f32) -> Vec<Vec<[f64; 2]>> {
    sign_preview::parse_pad(commands, w as f64, h as f64)
}

pub fn drawn(tab_id: i32, commands: &str, w: f32, h: f32) {
    let strokes = sign_preview::parse_pad(commands, w as f64, h as f64);
    edit(tab_id, |s| s.strokes = strokes);
}

/// The handwriting font of the typed name (`ScriptFont::ALL` index).
pub fn font(tab_id: i32, font: i32) {
    edit(tab_id, |s| s.font = sheaf_core::script::ScriptFont::from_index(font.max(0) as usize));
}

pub fn size(tab_id: i32, size: i32) {
    edit(tab_id, |s| s.size = size.clamp(0, 2));
}

pub fn pick_image(ui: &AppWindow, tab_id: i32) {
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Choose your signature image")).add_filter("Image", &["png", "jpg", "jpeg"]);
    crate::dialog::pick_file(ui, d, move |ui, p| {
        // Refused here (format, size), not at signing time.
        if let Err(e) = sign::aspect(&Graphic::Image(p.clone())) {
            return crate::flow::toast(ui, &e.to_string());
        }
        // A photo or scan of a signature on paper: its background goes (PDF Expert's).
        let p = sign::clear_background(&p).unwrap_or(p);
        edit(tab_id, |s| s.image = Some(p));
    });
}

/// A Mac's Signatures ▸ Camera (`phone` false: the Mac's camera) or iPhone (Continuity Camera's
/// Take Photo): the photo of a signature on paper, its background taken out as for an image
/// uploaded. The helper is waited for off the UI thread.
pub fn capture_image(ui: &AppWindow, tab_id: i32, phone: bool) {
    use slint::ComponentHandle;
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let to = std::env::temp_dir().join(format!("sheaf-signature-{}-{tab_id}-{}.png", std::process::id(), if phone { "iphone" } else { "camera" }));
        let taken = if phone { crate::run::from_phone(&to, false) } else { crate::run::from_camera(&to) };
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if let Err(e) = taken {
                return crate::flow::toast(&ui, &e);
            }
            if let Err(e) = sign::aspect(&Graphic::Image(to.clone())) {
                return crate::flow::toast(&ui, &e.to_string());
            }
            let p = sign::clear_background(&to).unwrap_or(to);
            edit(tab_id, |s| s.image = Some(p));
        });
    });
}

/// The certificate to sign with, when the panel asks for one.
pub fn digital(opts: &ToolOptions) -> Result<Option<DigitalSpec>> {
    if !opts.sign_cert {
        return Ok(None);
    }
    let p12 = opts.sign_p12.trim();
    if p12.is_empty() {
        return Err(OpdfError::Unsupported("Choose your certificate file first".into()));
    }
    let text = |s: &slint::SharedString| (!s.trim().is_empty()).then(|| s.trim().to_string());
    Ok(Some(DigitalSpec { p12: p12.into(), password: opts.sign_p12_password.to_string(), reason: text(&opts.sign_reason), location: text(&opts.sign_location), certify: (opts.sign_certify > 0).then_some(opts.sign_certify.clamp(1, 3) as u8) }))
}

/// The time stamp server to ask, when the panel asks for a time stamp.
pub fn tsa(opts: &ToolOptions) -> Result<Option<String>> {
    if !opts.sign_timestamp {
        return Ok(None);
    }
    let url = opts.sign_tsa.trim();
    if url.is_empty() {
        return Err(OpdfError::Unsupported("Add a time stamp server in Settings ▸ Tools first".into()));
    }
    sheaf_core::timestamp::HttpAuthority::new(url)?;
    Ok(Some(url.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_graphic_follows_the_mode() {
        let mut s = SignState::new();
        assert_eq!(s.width(), 150.0);
        assert!(s.graphic().is_none());
        s.name = "  Ada ".into();
        assert_eq!(s.graphic(), Some(Graphic::Typed("Ada".into(), Default::default())));
        s.mode = 1;
        assert!(s.graphic().is_none(), "nothing drawn yet");
        s.strokes = vec![vec![[0.0, 0.0], [1.0, 0.2]]];
        s.redraw();
        let (_, aspect) = s.preview.as_ref().unwrap();
        assert!((aspect - 0.2).abs() < 1e-9, "{aspect}");
        s.mode = 2;
        s.redraw();
        assert!(s.preview.is_none());
    }

    #[test]
    fn a_certificate_needs_its_file() {
        let mut o = ToolOptions::default();
        assert_eq!(digital(&o).unwrap(), None);
        o.sign_cert = true;
        assert!(digital(&o).is_err());
        o.sign_p12 = "/ids/me.p12".into();
        o.sign_p12_password = "pw".into();
        o.sign_reason = " ".into();
        o.sign_location = "Paris".into();
        let d = digital(&o).unwrap().unwrap();
        assert_eq!((d.p12, d.password.as_str(), d.reason, d.location.as_deref()), (PathBuf::from("/ids/me.p12"), "pw", None, Some("Paris")));
    }

    #[test]
    fn a_time_stamp_needs_a_server_address() {
        let mut o = ToolOptions::default();
        assert_eq!(tsa(&o).unwrap(), None);
        o.sign_timestamp = true;
        assert!(tsa(&o).is_err());
        o.sign_tsa = "ftp://nope".into();
        assert!(tsa(&o).is_err());
        o.sign_tsa = " https://freetsa.org/tsr ".into();
        assert_eq!(tsa(&o).unwrap().as_deref(), Some("https://freetsa.org/tsr"));
    }
}
