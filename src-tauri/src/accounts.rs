//! The other accounts ewe-sync manages (RFC-005/RFC-006, 2026-09-03).
//!
//! The ewe account itself is Nextcloud and lives in `cloud.rs`. These are the
//! two that sit beside it, and they moved here out of ewe-settings when
//! Settings became a read-only view of what is connected: Settings shows,
//! ewe-sync manages.
//!
//!   * **Mail** — any IMAP mailbox (`ewe-mail`). Not tied to the ewe account:
//!     the inbox a Nextcloud provider gives you, a self-hosted server, a work
//!     account. The password goes to the keyring; only host/user/port are
//!     recorded in `ewe.conf`.
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

/// Test + store an IMAP login. The password travels on **stdin**, never argv —
/// argv is world-readable in /proc. ewe-mail verifies the login before it
/// keeps anything, so a bad password fails here rather than silently later.
#[tauri::command]
pub async fn mail_login(
    host: String,
    port: u16,
    user: String,
    password: String,
    starttls: Option<bool>,
) -> Result<Value, String> {
    let host = host.trim().to_string();
    let user = user.trim().to_string();
    if !valid_host(&host) {
        return Err("that does not look like a mail server name".into());
    }
    if user.is_empty() || user.len() > 200 || user.chars().any(char::is_control) {
        return Err("invalid mail user".into());
    }
    if port == 0 {
        return Err("invalid port".into());
    }
    // one line on stdin: a newline inside the password would end it early
    if password.is_empty() || password.contains('\n') || password.contains('\r') {
        return Err("invalid password".into());
    }
    let port_s = port.to_string();
    let mut args = vec![
        "login",
        host.as_str(),
        user.as_str(),
        "--port",
        port_s.as_str(),
    ];
    if starttls.unwrap_or(false) {
        args.push("--starttls");
    }
    Ok(run_tool("ewe-mail", &args, Some(password.as_bytes()), &[])
        .await?
        .json)
}

#[tauri::command]
pub async fn mail_logout() -> Result<Value, String> {
    Ok(run_tool("ewe-mail", &["logout"], None, &[]).await?.json)
}

/// The badge's own view: unread count + the newest unseen messages.
#[tauri::command]
pub async fn mail_unseen() -> Result<Value, String> {
    Ok(run_tool("ewe-mail", &["unseen", "--limit", "5"], None, &[])
        .await?
        .json)
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
