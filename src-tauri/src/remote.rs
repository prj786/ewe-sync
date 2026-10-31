//! The account's files over WebDAV, read-only: a folder's ETag (Nextcloud
//! changes a folder's ETag whenever anything inside it changes — the same
//! signal the desktop client polls), a folder's children with their sizes,
//! and the account's free space. Credentials come from ewe-cloud (the app
//! password in the keyring) and stay in memory for a few minutes, never on
//! disk or argv.

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::tools::{hostname, run_tool};

#[derive(Clone)]
pub struct Creds {
    pub server: String,
    pub host: String,
    pub user: String,
    pub token: String,
}

impl Creds {
    /// `<server>/remote.php/dav/files/<user>/` — the files root.
    pub fn files_root(&self) -> String {
        format!(
            "{}/remote.php/dav/files/{}/",
            self.server,
            percent(&self.user, false)
        )
    }

    /// The DAV URL of an account path (`""` = the files root).
    pub fn url(&self, remote: &str) -> String {
        let r = norm(remote);
        if r.is_empty() {
            self.files_root()
        } else {
            format!("{}{}/", self.files_root(), percent(&r, true))
        }
    }
}

/// `/Documents/` → `Documents`; `/` → `` (the files root).
pub fn norm(r: &str) -> String {
    r.trim().trim_matches('/').to_string()
}

pub fn percent(s: &str, keep_slash: bool) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b'/' if keep_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn unpercent(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// `ewe-cloud token --json` → the signed-in account, or None.
pub async fn fresh_creds() -> Option<Creds> {
    let t = run_tool("ewe-cloud", &["token", "--json"], None, &[])
        .await
        .ok()?
        .json;
    if t.get("ok").and_then(|b| b.as_bool()) != Some(true) {
        return None;
    }
    let server = t["server"].as_str()?.trim_end_matches('/').to_string();
    let user = t["user"].as_str()?.to_string();
    let token = t["token"].as_str()?.to_string();
    if server.is_empty() || user.is_empty() || token.is_empty() {
        return None;
    }
    let host = server
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string();
    Some(Creds {
        server,
        host,
        user,
        token,
    })
}

/// The credentials, cached for a few minutes so the remote-change poll does
/// not start a Python process per folder per minute. `forget` drops them
/// (a 401: the app password was revoked or rotated).
pub struct CredCache {
    slot: Mutex<Option<(Creds, Instant)>>,
}

const CRED_TTL: Duration = Duration::from_secs(600);

impl CredCache {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }

    pub async fn get(&self) -> Option<Creds> {
        let mut s = self.slot.lock().await;
        if let Some((c, at)) = s.as_ref() {
            if at.elapsed() < CRED_TTL {
                return Some(c.clone());
            }
        }
        let c = fresh_creds().await;
        *s = c.clone().map(|c| (c, Instant::now()));
        c
    }

    pub async fn forget(&self) {
        *self.slot.lock().await = None;
    }
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("ewe-sync ({})", hostname()))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("reqwest client")
}

/// Why a request did not answer: no network, the account refused the app
/// password, the folder is not there, or anything else the server said.
#[derive(Debug, PartialEq)]
pub enum DavErr {
    Offline,
    Unauthorized,
    Missing,
    Other(String),
}

impl std::fmt::Display for DavErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DavErr::Offline => write!(f, "the server can’t be reached"),
            DavErr::Unauthorized => write!(f, "the account no longer accepts this computer"),
            DavErr::Missing => write!(f, "the folder isn’t in your account"),
            DavErr::Other(s) => write!(f, "{s}"),
        }
    }
}

const PROPS: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns"><d:prop><d:getetag/><d:resourcetype/><d:getcontentlength/><oc:size/><d:quota-available-bytes/><d:quota-used-bytes/></d:prop></d:propfind>"#;

async fn propfind(c: &Creds, remote: &str, depth: u8) -> Result<String, DavErr> {
    let method =
        reqwest::Method::from_bytes(b"PROPFIND").map_err(|e| DavErr::Other(e.to_string()))?;
    let r = client()
        .request(method, c.url(remote))
        .basic_auth(&c.user, Some(&c.token))
        .header("Depth", depth.to_string())
        .header("Content-Type", "application/xml")
        .body(PROPS)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                DavErr::Offline
            } else {
                DavErr::Other(e.to_string())
            }
        })?;
    match r.status().as_u16() {
        207 | 200 => r.text().await.map_err(|e| DavErr::Other(e.to_string())),
        401 | 403 => Err(DavErr::Unauthorized),
        404 => Err(DavErr::Missing),
        n => Err(DavErr::Other(format!("http {n}"))),
    }
}

// ── a small multistatus reader ──────────────────────────────────────────────

/// Drop namespace prefixes from tag names (`<d:getetag>` → `<getetag>`), so
/// the lookups below never care which prefix a server picked.
fn strip_ns(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut i = 0;
    while i < xml.len() {
        let rest = &xml[i..];
        let c = rest.chars().next().unwrap_or(' ');
        if c != '<' {
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        out.push('<');
        i += 1;
        if xml[i..].starts_with('/') {
            out.push('/');
            i += 1;
        }
        let end = xml[i..]
            .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
            .map(|e| i + e)
            .unwrap_or(xml.len());
        let name = &xml[i..end];
        out.push_str(name.rsplit(':').next().unwrap_or(name));
        i = end;
    }
    out
}

/// The inner text of the first `<tag …>…</tag>` from `from`, with the index
/// just past its end; `<tag/>` is "".
fn element<'a>(s: &'a str, tag: &str, from: usize) -> Option<(&'a str, usize)> {
    let open = format!("<{tag}");
    let mut at = from;
    loop {
        let i = at + s.get(at..)?.find(&open)?;
        let next = s[i + open.len()..].chars().next()?;
        if next == '>' || next == '/' || next.is_whitespace() {
            let gt = i + s[i..].find('>')?;
            if s[..gt].ends_with('/') {
                return Some(("", gt + 1));
            }
            let close = format!("</{tag}>");
            let end = gt + 1 + s[gt + 1..].find(&close)?;
            return Some((&s[gt + 1..end], end + close.len()));
        }
        at = i + 1;
    }
}

fn text(s: &str, tag: &str) -> Option<String> {
    element(s, tag, 0).map(|(t, _)| xml_unescape(t.trim()))
}

fn xml_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// One `<response>` of a multistatus reply.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub etag: String,
    pub size: u64,
    pub free: Option<i64>,
    pub used: Option<u64>,
}

pub fn parse_multistatus(xml: &str) -> Vec<Entry> {
    let xml = strip_ns(xml);
    let mut out = Vec::new();
    let mut at = 0;
    while let Some((block, next)) = element(&xml, "response", at) {
        at = next;
        let Some(href) = text(block, "href") else {
            continue;
        };
        let path = unpercent(href.trim_end_matches('/'));
        let num = |t: &str| text(block, t).and_then(|v| v.parse::<u64>().ok());
        out.push(Entry {
            name: path.rsplit('/').next().unwrap_or("").to_string(),
            dir: block.contains("<collection"),
            etag: text(block, "getetag")
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
            size: num("size").or_else(|| num("getcontentlength")).unwrap_or(0),
            free: text(block, "quota-available-bytes").and_then(|v| v.parse::<i64>().ok()),
            used: num("quota-used-bytes"),
        });
    }
    out
}

/// The folder's ETag — it changes when anything below it changes.
pub async fn etag(c: &Creds, remote: &str) -> Result<String, DavErr> {
    let body = propfind(c, remote, 0).await?;
    parse_multistatus(&body)
        .into_iter()
        .next()
        .map(|e| e.etag)
        .filter(|e| !e.is_empty())
        .ok_or_else(|| DavErr::Other("no etag in the reply".into()))
}

/// A folder's children (folders first, then by name) with sizes, plus the
/// folder itself (its size and, at the root, the account's free space).
pub async fn list(c: &Creds, remote: &str) -> Result<Value, DavErr> {
    let body = propfind(c, remote, 1).await?;
    let mut entries = parse_multistatus(&body);
    if entries.is_empty() {
        return Err(DavErr::Other("empty reply".into()));
    }
    let me = entries.remove(0);
    entries.sort_by(|a, b| {
        b.dir
            .cmp(&a.dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let base = norm(remote);
    let items: Vec<Value> = entries
        .into_iter()
        .filter(|e| !e.name.is_empty())
        .map(|e| {
            json!({
                "name": e.name,
                "path": if base.is_empty() { e.name.clone() } else { format!("{base}/{}", e.name) },
                "dir": e.dir,
                "size": e.size,
            })
        })
        .collect();
    Ok(json!({
        "path": base,
        "size": me.size,
        "free": me.free,
        "used": me.used,
        "items": items,
    }))
}

/// The size of an account folder, or None when the account has no such
/// folder (the add form's "already in your account" hint).
pub async fn size_of(c: &Creds, remote: &str) -> Result<Option<u64>, DavErr> {
    match propfind(c, remote, 0).await {
        Ok(body) => Ok(parse_multistatus(&body).into_iter().next().map(|e| e.size)),
        Err(DavErr::Missing) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NC: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
 <d:response><d:href>/remote.php/dav/files/u%40x.org/</d:href><d:propstat><d:prop><d:getetag>&quot;6720a1&quot;</d:getetag><d:resourcetype><d:collection/></d:resourcetype><oc:size>3000</oc:size><d:quota-available-bytes>8000</d:quota-available-bytes><d:quota-used-bytes>3000</d:quota-used-bytes></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat><d:propstat><d:prop><d:getcontentlength/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/files/u%40x.org/My%20Docs/</d:href><d:propstat><d:prop><d:getetag>&quot;aa&quot;</d:getetag><d:resourcetype><d:collection/></d:resourcetype><oc:size>2000</oc:size></d:prop></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/files/u%40x.org/a.txt</d:href><d:propstat><d:prop><d:getetag>&quot;bb&quot;</d:getetag><d:resourcetype/><d:getcontentlength>12</d:getcontentlength></d:prop></d:propstat></d:response>
</d:multistatus>"#;

    #[test]
    fn multistatus() {
        let e = parse_multistatus(NC);
        assert_eq!(e.len(), 3);
        assert!(e[0].dir);
        assert_eq!(e[0].etag, "6720a1");
        assert_eq!(e[0].size, 3000);
        assert_eq!(e[0].free, Some(8000));
        assert_eq!(e[0].used, Some(3000));
        assert_eq!(e[1].name, "My Docs");
        assert!(e[1].dir);
        assert_eq!(e[1].size, 2000);
        assert_eq!(e[2].name, "a.txt");
        assert!(!e[2].dir);
        assert_eq!(e[2].size, 12);
    }

    #[test]
    fn no_prefixes() {
        let x = r#"<multistatus xmlns="DAV:"><response><href>/dav/x/</href><propstat><prop><getetag>"e1"</getetag><resourcetype><collection/></resourcetype></prop></propstat></response></multistatus>"#;
        let e = parse_multistatus(x);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].etag, "e1");
        assert_eq!(e[0].name, "x");
        assert!(e[0].dir);
    }

    #[test]
    fn urls() {
        let c = Creds {
            server: "https://c.example.org".into(),
            host: "c.example.org".into(),
            user: "a b@c".into(),
            token: "t".into(),
        };
        assert_eq!(
            c.files_root(),
            "https://c.example.org/remote.php/dav/files/a%20b%40c/"
        );
        assert_eq!(
            c.url("/Photos/2026/"),
            "https://c.example.org/remote.php/dav/files/a%20b%40c/Photos/2026/"
        );
        assert_eq!(c.url("/"), c.files_root());
        assert_eq!(unpercent("My%20Docs"), "My Docs");
    }
}
