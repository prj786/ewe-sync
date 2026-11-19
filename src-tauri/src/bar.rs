//! The bar's sync glyph, from Rust.
//!
//! Until 0.16.2 the page owned this: a Svelte store (stores.js `trayState`)
//! derived the state from the account, the one file, the folder runner and
//! the button in flight, and an effect poked the shell. That is why
//! `ewe-sync --hidden` had to keep a WebKit page alive all session (~125 MB
//! of WebKitWebProcess + WebKitNetworkProcess for a window nobody saw). Now
//! the page exists only while the window is on screen (tray.rs), and this is
//! the state machine — same inputs, same verdicts, same IPC verb
//! (`qs ipc call sync state <state> <detail>`, public API: the shell's
//! IpcHub.qml depends on the name):
//!
//!   signed-out · offline · syncing · conflict · idle
//!
//! Inputs: `ewe-cloud status` and `ewe-conf sync-status`, read every 2
//! minutes (what the hidden page read since 0.16.2) and whenever the page
//! asks for them (its own refresh, every 30 s while shown, feeds the cache);
//! the folder runner reports every change of state (folders.rs `emit`);
//! Sync now and Bring everything back hold a `Busy` guard while they run.
//! The shell is poked on every change and once per poll regardless, so a
//! restarted shell gets the state back within 2 minutes.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::tools::run_tool;

/// What the hidden page polled since 0.16.2; the page's own 30 s refresh
/// feeds the cache on top while the window is shown.
const POLL: Duration = Duration::from_secs(120);

#[derive(Default)]
struct Folders {
    syncing: bool,
    /// a folder conflict or a folder that keeps failing needs the user
    needs_you: bool,
}

pub struct Bar {
    cloud: Mutex<Value>,
    sync: Mutex<Value>,
    folders: Mutex<Folders>,
    busy: AtomicUsize,
    last: Mutex<Option<(String, String)>>,
}

/// Held while Sync now / Bring everything back run: the glyph spins.
pub struct Busy(Arc<Bar>);

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.busy.fetch_sub(1, Ordering::SeqCst);
        let b = self.0.clone();
        tauri::async_runtime::spawn(async move { b.push(false).await });
    }
}

pub fn bar(app: &AppHandle) -> Arc<Bar> {
    app.state::<Arc<Bar>>().inner().clone()
}

impl Bar {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cloud: Mutex::new(Value::Null),
            sync: Mutex::new(Value::Null),
            folders: Mutex::new(Folders::default()),
            busy: AtomicUsize::new(0),
            last: Mutex::new(None),
        })
    }

    /// The poll, for the app's lifetime.
    pub fn start(self: &Arc<Self>) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                me.refresh().await;
                me.push(true).await;
                tokio::time::sleep(POLL).await;
            }
        });
    }

    /// Read the account and the one file now (after a sign-in, a sign-out,
    /// a Sync now) and tell the shell.
    pub async fn refresh(&self) {
        let cloud = run_tool("ewe-cloud", &["status"], None, &[])
            .await
            .map(|o| o.json)
            .unwrap_or(Value::Null);
        let signed_in = cloud["signed_in"].as_bool().unwrap_or(false);
        *self.cloud.lock().await = cloud;
        let sync = if signed_in {
            run_tool("ewe-conf", &["sync-status"], None, &[])
                .await
                .map(|o| o.json)
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        *self.sync.lock().await = sync;
    }

    /// The page asked `cloud_status`: the answer is the cache's too.
    pub async fn note_cloud(&self, v: &Value) {
        *self.cloud.lock().await = v.clone();
        if !v["signed_in"].as_bool().unwrap_or(false) {
            *self.sync.lock().await = Value::Null;
        }
        self.push(false).await;
    }

    /// The page asked `conf_sync_status` (or pushed / restored).
    pub async fn note_sync(&self, v: &Value) {
        *self.sync.lock().await = v.clone();
        self.push(false).await;
    }

    /// The folder runner's snapshot changed (folders.rs `emit`).
    pub async fn note_folders(&self, snap: &Value) {
        let f = Folders {
            syncing: snap["syncing"].as_bool().unwrap_or(false),
            needs_you: snap["conflicts"].as_u64().unwrap_or(0) > 0
                || snap["errors"].as_u64().unwrap_or(0) > 0,
        };
        *self.folders.lock().await = f;
        self.push(false).await;
    }

    pub fn busy(self: &Arc<Self>) -> Busy {
        self.busy.fetch_add(1, Ordering::SeqCst);
        let b = self.clone();
        tauri::async_runtime::spawn(async move { b.push(false).await });
        Busy(self.clone())
    }

    /// (state, detail) — the verdict of stores.js `trayState`, unchanged.
    pub async fn state(&self) -> (String, String) {
        let c = self.cloud.lock().await.clone();
        let s = self.sync.lock().await.clone();
        let f = self.folders.lock().await;
        let signed_in = c["signed_in"].as_bool().unwrap_or(false);
        let detail = if signed_in {
            let who = c["display_name"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| c["user"].as_str())
                .unwrap_or("");
            format!("ewe Sync — {who}")
        } else {
            "ewe Sync — not signed in".to_string()
        };
        let state = if !signed_in {
            "signed-out"
        } else if c["offline"].as_bool().unwrap_or(false) {
            "offline"
        } else if self.busy.load(Ordering::SeqCst) > 0 || f.syncing {
            "syncing"
        } else if conflict_of(&s).is_some() || f.needs_you {
            "conflict"
        } else {
            "idle"
        };
        (state.to_string(), detail)
    }

    /// Tell the shell; `force` repeats an unchanged state (the poll).
    async fn push(&self, force: bool) {
        let now = self.state().await;
        let mut last = self.last.lock().await;
        if !force && last.as_ref() == Some(&now) {
            return;
        }
        *last = Some(now.clone());
        drop(last);
        poke_shell(&now.0, &now.1);
    }
}

/// The settings-sync conflict in an `ewe-conf sync-status` (or push) reply:
/// "remote-newer" | "remote-exists" | None — stores.js `conflictOf`, in Rust.
/// `sync-status` never put the guard's verdict in `error` (that is the
/// transport error); newer ewe-conf answers `conflict`, an older one is read
/// from `in_sync`.
pub fn conflict_of(s: &Value) -> Option<&'static str> {
    if s.is_null() {
        return None;
    }
    match s["error"].as_str() {
        Some("remote-newer") => return Some("remote-newer"),
        Some("remote-exists") => return Some("remote-exists"),
        _ => {}
    }
    if let Some(obj) = s.as_object() {
        if obj.contains_key("conflict") {
            return match s["conflict"].as_str() {
                Some("remote-newer") => Some("remote-newer"),
                Some("remote-exists") => Some("remote-exists"),
                _ => None,
            };
        }
    }
    if s["ok"].as_bool() == Some(true) && !s["remote"].is_null() && s["in_sync"] == Value::Bool(false)
    {
        return Some(if s["local_synced_at"].is_null() || s["local_synced_at"] == Value::Bool(false) {
            "remote-exists"
        } else {
            "remote-newer"
        });
    }
    None
}

/// Tell the ewe bar what we are doing, so "is my stuff safe" is answerable
/// without opening this app. Same out-of-process contract ewe-conf and
/// ewe-settings use: poke a named IPC target, never touch the shell's state
/// directly. Best-effort by design — outside the ewe desktop `qs` is simply
/// not there.
fn poke_shell(state: &str, detail: &str) {
    // unit tests run on a developer's live desktop: never reach its shell
    if cfg!(test) {
        return;
    }
    let child = std::process::Command::new("qs")
        .args(["ipc", "call", "sync", "state", state, detail])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    // Reap it. A spawned child nobody waits for stays a zombie for the life
    // of this process: 39 `[qs] <defunct>` under a 20-minute-old ewe-sync
    // on 2026-09-20, one per state poke, forever. The wait happens off the
    // command thread so a wedged shell cannot stall anything.
    if let Ok(mut child) = child {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conflicts_read_like_the_page_did() {
        assert_eq!(conflict_of(&Value::Null), None);
        assert_eq!(conflict_of(&json!({"ok": false, "error": "remote-newer"})), Some("remote-newer"));
        assert_eq!(conflict_of(&json!({"ok": true, "conflict": "remote-exists"})), Some("remote-exists"));
        assert_eq!(conflict_of(&json!({"ok": true, "conflict": null, "remote": {}, "in_sync": false})), None);
        // an older ewe-conf: no `conflict` key, read from in_sync
        assert_eq!(
            conflict_of(&json!({"ok": true, "remote": {"machine": "a"}, "in_sync": false, "local_synced_at": "2026-01-01"})),
            Some("remote-newer")
        );
        assert_eq!(
            conflict_of(&json!({"ok": true, "remote": {"machine": "a"}, "in_sync": false})),
            Some("remote-exists")
        );
        assert_eq!(conflict_of(&json!({"ok": true, "remote": {"machine": "a"}, "in_sync": true})), None);
        assert_eq!(conflict_of(&json!({"ok": true, "remote": null, "in_sync": false})), None);
    }

    #[tokio::test]
    async fn the_state_machine_is_the_pages() {
        let b = Bar::new();
        assert_eq!(b.state().await.0, "signed-out");
        *b.cloud.lock().await = json!({"signed_in": true, "user": "me", "display_name": "Me"});
        assert_eq!(b.state().await, ("idle".into(), "ewe Sync — Me".into()));
        *b.folders.lock().await = Folders { syncing: false, needs_you: true };
        assert_eq!(b.state().await.0, "conflict");
        *b.folders.lock().await = Folders { syncing: true, needs_you: true };
        assert_eq!(b.state().await.0, "syncing");
        *b.folders.lock().await = Folders::default();
        *b.sync.lock().await = json!({"ok": false, "error": "remote-exists"});
        assert_eq!(b.state().await.0, "conflict");
        {
            let _g = b.busy();
            assert_eq!(b.state().await.0, "syncing");
        }
        // the guard's drop runs its push on tauri's runtime; the counter is down at once
        assert_eq!(b.busy.load(Ordering::SeqCst), 0);
        *b.cloud.lock().await = json!({"signed_in": true, "user": "me", "offline": true});
        assert_eq!(b.state().await, ("offline".into(), "ewe Sync — me".into()));
        *b.cloud.lock().await = json!({"signed_in": false});
        assert_eq!(b.state().await.1, "ewe Sync — not signed in");
    }
}
