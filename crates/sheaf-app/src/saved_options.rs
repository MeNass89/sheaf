//! A tool's options kept with its task history entry, so that Run again starts from them, not
//! from the defaults (Acrobat's Action Wizard keeps each step's settings; Sol 25/10). Passwords
//! (open, permissions, certificate) are never written to disk: they are typed again.

use serde_json::{Map, Value};

use crate::ToolOptions;

macro_rules! kept {
    (ints: $($i:ident),*; bools: $($b:ident),*; strings: $($s:ident),*) => {
        /// The options as JSON, passwords left out.
        pub fn to_json(o: &ToolOptions) -> Map<String, Value> {
            let mut m = Map::new();
            $(m.insert(stringify!($i).into(), o.$i.into());)*
            $(m.insert(stringify!($b).into(), o.$b.into());)*
            $(m.insert(stringify!($s).into(), o.$s.as_str().into());)*
            m
        }

        /// Options read back from [`to_json`]; a key missing or of another type keeps its default.
        pub fn from_json(m: &Map<String, Value>) -> ToolOptions {
            let mut o = ToolOptions { pdfa_allow_incomplete: true, ..Default::default() };
            $(if let Some(v) = m.get(stringify!($i)).and_then(Value::as_i64) { o.$i = v as i32; })*
            $(if let Some(v) = m.get(stringify!($b)).and_then(Value::as_bool) { o.$b = v; })*
            $(if let Some(v) = m.get(stringify!($s)).and_then(Value::as_str) { o.$s = v.into(); })*
            o
        }
    };
}

kept! {
    ints:
        redact_fill, sign_certify, html_screen_width,
        split_cols, split_rows,
        per_sheet,
        crop_paper,
        bates_digits,
        level, blank_level, rotate_parity, rotate_orientation, split_mode, every, degrees, layout, margin, quality,
        num_position, num_margin, num_text, num_color, num_family, wm_color, wm_family, wm_scale, wm_position,
        wm_transparency, wm_rotation, crop_mode, pdfa_level, excel_layout, text_format, compare_mode,
        adjust_brightness, adjust_contrast, adjust_saturation;
    bools:
        pdfa_allow_incomplete, html_long_page,
        redact_repeat, discard_thumbs, discard_tags, discard_meta, compress_gray, unembed_fonts, flatten_transparency, linearize, adjust_invert,
        hf_replace,
        allow_printing, allow_copying, allow_editing, allow_commenting, allow_filling_forms, remove_blank,
        protect_sanitize, split_one_file, landscape, one_per_image, png, extract_images, num_double, num_cover,
        num_bold, num_italic, num_underline, wm_image_mode, wm_bold, wm_italic, wm_underline, wm_mosaic, wm_below,
        wm_replace, wm_remove, ocr_local_engine, ocr_all_pages, ocr_enhance, export_ocr, export_exact, sign_cert, sign_timestamp;
    strings:
        redact_overlay, redact_margin, compress_ppi, compress_quality,
        organize_order,
        hf_tl, hf_tc, hf_tr, hf_bl, hf_bc, hf_br, bates_prefix, bates_start, compress_target,
        pages, compare_old_pages, compare_new_pages, ranges, split_size, out_name, from_page, to_page, num_start,
        num_custom, num_font_size, num_hex, wm_text, wm_font_size, wm_hex, wm_image, crop_top, crop_bottom,
        crop_left, crop_right, ocr_languages, sign_p12, sign_reason, sign_location, sign_tsa
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archival_history_keeps_the_old_copy_and_issues_default_but_honours_strict() {
        let mut old = Map::new();
        assert!(from_json(&old).pdfa_allow_incomplete);
        old.insert("pdfa_allow_incomplete".into(), false.into());
        assert!(!from_json(&old).pdfa_allow_incomplete);
        let strict = from_json(&old);
        assert!(!from_json(&to_json(&strict)).pdfa_allow_incomplete);
    }

    #[test]
    fn options_come_back_as_they_were_without_their_passwords() {
        let o = ToolOptions { flatten_transparency: true, linearize: true, html_screen_width: 1280, html_long_page: true, pdfa_allow_incomplete: true, ocr_local_engine: true, level: 2, degrees: 270, pages: "2-4".into(), remove_blank: true, wm_text: "DRAFT".into(), password: "secret".into(), owner: "boss".into(), sign_p12_password: "pin".into(), ..Default::default() };
        let json = to_json(&o);
        let text = serde_json::to_string(&json).unwrap();
        assert!(!text.contains("secret") && !text.contains("boss") && !text.contains("pin"), "{text}");
        assert_eq!(from_json(&json), ToolOptions { password: "".into(), owner: "".into(), sign_p12_password: "".into(), ..o });
    }
}
