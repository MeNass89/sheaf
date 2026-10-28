//! iLovePDF's screen-width / one-long-page mode, using a local Chromium layout engine.
//! Rust owns offline preparation, bounds, process lifetime and verification. No URL capture.

use crate::{office, paths::atomic_write, OpdfError, Result};
use super::Options;
use std::{io::Read, path::{Path, PathBuf}, process::{Command, Stdio}, time::{Duration, Instant}};

/// A local browser is optional; never download one or silently replace screen layout with Office.
pub(super) fn find() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHEAF_CHROMIUM").map(PathBuf::from).filter(|p| p.is_file()) { return Some(path); }
    let names: &[&str] = if cfg!(windows) { &["chrome.exe", "msedge.exe"] } else { &["chromium", "chromium-browser", "google-chrome", "google-chrome-stable", "chrome-headless-shell"] };
    if let Some(path) = std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).flat_map(|dir| names.iter().map(move |name| dir.join(name))).find(|p| p.is_file())) { return Some(path); }
    for path in ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "/Applications/Chromium.app/Contents/MacOS/Chromium"] {
        if Path::new(path).is_file() { return Some(path.into()); }
    }
    if cfg!(windows) {
        for root in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
            if let Some(root) = std::env::var_os(root).map(PathBuf::from) {
                for suffix in ["Google/Chrome/Application/chrome.exe", "Microsoft/Edge/Application/msedge.exe"] {
                    let path = root.join(suffix); if path.is_file() { return Some(path); }
                }
            }
        }
    }
    None
}

pub(super) fn to_pdf(html: &str, options: Options, out: &Path) -> Result<PathBuf> {
    let browser = find().ok_or_else(|| OpdfError::Unsupported("Screen layout needs local Chromium, Chrome or Edge (or SHEAF_CHROMIUM). Choose LibreOffice paper layout, or install a browser".into()))?;
    to_pdf_with(html, options, out, &browser)
}

fn to_pdf_with(html: &str, options: Options, out: &Path, browser: &Path) -> Result<PathBuf> {
    let width = options.screen_width.ok_or_else(|| OpdfError::Unsupported("Choose a screen width".into()))?;
    if !(320..=3840).contains(&width) { return Err(OpdfError::Unsupported("Screen width must be 320 to 3840 pixels".into())); }
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|e| OpdfError::Conversion(format!("Cannot prepare local HTML: {e}")))?;
    let nonce: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let tmp = tempfile::tempdir()?;
    let source = tmp.path().join("screen.html");
    std::fs::write(&source, prepare(html, width, options.margin, &nonce, None)?)?;
    // Screen CSS is resolved before print media, at the requested width, in a fresh local profile.
    let dom = run(browser, &source, tmp.path(), width, None)?;
    let measured = measurement(&dom, &nonce)?;
    let height = if options.long_page { Some(measured) } else { None };
    if let Some(height) = height
        && f64::from(height) * 0.75 + options.margin * 2.0 > 14_400.0
    {
        return Err(OpdfError::Unsupported("This screen layout exceeds a 200-inch PDF page. Choose paper pages or a wider screen width".into()));
    }
    std::fs::write(&source, prepare(html, width, options.margin, &nonce, height)?)?;
    let printed = tmp.path().join("screen.pdf");
    run(browser, &source, tmp.path(), width, Some(&printed))?;
    check_written(&printed, options, height)?;
    atomic_write(out, |part| { std::fs::copy(&printed, part)?; Ok(()) })
}

fn prepare(html: &str, width: u32, margin: f64, nonce: &str, height: Option<u32>) -> Result<String> {
    if html.len() > 16 * 1024 * 1024 || !margin.is_finite() || !(0.0..=144.0).contains(&margin) {
        return Err(OpdfError::Unsupported("Local HTML must be at most 16 MB and margins 0 to 144 points".into()));
    }
    let html = super::source::without_refresh(html);
    let head = super::opening_tag(&html.to_ascii_lowercase(), "head")
        .ok_or_else(|| OpdfError::Conversion("The prepared HTML has no head".into()))?;
    // Only our nonced measurement script runs. Source scripts, remote CSS/images/fonts, frames,
    // form submissions and fetches are disabled; local relative resources still use the base URL.
    let policy = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src file: data:; style-src file: 'unsafe-inline'; font-src file: data:; script-src 'nonce-{nonce}'; base-uri file:; form-action 'none'\">");
    let page = height.map(|h| format!("@page {{ size: {}pt {}pt; margin: {margin}pt; }} * {{ break-before: auto !important; break-after: auto !important; break-inside: auto !important; }}", f64::from(width) * 0.75 + margin * 2.0, f64::from(h) * 0.75 + margin * 2.0)).unwrap_or_default();
    let page = serde_json::to_string(&page).map_err(|e| OpdfError::Conversion(e.to_string()))?;
    let script = r#"<script nonce="NONCE">
window.addEventListener('load', async () => {
  try {
    await document.fonts.ready;
    document.documentElement.style.width = 'WIDTHpx';
    function freeze(rules) {
      for (const rule of rules) {
        if (rule.type === CSSRule.MEDIA_RULE) {
          const enabled = matchMedia(rule.conditionText).matches;
          rule.media.mediaText = enabled ? 'all' : 'not all';
          if (enabled) freeze(rule.cssRules);
        } else if (rule.type === CSSRule.IMPORT_RULE) {
          const enabled = matchMedia(rule.media.mediaText || 'all').matches;
          rule.media.mediaText = enabled ? 'all' : 'not all';
          if (enabled && rule.styleSheet) freeze(rule.styleSheet.cssRules);
        } else if (rule.cssRules) { freeze(rule.cssRules); }
      }
    }
    for (const sheet of document.styleSheets) {
      const enabled = matchMedia(sheet.media.mediaText || 'all').matches;
      sheet.media.mediaText = enabled ? 'all' : 'not all';
      if (enabled) freeze(sheet.cssRules);
    }
    const style = document.createElement('style');
    style.textContent = PAGE;
    document.head.append(style);
    const body = document.body;
    let bottom = Math.max(body.scrollHeight, body.getBoundingClientRect().bottom + scrollY);
    const elements = body.querySelectorAll('*');
    if (elements.length > 100000) throw new Error('layout limit');
    for (const element of elements) {
      bottom = Math.max(bottom, element.getBoundingClientRect().bottom + scrollY);
    }
    const marker = document.createElement('meta');
    marker.name = 'sheaf-screen-NONCE';
    marker.content = String(Math.ceil(bottom + parseFloat(getComputedStyle(body).marginBottom || '0')));
    document.head.append(marker);
  } catch (error) { /* Missing marker makes Rust refuse this layout, with no false fallback. */ }
});
</script>"#.replace("NONCE", nonce).replace("WIDTH", &width.to_string()).replace("PAGE", &page);
    Ok(format!("{}{}{}{}", &html[..head], policy, script, &html[head..]))
}

fn measurement(dom: &[u8], nonce: &str) -> Result<u32> {
    let text = String::from_utf8_lossy(dom);
    let marker = format!("<meta name=\"sheaf-screen-{nonce}\" content=\"");
    let height = text.find(&marker).and_then(|at| text[at + marker.len()..].split('"').next()).and_then(|s| s.parse::<u32>().ok());
    height.filter(|h| (1..=1_000_000).contains(h)).ok_or_else(|| OpdfError::Conversion("The browser could not measure the local screen layout. Check local styles/fonts or choose LibreOffice paper layout".into()))
}

fn run(browser: &Path, source: &Path, dir: &Path, width: u32, out: Option<&Path>) -> Result<Vec<u8>> {
    let mut cmd = Command::new(browser);
    cmd.args(["--headless", "--disable-background-networking", "--disable-extensions", "--disable-sync", "--no-first-run", "--no-default-browser-check", "--allow-file-access-from-files", "--hide-scrollbars", "--force-device-scale-factor=1", "--no-pdf-header-footer", "--virtual-time-budget=10000", "--host-resolver-rules=MAP * ~NOTFOUND"])
        .arg(format!("--user-data-dir={}", dir.join("profile").display()))
        .arg(format!("--window-size={width},900"));
    match out { Some(path) => { cmd.arg(format!("--print-to-pdf={}", path.display())); }, None => { cmd.arg("--dump-dom"); } }
    let mut child = cmd.arg(office::file_url(source)).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let stdout = capture(child.stdout.take().expect("stdout is piped"), 16 * 1024 * 1024);
    let stderr = capture(child.stderr.take().expect("stderr is piped"), 8192);
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < Duration::from_secs(60) => std::thread::sleep(Duration::from_millis(100)),
            result => {
                let _ = child.kill(); let _ = child.wait();
                return Err(OpdfError::Conversion(match result { Err(e) => e.to_string(), _ => "The local browser timed out after 60 seconds; no output saved".into() }));
            }
        }
    };
    let (stdout, overflow) = stdout.join().unwrap_or_default();
    let (stderr, _) = stderr.join().unwrap_or_default();
    if !status.success() { return Err(OpdfError::Conversion(format!("The local browser failed ({status}); no output saved. {}", String::from_utf8_lossy(&stderr).trim()))); }
    if overflow { return Err(OpdfError::Conversion("The browser layout exceeds the 16 MB inspection limit; no output saved".into())); }
    Ok(stdout)
}

fn capture(mut pipe: impl Read + Send + 'static, limit: usize) -> std::thread::JoinHandle<(Vec<u8>, bool)> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new(); let mut overflow = false; let mut chunk = [0; 4096];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 { break; }
            let keep = n.min(limit.saturating_sub(bytes.len()));
            bytes.extend_from_slice(&chunk[..keep]); overflow |= keep < n;
        }
        (bytes, overflow)
    })
}

fn check_written(path: &Path, options: Options, height: Option<u32>) -> Result<()> {
    let pdf = lopdf::Document::load(path)?;
    let pages = pdf.get_pages();
    if pages.is_empty() || pdf.is_encrypted() { return Err(OpdfError::Conversion("The browser did not write a readable PDF; no output saved".into())); }
    if let Some(height) = height {
        if pages.len() != 1 { return Err(OpdfError::Conversion("The browser still inserted page breaks. No output saved; choose paper pages or a wider screen width".into())); }
        let mut id = *pages.values().next().expect("pages is not empty");
        let mut media = None;
        for _ in 0..64 {
            let page = pdf.get_dictionary(id)?;
            if let Ok(value) = page.get(b"MediaBox") { media = Some(pdf.dereference(value)?.1.as_array()?.clone()); break; }
            let Ok(parent) = page.get(b"Parent").and_then(lopdf::Object::as_reference) else { break };
            id = parent;
        }
        let sizes = media.filter(|m| m.len() == 4).and_then(|m| m.iter().map(|v| pdf.dereference(v).ok()?.1.as_float().ok().map(f64::from)).collect::<Option<Vec<_>>>());
        let expected = [f64::from(options.screen_width.unwrap_or_default()) * 0.75 + options.margin * 2.0, f64::from(height) * 0.75 + options.margin * 2.0];
        if !sizes.is_some_and(|r| (r[2] - r[0] - expected[0]).abs() < 2.0 && (r[3] - r[1] - expected[1]).abs() < 2.0) {
            return Err(OpdfError::Conversion("The browser did not honor the measured continuous page size; no output saved".into()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_html_freezes_media_and_uses_one_measured_page_without_network_resources() {
        let html = "<!doctype html><html><head><meta http-equiv=\"refresh\" content=\"0;url=https://example.invalid\"><style>@media screen {.grid{display:grid}}</style></head><body>Local</body></html>";
        let out = prepare(html, 1280, 18.0, "abc123", Some(2400)).unwrap();
        assert!(out.starts_with("<!doctype html>"));
        assert!(!out.contains("example.invalid"));
        assert!(out.find("Content-Security-Policy").unwrap() < out.find("<style>").unwrap());
        assert!(out.contains("default-src 'none'") && out.contains("img-src file: data:"));
        assert!(out.contains("size: 996pt 1836pt"));
        assert!(out.contains("rule.media.mediaText = enabled ? 'all' : 'not all'"));
        assert!(out.contains("style.width = '1280px'"));
        assert_eq!(measurement(b"<meta name=\"sheaf-screen-abc123\" content=\"2400\">", "abc123").unwrap(), 2400);
        assert!(measurement(b"<meta name=\"sheaf-screen-abc123\" content=\"0\">", "abc123").is_err());
        assert!(measurement(b"no measurement", "abc123").is_err());
        assert!(prepare(html, 1280, f64::NAN, "abc", None).is_err());
    }

    #[test]
    fn browser_long_page_checks_reject_pagination_and_wrong_dimensions() {
        use lopdf::dictionary;
        let t = tempfile::tempdir().unwrap(); let path = t.path().join("written.pdf");
        let mut pdf = lopdf::Document::with_version("1.7"); let pages = pdf.new_object_id();
        let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 996.into(), 1836.into()] });
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()] }.into());
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages }); pdf.trailer.set("Root", root); pdf.save(&path).unwrap();
        let options = Options { paper: super::super::Paper::A4, landscape: false, margin: 18.0, long_page: true, screen_width: Some(1280) };
        check_written(&path, options, Some(2400)).unwrap();
        assert!(check_written(&path, options, Some(2500)).is_err());
        let other = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 996.into(), 1836.into()] });
        pdf.get_object_mut(pages).unwrap().as_dict_mut().unwrap().set("Kids", vec![page.into(), other.into()]);
        pdf.get_object_mut(pages).unwrap().as_dict_mut().unwrap().set("Count", 2); pdf.save(&path).unwrap();
        assert!(check_written(&path, options, Some(2400)).unwrap_err().to_string().contains("page breaks"));
    }

    #[test]
    fn real_local_screen_grid_has_no_paper_breaks_when_a_browser_is_explicitly_supplied() {
        let Some(browser) = std::env::var_os("SHEAF_TEST_CHROMIUM").map(PathBuf::from) else { eprintln!("skip: set SHEAF_TEST_CHROMIUM to opt into the real browser corpus"); return };
        let t = tempfile::tempdir().unwrap();
        let html = "<!doctype html><html><head><style>@media screen { main { display:grid; grid-template-columns:1fr 1fr; } } @media print { main { display:block; } } section { height:1600px; } </style></head><body><main><section>Left column</section><section>Right column</section></main></body></html>";
        let out = to_pdf_with(html, Options { paper: super::super::Paper::A4, landscape: false, margin: 18.0, long_page: true, screen_width: Some(800) }, &t.path().join("grid.pdf"), &browser).unwrap();
        let pdf = lopdf::Document::load(out).unwrap();
        assert_eq!(pdf.get_pages().len(), 1);
        let text = pdf.extract_text(&[1]).unwrap(); assert!(text.contains("Left column") && text.contains("Right column"));
        // Both sections share one row in screen media; print-media stacking would double height.
        let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
        let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
        assert!(media[3].as_float().unwrap() < 1500.0);
    }
}
