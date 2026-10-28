//! Check for updates against a mock of GitHub's releases API: a local `TcpListener` answering
//! `releases/latest`. No test calls GitHub.

use sheaf_core::updates::{check_at, Outcome, UpdateError};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;

/// A server answering one request with `status`, `headers` and `body`; its URL, and the request's
/// head lines as received.
fn mock(status: u16, headers: &'static str, body: String) -> (String, mpsc::Receiver<Vec<String>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/repos/MeNass89/sheaf/releases/latest", l.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut r = BufReader::new(s.try_clone().unwrap());
        let mut head = Vec::new();
        loop {
            let mut line = String::new();
            if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                break;
            }
            head.push(line.trim().to_string());
        }
        let _ = tx.send(head);
        let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    });
    (url, rx)
}

fn release(tag: &str) -> String {
    let name = format!("Sheaf-{}", tag.trim_start_matches('v'));
    let assets: Vec<String> = ["arm64.dmg", "x86_64.dmg", "windows-x64.zip", "linux-x64.tar.gz", "linux-arm64.tar.gz"]
        .iter()
        .map(|k| format!(r#"{{"name":"{name}-{k}","browser_download_url":"https://github.com/MeNass89/sheaf/releases/download/{tag}/{name}-{k}","size":1000}}"#))
        .collect();
    format!(
        r###"{{"tag_name":"{tag}","name":"Sheaf {tag}","body":"## New\n\n- Check for updates","html_url":"https://github.com/MeNass89/sheaf/releases/tag/{tag}","draft":false,"prerelease":false,"assets":[{}]}}"###,
        assets.join(",")
    )
}

#[test]
fn a_newer_release_is_found_with_this_computers_download() {
    let (url, seen) = mock(200, "", release("v99.0.0"));
    let Outcome::Available(r, asset) = check_at(&url, "0.2.1").unwrap() else { panic!("no update found") };
    assert_eq!((r.tag.as_str(), r.version.to_string().as_str()), ("v99.0.0", "99.0.0"));
    assert_eq!(r.page, "https://github.com/MeNass89/sheaf/releases/tag/v99.0.0");
    // Every computer the tests run on has its package in this release.
    let a = asset.expect("a download for this computer");
    assert!(a.url.starts_with("https://github.com/MeNass89/sheaf/releases/download/v99.0.0/"), "{}", a.url);
    // GitHub's API asks for a User-Agent and its media type.
    let head = seen.recv().unwrap();
    assert_eq!(head[0], "GET /repos/MeNass89/sheaf/releases/latest HTTP/1.1");
    let has = |h: &str| head.iter().any(|l| l.to_ascii_lowercase() == h.to_ascii_lowercase());
    assert!(has("user-agent: Sheaf/0.2.1"), "{head:?}");
    assert!(has("accept: application/vnd.github+json"), "{head:?}");
}

#[test]
fn the_same_or_an_older_release_is_up_to_date() {
    let (url, _) = mock(200, "", release("v0.2.1"));
    assert!(matches!(check_at(&url, "0.2.1"), Ok(Outcome::UpToDate(Some(_)))));
    let (url, _) = mock(404, "", r#"{"message":"Not Found"}"#.into());
    assert_eq!(check_at(&url, "0.2.1"), Ok(Outcome::UpToDate(None)));
}

#[test]
fn refusals_and_no_connection_are_said_plainly() {
    let (url, _) = mock(403, "x-ratelimit-remaining: 0\r\nx-ratelimit-reset: 1761649200\r\n", r#"{"message":"API rate limit exceeded"}"#.into());
    assert_eq!(check_at(&url, "0.2.1"), Err(UpdateError::RateLimited(Some(1_761_649_200))));
    let (url, _) = mock(500, "", "oops".into());
    assert_eq!(check_at(&url, "0.2.1"), Err(UpdateError::Http(500)));
    // A port nobody listens on: offline.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let e = check_at(&format!("http://127.0.0.1:{port}/latest"), "0.2.1").unwrap_err();
    assert!(matches!(e, UpdateError::Offline(_)), "{e:?}");
}
