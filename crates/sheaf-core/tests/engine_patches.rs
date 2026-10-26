//! Sheaf's own pieces of PdfCraft (third_party/pdfcraft-engine), whose crate tests are left out.

use sheaf_core::pdfcraft::engine::{copy_image, sfn};

#[test]
fn notation_reads_each_field_as_a_number() {
    assert_eq!(
        sfn::notation_js("(Price + Tax\\ rate) * Qty.1"),
        "/** BVCALC (Price + Tax\\ rate) * Qty.1 EVCALC **/ event.value = (AFMakeNumber(getField(\"Price\").value) + AFMakeNumber(getField(\"Tax rate\").value)) * AFMakeNumber(getField(\"Qty.1\").value);"
    );
}

#[test]
fn a_drawing_beside_a_picture_is_told_from_the_picture_alone() {
    assert!(copy_image::paints_paths(b"q 100 0 0 50 0 0 cm /Im0 Do Q 1 0 0 RG 2 w 0 0 m 100 50 l S"));
    assert!(!copy_image::paints_paths(b"q 100 0 0 50 0 0 cm /Im0 Do Q"));
    // Text with "S" in its strings, or an inline image's bytes, is no path.
    assert!(!copy_image::paints_paths(b"BT /F1 12 Tf (S f) Tj ET BI /W 1 /H 1 ID f S EI"));
    assert!(!copy_image::paints_paths(b"0 0 10 10 re W n /Im0 Do"));
}
