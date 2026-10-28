//! Trusted time stamps (RFC 3161) from a time stamp server: Acrobat's Time Stamp Document (a
//! `/DocTimeStamp` signature over the whole file, `ETSI.RFC3161`) and the time stamp a digital
//! signature carries (PAdES B-T, through [`crate::sign::sign_with_timestamp`]).
//!
//! The server is contacted only by the call that time stamps, at the address the user chose in
//! Preferences ▸ Signatures (Acrobat's Time Stamp Servers). The request carries a SHA-256
//! imprint, a fresh nonce and asks for the server's certificate; the reply is checked before
//! anything is written: granted, the token's signature, the imprint and the nonce.

use crate::pdfcraft::engine::sign::{self as crypto, der::Tlv, SignError, TimestampAuthority};
use crate::pdfcraft::{engine::SignatureStatus, Doc};
use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Servers offered in Preferences (free, public, no account): FreeTSA and DigiCert's.
pub const PRESETS: [&str; 2] = ["https://freetsa.org/tsr", "http://timestamp.digicert.com"];

/// The largest reply read (a token with its certificate chain is a few kilobytes).
const MAX_REPLY: u64 = 4 * 1024 * 1024;

/// A time stamp server reached over HTTP (RFC 3161 §3.4: a POST of `application/timestamp-query`).
pub struct HttpAuthority {
    url: String,
    timeout: Duration,
}

impl HttpAuthority {
    /// A server at `url` (http or https). Nothing is sent until a time stamp is asked for.
    pub fn new(url: &str) -> Result<HttpAuthority> {
        let url = url.trim();
        let ok = (url.starts_with("http://") || url.starts_with("https://")) && url.len() > "https://".len() && !url.contains(char::is_whitespace);
        if !ok {
            return Err(OpdfError::Unsupported(format!("\"{url}\" is not a time stamp server address (it starts with http:// or https://)")));
        }
        Ok(HttpAuthority { url: url.to_string(), timeout: Duration::from_secs(30) })
    }

    /// The server's host, as messages name it.
    pub fn host(&self) -> &str {
        host(&self.url)
    }
}

/// `https://freetsa.org/tsr` → `freetsa.org`.
pub fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

impl TimestampAuthority for HttpAuthority {
    fn timestamp(&self, request: &[u8]) -> std::result::Result<Vec<u8>, SignError> {
        let host = self.host();
        let fail = |what: String| SignError::Pdf(format!("The time stamp server {host} {what}"));
        // A fresh nonce (below 2^63, a positive DER INTEGER) ties the reply to this request.
        let mut query = crypto::timestamp::parse_request(request)?;
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|e| SignError::Crypto(format!("no random nonce: {e}")))?;
        let nonce = u64::from_be_bytes(random) >> 1;
        query.nonce = Some(nonce);
        query.certificate_request = true;
        let body = query.encode()?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            .timeout_connect(Some(Duration::from_secs(10)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut resp = agent.post(&self.url).header("Content-Type", "application/timestamp-query").send(&body[..]).map_err(|e| {
            fail(match e {
                ureq::Error::HostNotFound => "could not be found: is this computer offline?".into(),
                ureq::Error::ConnectionFailed => "refused the connection.".into(),
                ureq::Error::Io(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => "refused the connection.".into(),
                ureq::Error::Timeout(_) => "did not answer in time.".into(),
                e => format!("could not be reached ({e})."),
            })
        })?;
        let status = resp.status().as_u16();
        if status != 200 {
            return Err(fail(format!("answered HTTP {status} instead of a time stamp.")));
        }
        let reply = resp.body_mut().with_config().limit(MAX_REPLY).read_to_vec().map_err(|e| fail(format!("sent a reply that could not be read ({e}).")))?;
        if let Some(why) = refusal(&reply) {
            return Err(fail(format!("refused the request ({why}).")));
        }
        let token = crypto::timestamp::parse_response(&reply, &query).map_err(|e| fail(format!("sent a reply that is not a valid time stamp ({e}).")))?;
        if token_nonce(&token.raw) != Some(nonce) {
            return Err(fail("sent a time stamp that does not answer this request (its nonce differs).".into()));
        }
        Ok(reply)
    }
}

/// Why a `TimeStampResp` says no (PKIStatus 2 rejection and up, RFC 3161 §2.4.2), or `None` when
/// it grants the request or cannot be read (then the token check says what is wrong).
fn refusal(reply: &[u8]) -> Option<String> {
    let fields = Tlv::parse_all(reply).ok()?.children().ok()?;
    let info = fields.first()?.children().ok()?;
    let status = info.first()?.u64().ok()?;
    if status <= 1 {
        return None;
    }
    let name = match status {
        2 => "rejection",
        3 => "waiting",
        4 => "revocation warning",
        5 => "revoked",
        _ => "unknown status",
    };
    let mut why = vec![format!("status {status}, {name}")];
    for f in info.iter().skip(1) {
        match f.tag {
            // statusString: a SEQUENCE of UTF8Strings.
            0x30 => why.extend(f.children().ok().into_iter().flatten().filter_map(|s| s.text())),
            // failInfo: a BIT STRING, bit 0 first.
            0x03 => {
                const NAMES: [(usize, &str); 8] = [
                    (0, "unsupported algorithm"),
                    (2, "bad request"),
                    (5, "bad data format"),
                    (14, "time not available"),
                    (15, "policy not accepted"),
                    (16, "extension not accepted"),
                    (17, "information not available"),
                    (25, "server failure"),
                ];
                let bits = f.value.get(1..).unwrap_or_default();
                let set = |n: usize| bits.get(n / 8).is_some_and(|b| b & (0x80 >> (n % 8)) != 0);
                why.extend(NAMES.iter().filter(|(n, _)| set(*n)).map(|(_, s)| s.to_string()));
            }
            _ => {}
        }
    }
    Some(why.join(": "))
}

/// The nonce of a token's TSTInfo (the INTEGER after genTime and the optional accuracy and
/// ordering).
fn token_nonce(token: &[u8]) -> Option<u64> {
    let sd = crypto::cms::SignedData::parse(token).ok()?;
    let info = sd.content?;
    let fields = Tlv::parse_all(&info).ok()?.children().ok()?;
    fields.iter().skip(5).take_while(|f| f.tag != 0xA0 && f.tag != 0xA1).find(|f| f.tag == 0x02)?.u64().ok()
}

/// What [`timestamp_document`] wrote.
#[derive(Debug, Clone)]
pub struct TimestampReport {
    pub path: PathBuf,
    /// The time stamp authority (its certificate's name) and the time it vouches for, UTC.
    pub authority: Option<String>,
    pub time: Option<String>,
}

impl TimestampReport {
    /// The result line.
    pub fn summary(&self) -> String {
        match (&self.authority, &self.time) {
            (Some(a), Some(t)) => format!("Time stamped by {a} at {t} UTC"),
            (None, Some(t)) => format!("Time stamped at {t} UTC"),
            _ => "Time stamped".into(),
        }
    }
}

/// `2026-10-28 14:03:11` from a token time.
pub fn display_time(t: &crypto::Time) -> String {
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", t.year, t.month, t.day, t.hour, t.minute, t.second)
}

/// Acrobat's Time Stamp Document: `input` with a document time stamp from `authority` appended
/// as an incremental update (earlier signatures stay valid), written to `out` (never
/// overwriting). Nothing is written when the server fails or its reply does not check out.
pub fn timestamp_document(input: &Input, authority: &dyn TimestampAuthority, out: &Path) -> Result<TimestampReport> {
    let doc = Doc::open(input)?;
    let before = doc.document().signatures.iter().filter(|s| s.doc_timestamp).count();
    let bytes = doc.session().timestamp_document(doc.id(), authority, String::new()).map_err(|e| OpdfError::Engine(e.to_string()))?;
    let path = atomic_write(out, |part| Ok(std::fs::write(part, bytes.as_slice())?))?;
    // Read back as the signature panel reads it: the stamp just added is the last one.
    let written = Doc::open(&Input { path: path.clone(), ..input.clone() })?;
    let stamp = written.document().signatures.iter().filter(|s| s.doc_timestamp).nth(before).cloned();
    if stamp.as_ref().is_none_or(|s| s.status == SignatureStatus::Invalid) {
        let _ = std::fs::remove_file(&path);
        return Err(OpdfError::Engine("the time stamp written does not validate".into()));
    }
    Ok(TimestampReport {
        path,
        authority: stamp.as_ref().and_then(|s| s.signer.clone()),
        time: stamp.as_ref().and_then(|s| s.timestamp_time.as_ref()).map(display_time),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_checked_and_named_by_their_host() {
        assert!(HttpAuthority::new("ftp://x.org").is_err());
        assert!(HttpAuthority::new("https://").is_err());
        assert!(HttpAuthority::new("http://a b").is_err());
        assert_eq!(HttpAuthority::new(" https://freetsa.org/tsr ").unwrap().host(), "freetsa.org");
        assert_eq!(host("http://timestamp.digicert.com"), "timestamp.digicert.com");
        assert_eq!(host("http://127.0.0.1:8080/tsa?x"), "127.0.0.1:8080");
    }

    #[test]
    fn a_rejection_says_why() {
        use crypto::der;
        let text = der::seq(&[&der::tlv(0x0C, b"policy unknown")]);
        let fail = der::tlv(0x03, &[0x00, 0x00, 0x01]); // bit 15
        let reply = der::seq(&[&der::seq(&[&der::int(2), &text, &fail])]);
        assert_eq!(refusal(&reply).as_deref(), Some("status 2, rejection: policy unknown: policy not accepted"));
        assert_eq!(refusal(&der::seq(&[&der::seq(&[&der::int(0)])])), None);
        assert_eq!(refusal(b"garbage"), None);
    }
}
