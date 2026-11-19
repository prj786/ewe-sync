//! Tauri commands: the account (ewe-cloud), the one file (ewe-conf), the
//! machine registry (WebDAV next to ewe.conf), and the shell poke.

use std::process::Stdio;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::tools::{
    conf_get, conf_key_allowed, ewe_tool, ewe_version, home, hostname, machine_id, run_tool,
};

fn estr<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// The DE's live look (accent, scheme, surface), like Komble reads it.
#[tauri::command]
pub async fn de_prefs() -> Result<Value, String> {
    let p = home().join(".config/quickshell/user-theme.json");
    let Ok(text) = std::fs::read_to_string(&p) else {
        return Ok(Value::Null);
    };
    let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if j.is_null() {
        return Ok(Value::Null);
    }
    Ok(json!({
        // "" = the user has never picked one, so the app leaves the theme
        // default from tokens.css in place instead of overriding it
        "accent": j["accent"].as_str().unwrap_or(""),
        "themeName": j["themeName"].as_str().unwrap_or("flock"),
        "colorScheme": j["colorScheme"].as_str().unwrap_or("dark"),
        // The SHAPE half travels too: the front end keys its token injection
        // on this object, and keying on the accent alone meant a corner or
        // density change moved the shell and stopped at the app boundary.
        "themeCorner": j["themeCorner"].as_str().unwrap_or(""),
        "themeDensity": j["themeDensity"].as_str().unwrap_or(""),
        "themeStroke": j["themeStroke"].as_str().unwrap_or(""),
        "neutralTint": j["neutralTint"].as_i64().unwrap_or(-1),
        // the whole file too: the scheme, look presets and accessibility
        // modes ride in it as well, and the front end keys its theme
        // refresh on all of it (lib/theme.js watchTheme)
        "raw": j.clone(),
    }))
}

/// The generated tokens for one look, straight from `ewe-theme show` — which
/// reads ewe-theme.conf, the single source of truth for every colour and shape
/// in ewe. The app compiles design/tokens.css in as a fallback, so this is
/// what lets an edit to the conf land WITHOUT rebuilding the app: the values
/// are re-injected as CSS custom properties when the window regains focus.
/// The generator is reused rather than reimplemented here, so the mapping
/// (hover = "invert" -> --btn-hover-*, and the rest) exists in exactly one
/// place.
#[tauri::command]
pub async fn theme_tokens(theme: String) -> Result<Value, String> {
    if theme.is_empty()
        || theme.len() > 64
        || !theme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("bad theme name".into());
    }
    Ok(run_tool("ewe-theme", &["show", theme.as_str()], None, &[])
        .await?
        .json)
}

// ── account ─────────────────────────────────────────────────────────────────

/// The page's read of the account feeds the bar's state machine too
/// (bar.rs): one answer, two readers.
#[tauri::command]
pub async fn cloud_status(app: AppHandle) -> Result<Value, String> {
    let v = run_tool("ewe-cloud", &["status"], None, &[]).await?.json;
    crate::bar::bar(&app).note_cloud(&v).await;
    Ok(v)
}

fn server_ok(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty()
        && s.len() <= 200
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | ':' | '~' | '%')
        })
}

/// A login name for `ewe-cloud login --user`: what Nextcloud accepts (a user
/// id or an email), never empty, no control characters, 256 at most.
fn login_name_ok(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.chars().count() <= 256 && !t.chars().any(char::is_control)
}

/// Sign in through ewe-cloud, two ways.
///
/// With `user` and `password` (0.16.3 — no Nextcloud page): `ewe-cloud login
/// <server> --user <name> --password-stdin`; the password goes down the
/// tool's stdin, never its argv, and ewe-cloud trades it once for an app
/// password (OCS core/getapppassword) and keeps only that. Accounts with
/// two-step verification or single sign-on answer `browser: true`.
///
/// Without (Login Flow v2): the login URL arrives on the tool's stderr as
/// `login-url: …` long before the JSON result; it is forwarded to the window
/// as a `login-url` event so the pane can offer Open / Copy while the browser
/// step is pending.
#[tauri::command]
pub async fn cloud_login(
    app: AppHandle,
    server: String,
    user: Option<String>,
    password: Option<String>,
) -> Result<Value, String> {
    if !server_ok(&server) {
        return Err("that does not look like a server address".into());
    }
    let Some(path) = ewe_tool("ewe-cloud") else {
        return Err("ewe-cloud is not installed — is ewe deployed on this machine?".into());
    };
    let with_password = match (&user, &password) {
        (Some(u), Some(p)) if !p.is_empty() => {
            if !login_name_ok(u) {
                return Ok(
                    json!({"ok": false, "error": "no-user", "message": "Enter your username or email."}),
                );
            }
            if p.contains('\n') || p.contains('\r') {
                return Ok(
                    json!({"ok": false, "error": "bad-password", "message": "A password can't contain a line break."}),
                );
            }
            true
        }
        _ => false,
    };
    let mut cmd = Command::new("python3");
    cmd.arg(&path).args(["login", server.trim()]);
    if with_password {
        cmd.args([
            "--user",
            user.as_deref().unwrap_or("").trim(),
            "--password-stdin",
        ]);
    }
    let mut child = cmd
        .env("LANG", "C")
        .stdin(if with_password {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(estr)?;
    if with_password {
        use tokio::io::AsyncWriteExt;
        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        let mut line = password.unwrap_or_default().into_bytes();
        line.push(b'\n');
        stdin.write_all(&line).await.map_err(estr)?;
        line.fill(0);
        drop(stdin);
    }
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let app2 = app.clone();
    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut tail: Vec<String> = Vec::new();
        while let Ok(Some(l)) = lines.next_line().await {
            if let Some(url) = l.strip_prefix("login-url: ") {
                let _ = app2.emit("login-url", url.trim().to_string());
            }
            if tail.len() >= 20 {
                tail.remove(0);
            }
            tail.push(l);
        }
        tail
    });
    let out = child.wait_with_output().await.map_err(estr)?;
    let _ = reader.await;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json = stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
        .unwrap_or(json!({"ok": false, "error": "internal", "message": "no reply from ewe-cloud"}));
    Ok(json)
}

#[tauri::command]
pub async fn cloud_logout() -> Result<Value, String> {
    Ok(run_tool("ewe-cloud", &["logout"], None, &[]).await?.json)
}

/// The account's picture as a `data:` URL. `ewe-cloud avatar` saves it to
/// ~/.cache/ewe/cloud-avatar.<ext> and answers the path; this app never had
/// Tauri's asset protocol (no `protocol-asset` feature, no scope), so the
/// `asset://` URL `convertFileSrc` made of that path failed in every build and
/// WebKit drew its broken-image "?" in the avatar (0.16.3). Inlining the bytes
/// needs no file access from the page at all. Anything unreadable, not an
/// image or over 4 MiB → `ok: false`, and the page shows the initial.
#[tauri::command]
pub async fn cloud_avatar() -> Result<Value, String> {
    let reply = run_tool("ewe-cloud", &["avatar"], None, &[]).await?.json;
    if reply.get("ok").and_then(Value::as_bool) != Some(true) {
        return Ok(reply);
    }
    let Some(path) = reply.get("path").and_then(Value::as_str) else {
        return Ok(json!({"ok": false, "error": "no-avatar"}));
    };
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) => return Ok(json!({"ok": false, "error": "unreadable", "message": e.to_string()})),
    };
    let declared = reply.get("type").and_then(Value::as_str).unwrap_or("");
    match avatar_data_url(&bytes, declared) {
        Some(url) => Ok(json!({"ok": true, "path": url, "bytes": bytes.len()})),
        None => Ok(json!({"ok": false, "error": "not-an-image"})),
    }
}

const AVATAR_MAX: usize = 4 * 1024 * 1024;

/// `data:<image type>;base64,…` for the bytes of a picture, None when it is
/// empty, too big or not a PNG / JPEG / GIF / WebP / SVG. The type comes from
/// the bytes (a server's error page saved as .png must not become an image).
fn avatar_data_url(bytes: &[u8], declared: &str) -> Option<String> {
    use base64::Engine as _;
    if bytes.is_empty() || bytes.len() > AVATAR_MAX {
        return None;
    }
    let sniffed = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if declared == "image/svg+xml" {
        "image/svg+xml"
    } else {
        return None;
    };
    Some(format!(
        "data:{sniffed};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod login_tests {
    use super::login_name_ok;

    #[test]
    fn login_names_are_ids_or_emails() {
        assert!(login_name_ok("scubba"));
        assert!(login_name_ok(" me@example.org "));
        assert!(!login_name_ok(""));
        assert!(!login_name_ok("   "));
        assert!(!login_name_ok("a\nb"));
        assert!(!login_name_ok(&"x".repeat(257)));
    }
}

#[cfg(test)]
mod avatar_tests {
    use super::avatar_data_url;

    #[test]
    fn a_png_becomes_a_data_url() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        let url = avatar_data_url(png, "image/png").unwrap();
        assert!(url.starts_with("data:image/png;base64,iVBORw0KGgo"));
    }

    #[test]
    fn the_bytes_decide_not_the_name() {
        assert_eq!(
            avatar_data_url(b"<!doctype html><p>502</p>", "image/png"),
            None
        );
        assert!(avatar_data_url(&[0xff, 0xd8, 0xff, 0xe0], "image/png")
            .unwrap()
            .starts_with("data:image/jpeg;"));
        assert_eq!(avatar_data_url(b"", "image/png"), None);
        assert_eq!(
            avatar_data_url(&vec![0x89; super::AVATAR_MAX + 1], "image/png"),
            None
        );
    }
}

/// The keyring playbook (RFC-005 / DE 0.9.18): move the keyring aside,
/// restart the daemon; PAM recreates it at the next login.
#[tauri::command]
pub async fn keyring_reset() -> Result<Value, String> {
    Ok(run_tool("ewe-auth", &["keyring-reset"], None, &[])
        .await?
        .json)
}

/// Log out of the session the way the shell does (power.sh, loginctl).
#[tauri::command]
pub async fn session_logout() -> Result<(), String> {
    let script = home().join(".config/hypr/scripts/power.sh");
    let status = Command::new(script)
        .arg("logout")
        .stdin(Stdio::null())
        .status()
        .await
        .map_err(estr)?;
    if status.success() {
        Ok(())
    } else {
        Err("could not log out".into())
    }
}

// ── the one file ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn conf_sync_status(app: AppHandle) -> Result<Value, String> {
    let v = run_tool("ewe-conf", &["sync-status"], None, &[])
        .await?
        .json;
    crate::bar::bar(&app).note_sync(&v).await;
    Ok(v)
}

#[tauri::command]
pub async fn conf_push(force: Option<bool>) -> Result<Value, String> {
    let mut args = vec!["push"];
    if force.unwrap_or(false) {
        args.push("--force");
    }
    let o = run_tool("ewe-conf", &args, None, &[]).await?;
    if o.json.get("ok").and_then(|b| b.as_bool()) == Some(true) {
        let _ = write_machine_record().await;
        crate::tools::shell_poke("refresh").await;
    }
    Ok(o.json)
}

/// Pull the account's copy over this machine's file. The caller confirms
/// first; afterwards `apply` (with hooks: theme, shell reload, Hyprland
/// reload) makes it real, and the shell is told so its state follows.
#[tauri::command]
pub async fn conf_restore() -> Result<Value, String> {
    let pulled = run_tool("ewe-conf", &["pull"], None, &[]).await?.json;
    if pulled.get("ok").and_then(|b| b.as_bool()) != Some(true) {
        return Ok(pulled);
    }
    let applied = run_tool("ewe-conf", &["apply"], None, &[]).await?;
    crate::tools::shell_poke("refresh").await;
    let apps = conf_get("apps.installed")
        .await
        .unwrap_or(Value::Null)
        .get("packages")
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(json!({
        "ok": applied.code == 0,
        "pulled": pulled,
        "apps": apps,
        "apply_stderr": applied.stderr.lines().last().unwrap_or("").to_string(),
    }))
}

#[tauri::command]
pub async fn conf_get_cmd(key: String) -> Result<Value, String> {
    if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    {
        return Err(format!("bad conf key: {key}"));
    }
    conf_get(&key).await
}

#[tauri::command]
pub async fn conf_set_cmd(key: String, value: Value) -> Result<(), String> {
    if !conf_key_allowed(&key) {
        return Err(format!("ewe Sync may not set {key}"));
    }
    let v = value.to_string();
    let o = run_tool("ewe-conf", &["set", "--no-hooks", &key, &v], None, &[]).await?;
    if o.code != 0 {
        return Err(format!("ewe-conf set {key} failed: {}", o.stderr.trim()));
    }
    crate::tools::shell_poke("refresh").await;
    Ok(())
}

#[tauri::command]
pub async fn shell_poke_cmd(verb: String) -> Result<(), String> {
    crate::tools::shell_poke(&verb).await;
    Ok(())
}

// ── the machine registry: <folder>/machines/<id>.json (id = machine_id()) ──
// Until 0.15 the file was <hostname>.json — two computers with one hostname
// overwrote each other. Legacy files are read (and hidden once an id record
// of the same name exists); this computer deletes its own legacy file.

struct Dav {
    base: String, // …/remote.php/dav/files/<user>/<folder>/machines/
    user: String,
    token: String,
}

async fn dav() -> Result<Dav, String> {
    let t = run_tool("ewe-cloud", &["token", "--json"], None, &[])
        .await?
        .json;
    if t.get("ok").and_then(|b| b.as_bool()) != Some(true) {
        return Err("not-signed-in".into());
    }
    let server = t["server"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    let user = t["user"].as_str().unwrap_or("").to_string();
    let token = t["token"].as_str().unwrap_or("").to_string();
    if server.is_empty() || user.is_empty() || token.is_empty() {
        return Err("not-signed-in".into());
    }
    let folder = conf_get("sync.folder")
        .await
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .filter(|s| !s.trim_matches('/').is_empty())
        .unwrap_or_else(|| "ewe".into());
    let base = format!(
        "{server}/remote.php/dav/files/{}/{}/machines/",
        percent(&user),
        percent(folder.trim_matches('/'))
    );
    Ok(Dav { base, user, token })
}

fn percent(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("ewe-sync ({})", hostname()))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client")
}

pub async fn write_machine_record() -> Result<(), String> {
    let d = dav().await?;
    let c = client();
    // MKCOL: 201 created, 405 exists — both fine
    let _ = c
        .request(
            reqwest::Method::from_bytes(b"MKCOL").map_err(estr)?,
            &d.base,
        )
        .basic_auth(&d.user, Some(&d.token))
        .send()
        .await;
    let apps = conf_get("apps.installed")
        .await
        .unwrap_or(Value::Null)
        .get("packages")
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let record = json!({
        "id": machine_id(),
        "name": hostname(),
        "last_seen": now,
        "ewe_version": ewe_version(),
        "apps_count": apps,
        "app_version": env!("CARGO_PKG_VERSION"),
    });
    let url = format!("{}{}.json", d.base, machine_id());
    let r = c
        .put(&url)
        .basic_auth(&d.user, Some(&d.token))
        .header("Content-Type", "application/json")
        .body(record.to_string())
        .send()
        .await
        .map_err(estr)?;
    if r.status().is_success() {
        // the pre-id file of this computer's name: only ever ours or a
        // same-named twin's, and the twin writes its own id file now too
        let legacy = format!("{}{}.json", d.base, percent(&hostname()));
        let _ = c
            .delete(&legacy)
            .basic_auth(&d.user, Some(&d.token))
            .send()
            .await;
        Ok(())
    } else {
        Err(format!("machine record: http {}", r.status().as_u16()))
    }
}

#[tauri::command]
pub async fn machines_write() -> Result<(), String> {
    write_machine_record().await
}

/// PROPFIND depth 1 on the registry folder, then GET every <name>.json.
#[tauri::command]
pub async fn machines_list() -> Result<Value, String> {
    let d = dav().await?;
    let c = client();
    let r = c
        .request(
            reqwest::Method::from_bytes(b"PROPFIND").map_err(estr)?,
            &d.base,
        )
        .basic_auth(&d.user, Some(&d.token))
        .header("Depth", "1")
        .send()
        .await
        .map_err(estr)?;
    if r.status().as_u16() == 404 {
        return Ok(json!([]));
    }
    if !r.status().is_success() && r.status().as_u16() != 207 {
        return Err(format!("machines: http {}", r.status().as_u16()));
    }
    let body = r.text().await.map_err(estr)?;
    let mut names: Vec<String> = Vec::new();
    let mut rest = body.as_str();
    while let Some(i) = rest.find("<d:href>") {
        rest = &rest[i + 8..];
        let Some(j) = rest.find("</d:href>") else {
            break;
        };
        let href = &rest[..j];
        if let Some(name) = href.rsplit('/').next() {
            if let Some(stem) = name.strip_suffix(".json") {
                names.push(stem.to_string());
            }
        }
        rest = &rest[j..];
    }
    let mut out: Vec<Value> = Vec::new();
    for n in names {
        let url = format!("{}{}.json", d.base, n);
        if let Ok(r) = c.get(&url).basic_auth(&d.user, Some(&d.token)).send().await {
            if let Ok(text) = r.text().await {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    out.push(v);
                }
            }
        }
    }
    // a legacy <hostname>.json (no "id") hides behind an id record of that name
    let named: std::collections::HashSet<String> = out
        .iter()
        .filter(|v| v.get("id").and_then(|i| i.as_str()).is_some())
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();
    out.retain(|v| {
        v.get("id").and_then(|i| i.as_str()).is_some()
            || !v
                .get("name")
                .and_then(|n| n.as_str())
                .is_some_and(|n| named.contains(n))
    });
    out.sort_by(|a, b| {
        b["last_seen"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["last_seen"].as_u64().unwrap_or(0))
    });
    Ok(Value::Array(out))
}

#[tauri::command]
pub fn this_machine() -> Value {
    json!({ "id": machine_id(), "name": hostname(), "ewe_version": ewe_version() })
}

/// `ewe-sync --check`: are the tools here, what does the account say. For
/// the smoke test and for support; prints one JSON line.
pub async fn self_check() -> Value {
    let tools: Vec<Value> = ["ewe-cloud", "ewe-conf", "ewe-auth"]
        .iter()
        .map(|n| json!({ "tool": n, "path": ewe_tool(n).map(|p| p.display().to_string()) }))
        .collect();
    let status = run_tool("ewe-cloud", &["status"], None, &[])
        .await
        .map(|o| o.json)
        .unwrap_or_else(|e| json!({ "error": e }));
    let sync = run_tool("ewe-conf", &["sync-status"], None, &[])
        .await
        .map(|o| o.json)
        .unwrap_or_else(|e| json!({ "error": e }));
    let machines = machines_list()
        .await
        .unwrap_or_else(|e| json!({ "error": e }));
    json!({
        "ok": tools.iter().all(|t| !t["path"].is_null()),
        "tools": tools,
        "cloud": status,
        "sync": sync,
        "machines": machines,
        "this_machine": this_machine(),
    })
}
