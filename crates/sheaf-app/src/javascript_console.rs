//! Acrobat's run-selection command uses byte offsets from the multiline editor.
use sheaf_core::pdfcraft::engine::js::Request;

pub fn selection(source: &str, anchor: i32, cursor: i32) -> Result<&str, String> {
    let start = usize::try_from(anchor.min(cursor)).map_err(|_| "Invalid script selection")?;
    let end = usize::try_from(anchor.max(cursor)).map_err(|_| "Invalid script selection")?;
    if start == end { return Err("Select the JavaScript to run first".into()); }
    source.get(start..end).ok_or_else(|| "The selection changed. Select the script again".into())
}

/// A request that is deliberately not dispatched must have a visible explanation.
pub fn request_notice(request: &Request) -> Option<String> {
    match request {
        Request::LaunchUrl(url) => Some(format!("Network request not executed (outside this feature's scope): open {url}")),
        Request::Submit(url) => Some(format!("Network request not executed (outside this feature's scope): submit to {url}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_selection_keeps_unicode_boundaries_reverse_ranges_and_visible_network_refusals() {
        let source = "// é\nconsole.println('Selected');\nthrow new Error('Not selected');";
        let start = source.find("console").unwrap() as i32;
        let end = source.find("throw").unwrap() as i32;
        assert_eq!(selection(source, end, start).unwrap(), "console.println('Selected');\n");
        assert!(selection(source, 4, 5).is_err());
        assert!(selection(source, 0, 0).is_err());
        assert!(selection(source, -1, 10).is_err());
        assert!(selection(source, 0, i32::MAX).is_err());
        for request in [Request::LaunchUrl("https://example.test".into()), Request::Submit("https://example.test/form".into())] {
            let note = request_notice(&request).unwrap();
            assert!(note.contains("not executed")); assert!(note.contains("example.test"));
        }
        assert!(request_notice(&Request::Print).is_none());
    }
}
