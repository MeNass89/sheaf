//! Time stamps from a time stamp server (C-sign-015), without the internet: a local TSA made in
//! the test (a test CA, a TSA certificate it issued, `openssl ts -reply`) behind a tiny HTTP
//! server thread. The output is checked by PdfCraft, poppler's `pdfsig`, `openssl ts -verify`
//! and `qpdf --check`.
mod common;
use common::qpdf_clean;
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::engine::SignatureStatus;
use sheaf_core::pdfcraft::Doc;
use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
use sheaf_core::timestamp::{self, HttpAuthority};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

fn invoice() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf")
}

fn openssl(dir: &Path, args: &[&str]) {
    let o = Command::new("openssl").current_dir(dir).args(args).output().unwrap();
    assert!(o.status.success(), "openssl {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

/// A test CA and a time stamp authority it certified, with `openssl ts`'s configuration, in `dir`.
fn make_tsa(dir: &Path) -> PathBuf {
    openssl(dir, &["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-subj", "/CN=Sheaf Test Root CA", "-keyout", "ca.key", "-out", "ca.pem",
        "-addext", "basicConstraints=critical,CA:TRUE", "-addext", "keyUsage=critical,keyCertSign,cRLSign"]);
    openssl(dir, &["req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=Sheaf Test TSA", "-keyout", "tsa.key", "-out", "tsa.csr"]);
    std::fs::write(dir.join("ext.cnf"), "extendedKeyUsage=critical,timeStamping\nkeyUsage=critical,digitalSignature\n").unwrap();
    openssl(dir, &["x509", "-req", "-in", "tsa.csr", "-CA", "ca.pem", "-CAkey", "ca.key", "-CAcreateserial", "-days", "2", "-extfile", "ext.cnf", "-out", "tsa.pem"]);
    std::fs::write(dir.join("serial"), "01\n").unwrap();
    let d = dir.display();
    let cfg = format!(
        "[ tsa ]\ndefault_tsa = tsa1\n[ tsa1 ]\nserial = {d}/serial\ncrypto_device = builtin\nsigner_cert = {d}/tsa.pem\ncerts = {d}/ca.pem\n\
         signer_key = {d}/tsa.key\nsigner_digest = sha256\ndefault_policy = 1.2.3.4.1\ndigests = sha256\naccuracy = secs:1\nordering = no\n\
         tsa_name = no\ness_cert_id_chain = no\ness_cert_id_alg = sha256\n"
    );
    let path = dir.join("tsa.cnf");
    std::fs::write(&path, cfg).unwrap();
    path
}

#[derive(Clone, Copy)]
enum Reply {
    /// `openssl ts -reply` to the query received.
    Granted,
    /// A TimeStampResp with PKIStatus rejection, a status string and failInfo badAlg.
    Rejected,
    /// Bytes that are no time stamp at all.
    Garbage,
    /// HTTP 500.
    ServerError,
}

/// An HTTP server on 127.0.0.1 answering `n` POSTs as `reply` says; its URL. The queries received
/// are kept in `dir` (query-N.tsq).
fn serve(dir: &Path, reply: Reply, n: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/tsr", listener.local_addr().unwrap());
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        for (k, stream) in listener.incoming().take(n).enumerate() {
            let mut s = stream.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head_end, length) = loop {
                let r = s.read(&mut chunk).unwrap();
                assert!(r > 0, "the client hung up mid-request");
                buf.extend_from_slice(&chunk[..r]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                    assert!(head.starts_with("post "), "{head}");
                    assert!(head.contains("content-type: application/timestamp-query"), "{head}");
                    let len = head.lines().find_map(|l| l.strip_prefix("content-length:")).map(|v| v.trim().parse::<usize>().unwrap()).unwrap();
                    break (i + 4, len);
                }
            };
            while buf.len() < head_end + length {
                let r = s.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..r]);
            }
            let query = dir.join(format!("query-{k}.tsq"));
            std::fs::write(&query, &buf[head_end..head_end + length]).unwrap();
            let (status, body) = match reply {
                Reply::Granted => {
                    let out = dir.join(format!("reply-{k}.tsr"));
                    let o = Command::new("openssl").args(["ts", "-reply", "-config"]).arg(dir.join("tsa.cnf")).arg("-queryfile").arg(&query).arg("-out").arg(&out).output().unwrap();
                    assert!(o.status.success(), "openssl ts -reply: {}", String::from_utf8_lossy(&o.stderr));
                    ("200 OK", std::fs::read(out).unwrap())
                }
                Reply::Rejected => {
                    // SEQUENCE { SEQUENCE { INTEGER 2, SEQUENCE { UTF8String }, BIT STRING badAlg } }
                    let text = b"unsupported algorithm";
                    let mut info = vec![0x02, 0x01, 0x02, 0x30, text.len() as u8 + 2, 0x0C, text.len() as u8];
                    info.extend_from_slice(text);
                    info.extend_from_slice(&[0x03, 0x02, 0x07, 0x80]);
                    let mut status = vec![0x30, info.len() as u8];
                    status.extend(info);
                    let mut resp = vec![0x30, status.len() as u8];
                    resp.extend(status);
                    ("200 OK", resp)
                }
                Reply::Garbage => ("200 OK", b"<html>not a time stamp</html>".to_vec()),
                Reply::ServerError => ("500 Internal Server Error", b"oops".to_vec()),
            };
            let head = format!("HTTP/1.1 {status}\r\nContent-Type: application/timestamp-reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            s.write_all(head.as_bytes()).unwrap();
            s.write_all(&body).unwrap();
        }
    });
    url
}

/// The file's last signature dictionary's covered bytes and its `/Contents` (hex-decoded, zero
/// padding dropped by DER length), as an outside verifier reads them.
fn covered_and_token(pdf: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let text = String::from_utf8_lossy(pdf);
    let at = text.rfind("/ByteRange").unwrap();
    let open = at + text[at..].find('[').unwrap();
    let close = open + text[open..].find(']').unwrap();
    let n: Vec<usize> = text[open + 1..close].split_whitespace().map(|v| v.parse().unwrap()).collect();
    let mut covered = pdf[n[0]..n[0] + n[1]].to_vec();
    covered.extend_from_slice(&pdf[n[2]..n[2] + n[3]]);
    let hex = &pdf[n[1] + 1..n[2] - 1];
    let bytes: Vec<u8> = hex.chunks(2).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect();
    // DER: 30 82 hi lo …
    assert_eq!(bytes[..2], [0x30, 0x82]);
    let len = 4 + ((bytes[2] as usize) << 8 | bytes[3] as usize);
    (covered, bytes[..len].to_vec())
}

#[test]
fn a_document_time_stamp_from_a_server_validates_everywhere() {
    let t = tempfile::tempdir().unwrap();
    make_tsa(t.path());
    let url = serve(t.path(), Reply::Granted, 1);
    let authority = HttpAuthority::new(&url).unwrap();
    let out = t.path().join("stamped.pdf");
    let r = timestamp::timestamp_document(&Input::new(invoice()), &authority, &out).unwrap();
    assert_eq!(r.authority.as_deref(), Some("Sheaf Test TSA"));
    assert!(r.summary().starts_with("Time stamped by Sheaf Test TSA at 20"), "{}", r.summary());

    // The request: SHA-256 imprint, a nonce, the certificate asked for.
    let q = Command::new("openssl").args(["ts", "-query", "-text", "-in"]).arg(t.path().join("query-0.tsq")).output().unwrap();
    let q = String::from_utf8_lossy(&q.stdout);
    assert!(q.contains("Hash Algorithm: sha256") && q.contains("Certificate required: yes") && !q.contains("Nonce: unspecified"), "{q}");

    // An incremental update: the original bytes come first, unchanged.
    let original = std::fs::read(invoice()).unwrap();
    let bytes = std::fs::read(&r.path).unwrap();
    assert_eq!(&bytes[..original.len()], &original[..]);
    let tail = String::from_utf8_lossy(&bytes[original.len()..]);
    assert!(tail.contains("/DocTimeStamp") && tail.contains("/ETSI.RFC3161") && tail.replace(' ', "").contains("/FT/Sig"), "{tail}");
    qpdf_clean(&r.path);

    // PdfCraft (the signature panel's source): one document time stamp, its time and authority.
    let doc = Doc::open(&Input::new(&r.path)).unwrap();
    let sigs = &doc.document().signatures;
    assert_eq!(sigs.len(), 1);
    let s = &sigs[0];
    assert!(s.doc_timestamp && s.signed && s.timestamp_time.is_some());
    assert_eq!(s.signer.as_deref(), Some("Sheaf Test TSA"));
    // The test CA is not trusted on this machine: intact, authority unknown.
    assert_eq!(s.status, SignatureStatus::Unknown, "{:?}", s.details);
    assert!(s.details.iter().any(|d| d.message.contains("not been modified")), "{:?}", s.details);

    // openssl verifies the token over the bytes the ByteRange covers, chained to the test CA.
    let (covered, token) = covered_and_token(&bytes);
    std::fs::write(t.path().join("covered.bin"), &covered).unwrap();
    std::fs::write(t.path().join("token.der"), &token).unwrap();
    let v = Command::new("openssl").current_dir(t.path())
        .args(["ts", "-verify", "-data", "covered.bin", "-in", "token.der", "-token_in", "-CAfile", "ca.pem"]).output().unwrap();
    let vs = format!("{}{}", String::from_utf8_lossy(&v.stdout), String::from_utf8_lossy(&v.stderr));
    assert!(v.status.success() && vs.contains("Verification: OK"), "openssl ts -verify: {vs}");

    // poppler lists it as a signature field covering the whole file (poppler 24 does not
    // validate ETSI.RFC3161 time stamps: "Unable to validate this type of signature").
    if let Ok(o) = Command::new("pdfsig").arg("-nocert").arg(&r.path).output() {
        let p = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        assert!(p.contains("Signature #1") && p.contains("Field Name: Timestamp1") && p.contains("Total document signed"), "pdfsig: {p}");
    }
}

#[test]
fn a_signed_document_stays_valid_once_time_stamped_and_a_signature_gets_its_own_stamp() {
    let t = tempfile::tempdir().unwrap();
    make_tsa(t.path());
    // A self-signed digital ID for the signature.
    openssl(t.path(), &["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-subj", "/CN=Sheaf Test Signer", "-keyout", "k.pem", "-out", "c.pem"]);
    openssl(t.path(), &["pkcs12", "-export", "-passout", "pass:secret", "-inkey", "k.pem", "-in", "c.pem", "-out", "id.p12"]);
    let digital = DigitalSpec { p12: t.path().join("id.p12"), password: "secret".into(), reason: None, location: None, certify: None };
    let spec = SignSpec { page: 0, rect: [72.0, 100.0, 240.0, 160.0], graphic: Graphic::Typed("Ada".into(), Default::default()), digital: Some(digital) };
    let url = serve(t.path(), Reply::Granted, 2);
    let authority = HttpAuthority::new(&url).unwrap();
    // Signature time stamp (PAdES B-T) through the server.
    let signed = sign::sign_with_timestamp(&Input::new(invoice()), &spec, &t.path().join("signed.pdf"), &authority).unwrap();
    // Then a document time stamp over it.
    let r = timestamp::timestamp_document(&Input::new(&signed.path), &authority, &t.path().join("stamped.pdf")).unwrap();
    qpdf_clean(&r.path);
    let doc = Doc::open(&Input::new(&r.path)).unwrap();
    let sigs = &doc.document().signatures;
    assert_eq!(sigs.len(), 2, "{sigs:?}");
    let field = sigs.iter().find(|s| !s.doc_timestamp).unwrap();
    assert!(field.timestamp, "the signature carries its own token");
    assert_ne!(field.status, SignatureStatus::Invalid, "{:?}", field.details);
    let stamp = sigs.iter().find(|s| s.doc_timestamp).unwrap();
    assert_eq!(stamp.status, SignatureStatus::Unknown, "{:?}", stamp.details);
    // poppler: the signature under the time stamp still verifies; the stamp covers everything.
    if let Ok(o) = Command::new("pdfsig").arg("-nocert").arg(&r.path).output() {
        let p = String::from_utf8_lossy(&o.stdout);
        let (first, second) = p.split_once("Signature #2").expect("pdfsig lists both");
        assert!(first.contains("Signature is Valid") && first.contains("Not total document signed"), "pdfsig: {p}");
        assert!(second.contains("Timestamp1") && second.contains("Total document signed"), "pdfsig: {p}");
    }
}

#[test]
fn a_refused_or_bad_reply_writes_nothing_and_says_why() {
    let t = tempfile::tempdir().unwrap();
    for (reply, says) in [
        (Reply::Rejected, "refused the request (status 2, rejection: unsupported algorithm: unsupported algorithm)"),
        (Reply::Garbage, "sent a reply that is not a valid time stamp"),
        (Reply::ServerError, "answered HTTP 500"),
    ] {
        let url = serve(t.path(), reply, 1);
        let out = t.path().join("never.pdf");
        let e = timestamp::timestamp_document(&Input::new(invoice()), &HttpAuthority::new(&url).unwrap(), &out).unwrap_err().to_string();
        assert!(e.contains("The time stamp server 127.0.0.1:") && e.contains(says), "{e}");
        assert!(!out.exists());
    }
    // Nothing listening: the connection is refused.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let e = timestamp::timestamp_document(&Input::new(invoice()), &HttpAuthority::new(&format!("http://127.0.0.1:{port}/")).unwrap(), &t.path().join("never.pdf"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("refused the connection"), "{e}");
}
