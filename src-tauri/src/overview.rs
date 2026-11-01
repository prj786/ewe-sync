//! Everything the account carries, in one place (0.15, "log in, get
//! everything back"): the browser's and Safe's sync state for the
//! Overview, the one Sync now that moves everything, the restore preview
//! ("what comes back") and the one-click restore of a new machine, and the
//! account steps the shell's sign-in has always done (so signing in here
//! is the same as signing in from the Welcome screen).
//!
//! ewe-sync never syncs the browser or Safe itself: the browser follows the
//! account on its own (`sync.browser`, the nudge file) and Safe syncs its
//! own place (`ewe-safe sync`). This is the one place that shows them and
//! the one button that asks them to go — rule 6.

use std::process::Stdio;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::folders::{self, Pair};
use crate::remote;
use crate::tools::{
    conf_get, config_home, run_script, run_tool, shell_poke, spawn_reaped, state_dir, which,
    write_atomic,
};

/// `ewe-sync --pane folders` / `--add-folder <path>` from the command line,
/// held until the window asks for it (the first launch), or sent as an
/// event (a second launch: single-instance forwards argv).
pub struct DeepLink(pub StdMutex<Option<Value>>);

pub fn parse_deep_link(args: &[String]) -> Option<Value> {
    let mut pane: Option<String> = None;
    let mut add: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--pane" => pane = it.next().cloned(),
            "--add-folder" => add = it.next().cloned(),
            _ => {
                if let Some(v) = a.strip_prefix("--pane=") {
                    pane = Some(v.to_string());
                } else if let Some(v) = a.strip_prefix("--add-folder=") {
                    add = Some(v.to_string());
                }
            }
        }
    }
    const PANES: [&str; 7] = [
        "overview", "folders", "machine", "machines", "account", "mail", "google",
    ];
    let pane = pane.filter(|p| PANES.contains(&p.as_str()));
    let add = add.filter(|p| p.starts_with('/') && !p.split('/').any(|c| c == ".."));
    if pane.is_none() && add.is_none() {
        return None;
    }
    Some(json!({
        "pane": if add.is_some() { Some("folders".to_string()) } else { pane },
        "add_folder": add,
    }))
}

pub fn deliver(app: &AppHandle, link: Value) {
    let _ = app.emit("deep-link", link);
}

#[tauri::command]
pub fn take_deep_link(app: AppHandle) -> Value {
    app.state::<DeepLink>()
        .0
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .unwrap_or(Value::Null)
}

/// `ewe-sync --status`: the published snapshot, or what is known without
/// the app running — one JSON line, exit 0 (rule 5).
pub fn status_line() -> String {
    std::fs::read_to_string(folders::public_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or_else(|| {
            json!({
                "version": 1,
                "signed_in": crate::tools::cloud_meta().is_some(),
                "state": if crate::tools::cloud_meta().is_some() { "idle" } else { "signed-out" },
                "folders": [],
                "note": "ewe Sync has not run yet on this computer",
            })
        })
        .to_string()
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Ask every app that syncs by itself to go now (the browser watches this
/// file). The contract: epoch milliseconds, written atomically.
pub fn nudge() {
    let _ = write_atomic(
        &state_dir().join("sync-now"),
        now_ms().to_string().as_bytes(),
    );
}

// ── the browser ─────────────────────────────────────────────────────────────

/// The ewe browser's own record of its sync (it writes it after every
/// cycle) plus this computer's switch (`sync.browser`, absent = on).
#[tauri::command]
pub async fn browser_status() -> Result<Value, String> {
    let installed = which("ewe-browser");
    let on = conf_get("sync.browser")
        .await
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let file = config_home().join("ewe-browser/sync/status.json");
    let status = std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or(Value::Null);
    Ok(json!({
        "installed": installed,
        "on": on,
        "state": status.get("state").cloned().unwrap_or(Value::Null),
        "last_sync": status.get("lastSync").cloned().unwrap_or(Value::Null),
        "error": status.get("error").cloned().unwrap_or(Value::Null),
    }))
}

// ── Safe ────────────────────────────────────────────────────────────────────

/// Where Safe keeps the passwords and whether that place is in step.
/// Safe owns its place (RFC-008); this only reports it.
#[tauri::command]
pub async fn safe_status() -> Result<Value, String> {
    if !which("ewe-safe") {
        return Ok(json!({ "installed": false }));
    }
    let child = tokio::process::Command::new("ewe-safe")
        .arg("status")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = match tokio::time::timeout(Duration::from_secs(5), child).await {
        Ok(Ok(o)) => o,
        _ => return Ok(json!({ "installed": true, "answered": false })),
    };
    let j: Value = String::from_utf8_lossy(&out.stdout)
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str(l).ok())
        .unwrap_or(Value::Null);
    Ok(json!({
        "installed": true,
        "answered": !j.is_null(),
        "set_up": j["set_up"].as_bool().unwrap_or(false),
        "place": j["place"]["kind"],
        "state": j["sync"]["state"],
        "last": j["sync"]["last"],
        "warning": j["sync"]["warning"],
    }))
}

#[tauri::command]
pub fn open_safe() -> Result<(), String> {
    if which("ewe-safe-app") {
        spawn_reaped("ewe-safe-app", &[]);
        Ok(())
    } else {
        Err("Safe isn’t installed — find it in Komble.".into())
    }
}

// ── one Sync now for everything ─────────────────────────────────────────────

/// Settings & apps (`ewe-conf sync`: pull, merge key by key, push — the
/// same verb the shell runs), then every folder, the browser (nudge) and
/// Safe. The settings result comes back; the rest reports through its own
/// status.
#[tauri::command]
pub async fn sync_everything(app: AppHandle) -> Result<Value, String> {
    let conf = run_tool("ewe-conf", &["sync"], None, &[]).await?.json;
    if conf.get("ok").and_then(|b| b.as_bool()) == Some(true) {
        let _ = crate::cloud::write_machine_record().await;
    }
    shell_poke("refresh").await;
    nudge();
    if which("ewe-safe") {
        spawn_reaped("ewe-safe", &["sync"]);
    }
    let r = folders::runner(&app);
    tokio::spawn(async move {
        r.run_all().await;
    });
    Ok(conf)
}

// ── a new machine: what comes back, and bringing it back ────────────────────

/// Read the account's copy of the one file WITHOUT touching this machine's
/// (`ewe-conf pull --out`): which machine saved it, the folders it syncs
/// (with their size in the account), how many apps Komble will offer, and
/// whether the browser and Safe have data in the account.
#[tauri::command]
pub async fn restore_preview(app: AppHandle) -> Result<Value, String> {
    let st = run_tool("ewe-conf", &["sync-status"], None, &[])
        .await?
        .json;
    if st.get("remote").map(|r| r.is_null()).unwrap_or(true) {
        return Ok(json!({ "exists": false }));
    }
    let tmp = state_dir().join(format!("restore-preview-{}.conf", std::process::id()));
    let tmp_s = tmp.display().to_string();
    let pulled = run_tool("ewe-conf", &["pull", "--out", &tmp_s], None, &[])
        .await?
        .json;
    let text = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    if pulled.get("ok").and_then(|b| b.as_bool()) != Some(true) {
        return Err(pulled["message"]
            .as_str()
            .or(pulled["error"].as_str())
            .unwrap_or("couldn’t read the backup")
            .to_string());
    }
    let doc: toml::Value = toml::from_str(&text).map_err(|e| format!("the backup: {e}"))?;
    let doc = serde_json::to_value(doc).unwrap_or(Value::Null);
    let pairs: Vec<Pair> =
        serde_json::from_value(doc["sync"]["folders"].clone()).unwrap_or_default();
    let installed = &doc["apps"]["installed"];
    let apps = installed["packages"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0)
        + installed["appimages"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);
    let creds = folders::runner(&app).creds.get().await;
    let mut out_folders: Vec<Value> = Vec::new();
    for p in pairs.iter().filter(|p| p.validate().is_ok()) {
        let size = match &creds {
            Some(c) if p.mode != "upload" => remote::size_of(c, &p.remote).await.ok().flatten(),
            _ => None,
        };
        let local = folders::expand_local(&p.local);
        out_folders.push(json!({
            "name": p.name(&local),
            "local": p.local,
            "remote": folders::norm_remote(&p.remote),
            "mode": p.mode,
            "size": size,
        }));
    }
    let (browser, safe) = match &creds {
        Some(c) => (
            remote::size_of(c, "ewe/browser")
                .await
                .ok()
                .flatten()
                .is_some(),
            remote::size_of(c, "ewe/safe")
                .await
                .ok()
                .flatten()
                .is_some(),
        ),
        None => (false, false),
    };
    Ok(json!({
        "exists": true,
        "machine": st["remote_machine"],
        "mine": st["remote_is_this_machine"],
        "modified": st["remote_modified"],
        "folders": out_folders,
        "apps": apps,
        "browser": browser,
        "safe": safe,
        "scheme": doc["desktop"]["theme"]["scheme"],
    }))
}

/// Bring everything back on a new machine: the one file (pull + apply —
/// the desktop re-themes, apps wait in Komble), auto-sync on, the folders
/// it lists start now, the browser and Safe are nudged.
#[tauri::command]
pub async fn restore_everything(app: AppHandle) -> Result<Value, String> {
    let r = crate::cloud::conf_restore().await?;
    if r.get("ok").and_then(|b| b.as_bool()) != Some(true) {
        return Ok(r);
    }
    let _ = run_tool(
        "ewe-conf",
        &["set", "--no-hooks", "sync.enabled", "true"],
        None,
        &[],
    )
    .await;
    let runner = folders::runner(&app);
    runner.reload(false).await;
    let keys: Vec<String> = folders_keys(&runner).await;
    for k in &keys {
        runner.request(k, false).await;
    }
    nudge();
    if which("ewe-safe") {
        spawn_reaped("ewe-safe", &["sync"]);
    }
    let _ = crate::cloud::write_machine_record().await;
    shell_poke("refresh").await;
    Ok(json!({
        "ok": true,
        "apps": r["apps"],
        "folders": keys.len(),
        "machine": r["pulled"]["machine"],
    }))
}

async fn folders_keys(r: &std::sync::Arc<folders::Runner>) -> Vec<String> {
    r.snapshot().await["pairs"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|i| i["key"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// ── sign-in / sign-out, the same steps as the shell's ───────────────────────

/// What the shell's sign-in has always done next (Cloud.qml): the one file
/// learns where its backup lives (identity, never a secret), auto-sync is
/// on, and the whole account appears as ~/Nextcloud (ewe-files: a mount,
/// unless a synced folder is already there).
#[tauri::command]
pub async fn after_sign_in(app: AppHandle) -> Result<(), String> {
    let Some((server, user)) = crate::tools::cloud_meta() else {
        return Err("not signed in".into());
    };
    for (k, v) in [
        ("sync.provider", "nextcloud".to_string()),
        ("sync.server", server),
        ("sync.user", user),
        ("sync.folder", "ewe".to_string()),
    ] {
        let _ = run_tool("ewe-conf", &["set", "--no-hooks", k, &v], None, &[]).await;
    }
    let _ = run_tool(
        "ewe-conf",
        &["set", "--no-hooks", "sync.enabled", "true"],
        None,
        &[],
    )
    .await;
    let _ = run_script("ewe-files", &["setup"]).await;
    let _ = crate::cloud::write_machine_record().await;
    let r = folders::runner(&app);
    r.creds.forget().await;
    r.reload(true).await;
    shell_poke("refresh").await;
    nudge();
    Ok(())
}

/// Sign-out's other half (the shell's too): the mount goes, and this
/// machine stops syncing until the next sign-in. Files stay where they are.
#[tauri::command]
pub async fn after_sign_out(app: AppHandle) -> Result<(), String> {
    let _ = run_script("ewe-files", &["unmount"]).await;
    let _ = run_script("ewe-files", &["forget"]).await;
    let _ = run_tool(
        "ewe-conf",
        &["set", "--no-hooks", "sync.enabled", "false"],
        None,
        &[],
    )
    .await;
    let r = folders::runner(&app);
    r.creds.forget().await;
    r.reload(false).await;
    shell_poke("refresh").await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn deep_links() {
        assert_eq!(parse_deep_link(&a(&["--hidden"])), None);
        assert_eq!(
            parse_deep_link(&a(&["--pane", "folders"])),
            Some(json!({ "pane": "folders", "add_folder": null }))
        );
        assert_eq!(
            parse_deep_link(&a(&["--pane=overview"])),
            Some(json!({ "pane": "overview", "add_folder": null }))
        );
        // an add opens Folders, whatever pane was asked for
        assert_eq!(
            parse_deep_link(&a(&["--pane", "account", "--add-folder", "/home/u/Work"])),
            Some(json!({ "pane": "folders", "add_folder": "/home/u/Work" }))
        );
        assert_eq!(parse_deep_link(&a(&["--pane", "nope"])), None);
        assert_eq!(parse_deep_link(&a(&["--add-folder", "relative"])), None);
        assert_eq!(
            parse_deep_link(&a(&["--add-folder", "/home/u/../etc"])),
            None
        );
    }
}
