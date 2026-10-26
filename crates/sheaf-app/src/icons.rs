//! Tool tile colours and Lucide icons shipped in `ui/icons`.

use sheaf_core::tools::ToolId;
use slint::{Color, Image};

/// Colour of the square tile behind a tool's icon.
pub fn tile_colour(id: ToolId) -> Color {
    let hex: u32 = match id {
        ToolId::Merge
        | ToolId::Split
        | ToolId::RemovePages
        | ToolId::ExtractPages
        | ToolId::Organize
        | ToolId::Rotate => 0xEE6C4D,
        ToolId::Compress | ToolId::Repair | ToolId::Ocr => 0x8FBC5D,
        ToolId::AdjustColours => 0xD9A21B,
        ToolId::JpgToPdf | ToolId::PdfToJpg => 0xF2B705,
        ToolId::WordToPdf | ToolId::PdfToWord => 0x2B579A,
        ToolId::PowerPointToPdf | ToolId::PdfToPowerPoint => 0xD24726,
        ToolId::ExcelToPdf | ToolId::PdfToExcel => 0x217346,
        ToolId::HtmlToPdf | ToolId::PdfToText => 0xE44D26,
        ToolId::PdfToPdfA => 0x4A6FA5,
        ToolId::Edit | ToolId::PageNumbers | ToolId::Watermark | ToolId::Crop => 0x9B59B6,
        ToolId::Sign | ToolId::Protect | ToolId::Unlock => 0x4A6FA5,
        ToolId::Redact => 0xC0392B,
        ToolId::Compare => 0x16A085,
    };
    Color::from_rgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// SVG source of a Lucide icon shipped in `ui/icons`.
pub fn icon_svg(name: &str) -> Option<&'static [u8]> {
    macro_rules! icons {
        ($($n:literal),* $(,)?) => {
            match name {
                $($n => Some(include_bytes!(concat!("../ui/icons/", $n, ".svg")).as_slice()),)*
                _ => None,
            }
        };
    }
    icons!(
        "combine", "scissors", "file-minus", "file-output", "layout-grid", "rotate-cw",
        "minimize-2", "wrench", "scan-text", "image", "file-text", "presentation", "sheet",
        "code", "file-code", "images", "file-type", "archive", "pen-line", "list-ordered", "stamp", "crop",
        "signature", "lock", "lock-open", "eraser", "git-compare", "sun",
    )
}

pub fn icon_image(name: &str) -> Image {
    icon_svg(name)
        .and_then(|svg| Image::load_from_svg_data(svg).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_core::tools;

    #[test]
    fn every_tool_has_an_icon_and_a_tile() {
        for t in tools::catalogue() {
            let svg = icon_svg(t.icon).unwrap_or_else(|| panic!("no icon for {}", t.icon));
            let img = Image::load_from_svg_data(svg).expect("svg loads");
            let size = img.size();
            assert!(size.width > 0 && size.height > 0, "empty icon {}", t.icon);
            assert_eq!(tile_colour(t.id).alpha(), 255, "tile colour for {}", t.name);
        }
    }
}
