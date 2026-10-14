//! HTTP/1.1, as little as the door needs: the shell page, the client's
//! files by content name, and the upgrade to a WebSocket on `/ws`.

use crate::hash::{hex, sha1};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

/// Every response says where scripts, sockets and media may come from.
/// Scripts only from this host, and wasm may compile; sockets to any host
/// (`ws:`/`wss:` are named because older Safari does not count a socket
/// to this host as `'self'`), and later a relay's; inline styles are
/// allowed so the shell page can carry its own few rules.
pub const CSP: &str = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' ws: wss:; \
img-src 'self' data: blob:; media-src 'self' blob:; style-src 'self' 'unsafe-inline'; font-src 'self'; \
manifest-src 'self'; worker-src 'self' blob:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// A request's head, read.
pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    /// HTTP/1.0 closes after one response unless asked not to.
    pub http10: bool,
    headers: Vec<(&'a str, &'a str)>,
}

impl<'a> Request<'a> {
    /// The head (without its blank line), or `None` when it is not one.
    pub fn parse(head: &'a str) -> Option<Request<'a>> {
        let mut lines = head.split("\r\n");
        let mut first = lines.next()?.split(' ');
        let (method, target, version) = (first.next()?, first.next()?, first.next()?);
        if first.next().is_some() || !target.starts_with('/') {
            return None;
        }
        let http10 = match version {
            "HTTP/1.1" => false,
            "HTTP/1.0" => true,
            _ => return None,
        };
        let path = target.split(['?', '#']).next().unwrap_or(target);
        let mut headers = Vec::new();
        for line in lines {
            let (name, value) = line.split_once(':')?;
            if name.is_empty() || name.contains([' ', '\t']) {
                return None;
            }
            headers.push((name, value.trim_matches([' ', '\t'])));
        }
        Some(Request { method, path, http10, headers })
    }

    pub fn header(&self, name: &str) -> Option<&'a str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| *v)
    }

    /// Whether a comma-separated header lists `token` (any case).
    pub fn lists(&self, name: &str, token: &str) -> bool {
        self.headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .flat_map(|(_, v)| v.split(','))
            .any(|t| t.trim().eq_ignore_ascii_case(token))
    }

    /// The client takes gzip (`gzip;q=0` says it does not).
    pub fn takes_gzip(&self) -> bool {
        self.headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("accept-encoding"))
            .flat_map(|(_, v)| v.split(','))
            .any(|t| {
                let mut parts = t.split(';');
                let coding = parts.next().unwrap_or("").trim();
                let refused = parts.any(|p| p.trim().replace(' ', "").eq_ignore_ascii_case("q=0"));
                coding.eq_ignore_ascii_case("gzip") && !refused
            })
    }

    /// The connection stays open for another request.
    pub fn keep_alive(&self) -> bool {
        if self.http10 { self.lists("connection", "keep-alive") } else { !self.lists("connection", "close") }
    }
}

/// The browser's half of a game, as the browser runtime builds it: the
/// shell page, whose `{{asset:NAME}}` marks become the files' content
/// names, and the files (JS, wasm, sounds) by name.
#[derive(Clone, Debug, Default)]
pub struct ClientBundle {
    pub index_html: String,
    /// (name, bytes, MIME type)
    pub files: Vec<(String, Vec<u8>, String)>,
}

impl ClientBundle {
    /// A directory's `index.html` and every other file in it (not in
    /// subdirectories), typed by extension.
    pub fn from_dir(dir: &Path) -> io::Result<ClientBundle> {
        let index_html = std::fs::read_to_string(dir.join("index.html"))?;
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "index.html" || !entry.file_type()?.is_file() {
                continue;
            }
            let mime = mime_for(&name).to_string();
            files.push((name, std::fs::read(entry.path())?, mime));
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(ClientBundle { index_html, files })
    }

    /// What a host without a bundle serves: a page that says so.
    pub fn placeholder() -> ClientBundle {
        ClientBundle {
            index_html: "<!doctype html><meta charset=utf-8><title>angelX</title>\
                <p>This host serves the game's socket only; no browser client was given to it.</p>"
                .to_string(),
            files: Vec::new(),
        }
    }
}

/// A file's MIME type from its extension.
pub fn mime_for(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e).to_ascii_lowercase();
    match ext.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "ogg" | "oga" => "audio/ogg",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "txt" => "text/plain; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

/// Whether gzip is worth trying: text and wasm shrink; images and sound
/// are compressed already.
fn compressible(mime: &str) -> bool {
    mime.starts_with("text/")
        || mime.starts_with("application/wasm")
        || mime.starts_with("application/json")
        || mime.starts_with("application/manifest")
        || mime.starts_with("image/svg")
}

/// A body ready to send, and its gzip when that is smaller.
pub struct Body {
    pub mime: String,
    pub plain: Vec<u8>,
    pub gzip: Option<Vec<u8>>,
    /// A strong validator, for the page (the files never change).
    pub etag: String,
}

impl Body {
    fn new(mime: &str, plain: Vec<u8>) -> io::Result<Body> {
        let gzip = if compressible(mime) {
            let mut enc = GzEncoder::new(Vec::new(), Compression::best());
            enc.write_all(&plain)?;
            Some(enc.finish()?).filter(|z| z.len() < plain.len())
        } else {
            None
        };
        let etag = format!("\"{}\"", &hex(&sha1(&plain))[..16]);
        Ok(Body { mime: mime.to_string(), plain, gzip, etag })
    }
}

/// The bundle as served: the page with its marks filled, and the files by
/// content name.
pub struct Assets {
    pub index: Body,
    /// "<hash>.<ext>" to body.
    pub files: HashMap<String, Body>,
}

/// Hex digits of a content hash in a file's served name: 64 bits, so two
/// versions of a file never share a name.
const NAME_HASH: usize = 16;

impl Assets {
    /// Hash and compress everything once, at start. A mark naming a file
    /// the bundle does not have is an error.
    pub fn prepare(bundle: &ClientBundle) -> io::Result<Assets> {
        let mut files = HashMap::new();
        let mut urls = HashMap::new();
        for (name, bytes, mime) in &bundle.files {
            let ext = name.rsplit_once('.').map_or("bin", |(_, e)| e);
            let served = format!("{}.{ext}", &hex(&sha1(bytes))[..NAME_HASH]);
            urls.insert(name.as_str(), format!("/a/{served}"));
            files.insert(served, Body::new(mime, bytes.clone())?);
        }
        let mut page = String::with_capacity(bundle.index_html.len());
        let mut rest = bundle.index_html.as_str();
        while let Some(at) = rest.find("{{asset:") {
            page.push_str(&rest[..at]);
            let after = &rest[at + "{{asset:".len()..];
            let end = after
                .find("}}")
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "an {{asset: mark is not closed"))?;
            let name = &after[..end];
            let url = urls.get(name).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, format!("the page names {name}, which the bundle does not have"))
            })?;
            page.push_str(url);
            rest = &after[end + 2..];
        }
        page.push_str(rest);
        Ok(Assets { index: Body::new(mime_for("index.html"), page.into_bytes())?, files })
    }
}

/// A response head with the headers every response carries.
pub fn head(status: &str, extra: &[(&str, &str)], len: usize, keep_alive: bool) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {len}\r\nContent-Security-Policy: {CSP}\r\n\
         X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nConnection: {}\r\n",
        if keep_alive { "keep-alive" } else { "close" }
    );
    for (name, value) in extra {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    out.into_bytes()
}

/// A short plain-text response (errors).
pub fn plain(status: &str, text: &str, keep_alive: bool, extra: &[(&str, &str)]) -> Vec<u8> {
    let mut headers = vec![("Content-Type", "text/plain; charset=utf-8"), ("Cache-Control", "no-store")];
    headers.extend_from_slice(extra);
    let mut out = head(status, &headers, text.len(), keep_alive);
    out.extend_from_slice(text.as_bytes());
    out
}

/// A body: gzip when the client takes it, nothing after the head for HEAD.
pub fn body(req: &Request, body: &Body, cache: &str, keep_alive: bool) -> Vec<u8> {
    if req.header("if-none-match").is_some_and(|tag| tag == body.etag) {
        return head("304 Not Modified", &[("Cache-Control", cache), ("ETag", &body.etag)], 0, keep_alive);
    }
    let (bytes, gzip) = match &body.gzip {
        Some(z) if req.takes_gzip() => (z.as_slice(), true),
        _ => (body.plain.as_slice(), false),
    };
    let mut headers = vec![("Content-Type", body.mime.as_str()), ("Cache-Control", cache), ("ETag", body.etag.as_str())];
    if body.gzip.is_some() {
        headers.push(("Vary", "Accept-Encoding"));
    }
    if gzip {
        headers.push(("Content-Encoding", "gzip"));
    }
    let mut out = head("200 OK", &headers, bytes.len(), keep_alive);
    if req.method != "HEAD" {
        out.extend_from_slice(bytes);
    }
    out
}

/// Files are named by their content, so a browser keeps them for good.
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// The page names the files, so it is checked every time (cheaply, by ETag).
pub const REVALIDATE: &str = "no-cache";

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> ClientBundle {
        ClientBundle {
            index_html: "<script src=\"{{asset:shell.js}}\"></script><x>{{asset:game.wasm}}</x>".into(),
            files: vec![
                ("shell.js".into(), b"console.log('hi'); ".repeat(50), mime_for("shell.js").into()),
                ("game.wasm".into(), vec![0, 97, 115, 109, 1, 0, 0, 0], mime_for("game.wasm").into()),
            ],
        }
    }

    #[test]
    fn the_page_names_each_file_by_its_content() {
        let assets = Assets::prepare(&bundle()).unwrap();
        let page = String::from_utf8(assets.index.plain.clone()).unwrap();
        assert!(!page.contains("{{"), "{page}");
        let names: Vec<&String> = assets.files.keys().collect();
        assert_eq!(names.len(), 2);
        for name in names {
            assert!(page.contains(&format!("/a/{name}")), "{page}");
        }
        let wasm = assets.files.values().find(|b| b.mime == "application/wasm").unwrap();
        // Eight bytes do not shrink, so no gzip is kept for them.
        assert!(wasm.gzip.is_none());
        let js = assets.files.values().find(|b| b.mime.starts_with("text/javascript")).unwrap();
        assert!(js.gzip.as_ref().unwrap().len() < js.plain.len());
    }

    #[test]
    fn a_mark_without_its_file_is_an_error() {
        let mut b = bundle();
        b.index_html.push_str("{{asset:missing.png}}");
        assert!(Assets::prepare(&b).is_err());
    }

    #[test]
    fn request_heads_parse_and_bad_ones_do_not() {
        let r = Request::parse(
            "GET /ws?x=1 HTTP/1.1\r\nHost: a\r\nUpgrade: WebSocket\r\nConnection: keep-alive, Upgrade\r\n\
             Accept-Encoding: br, gzip;q=0.8",
        )
        .unwrap();
        assert_eq!((r.method, r.path), ("GET", "/ws"));
        assert_eq!(r.header("upgrade"), Some("WebSocket"));
        assert!(r.lists("Connection", "upgrade"));
        assert!(r.takes_gzip());
        assert!(r.keep_alive());
        let no = Request::parse("GET / HTTP/1.1\r\nAccept-Encoding: gzip;q=0").unwrap();
        assert!(!no.takes_gzip());
        assert!(Request::parse("GET / HTTP/2\r\n").is_none());
        assert!(Request::parse("GET http://x/ HTTP/1.1").is_none());
        assert!(Request::parse("GET / HTTP/1.1\r\nBad Header: x").is_none());
        assert!(!Request::parse("GET / HTTP/1.0").unwrap().keep_alive());
    }

    #[test]
    fn every_response_carries_the_policy() {
        let text = String::from_utf8(plain("404 Not Found", "no", false, &[])).unwrap();
        assert!(text.contains("Content-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval'"));
        assert!(text.contains("X-Content-Type-Options: nosniff"));
    }
}
