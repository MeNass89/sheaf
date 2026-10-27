//! Other files to PDF (Stirling's Email to PDF and Markdown to PDF, Preview's PostScript): a
//! Markdown file and an e-mail (`.eml`) become a web page LibreOffice sets as a PDF, the mail's
//! attachments attached to it; PostScript goes through Ghostscript (`ps2pdf`) or macOS's
//! `pstopdf`.

use crate::{OpdfError, Result};
use std::path::{Path, PathBuf};

/// The kinds handled here, by file extension.
pub fn kind(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_string_lossy().to_ascii_lowercase().as_str() {
        "md" | "markdown" => Some("markdown"),
        "eml" => Some("mail"),
        "ps" | "eps" => Some("postscript"),
        _ => None,
    }
}

/// `input` (Markdown, e-mail or PostScript) as a PDF at `out`; `html_to_pdf` sets a web page.
pub fn to_pdf(input: &Path, out: &Path, html_to_pdf: impl FnOnce(&Path, &Path) -> Result<PathBuf>) -> Result<PathBuf> {
    let dir = tempfile::Builder::new().prefix("sheaf-text-").tempdir()?;
    match kind(input) {
        Some("postscript") => postscript(input, out),
        Some(k) => {
            let bytes = std::fs::read(input)?;
            let (html, attachments) = if k == "mail" { mail_to_html(&bytes) } else { (markdown_to_html(&crate::pdfcraft::engine::decode_text(&bytes)), Vec::new()) };
            let page = dir.path().join(format!("{}.html", input.file_stem().unwrap_or_default().to_string_lossy()));
            std::fs::write(&page, html)?;
            let made = html_to_pdf(&page, out)?;
            if !attachments.is_empty() {
                attach(&made, &attachments)?;
            }
            Ok(made)
        }
        None => Err(OpdfError::Unsupported("not a Markdown, e-mail or PostScript file".into())),
    }
}

fn postscript(input: &Path, out: &Path) -> Result<PathBuf> {
    crate::paths::atomic_write(out, |part| {
        let tries: [(&str, Vec<&std::ffi::OsStr>); 2] = [("ps2pdf", vec![input.as_os_str(), part.as_os_str()]), ("pstopdf", vec![input.as_os_str(), "-o".as_ref(), part.as_os_str()])];
        for (cmd, args) in tries {
            if let Ok(s) = std::process::Command::new(cmd).args(&args).status() {
                return if s.success() { Ok(()) } else { Err(OpdfError::Conversion(format!("{cmd} could not read the PostScript file"))) };
            }
        }
        Err(OpdfError::Unsupported("PostScript needs Ghostscript (ps2pdf): install it, then try again".into()))
    })
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Bold, italic, code and links within a line (already escaped).
fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        let pair = |open: &str, close: &str, tag: &str, rest: &str| -> Option<(String, usize)> {
            let body = rest.strip_prefix(open)?;
            let end = body.find(close).filter(|&e| e > 0)?;
            Some((format!("<{tag}>{}</{tag}>", if tag == "code" { body[..end].to_string() } else { inline(&body[..end]) }), open.len() + end + close.len()))
        };
        let link = || -> Option<(String, usize)> {
            let body = rest.strip_prefix('[')?;
            let t = body.find("](")?;
            let u = body[t + 2..].find(')')?;
            Some((format!("<a href=\"{}\">{}</a>", &body[t + 2..t + 2 + u], inline(&body[..t])), 1 + t + 2 + u + 1))
        };
        let found = match c {
            '`' => pair("`", "`", "code", rest),
            '*' => pair("**", "**", "b", rest).or_else(|| pair("*", "*", "i", rest)),
            '_' => pair("__", "__", "b", rest).or_else(|| pair("_", "_", "i", rest)),
            '[' => link(),
            _ => None,
        };
        match found {
            Some((html, used)) => {
                out.push_str(&html);
                rest = &rest[used..];
            }
            None => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// Markdown (CommonMark's usual parts: headings, paragraphs, lists, quotes, code, rules, tables,
/// bold, italic, code and links) as a web page.
pub fn markdown_to_html(md: &str) -> String {
    let mut out = String::from("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>body{font-family:sans-serif;line-height:1.4}pre{background:#f4f4f4;padding:8px}code{font-family:monospace}table{border-collapse:collapse}td,th{border:1px solid #999;padding:3px 6px}blockquote{border-left:3px solid #ccc;margin-left:0;padding-left:10px;color:#555}</style></head><body>\n");
    let mut para: Vec<String> = Vec::new();
    let mut list: Option<&str> = None;
    let mut lines = md.lines().peekable();
    let flush = |out: &mut String, para: &mut Vec<String>| {
        if !para.is_empty() {
            out.push_str(&format!("<p>{}</p>\n", inline(&escape(&para.join(" ")))));
            para.clear();
        }
    };
    while let Some(line) = lines.next() {
        let t = line.trim();
        let item = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")).map(|s| ("ul", s)).or_else(|| {
            let (n, s) = t.split_once(". ")?;
            (!n.is_empty() && n.chars().all(|c| c.is_ascii_digit())).then_some(("ol", s))
        });
        if list.is_some() && item.map(|i| i.0) != list {
            out.push_str(&format!("</{}>\n", list.take().unwrap_or("ul")));
        }
        if let Some((kind, text)) = item {
            flush(&mut out, &mut para);
            if list.is_none() {
                out.push_str(&format!("<{kind}>\n"));
                list = Some(kind);
            }
            out.push_str(&format!("<li>{}</li>\n", inline(&escape(text))));
            continue;
        }
        if t.is_empty() {
            flush(&mut out, &mut para);
        } else if t.starts_with("```") {
            flush(&mut out, &mut para);
            let mut code = Vec::new();
            for l in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push(escape(l));
            }
            out.push_str(&format!("<pre><code>{}</code></pre>\n", code.join("\n")));
        } else if let Some(level) = (1..=6).rev().find(|&n| t.starts_with(&format!("{} ", "#".repeat(n)))) {
            flush(&mut out, &mut para);
            out.push_str(&format!("<h{level}>{}</h{level}>\n", inline(&escape(t[level + 1..].trim().trim_end_matches('#').trim()))));
        } else if t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_')) {
            flush(&mut out, &mut para);
            out.push_str("<hr>\n");
        } else if let Some(q) = t.strip_prefix('>') {
            flush(&mut out, &mut para);
            out.push_str(&format!("<blockquote>{}</blockquote>\n", inline(&escape(q.trim()))));
        } else if t.starts_with('|') && lines.peek().is_some_and(|n| n.trim().starts_with('|') && n.contains("---")) {
            flush(&mut out, &mut para);
            let cells = |l: &str| l.trim().trim_matches('|').split('|').map(|c| inline(&escape(c.trim()))).collect::<Vec<_>>();
            out.push_str("<table><tr>");
            cells(t).iter().for_each(|c| out.push_str(&format!("<th>{c}</th>")));
            out.push_str("</tr>\n");
            lines.next();
            while let Some(row) = lines.peek().filter(|l| l.trim().starts_with('|')) {
                out.push_str("<tr>");
                cells(row).iter().for_each(|c| out.push_str(&format!("<td>{c}</td>")));
                out.push_str("</tr>\n");
                lines.next();
            }
            out.push_str("</table>\n");
        } else {
            para.push(t.to_string());
        }
    }
    flush(&mut out, &mut para);
    if let Some(kind) = list {
        out.push_str(&format!("</{kind}>\n"));
    }
    out.push_str("</body></html>\n");
    out
}

/// A MIME part: its headers (names lower case, values unfolded) and its body.
struct Part {
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Part {
    fn parse(bytes: &[u8]) -> Part {
        let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").map(|p| (p, 4)).or_else(|| bytes.windows(2).position(|w| w == b"\n\n").map(|p| (p, 2)));
        let (head, body) = match split {
            Some((p, n)) => (&bytes[..p], bytes[p + n..].to_vec()),
            None => (bytes, Vec::new()),
        };
        let mut headers: Vec<(String, String)> = Vec::new();
        for line in String::from_utf8_lossy(head).lines() {
            if line.starts_with([' ', '\t']) {
                if let Some(h) = headers.last_mut() {
                    h.1.push(' ');
                    h.1.push_str(line.trim());
                }
            } else if let Some((k, v)) = line.split_once(':') {
                headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
            }
        }
        Part { headers, body }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|h| h.0 == name).map(|h| h.1.as_str())
    }

    /// A header's main value ("text/plain") and its parameter `param` ("charset").
    fn param(&self, name: &str, param: &str) -> Option<String> {
        let v = self.header(name)?;
        v.split(';').skip(1).find_map(|p| {
            let (k, v) = p.split_once('=')?;
            (k.trim().eq_ignore_ascii_case(param)).then(|| decode_words(v.trim().trim_matches('"')))
        })
    }

    fn mime(&self) -> String {
        self.header("content-type").and_then(|v| v.split(';').next()).unwrap_or("text/plain").trim().to_ascii_lowercase()
    }

    /// The body with its transfer encoding undone.
    fn decoded(&self) -> Vec<u8> {
        match self.header("content-transfer-encoding").map(|e| e.trim().to_ascii_lowercase()).as_deref() {
            Some("base64") => base64(&self.body),
            Some("quoted-printable") => quoted_printable(&self.body),
            _ => self.body.clone(),
        }
    }

    /// The parts of a multipart body, in order.
    fn children(&self) -> Vec<Part> {
        let Some(b) = self.param("content-type", "boundary") else { return Vec::new() };
        let marker = format!("--{b}");
        let text = &self.body;
        let mut out = Vec::new();
        let starts: Vec<usize> = text.windows(marker.len()).enumerate().filter(|(i, w)| *w == marker.as_bytes() && (*i == 0 || text[i - 1] == b'\n')).map(|(i, _)| i).collect();
        for (k, &s) in starts.iter().enumerate() {
            let from = s + marker.len();
            if text[from..].starts_with(b"--") {
                break;
            }
            let end = starts.get(k + 1).copied().unwrap_or(text.len());
            let chunk = &text[from..end];
            let chunk = chunk.strip_prefix(b"\r\n").or_else(|| chunk.strip_prefix(b"\n")).unwrap_or(chunk);
            out.push(Part::parse(chunk));
        }
        out
    }

    fn filename(&self) -> Option<String> {
        self.param("content-disposition", "filename").or_else(|| self.param("content-type", "name"))
    }
}

fn base64(data: &[u8]) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let digits: Vec<u8> = data.iter().filter_map(|&c| val(c)).collect();
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &d)| n | u32::from(d) << (18 - 6 * i));
        out.extend_from_slice(&n.to_be_bytes()[1..chunk.len()]);
    }
    out
}

fn base64_encode(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = c.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for k in 0..4 {
            out.push(if k <= c.len() { A[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

fn quoted_printable(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b'=' if data.get(i + 1) == Some(&b'\r') && data.get(i + 2) == Some(&b'\n') => i += 3,
            b'=' if data.get(i + 1) == Some(&b'\n') => i += 2,
            b'=' => match data.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()) {
                Some(b) => {
                    out.push(b);
                    i += 3;
                }
                None => {
                    out.push(b'=');
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

/// A header's encoded words (RFC 2047: `=?utf-8?B?…?=`, `=?iso-8859-1?Q?…?=`) decoded.
fn decode_words(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("=?") {
        let word = &rest[start + 2..];
        let parts: Vec<&str> = word.splitn(3, '?').collect();
        let Some(end) = parts.get(2).and_then(|t| t.find("?=")) else { break };
        // The charset is told by the bytes (UTF-8, else Windows-1252, as `decode_text` reads them).
        let (enc, text) = (parts[1], &parts[2][..end]);
        let bytes = match enc.to_ascii_uppercase().as_str() {
            "B" => base64(text.as_bytes()),
            _ => quoted_printable(text.replace('_', " ").as_bytes()),
        };
        // Spaces between two encoded words are not part of the text.
        let before = &rest[..start];
        if !(before.trim().is_empty() && !out.is_empty()) {
            out.push_str(before);
        }
        out.push_str(&crate::pdfcraft::engine::decode_text(&bytes));
        rest = &word[parts[0].len() + parts[1].len() + 2 + end + 2..];
    }
    out.push_str(rest);
    out
}

/// The parts that show (text and pictures inline) and the attachments, depth first.
/// A picture an HTML part shows by `cid:` (its Content-ID, its type, its bytes).
type Inline = (String, String, Vec<u8>);

fn walk(p: Part, texts: &mut Vec<(String, String)>, files: &mut Vec<(String, Vec<u8>)>, inline: &mut Vec<Inline>) {
    let mime = p.mime();
    if mime.starts_with("multipart/") {
        let kids = p.children();
        // An alternative: its richest part only (HTML over plain text).
        if mime == "multipart/alternative" {
            let pick = kids.iter().rposition(|k| k.mime() == "text/html").or_else(|| kids.iter().rposition(|k| k.mime() == "text/plain"));
            if let Some(i) = pick {
                let kid = kids.into_iter().nth(i).unwrap_or(Part { headers: Vec::new(), body: Vec::new() });
                walk(kid, texts, files, inline);
            }
            return;
        }
        for k in kids {
            walk(k, texts, files, inline);
        }
        return;
    }
    let attached = p.header("content-disposition").is_some_and(|d| d.trim_start().to_ascii_lowercase().starts_with("attachment"));
    if !attached && (mime == "text/plain" || mime == "text/html") {
        texts.push((mime, crate::pdfcraft::engine::decode_text(&p.decoded())));
    } else if let Some(cid) = p.header("content-id").map(|c| c.trim().trim_start_matches('<').trim_end_matches('>').to_string()).filter(|_| !attached && mime.starts_with("image/")) {
        // A picture the message shows in its text (`<img src="cid:…">`), not an attachment.
        inline.push((cid, mime, p.decoded()));
    } else if attached || p.filename().is_some() {
        files.push((p.filename().unwrap_or_else(|| format!("attachment {}", files.len() + 1)), p.decoded()));
    }
}

/// An e-mail as a web page (its sender, recipients, date and subject above its text) and its
/// attachments (name, bytes).
pub fn mail_to_html(bytes: &[u8]) -> (String, Vec<(String, Vec<u8>)>) {
    let top = Part::parse(bytes);
    let mut head = String::new();
    for (label, name) in [("From", "from"), ("To", "to"), ("Cc", "cc"), ("Date", "date"), ("Subject", "subject")] {
        if let Some(v) = top.header(name) {
            head.push_str(&format!("<tr><th>{label}</th><td>{}</td></tr>", escape(&decode_words(v))));
        }
    }
    let subject = top.header("subject").map(decode_words).unwrap_or_default();
    let (mut texts, mut files) = (Vec::new(), Vec::new());
    let mut inline = Vec::new();
    walk(top, &mut texts, &mut files, &mut inline);
    let mut body = String::new();
    for (mime, text) in texts {
        if mime == "text/html" {
            // The message's own page, its head and outer tags left out.
            let lower = text.to_ascii_lowercase();
            let from = lower.find("<body").and_then(|b| lower[b..].find('>').map(|e| b + e + 1)).unwrap_or(0);
            let to = lower.rfind("</body>").unwrap_or(text.len()).max(from);
            // Its pictures in place (Sol, 26/10: they came as attachments, the page showing none).
            let mut part = text[from..to].to_string();
            for (cid, mime, data) in &inline {
                part = part.replace(&format!("cid:{cid}"), &format!("data:{mime};base64,{}", base64_encode(data)));
            }
            body.push_str(&part);
        } else {
            body.push_str(&format!("<pre style=\"white-space:pre-wrap;font-family:sans-serif\">{}</pre>", escape(&text)));
        }
    }
    if !files.is_empty() {
        body.push_str("<hr><p><b>Attachments</b> (attached to this PDF)</p><ul>");
        for (name, data) in &files {
            body.push_str(&format!("<li>{} ({} KB)</li>", escape(name), data.len().div_ceil(1024)));
        }
        body.push_str("</ul>");
    }
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font-family:sans-serif}}table.h th{{text-align:left;padding-right:12px;color:#555;vertical-align:top}}</style></head><body><table class=\"h\">{head}</table><hr>{body}</body></html>",
        escape(&subject)
    );
    (html, files)
}

/// `files` attached to the PDF at `pdf` (its EmbeddedFiles).
fn attach(pdf: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
    use lopdf::{Object, Stream, dictionary};
    let mut d = lopdf::Document::load(pdf)?;
    let mut names = Vec::new();
    for (name, data) in files {
        let ef = d.add_object(Stream::new(dictionary! { "Type" => "EmbeddedFile", "Params" => dictionary! { "Size" => data.len() as i64 } }, data.clone()));
        let spec = d.add_object(dictionary! { "Type" => "Filespec", "F" => Object::string_literal(name.as_str()), "UF" => Object::string_literal(name.as_str()), "EF" => dictionary! { "F" => ef } });
        names.push(Object::string_literal(name.as_str()));
        names.push(Object::Reference(spec));
    }
    let tree = d.add_object(dictionary! { "Names" => names });
    let root = d.trailer.get(b"Root").and_then(Object::as_reference)?;
    // The catalog's other name trees (destinations) kept.
    let mut all = match d.get_dictionary(root)?.get(b"Names").ok().cloned() {
        Some(Object::Reference(id)) => d.get_dictionary(id).cloned().unwrap_or_default(),
        Some(Object::Dictionary(n)) => n,
        _ => lopdf::Dictionary::new(),
    };
    all.set("EmbeddedFiles", tree);
    d.get_object_mut(root).and_then(Object::as_dict_mut)?.set("Names", all);
    d.save(pdf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_becomes_headings_lists_tables_and_inline_styles() {
        let html = markdown_to_html("# Title\n\nSome **bold** and *it* with `x<y`.\n\n- one\n- two\n\n1. first\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```\ncode <here>\n```\n> quoted\n---\n[link](https://e.x)");
        for want in ["<h1>Title</h1>", "<b>bold</b>", "<i>it</i>", "<code>x&lt;y</code>", "<ul>\n<li>one</li>\n<li>two</li>\n</ul>", "<ol>\n<li>first</li>", "<th>A</th><th>B</th>", "<td>1</td><td>2</td>", "<pre><code>code &lt;here&gt;</code></pre>", "<blockquote>quoted</blockquote>", "<hr>", "<a href=\"https://e.x\">link</a>"] {
            assert!(html.contains(want), "{want} missing in {html}");
        }
    }

    #[test]
    fn a_mail_shows_its_headers_its_text_and_lists_its_attachments() {
        let mail = "From: =?utf-8?B?QmrDtnJu?= <b@x.org>\r\nTo: a@y.org\r\nSubject: =?iso-8859-1?Q?Caf=E9?= report\r\nContent-Type: multipart/mixed; boundary=\"B1\"\r\n\r\n--B1\r\nContent-Type: multipart/alternative; boundary=B2\r\n\r\n--B2\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nPlain caf=C3=A9\r\n--B2\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><body><p>Rich <b>text</b></p></body></html>\r\n--B2--\r\n--B1\r\nContent-Type: application/pdf; name=\"a.pdf\"\r\nContent-Disposition: attachment; filename=\"a.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQ=\r\n--B1--\r\n";
        let (html, files) = mail_to_html(mail.as_bytes());
        assert!(html.contains("Björn"), "{html}");
        assert!(html.contains("Café report"), "{html}");
        assert!(html.contains("<p>Rich <b>text</b></p>") && !html.contains("Plain café"), "the HTML part, not the plain one: {html}");
        assert_eq!(files, vec![("a.pdf".to_string(), b"%PDF-1.4".to_vec())]);
        assert!(html.contains("a.pdf (1 KB)"));
        // A picture shown in the text by its Content-ID is drawn there, not attached (Sol, 26/10).
        let related = "Subject: Logo\r\nContent-Type: multipart/related; boundary=R\r\n\r\n--R\r\nContent-Type: text/html\r\n\r\n<p>See <img src=\"cid:logo1@x\"></p>\r\n--R\r\nContent-Type: image/png\r\nContent-ID: <logo1@x>\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--R--\r\n";
        let (html, files) = mail_to_html(related.as_bytes());
        assert!(files.is_empty(), "{files:?}");
        assert!(html.contains("src=\"data:image/png;base64,iVBORw0KGgo=\""), "{html}");
        assert_eq!(base64_encode(b"%PDF-1.4"), "JVBERi0xLjQ=");
    }
}
