//! The other accounts ewe-sync manages (RFC-005/RFC-006, 2026-09-03).
//!
//! The ewe account itself is Nextcloud and lives in `cloud.rs`. These are the
//! two that sit beside it, and they moved here out of ewe-settings when
//! Settings became a read-only view of what is connected: Settings shows,
//! ewe-sync manages.
//!
//!   * **Mail** — any number of IMAP/SMTP mailboxes (`ewe-mail`). Not tied to
//!     the ewe account: the inbox a Nextcloud provider gives you, a
//!     self-hosted server, a work account. The passwords go to the keyring;
//!     only the servers and the user are recorded in `ewe.conf`.
//!   * **Google** — an OPTIONAL extra for Gmail and a Drive folder
//!     (`ewe-auth`). ewe ships no Google client of its own, so this pane's
//!     first job is telling the user where their own `oauth-client.json`
//!     goes and whether the one they dropped there actually parses.
//!
//! Every verb is `run_tool` (argv, never a shell) and every tool answers with
//! one JSON object, so failures arrive as data rather than exceptions.

use serde_json::{json, Value};
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::tools::{ewe_tool, home, run_tool};

fn estr<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ── mail (ewe-mail) ─────────────────────────────────────────────────────────

fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 253
        && h.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && !h.starts_with('-')
}

#[tauri::command]
pub async fn mail_status() -> Result<Value, String> {
    Ok(run_tool("ewe-mail", &["status"], None, &[]).await?.json)
}

/// What the add/change sheet sends. Everything past `starttls` is optional,
/// so the old five-argument call (host, port, user, password, starttls) still
/// works unchanged.
#[derive(serde::Deserialize, Default)]
pub struct MailExtra {
    pub email: Option<String>,
    pub name: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_security: Option<String>,
    pub smtp_user: Option<String>,
    /// only when the outgoing server wants a different password
    pub smtp_password: Option<String>,
}

/// Push `flag value` when the optional value is set; refuse a flag-looking one.
fn opt(args: &mut Vec<String>, flag: &str, v: &Option<String>, max: usize) -> Result<(), String> {
    if let Some(v) = v.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        if !plain_line(v, max) || v.starts_with('-') {
            return Err(format!("invalid value for {flag}"));
        }
        args.push(flag.into());
        args.push(v.into());
    }
    Ok(())
}

fn plain_line(v: &str, max: usize) -> bool {
    v.len() <= max && !v.chars().any(char::is_control)
}

/// The `ewe-mail login` argv and its stdin (password line, optional second
/// password line), checked. Pure, so it is unit-tested.
#[allow(clippy::too_many_arguments)]
fn login_args(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    starttls: bool,
    x: &MailExtra,
) -> Result<(Vec<String>, String), String> {
    let host = host.trim();
    let user = user.trim();
    if !valid_host(host) {
        return Err("that does not look like a mail server name".into());
    }
    if user.is_empty() || !plain_line(user, 200) {
        return Err("invalid mail user".into());
    }
    if port == 0 {
        return Err("invalid port".into());
    }
    // one line on stdin: a newline inside the password would end it early
    if password.is_empty() || password.contains('\n') || password.contains('\r') {
        return Err("invalid password".into());
    }
    let mut args: Vec<String> = ["login", host, user, "--port", &port.to_string()]
        .map(String::from)
        .to_vec();
    if starttls {
        args.push("--starttls".into());
    }
    opt(&mut args, "--email", &x.email, 200)?;
    opt(&mut args, "--name", &x.name, 200)?;
    if let Some(h) = x
        .smtp_host
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
    {
        if !valid_host(h) {
            return Err("that does not look like an outgoing server name".into());
        }
        opt(&mut args, "--smtp-host", &x.smtp_host, 253)?;
        if let Some(p) = x.smtp_port.filter(|p| *p != 0) {
            args.push("--smtp-port".into());
            args.push(p.to_string());
        }
        match x.smtp_security.as_deref() {
            Some(m @ ("tls" | "starttls")) => {
                args.push("--smtp-security".into());
                args.push(m.into());
            }
            None | Some("") => {}
            Some(_) => return Err("invalid outgoing security".into()),
        }
    }
    opt(&mut args, "--smtp-user", &x.smtp_user, 200)?;
    let mut stdin = format!("{password}\n");
    if let Some(sp) = x.smtp_password.as_deref().filter(|p| !p.is_empty()) {
        if sp.contains('\n') || sp.contains('\r') {
            return Err("invalid password".into());
        }
        stdin.push_str(sp);
        stdin.push('\n');
    }
    Ok((args, stdin))
}

/// Test + store a mail login (IMAP and SMTP). The passwords travel on
/// **stdin**, never argv — argv is world-readable in /proc. ewe-mail verifies
/// both servers before it keeps anything, so a bad password fails here rather
/// than silently later.
#[tauri::command]
pub async fn mail_login(
    host: String,
    port: u16,
    user: String,
    password: String,
    starttls: Option<bool>,
    extra: Option<MailExtra>,
) -> Result<Value, String> {
    let (args, stdin) = login_args(
        &host,
        port,
        &user,
        &password,
        starttls.unwrap_or(false),
        &extra.unwrap_or_default(),
    )?;
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    Ok(run_tool("ewe-mail", &argv, Some(stdin.as_bytes()), &[])
        .await?
        .json)
}

/// Remove one account by id; no id = the only one (what older versions did).
#[tauri::command]
pub async fn mail_logout(id: Option<String>) -> Result<Value, String> {
    let mut args = vec!["logout"];
    let id = id.map(|i| i.trim().to_string()).filter(|i| !i.is_empty());
    if let Some(i) = id.as_deref() {
        if !valid_mail_id(i) {
            return Err("invalid account".into());
        }
        args.push(i);
    }
    Ok(run_tool("ewe-mail", &args, None, &[]).await?.json)
}

/// Every mail account: `{ok, accounts: [{id, email, name, host, …, keyring}]}`.
#[tauri::command]
pub async fn mail_accounts() -> Result<Value, String> {
    Ok(run_tool("ewe-mail", &["accounts"], None, &[]).await?.json)
}

/// The servers for an address — `{ok, source, imap, smtp, user, note}`.
#[tauri::command]
pub async fn mail_autoconfig(email: String) -> Result<Value, String> {
    let email = email.trim();
    if !valid_email(email) {
        return Err("that does not look like an email address".into());
    }
    Ok(run_tool("ewe-mail", &["autoconfig", email], None, &[])
        .await?
        .json)
}

/// The From name of an account (no secret).
#[tauri::command]
pub async fn mail_set_name(id: String, name: String) -> Result<Value, String> {
    let name = name.trim();
    if !valid_mail_id(&id) || !plain_line(name, 200) {
        return Err("invalid account or name".into());
    }
    Ok(
        run_tool("ewe-mail", &["set", id.as_str(), "name", name], None, &[])
            .await?
            .json,
    )
}

fn valid_email(e: &str) -> bool {
    e.len() <= 254
        && !e.starts_with('-')
        && plain_line(e, 254)
        && e.split_once('@')
            .is_some_and(|(l, d)| !l.is_empty() && d.contains('.') && !d.contains('@'))
        && !e.contains(char::is_whitespace)
}

fn valid_mail_id(i: &str) -> bool {
    !i.starts_with('-') && i.len() <= 300 && plain_line(i, 300) && !i.contains(char::is_whitespace)
}

/// The badge's own view: unread count + the newest unseen messages, every account.
#[tauri::command]
pub async fn mail_unseen() -> Result<Value, String> {
    Ok(
        run_tool("ewe-mail", &["unseen", "--all", "--limit", "5"], None, &[])
            .await?
            .json,
    )
}

// ── google, the optional extra (ewe-auth) ───────────────────────────────────

/// `{path, exists, valid}` for the user's own OAuth client. ewe ships none
/// (RFC-005), so the pane leads with this: without a valid client file there
/// is nothing to connect, and "invalid" must read differently from "missing"
/// — a truncated or wrong-type download is the common mistake.
#[tauri::command]
pub async fn google_client_info() -> Result<Value, String> {
    let path = home().join(".config/ewe/oauth-client.json");
    let exists = path.exists();
    let valid = exists
        && std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| {
                // the console downloads {"installed": {...}}; a flat object works too
                let c = v.get("installed").or_else(|| v.get("web"));
                c.and_then(|c| c.get("client_id"))
                    .or_else(|| v.get("client_id"))
                    .and_then(|i| i.as_str())
                    .map(|i| !i.is_empty())
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    Ok(json!({ "path": path.to_string_lossy(), "exists": exists, "valid": valid }))
}

#[tauri::command]
pub async fn google_status() -> Result<Value, String> {
    Ok(run_tool("ewe-auth", &["status"], None, &[]).await?.json)
}

/// The PKCE + loopback consent flow. Like `cloud_login`, the URL appears on
/// the tool's stderr (`consent-url: …`) long before the JSON result, so it is
/// forwarded as a `google-consent-url` event — a DISTINCT event from the
/// Nextcloud one, so two panes can never show each other's link.
#[tauri::command]
pub async fn google_login(app: AppHandle) -> Result<Value, String> {
    let Some(path) = ewe_tool("ewe-auth") else {
        return Err("ewe-auth is not installed — is ewe deployed on this machine?".into());
    };
    let mut child = Command::new("python3")
        .arg(&path)
        .arg("login")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(estr)?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let app2 = app.clone();
    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            if let Some(url) = l.strip_prefix("consent-url: ") {
                let _ = app2.emit("google-consent-url", url.trim().to_string());
            }
        }
    });
    let out = child.wait_with_output().await.map_err(estr)?;
    let _ = reader.await;
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
        .unwrap_or(json!({"ok": false, "error": "internal", "message": "no reply from ewe-auth"})))
}

#[tauri::command]
pub async fn google_logout() -> Result<Value, String> {
    Ok(run_tool("ewe-auth", &["logout"], None, &[]).await?.json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_login_call_is_unchanged() {
        let (a, stdin) = login_args(
            "imap.example.org",
            993,
            "me@example.org",
            "pw",
            false,
            &MailExtra::default(),
        )
        .unwrap();
        assert_eq!(
            a,
            [
                "login",
                "imap.example.org",
                "me@example.org",
                "--port",
                "993"
            ]
        );
        assert_eq!(stdin, "pw\n");
    }

    #[test]
    fn passwords_only_on_stdin() {
        let x = MailExtra {
            email: Some("me@example.org".into()),
            smtp_host: Some("smtp.example.org".into()),
            smtp_port: Some(587),
            smtp_security: Some("starttls".into()),
            smtp_user: Some("smtp-me".into()),
            smtp_password: Some("second".into()),
            ..Default::default()
        };
        let (a, stdin) = login_args("imap.example.org", 143, "me", "first", true, &x).unwrap();
        assert!(a.contains(&"--starttls".to_string()));
        assert!(a.windows(2).any(|w| w == ["--smtp-port", "587"]));
        assert!(!a.iter().any(|v| v == "first" || v == "second"));
        assert_eq!(stdin, "first\nsecond\n");
    }

    #[test]
    fn bad_input_is_refused() {
        let x = MailExtra::default();
        assert!(login_args("-bad", 993, "u", "p", false, &x).is_err());
        assert!(login_args("h.org", 993, "u", "a\nb", false, &x).is_err());
        assert!(login_args("h.org", 0, "u", "p", false, &x).is_err());
        let bad = MailExtra {
            smtp_host: Some("smtp.x.org".into()),
            smtp_security: Some("none".into()),
            ..Default::default()
        };
        assert!(login_args("h.org", 993, "u", "p", false, &bad).is_err());
        let dash = MailExtra {
            name: Some("--evil".into()),
            ..Default::default()
        };
        assert!(login_args("h.org", 993, "u", "p", false, &dash).is_err());
    }

    #[test]
    fn emails_and_ids() {
        assert!(valid_email("me@example.org"));
        assert!(!valid_email("me@localhost"));
        assert!(!valid_email("-x@y.org"));
        assert!(!valid_email("a b@y.org"));
        assert!(valid_mail_id("imap:me@example.org"));
        assert!(!valid_mail_id("--all"));
    }
}
