//! Folder sync (RFC-006 F2; the ecosystem pass, 0.15): pairs of a local
//! folder and a folder in the account. Two-way pairs run the Nextcloud sync
//! engine headless (`nextcloudcmd`); one-way pairs copy with rclone over
//! WebDAV. Runs are serialised and de-duplicated. A pair runs when its
//! local folder changes (inotify), when its folder in the account changes
//! (the folder's ETag, polled — Nextcloud moves it whenever anything inside
//! changes, the signal the desktop client uses), once shortly after start
//! (catch-up), on an interval, or at login. The definitions live in
//! ewe.conf (`[[sync.folders]]`, shared by every machine); each machine
//! keeps its own local path override, pause switch and run state.
//!
//! Every state change is published to `~/.local/state/ewe/sync-folders.json`
//! (the contract other apps read — the ewe browser's file manager, ewe-files)
//! and connected folders get a GTK bookmark, so Nemo and every Open/Save
//! dialog list them. Everything is the user's own files — no privileges.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};

use crate::ncapp;
use crate::remote::{self, CredCache, Creds, DavErr};
use crate::tools::{
    cloud_meta, conf_get, config_home, home, run_tool, state_dir, tilde, which, write_atomic,
};

/// `nextcloudcmd`'s interface, in ONE place. Checked against the real
/// binary (Nextcloud 35.0.0, `nextcloudcmd --help`, 2026-10-31):
/// `nextcloudcmd [OPTION] <source_dir> <server_url>`.
mod nccmd {
    pub const BIN: &str = "nextcloudcmd";
    /// never block on a prompt
    pub const NON_INTERACTIVE: &str = "--non-interactive";
    pub const SILENT: &str = "--silent";
    /// credentials from `~/.netrc` — we point HOME at a private directory
    /// holding only that file, so the app password is never on the command
    /// line (visible in `ps`)
    pub const NETRC: &str = "-n";
    /// `--exclude <file>`: one pattern per line
    pub const EXCLUDE: &str = "--exclude";
    /// `--path <remote>`: folder on the server, relative to the files root
    pub const PATH: &str = "--path";
    /// sync hidden files too (the desktop app's "ignore hidden files" off)
    pub const HIDDEN: &str = "-h";
}

/// rclone for the one-way modes (`nextcloudcmd` has none): `copy` adds and
/// updates, never deletes — the honest one-way.
mod rc {
    pub const BIN: &str = "rclone";
    pub const REMOTE: &str = "EWESYNC";
}

const DEBOUNCE: Duration = Duration::from_secs(5);

/// A duration from the environment (the end-to-end test shortens the waits),
/// else the default.
fn secs(var: &str, default: u64) -> Duration {
    Duration::from_secs(
        std::env::var(var)
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|n: &u64| *n > 0)
            .unwrap_or(default),
    )
}
/// how often a folder in the account is asked for its ETag
fn poll_every() -> Duration {
    secs("EWE_SYNC_POLL_SECS", 45)
}
/// the catch-up run after start / after another machine added a folder
fn start_delay() -> Duration {
    secs("EWE_SYNC_START_DELAY_SECS", 15)
}
const CONFLICT_CAP: usize = 200;

fn d_mode() -> String {
    "two-way".into()
}
fn d_trigger() -> String {
    "change".into()
}
fn d_interval() -> u64 {
    10
}
fn is_false(b: &bool) -> bool {
    !*b
}

/// One `[[sync.folders]]` record — synced in ewe.conf like everything else.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pair {
    pub remote: String,
    pub local: String,
    #[serde(default = "d_mode")]
    pub mode: String,
    #[serde(default = "d_trigger")]
    pub trigger: String,
    #[serde(default = "d_interval")]
    pub interval: u64,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// sync hidden files (dotfiles) too — off unless asked, like nextcloudcmd
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
}

impl Pair {
    /// The key a machine's state is filed under: the remote side and the
    /// mode — the local path is per machine and may differ.
    pub fn key(&self) -> String {
        format!("{}|{}", norm_remote(&self.remote), self.mode)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.mode.as_str(), "two-way" | "upload" | "download") {
            return Err(format!("unknown mode {:?}", self.mode));
        }
        if !matches!(self.trigger.as_str(), "change" | "interval" | "login") {
            return Err(format!("unknown trigger {:?}", self.trigger));
        }
        if self.trigger == "interval" && !(1..=1440).contains(&self.interval) {
            return Err("interval must be 1–1440 minutes".into());
        }
        if self.local.trim().is_empty() {
            return Err("a local folder is required".into());
        }
        if self.remote.contains("..") || self.local.contains("..") {
            return Err("paths may not contain '..'".into());
        }
        if self.exclude.iter().any(|e| e.contains('\n')) {
            return Err("exclude patterns are one per line".into());
        }
        Ok(())
    }

    /// The name people know the folder by: the local folder's name.
    pub fn name(&self, local: &Path) -> String {
        local
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                let r = norm_remote(&self.remote);
                if r.is_empty() {
                    "Your account".into()
                } else {
                    r
                }
            })
    }
}

/// `/Documents/` → `Documents`; `/` → `` (the files root).
pub fn norm_remote(r: &str) -> String {
    remote::norm(r)
}

pub fn expand_local(p: &str) -> PathBuf {
    let p = p.trim();
    if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
    } else if p == "~" {
        home()
    } else {
        PathBuf::from(p)
    }
}

/// Paths compared as folders: no trailing slash, `..` already refused.
fn clean(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    let t = s.trim_end_matches('/');
    PathBuf::from(if t.is_empty() { "/" } else { t })
}

fn overlaps(a: &Path, b: &Path) -> bool {
    let (a, b) = (clean(a), clean(b));
    a == b || a.starts_with(&b) || b.starts_with(&a)
}

/// Where the folders of one machine may go: any folder but the home folder
/// itself (or one above it), never one inside another, never on top of a
/// folder another engine owns (`blocked`: the account's mount, the
/// Nextcloud app's folders). Pure, for the tests.
pub fn check_layout(
    locals: &[(String, PathBuf)],
    home: &Path,
    blocked: &[(PathBuf, String)],
) -> Result<(), String> {
    for (i, (_, a)) in locals.iter().enumerate() {
        let a = clean(a);
        if !a.is_absolute() {
            return Err(format!("{} is not a full path", a.display()));
        }
        if a == clean(home) || clean(home).starts_with(&a) {
            return Err(format!(
                "{} is your whole home folder — pick a folder inside it, like ~/Documents",
                tilde(&a)
            ));
        }
        for (_, b) in &locals[i + 1..] {
            if overlaps(&a, b) {
                return Err(format!(
                    "{} and {} overlap — a folder can’t sync inside another synced folder",
                    tilde(&a),
                    tilde(b)
                ));
            }
        }
        for (p, why) in blocked {
            if overlaps(&a, p) {
                return Err(why.clone());
            }
        }
    }
    Ok(())
}

// ── per-machine state: ~/.local/state/ewe/ewe-sync.json ─────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PairState {
    #[serde(default)]
    pub local_override: Option<String>,
    #[serde(default)]
    pub last_run: u64,
    #[serde(default)]
    pub last_ok: bool,
    #[serde(default)]
    pub last_error: String,
    #[serde(default)]
    pub conflicts: Vec<String>,
    /// paused on THIS machine (the definition stays in the account)
    #[serde(default)]
    pub paused: bool,
    /// the account folder's ETag after the last good run
    #[serde(default)]
    pub remote_etag: String,
    /// live only — never persisted
    #[serde(skip)]
    pub running: bool,
    #[serde(skip)]
    pub offline: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct StateFile {
    #[serde(default)]
    pairs: HashMap<String, PairState>,
    /// GTK bookmarks ewe-sync added (only these are ever removed again)
    #[serde(default)]
    bookmarks: Vec<String>,
    /// when the Nextcloud app's folders moved here, and where its autostart
    /// entry went
    #[serde(default)]
    takeover: Option<Value>,
}

fn state_path() -> PathBuf {
    state_dir().join("ewe-sync.json")
}

/// The contract other apps read (see the vault: Account and Sync).
pub fn public_path() -> PathBuf {
    state_dir().join("sync-folders.json")
}

fn load_state() -> StateFile {
    std::fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str::<StateFile>(&s).ok())
        .unwrap_or_default()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `~/Nextcloud` — where ewe-files mounts the whole account.
pub fn account_folder() -> PathBuf {
    std::env::var_os("EWE_FILES_MOUNT")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("Nextcloud"))
}

fn is_mountpoint(p: &Path) -> bool {
    let want = clean(p).to_string_lossy().to_string();
    std::fs::read_to_string("/proc/mounts")
        .map(|m| {
            m.lines().any(|l| {
                l.split(' ')
                    .nth(1)
                    .map(|mp| mp.replace("\\040", " ").replace("\\011", "\t") == want)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// The XDG user folders (Desktop, Documents, …) from `user-dirs.dirs`, with
/// the usual names as the fallback.
pub fn user_dirs() -> Vec<(String, PathBuf)> {
    let h = home();
    let text = std::fs::read_to_string(config_home().join("user-dirs.dirs")).unwrap_or_default();
    let mut found: HashMap<String, PathBuf> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("XDG_") else {
            continue;
        };
        let Some((key, val)) = rest.split_once("_DIR=") else {
            continue;
        };
        let v = val
            .trim()
            .trim_matches('"')
            .replace("$HOME", &h.to_string_lossy());
        found.insert(key.to_string(), PathBuf::from(v));
    }
    [
        ("DESKTOP", "Desktop"),
        ("DOCUMENTS", "Documents"),
        ("PICTURES", "Pictures"),
        ("MUSIC", "Music"),
        ("VIDEOS", "Videos"),
        ("DOWNLOAD", "Downloads"),
    ]
    .iter()
    .map(|(k, name)| {
        (
            k.to_lowercase(),
            found.get(*k).cloned().unwrap_or_else(|| h.join(name)),
        )
    })
    .filter(|(_, p)| clean(p) != clean(&h))
    .collect()
}

// ── conflicts ───────────────────────────────────────────────────────────────

/// If `name` is a conflict copy the Nextcloud engine wrote, the name of the
/// original it shadows. Two dialects: the current
/// `report (conflicted copy 2026-09-02 153000).docx` and the older
/// `report_conflict-20260902-153000.docx`.
pub fn conflict_original(name: &str) -> Option<String> {
    if let Some(i) = name.find(" (conflicted copy ") {
        let rest = &name[i..];
        let close = rest.find(')')?;
        let stamp = &rest[" (conflicted copy ".len()..close];
        // "YYYY-MM-DD HHMMSS", possibly with a suffix the client adds
        if stamp.len() < 17 || !stamp.as_bytes()[..4].iter().all(u8::is_ascii_digit) {
            return None;
        }
        return Some(format!("{}{}", &name[..i], &rest[close + 1..]));
    }
    if let Some(i) = name.find("_conflict-") {
        let rest = &name[i + "_conflict-".len()..];
        let b = rest.as_bytes();
        let ok = b.len() >= 15
            && b[..8].iter().all(u8::is_ascii_digit)
            && b[8] == b'-'
            && b[9..15].iter().all(u8::is_ascii_digit);
        if ok {
            return Some(format!("{}{}", &name[..i], &rest[15..]));
        }
    }
    None
}

fn scan_conflicts(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue; // hidden, and the engine's ._sync_*.db journals
            }
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if conflict_original(&name).is_some() {
                if let Ok(rel) = p.strip_prefix(root) {
                    out.push(rel.to_string_lossy().to_string());
                    if out.len() >= CONFLICT_CAP {
                        return out;
                    }
                }
            }
        }
    }
    out.sort();
    out
}

/// A relative path handed back by the UI must resolve INSIDE the pair's
/// local folder — the only place ewe-sync ever renames or deletes.
fn inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|c| c == "..") {
        return Err("bad path".into());
    }
    let root_c = root.canonicalize().map_err(|e| format!("folder: {e}"))?;
    let p = root_c.join(rel);
    let parent = p.parent().ok_or("bad path")?;
    let parent_c = parent.canonicalize().map_err(|e| format!("path: {e}"))?;
    if !parent_c.starts_with(&root_c) {
        return Err("path escapes the folder".into());
    }
    Ok(parent_c.join(p.file_name().ok_or("bad path")?))
}

/// The engine's own bookkeeping churns during a run — not a change.
fn engine_noise(p: &Path) -> bool {
    p.file_name()
        .map(|n| {
            let n = n.to_string_lossy();
            n.starts_with("._sync_")
                || n.starts_with(".sync_")
                || n.starts_with(".~")
                || n.starts_with(".nextcloudsync")
                || n.ends_with(".~tmp")
        })
        .unwrap_or(false)
}

/// Bytes and files under a folder, counted for at most `budget` — a big
/// tree answers "more than" instead of hanging the add form.
pub fn dir_size(root: &Path, budget: Duration) -> (u64, u64, bool) {
    let start = std::time::Instant::now();
    let (mut bytes, mut files) = (0u64, 0u64);
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                stack.push(e.path());
            } else if md.is_file() {
                bytes += md.len();
                files += 1;
            }
            if files % 512 == 0 && start.elapsed() > budget {
                return (bytes, files, true);
            }
        }
    }
    (bytes, files, false)
}

// ── GTK bookmarks: connected folders in every sidebar and file dialog ───────

fn file_uri(p: &Path) -> String {
    format!("file://{}", remote::percent(&p.to_string_lossy(), true))
}

fn bookmarks_path() -> PathBuf {
    config_home().join("gtk-3.0").join("bookmarks")
}

/// Add a bookmark for each connected folder that is not a user folder the
/// sidebars list anyway, and drop the ones ewe-sync added for folders that
/// are gone. A bookmark the user made is never touched. Returns what
/// ewe-sync owns now.
fn sync_bookmarks(want: &[PathBuf], ours: &[String]) -> Vec<String> {
    let path = bookmarks_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let want_uris: Vec<String> = want.iter().map(|p| file_uri(p)).collect();
    let uri_of = |l: &str| {
        l.split(' ')
            .next()
            .unwrap_or("")
            .trim_end_matches('/')
            .to_string()
    };
    let present: HashSet<String> = text.lines().map(uri_of).collect();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| {
            let uri = uri_of(l);
            !(ours.contains(&uri) && !want_uris.contains(&uri))
        })
        .map(String::from)
        .collect();
    let mut owned: Vec<String> = ours
        .iter()
        .filter(|o| want_uris.contains(o))
        .cloned()
        .collect();
    for (p, uri) in want.iter().zip(&want_uris) {
        if present.contains(uri) {
            continue;
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        lines.push(format!("{uri} {name}"));
        owned.push(uri.clone());
    }
    let mut body = lines.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    if body != text {
        let _ = write_atomic(&path, body.as_bytes());
    }
    owned
}

// ── the runner ──────────────────────────────────────────────────────────────

pub struct Runner {
    app: AppHandle,
    run_lock: Mutex<()>,
    state: Mutex<HashMap<String, PairState>>,
    extra: Mutex<(Vec<String>, Option<Value>)>, // (bookmarks, takeover)
    pairs: Mutex<Vec<Pair>>,
    queued: Mutex<HashSet<String>>,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    watchers: Mutex<Vec<notify::RecommendedWatcher>>,
    conf_watcher: Mutex<Option<notify::RecommendedWatcher>>,
    pub creds: CredCache,
    auto: Mutex<bool>,
}

impl Runner {
    pub fn new(app: AppHandle) -> Arc<Self> {
        let f = load_state();
        Arc::new(Self {
            app,
            run_lock: Mutex::new(()),
            state: Mutex::new(f.pairs),
            extra: Mutex::new((f.bookmarks, f.takeover)),
            pairs: Mutex::new(Vec::new()),
            queued: Mutex::new(HashSet::new()),
            tasks: Mutex::new(Vec::new()),
            watchers: Mutex::new(Vec::new()),
            conf_watcher: Mutex::new(None),
            creds: CredCache::new(),
            auto: Mutex::new(false),
        })
    }

    async fn save(&self) {
        let pairs = self.state.lock().await.clone();
        let (bookmarks, takeover) = self.extra.lock().await.clone();
        let f = StateFile {
            pairs,
            bookmarks,
            takeover,
        };
        if let Ok(s) = serde_json::to_string_pretty(&f) {
            let _ = write_atomic(&state_path(), s.as_bytes());
        }
    }

    async fn locals(&self) -> Vec<PathBuf> {
        let pairs = self.pairs.lock().await.clone();
        let st = self.state.lock().await;
        pairs
            .iter()
            .map(|p| {
                expand_local(
                    &st.get(&p.key())
                        .and_then(|s| s.local_override.clone())
                        .unwrap_or_else(|| p.local.clone()),
                )
            })
            .collect()
    }

    /// The Nextcloud app's folders that it still owns: the app runs or
    /// starts at login, the folder is for this account and not already an
    /// ewe-sync folder (after a takeover the app's file still lists them).
    async fn app_folders(&self) -> (Vec<ncapp::AppFolder>, bool) {
        let Some((server, _)) = cloud_meta() else {
            return (Vec::new(), false);
        };
        let running = ncapp::running();
        let folders = ncapp::app_folders(&server);
        if folders.is_empty() || !(running || ncapp::autostarts()) {
            return (Vec::new(), running);
        }
        let ours = self.locals().await;
        let shown = folders
            .into_iter()
            .filter(|f| !ours.iter().any(|o| clean(o) == clean(Path::new(&f.local))))
            .collect();
        (shown, running)
    }

    fn pair_state_word(st: &PairState, auto: bool) -> &'static str {
        if st.running {
            "syncing"
        } else if st.paused || !auto {
            "paused"
        } else if !st.conflicts.is_empty() {
            "conflict"
        } else if st.offline {
            "offline"
        } else if !st.last_error.is_empty() {
            "error"
        } else if st.last_run > 0 {
            "synced"
        } else {
            "pending"
        }
    }

    pub async fn snapshot(&self) -> Value {
        let pairs = self.pairs.lock().await.clone();
        let auto = *self.auto.lock().await;
        let (apps, app_running) = self.app_folders().await;
        let state = self.state.lock().await.clone();
        let takeover = self.extra.lock().await.1.clone();
        let mut items: Vec<Value> = Vec::new();
        for p in &pairs {
            let st = state.get(&p.key()).cloned().unwrap_or_default();
            let local = expand_local(&st.local_override.clone().unwrap_or_else(|| p.local.clone()));
            items.push(json!({
                "key": p.key(),
                "pair": p,
                "name": p.name(&local),
                "local_effective": local.display().to_string(),
                "state": Self::pair_state_word(&st, auto),
                "running": st.running,
                "paused": st.paused,
                "offline": st.offline,
                "last_run": st.last_run,
                "last_ok": st.last_ok,
                "last_error": st.last_error,
                "conflicts": st.conflicts,
            }));
        }
        let app_items: Vec<Value> = apps
            .iter()
            .map(|f| {
                json!({
                    "local": f.local,
                    "remote": f.remote,
                    "paused": f.paused,
                    "name": Path::new(&f.local)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                })
            })
            .collect();
        let af = account_folder();
        let synced_here = |l: &str| clean(Path::new(l)) == clean(&af);
        let af_state = if is_mountpoint(&af) {
            "mounted"
        } else if items.iter().any(|i| {
            i["local_effective"]
                .as_str()
                .map(synced_here)
                .unwrap_or(false)
        }) || apps.iter().any(|f| synced_here(&f.local))
        {
            "synced"
        } else {
            "off"
        };
        json!({
            "pairs": items,
            "auto": auto,
            "syncing": state.values().any(|s| s.running),
            "conflicts": state.values().map(|s| s.conflicts.len()).sum::<usize>(),
            "errors": items.iter().filter(|i| i["state"] == "error").count(),
            "engines": { "nextcloudcmd": which(nccmd::BIN), "rclone": which(rc::BIN) },
            "app": { "running": app_running, "folders": app_items },
            "account_folder": { "path": af.display().to_string(), "state": af_state },
            "takeover": takeover,
        })
    }

    /// The contract (`sync-folders.json`) from the UI snapshot.
    pub fn public(snap: &Value) -> Value {
        let meta = cloud_meta();
        let auto = snap["auto"].as_bool().unwrap_or(false);
        let mut folders: Vec<Value> = Vec::new();
        for i in snap["pairs"].as_array().cloned().unwrap_or_default() {
            let ok = i["last_ok"].as_bool().unwrap_or(false);
            folders.push(json!({
                "key": i["key"],
                "name": i["name"],
                "local": i["local_effective"],
                "remote": norm_remote(i["pair"]["remote"].as_str().unwrap_or("")),
                "mode": i["pair"]["mode"],
                "engine": "ewe",
                "state": i["state"],
                "last_sync": if ok { i["last_run"].clone() } else { json!(0) },
                "conflicts": i["conflicts"].as_array().map(|a| a.len()).unwrap_or(0),
                "error": i["last_error"],
            }));
        }
        let app_running = snap["app"]["running"].as_bool().unwrap_or(false);
        for f in snap["app"]["folders"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let paused = f["paused"].as_bool().unwrap_or(false);
            folders.push(json!({
                "key": format!("app|{}", f["local"].as_str().unwrap_or("")),
                "name": f["name"],
                "local": f["local"],
                "remote": f["remote"],
                "mode": "two-way",
                "engine": "nextcloud-app",
                "state": if paused { "paused" } else if app_running { "synced" } else { "offline" },
                "last_sync": 0,
                "conflicts": 0,
                "error": "",
            }));
        }
        let any = |w: &str| folders.iter().any(|f| f["state"] == w);
        let state = if meta.is_none() {
            "signed-out"
        } else if any("syncing") {
            "syncing"
        } else if any("error") || any("conflict") {
            "attention"
        } else if !auto {
            "paused"
        } else if !folders.is_empty() && folders.iter().all(|f| f["state"] == "offline") {
            "offline"
        } else {
            "idle"
        };
        let account = meta.as_ref().map(|(s, u)| {
            format!(
                "{u} on {}",
                s.trim_start_matches("https://")
                    .trim_start_matches("http://")
            )
        });
        json!({
            "version": 1,
            "updated_at": now(),
            "signed_in": meta.is_some(),
            "account": account,
            "auto": auto,
            "state": state,
            "folders": folders,
            "account_folder": snap["account_folder"],
        })
    }

    async fn emit(&self) {
        let v = self.snapshot().await;
        let body = Self::public(&v).to_string();
        let _ = write_atomic(&public_path(), body.as_bytes());
        let _ = self.app.emit("folders-status", v);
    }

    async fn effective_local(&self, p: &Pair) -> PathBuf {
        let st = self.state.lock().await;
        let l = st
            .get(&p.key())
            .and_then(|s| s.local_override.clone())
            .unwrap_or_else(|| p.local.clone());
        expand_local(&l)
    }

    /// What no folder of this machine may sit on: the whole account while it
    /// is mounted, and folders the Nextcloud app still syncs.
    async fn blocked(&self) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        let af = account_folder();
        if is_mountpoint(&af) {
            out.push((
                af.clone(),
                format!(
                    "{} is your whole account, opened as a folder — pick a folder outside it",
                    tilde(&af)
                ),
            ));
        }
        for f in self.app_folders().await.0 {
            if f.paused {
                continue;
            }
            out.push((
                PathBuf::from(&f.local),
                format!(
                    "The Nextcloud app already syncs {}. Let ewe Sync take it over first, or pick another folder.",
                    tilde(Path::new(&f.local))
                ),
            ));
        }
        out
    }

    /// Start once: the ewe.conf watcher (folders another machine added, the
    /// auto-sync switch, sign-in) and the remote-change poll live for the
    /// app's lifetime; reload() owns the per-pair triggers.
    pub async fn start(self: &Arc<Self>) {
        self.watch_conf().await;
        let me = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(poll_every()).await;
                me.poll_remote().await;
            }
        });
        self.reload(true).await;
    }

    async fn read_auto(&self) -> bool {
        let on = conf_get("sync.enabled")
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        *self.auto.lock().await = on;
        on
    }

    /// Read `[[sync.folders]]`, (re)build every trigger. `catch_up` runs every
    /// pair once shortly after (start, or folders arriving from the account).
    pub async fn reload(self: &Arc<Self>, catch_up: bool) {
        let v = conf_get("sync.folders").await.unwrap_or(Value::Null);
        let pairs: Vec<Pair> = serde_json::from_value(v).unwrap_or_default();
        let pairs: Vec<Pair> = pairs.into_iter().filter(|p| p.validate().is_ok()).collect();
        self.read_auto().await;
        {
            let mut t = self.tasks.lock().await;
            for h in t.drain(..) {
                h.abort();
            }
            self.watchers.lock().await.clear();
        }
        *self.pairs.lock().await = pairs.clone();
        {
            // forget state of pairs that no longer exist
            let keys: Vec<String> = pairs.iter().map(Pair::key).collect();
            self.state.lock().await.retain(|k, _| keys.contains(k));
        }
        for p in &pairs {
            let key = p.key();
            let local = self.effective_local(p).await;
            match p.trigger.as_str() {
                "change" => self.watch(key.clone(), local).await,
                "interval" => {
                    let me = self.clone();
                    let mins = p.interval.max(1);
                    let k = key.clone();
                    self.tasks.lock().await.push(tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(mins * 60)).await;
                            me.request(&k, true).await;
                        }
                    }));
                }
                _ => {}
            }
        }
        if catch_up {
            let me = self.clone();
            let keys: Vec<String> = pairs.iter().map(Pair::key).collect();
            self.tasks.lock().await.push(tokio::spawn(async move {
                tokio::time::sleep(start_delay()).await;
                for k in keys {
                    me.request(&k, true).await;
                }
            }));
        }
        self.refresh_bookmarks().await;
        self.save().await;
        self.emit().await;
    }

    async fn refresh_bookmarks(&self) {
        let h = clean(&home());
        let af = clean(&account_folder());
        let xdg: Vec<PathBuf> = user_dirs().into_iter().map(|(_, p)| clean(&p)).collect();
        let mut want: Vec<PathBuf> = Vec::new();
        for l in self.locals().await {
            let l = clean(&l);
            if l != h && l != af && !xdg.contains(&l) && !want.contains(&l) {
                want.push(l);
            }
        }
        let mut extra = self.extra.lock().await;
        extra.0 = sync_bookmarks(&want, &extra.0);
    }

    /// ewe.conf changed (a merge from another machine, Settings, a restore):
    /// pick up a changed folder list or switch; cloud.json changed: signed
    /// in or out. Debounced — one write is several events.
    async fn watch_conf(self: &Arc<Self>) {
        let dir = config_home().join("ewe");
        let _ = std::fs::create_dir_all(&dir);
        let (tx, mut rx) = mpsc::channel::<()>(16);
        let w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                let ours = ev.paths.iter().any(|p| {
                    matches!(
                        p.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .as_deref(),
                        Some("ewe.conf") | Some("cloud.json")
                    )
                });
                if ours {
                    let _ = tx.try_send(());
                }
            }
        });
        let Ok(mut w) = w else { return };
        if w.watch(&dir, RecursiveMode::NonRecursive).is_err() {
            return;
        }
        *self.conf_watcher.lock().await = Some(w);
        let me = self.clone();
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                while let Ok(Some(())) =
                    tokio::time::timeout(Duration::from_secs(2), rx.recv()).await
                {}
                let v = conf_get("sync.folders").await.unwrap_or(Value::Null);
                let fresh: Vec<Pair> = serde_json::from_value::<Vec<Pair>>(v)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|p| p.validate().is_ok())
                    .collect();
                let current = me.pairs.lock().await.clone();
                me.creds.forget().await;
                if fresh != current {
                    me.reload(true).await;
                    continue;
                }
                let was = *me.auto.lock().await;
                if me.read_auto().await && !was {
                    // auto-sync turned back on: catch up
                    for p in &current {
                        me.request(&p.key(), true).await;
                    }
                }
                me.emit().await;
            }
        });
    }

    /// inotify on the local folder, debounced: a burst of writes becomes one
    /// run once the folder has been quiet for DEBOUNCE.
    async fn watch(self: &Arc<Self>, key: String, local: PathBuf) {
        let _ = std::fs::create_dir_all(&local);
        let (tx, mut rx) = mpsc::channel::<()>(64);
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                if !ev.paths.iter().all(|p| engine_noise(p)) {
                    let _ = tx.try_send(());
                }
            }
        });
        let Ok(mut w) = watcher else { return };
        if w.watch(&local, RecursiveMode::Recursive).is_err() {
            return;
        }
        self.watchers.lock().await.push(w);
        let me = self.clone();
        self.tasks.lock().await.push(tokio::spawn(async move {
            while rx.recv().await.is_some() {
                // quiet period: keep resetting while events arrive
                while let Ok(Some(())) = tokio::time::timeout(DEBOUNCE, rx.recv()).await {}
                me.request(&key, true).await;
            }
        }));
    }

    /// The account side of "on change": ask each folder for its ETag; a
    /// different one than after the last good run means another machine (or
    /// the web) changed something inside — run.
    async fn poll_remote(self: &Arc<Self>) {
        if !*self.auto.lock().await || cloud_meta().is_none() {
            return;
        }
        let pairs: Vec<Pair> = self
            .pairs
            .lock()
            .await
            .iter()
            .filter(|p| p.trigger == "change" && p.mode != "upload")
            .cloned()
            .collect();
        if pairs.is_empty() {
            return;
        }
        let Some(c) = self.creds.get().await else {
            return;
        };
        let mut changed = false;
        for p in pairs {
            let key = p.key();
            let (skip, last) = {
                let st = self.state.lock().await;
                let s = st.get(&key).cloned().unwrap_or_default();
                (s.paused || s.running, s.remote_etag)
            };
            if skip {
                continue;
            }
            match remote::etag(&c, &p.remote).await {
                Ok(e) => {
                    {
                        let mut st = self.state.lock().await;
                        let s = st.entry(key.clone()).or_default();
                        if s.offline {
                            s.offline = false;
                            changed = true;
                        }
                    }
                    if e != last {
                        self.request(&key, true).await;
                    }
                }
                Err(DavErr::Offline) => {
                    let mut st = self.state.lock().await;
                    let s = st.entry(key).or_default();
                    if !s.offline {
                        s.offline = true;
                        changed = true;
                    }
                }
                Err(DavErr::Unauthorized) => {
                    self.creds.forget().await;
                    return;
                }
                // the folder is not in the account yet (a two-way pair
                // creates it on its first run) — nothing to compare
                Err(_) => {}
            }
        }
        if changed {
            self.emit().await;
        }
    }

    /// Queue a run (once — a folder already queued is not queued again).
    /// `auto`: honour the auto-sync switch and this machine's pause.
    pub async fn request(self: &Arc<Self>, key: &str, auto: bool) {
        if auto {
            if !self.read_auto().await {
                return;
            }
            if self
                .state
                .lock()
                .await
                .get(key)
                .map(|s| s.paused)
                .unwrap_or(false)
            {
                return;
            }
        }
        if !self.queued.lock().await.insert(key.to_string()) {
            return;
        }
        let me = self.clone();
        let k = key.to_string();
        // detached from the trigger tasks: a reload must never abort a run
        // half way (its "running" flag would stick)
        tokio::spawn(async move {
            let _ = me.run(&k).await;
        });
    }

    pub async fn run(&self, key: &str) -> Result<Value, String> {
        let pair = self
            .pairs
            .lock()
            .await
            .iter()
            .find(|p| p.key() == key)
            .cloned();
        let Some(pair) = pair else {
            self.queued.lock().await.remove(key);
            return Err("no such folder pair".into());
        };
        let _guard = self.run_lock.lock().await;
        self.queued.lock().await.remove(key);
        {
            let mut st = self.state.lock().await;
            st.entry(key.to_string()).or_default().running = true;
        }
        self.emit().await;
        let local = self.effective_local(&pair).await;
        let result = self.run_checked(&pair, &local).await;
        let conflicts = if pair.mode == "two-way" && local.is_dir() {
            scan_conflicts(&local)
        } else {
            Vec::new()
        };
        let etag = if result.is_ok() && pair.mode != "upload" {
            match self.creds.get().await {
                Some(c) => remote::etag(&c, &pair.remote).await.ok(),
                None => None,
            }
        } else {
            None
        };
        let new_conflicts;
        {
            let mut st = self.state.lock().await;
            let s = st.entry(key.to_string()).or_default();
            new_conflicts = conflicts.len() > s.conflicts.len();
            s.running = false;
            s.last_run = now();
            s.conflicts = conflicts.clone();
            if let Some(e) = etag {
                s.remote_etag = e;
            }
            match &result {
                Ok(()) => {
                    s.last_ok = true;
                    s.offline = false;
                    s.last_error.clear();
                }
                Err((msg, offline)) => {
                    s.last_ok = false;
                    s.offline = *offline;
                    s.last_error = msg.clone();
                }
            }
        }
        self.save().await;
        self.emit().await;
        if new_conflicts {
            self.notify_conflicts(&pair.name(&local), conflicts.len());
        }
        result.map(|_| json!({ "ok": true })).map_err(|(m, _)| m)
    }

    /// The guards that hold at run time too (the Nextcloud app may have been
    /// started again on the same folder), then the engine.
    async fn run_checked(&self, pair: &Pair, local: &Path) -> Result<(), (String, bool)> {
        for f in self.app_folders().await.0 {
            if !f.paused && overlaps(local, Path::new(&f.local)) {
                return Err((
                    format!(
                        "The Nextcloud app is syncing {} too. Quit it, or let ewe Sync take its folders over.",
                        tilde(Path::new(&f.local))
                    ),
                    false,
                ));
            }
        }
        let af = account_folder();
        if is_mountpoint(&af) && overlaps(local, &af) {
            return Err((
                format!(
                    "{} is your whole account, opened as a folder — it can’t sync into itself.",
                    tilde(&af)
                ),
                false,
            ));
        }
        let Some(c) = self.creds.get().await else {
            return Err((
                "Not signed in — sign in to your account first.".into(),
                false,
            ));
        };
        run_pair(pair, local, &c).await.map_err(|e| {
            let offline = looks_offline(&e);
            (e, offline)
        })
    }

    fn notify_conflicts(&self, folder: &str, n: usize) {
        let app = self.app.clone();
        let title = format!(
            "{folder}: {n} file{} changed in two places",
            if n == 1 { "" } else { "s" }
        );
        std::thread::spawn(move || {
            let out = std::process::Command::new("notify-send")
                .args([
                    "-a",
                    "ewe Sync",
                    "-i",
                    "folder-sync",
                    "-A",
                    "open=Choose which to keep",
                    &title,
                    "Both copies are kept. Open ewe Sync to choose which one stays.",
                ])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output();
            if let Ok(o) = out {
                if String::from_utf8_lossy(&o.stdout).trim() == "open" {
                    let _ = app.emit("deep-link", json!({ "pane": "folders" }));
                    crate::tray::show_main(&app);
                }
            }
        });
    }

    pub async fn run_all(self: &Arc<Self>) -> Value {
        let keys: Vec<String> = self.pairs.lock().await.iter().map(Pair::key).collect();
        let mut failed: Vec<String> = Vec::new();
        for k in keys {
            let paused = self
                .state
                .lock()
                .await
                .get(&k)
                .map(|s| s.paused)
                .unwrap_or(false);
            if paused {
                continue;
            }
            if let Err(e) = self.run(&k).await {
                failed.push(format!("{k}: {e}"));
            }
        }
        json!({ "ok": failed.is_empty(), "failed": failed })
    }

    /// Check a proposed list against this machine (overlaps, the mount, the
    /// Nextcloud app). `overrides` replaces local paths for the check.
    async fn check(
        &self,
        pairs: &[Pair],
        overrides: &HashMap<String, String>,
    ) -> Result<(), String> {
        let st = self.state.lock().await.clone();
        let locals: Vec<(String, PathBuf)> = pairs
            .iter()
            .map(|p| {
                let k = p.key();
                let l = overrides
                    .get(&k)
                    .cloned()
                    .or_else(|| st.get(&k).and_then(|s| s.local_override.clone()))
                    .unwrap_or_else(|| p.local.clone());
                (k, expand_local(&l))
            })
            .collect();
        check_layout(&locals, &home(), &self.blocked().await)
    }
}

fn looks_offline(e: &str) -> bool {
    let l = e.to_lowercase();
    [
        "host not found",
        "connection refused",
        "network is unreachable",
        "network unreachable",
        "timed out",
        "temporary failure in name resolution",
        "could not resolve",
        "no route to host",
        "the server can’t be reached",
    ]
    .iter()
    .any(|w| l.contains(w))
}

/// A private HOME for nextcloudcmd holding only `.netrc` (0600), so the app
/// password never rides on argv. Rewritten on every run (the password may
/// have been rotated by a fresh sign-in).
fn private_home(c: &Creds) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let dir = state_dir().join("ewe-sync-home");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let netrc = dir.join(".netrc");
    let body = format!("machine {} login {} password {}\n", c.host, c.user, c.token);
    std::fs::write(&netrc, body).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&netrc, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    Ok(dir)
}

fn exclude_file(pair: &Pair) -> Result<Option<PathBuf>, String> {
    if pair.exclude.is_empty() {
        return Ok(None);
    }
    let dir = state_dir().join("ewe-sync-home");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut h: u64 = 1469598103934665603;
    for b in pair.key().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    let f = dir.join(format!("exclude-{h:016x}.lst"));
    let mut body = String::new();
    for e in &pair.exclude {
        body.push_str(e.trim());
        body.push('\n');
    }
    std::fs::write(&f, body).map_err(|e| e.to_string())?;
    Ok(Some(f))
}

async fn run_pair(pair: &Pair, local: &Path, c: &Creds) -> Result<(), String> {
    std::fs::create_dir_all(local).map_err(|e| format!("local folder: {e}"))?;
    match pair.mode.as_str() {
        "two-way" => run_nextcloudcmd(pair, local, c).await,
        "upload" | "download" => run_rclone(pair, local, c).await,
        m => Err(format!("unknown mode {m}")),
    }
}

/// MKCOL every missing segment of an account path (the engine syncs INTO a
/// folder; it does not create the folder it was pointed at).
async fn ensure_remote(c: &Creds, remote_path: &str) -> Result<(), String> {
    let r = norm_remote(remote_path);
    if r.is_empty() {
        return Ok(());
    }
    match remote::size_of(c, &r).await {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(e) => return Err(e.to_string()),
    }
    let mut acc = String::new();
    for seg in r.split('/') {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(seg);
        let method = reqwest::Method::from_bytes(b"MKCOL").map_err(|e| e.to_string())?;
        let resp = remote::client()
            .request(method, c.url(&acc))
            .basic_auth(&c.user, Some(&c.token))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        // 201 created, 405 exists
        if !(resp.status().is_success() || resp.status().as_u16() == 405) {
            return Err(format!(
                "couldn’t create {acc} in your account (http {})",
                resp.status().as_u16()
            ));
        }
    }
    Ok(())
}

async fn run_nextcloudcmd(pair: &Pair, local: &Path, c: &Creds) -> Result<(), String> {
    if !which(nccmd::BIN) {
        return Err(format!(
            "{} is not installed — it comes with the nextcloud-client package",
            nccmd::BIN
        ));
    }
    ensure_remote(c, &pair.remote).await?;
    let phome = private_home(c)?;
    let excl = exclude_file(pair)?;
    let remote = norm_remote(&pair.remote);
    let mut cmd = Command::new(nccmd::BIN);
    cmd.arg(nccmd::NON_INTERACTIVE)
        .arg(nccmd::SILENT)
        .arg(nccmd::NETRC);
    if pair.hidden {
        cmd.arg(nccmd::HIDDEN);
    }
    if !remote.is_empty() {
        cmd.arg(nccmd::PATH).arg(format!("/{remote}"));
    }
    if let Some(f) = &excl {
        cmd.arg(nccmd::EXCLUDE).arg(f);
    }
    cmd.arg(local)
        .arg(&c.server)
        .env("HOME", &phome)
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let out = cmd
        .output()
        .await
        .map_err(|e| format!("{}: {e}", nccmd::BIN))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(tail(&out.stdout, &out.stderr))
    }
}

/// rclone over WebDAV, configured through environment variables for this
/// one process (no config file, the obscured password never touches argv).
async fn run_rclone(pair: &Pair, local: &Path, c: &Creds) -> Result<(), String> {
    if !which(rc::BIN) {
        return Err("rclone is not installed (needed for one-way folders)".into());
    }
    let obscured = {
        let mut ch = Command::new(rc::BIN)
            .args(["obscure", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("rclone obscure: {e}"))?;
        if let Some(mut si) = ch.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let _ = si.write_all(c.token.as_bytes()).await;
            drop(si);
        }
        let o = ch
            .wait_with_output()
            .await
            .map_err(|e| format!("rclone obscure: {e}"))?;
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    if obscured.is_empty() {
        return Err("rclone obscure gave nothing".into());
    }
    let remote = format!("{}:{}", rc::REMOTE, norm_remote(&pair.remote));
    let mut cmd = Command::new(rc::BIN);
    cmd.arg("copy").arg("--create-empty-src-dirs");
    for e in &pair.exclude {
        cmd.arg("--exclude").arg(e.trim());
    }
    if pair.mode == "upload" {
        cmd.arg(local).arg(&remote);
    } else {
        cmd.arg(&remote).arg(local);
    }
    let pfx = format!("RCLONE_CONFIG_{}_", rc::REMOTE);
    cmd.env(format!("{pfx}TYPE"), "webdav")
        .env(format!("{pfx}URL"), c.files_root())
        .env(format!("{pfx}VENDOR"), "nextcloud")
        .env(format!("{pfx}USER"), &c.user)
        .env(format!("{pfx}PASS"), obscured)
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let out = cmd.output().await.map_err(|e| format!("rclone: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(tail(&out.stdout, &out.stderr))
    }
}

fn tail(stdout: &[u8], stderr: &[u8]) -> String {
    let mut s = String::from_utf8_lossy(stdout).to_string();
    s.push_str(&String::from_utf8_lossy(stderr));
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    let n = lines.len();
    let keep = lines[n.saturating_sub(6)..].join("\n");
    if keep.is_empty() {
        "the sync engine failed without a message".into()
    } else {
        keep
    }
}

// ── commands ────────────────────────────────────────────────────────────────

pub fn runner(app: &AppHandle) -> Arc<Runner> {
    app.state::<Arc<Runner>>().inner().clone()
}

#[tauri::command]
pub async fn folders_list(app: AppHandle) -> Result<Value, String> {
    Ok(runner(&app).snapshot().await)
}

/// Save the whole list through ewe-conf (the one file carries it to every
/// machine), rebuild the triggers, and start the folders that are new.
#[tauri::command]
pub async fn folders_set(app: AppHandle, pairs: Vec<Pair>) -> Result<Value, String> {
    for p in &pairs {
        p.validate()?;
    }
    let mut seen: Vec<String> = Vec::new();
    for p in &pairs {
        let k = p.key();
        if seen.contains(&k) {
            return Err(format!("{} is listed twice", p.remote));
        }
        seen.push(k);
    }
    let r = runner(&app);
    r.check(&pairs, &HashMap::new()).await?;
    let before: Vec<String> = r.pairs.lock().await.iter().map(Pair::key).collect();
    write_pairs(&pairs).await?;
    r.reload(false).await;
    for p in &pairs {
        if !before.contains(&p.key()) {
            r.request(&p.key(), false).await;
        }
    }
    crate::tools::shell_poke("refresh").await;
    Ok(r.snapshot().await)
}

async fn write_pairs(pairs: &[Pair]) -> Result<(), String> {
    let v = serde_json::to_string(pairs).map_err(|e| e.to_string())?;
    let o = run_tool(
        "ewe-conf",
        &["set", "--no-hooks", "sync.folders", &v],
        None,
        &[],
    )
    .await?;
    if o.code != 0 {
        return Err(format!(
            "ewe-conf set sync.folders failed: {}",
            o.stderr.trim()
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn folders_run(app: AppHandle, key: String) -> Result<Value, String> {
    runner(&app).run(&key).await
}

#[tauri::command]
pub async fn folders_run_all(app: AppHandle) -> Result<Value, String> {
    Ok(runner(&app).run_all().await)
}

/// This machine's local path for a pair (the definition's path is the
/// default shared by every machine).
#[tauri::command]
pub async fn folders_local_override(
    app: AppHandle,
    key: String,
    local: Option<String>,
) -> Result<Value, String> {
    if let Some(l) = &local {
        if l.contains("..") || l.trim().is_empty() {
            return Err("bad path".into());
        }
    }
    let r = runner(&app);
    let pairs = r.pairs.lock().await.clone();
    let mut ov = HashMap::new();
    if let Some(l) = &local {
        ov.insert(key.clone(), l.trim().to_string());
    }
    r.check(&pairs, &ov).await?;
    {
        let mut st = r.state.lock().await;
        st.entry(key).or_default().local_override = local.map(|s| s.trim().to_string());
    }
    r.reload(false).await;
    Ok(r.snapshot().await)
}

/// Pause or resume one folder on THIS machine — the account keeps it, other
/// machines keep syncing it.
#[tauri::command]
pub async fn folders_pause(app: AppHandle, key: String, paused: bool) -> Result<Value, String> {
    let r = runner(&app);
    {
        let mut st = r.state.lock().await;
        st.entry(key.clone()).or_default().paused = paused;
    }
    r.save().await;
    r.emit().await;
    if !paused {
        r.request(&key, false).await;
    }
    Ok(r.snapshot().await)
}

/// Resolve one conflict copy: `mine` renames the copy over the original
/// (the next run uploads it); `theirs` deletes the copy (the server's
/// version, already in place, wins). Only inside the pair's folder.
#[tauri::command]
pub async fn folders_resolve(
    app: AppHandle,
    key: String,
    path: String,
    keep: String,
) -> Result<Value, String> {
    let r = runner(&app);
    let pair = r
        .pairs
        .lock()
        .await
        .iter()
        .find(|p| p.key() == key)
        .cloned()
        .ok_or("no such folder pair")?;
    let root = r.effective_local(&pair).await;
    let copy = inside(&root, &path)?;
    let name = copy
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or("bad path")?;
    let original_name = conflict_original(&name).ok_or("not a conflict copy")?;
    let original = copy.with_file_name(original_name);
    match keep.as_str() {
        "mine" => std::fs::rename(&copy, &original).map_err(|e| format!("rename: {e}"))?,
        "theirs" => std::fs::remove_file(&copy).map_err(|e| format!("remove: {e}"))?,
        _ => return Err("keep must be mine or theirs".into()),
    }
    {
        let mut st = r.state.lock().await;
        let s = st.entry(key.clone()).or_default();
        s.conflicts = if root.is_dir() {
            scan_conflicts(&root)
        } else {
            Vec::new()
        };
    }
    r.save().await;
    r.emit().await;
    Ok(r.snapshot().await)
}

/// Move the Nextcloud app's folders (for this account) to ewe-sync: quit the
/// app, stop it starting at login (its autostart entry is moved aside, kept),
/// and add the same folders here. The files stay where they are; the first
/// run reconciles them (files that match on both sides are not copied).
#[tauri::command]
pub async fn folders_takeover(app: AppHandle) -> Result<Value, String> {
    let r = runner(&app);
    let Some((server, _)) = cloud_meta() else {
        return Err("Sign in first.".into());
    };
    let folders = ncapp::app_folders(&server);
    if folders.is_empty() {
        return Err("The Nextcloud app has no folders for this account.".into());
    }
    let mut pairs = r.pairs.lock().await.clone();
    let mut added: Vec<String> = Vec::new();
    for f in &folders {
        let p = Pair {
            remote: f.remote.clone(),
            local: tilde(Path::new(&f.local)),
            mode: "two-way".into(),
            trigger: "change".into(),
            interval: 10,
            exclude: Vec::new(),
            hidden: f.hidden,
        };
        let dup = pairs.iter().any(|q| {
            q.key() == p.key() || clean(&expand_local(&q.local)) == clean(Path::new(&f.local))
        });
        if !dup {
            added.push(p.key());
            pairs.push(p);
        }
    }
    // the app's folders block themselves until it is gone: check the rest
    let locals: Vec<(String, PathBuf)> = pairs
        .iter()
        .map(|p| (p.key(), expand_local(&p.local)))
        .collect();
    let af = account_folder();
    let mount: Vec<(PathBuf, String)> = if is_mountpoint(&af) {
        vec![(
            af.clone(),
            format!("{} is mounted — unmount it first", tilde(&af)),
        )]
    } else {
        Vec::new()
    };
    check_layout(&locals, &home(), &mount)?;
    ncapp::quit().await?;
    let moved = ncapp::disable_autostart(&state_dir().join("nextcloud-app"))?;
    write_pairs(&pairs).await?;
    r.extra.lock().await.1 = Some(json!({
        "at": now(),
        "autostart_backup": moved,
        "folders": folders.iter().map(|f| f.local.clone()).collect::<Vec<_>>(),
    }));
    r.reload(false).await;
    for k in &added {
        r.request(k, false).await;
    }
    Ok(r.snapshot().await)
}

/// The user folders that are not connected yet — "keep these in your
/// account" — with whether the account already has a folder of that name.
#[tauri::command]
pub async fn folders_suggest(app: AppHandle) -> Result<Value, String> {
    let r = runner(&app);
    let taken = r.locals().await;
    let blocked: Vec<PathBuf> = r.blocked().await.into_iter().map(|(p, _)| p).collect();
    let mut cands: Vec<(String, PathBuf)> = user_dirs()
        .into_iter()
        .filter(|(k, _)| k != "download")
        .collect();
    let projects = home().join("Projects");
    if projects.is_dir() {
        cands.push(("projects".into(), projects));
    }
    let c = r.creds.get().await;
    let mut out: Vec<Value> = Vec::new();
    for (kind, p) in cands {
        if !p.is_dir()
            || taken.iter().any(|t| overlaps(t, &p))
            || blocked.iter().any(|b| overlaps(b, &p))
        {
            continue;
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let in_account = match &c {
            Some(c) => remote::size_of(c, &name).await.ok().flatten(),
            None => None,
        };
        // code folders are big in the wrong places
        let exclude = if kind == "projects" {
            json!(["node_modules", "target", ".venv", "__pycache__"])
        } else {
            json!([])
        };
        out.push(json!({
            "kind": kind,
            "name": name,
            "local": tilde(&p),
            "remote": name,
            "in_account": in_account.is_some(),
            "remote_size": in_account,
            "exclude": exclude,
        }));
    }
    Ok(json!({ "items": out }))
}

/// How big a local folder is, counted for at most two seconds.
#[tauri::command]
pub async fn folder_size(path: String) -> Result<Value, String> {
    if path.contains("..") {
        return Err("bad path".into());
    }
    let p = expand_local(&path);
    let (bytes, files, more) =
        tokio::task::spawn_blocking(move || dir_size(&p, Duration::from_secs(2)))
            .await
            .map_err(|e| e.to_string())?;
    Ok(json!({ "bytes": bytes, "files": files, "more": more }))
}

/// A folder in the account, for the picker: its children and sizes, and at
/// the root the account's free space.
#[tauri::command]
pub async fn remote_list(app: AppHandle, path: String) -> Result<Value, String> {
    if path.contains("..") {
        return Err("bad path".into());
    }
    let r = runner(&app);
    let Some(c) = r.creds.get().await else {
        return Err("Not signed in.".into());
    };
    match remote::list(&c, &path).await {
        Ok(v) => Ok(v),
        Err(e) => {
            if e == DavErr::Unauthorized {
                r.creds.forget().await;
            }
            Err(e.to_string())
        }
    }
}

/// Open a connected folder (or the whole account) in the file manager —
/// the ewe browser when it is there. Only folders ewe-sync knows.
#[tauri::command]
pub async fn folder_open(app: AppHandle, path: String) -> Result<(), String> {
    let r = runner(&app);
    let want = clean(&expand_local(&path));
    let mut known: Vec<PathBuf> = vec![clean(&account_folder())];
    known.extend(r.locals().await.iter().map(|p| clean(p)));
    for f in r.app_folders().await.0 {
        known.push(clean(Path::new(&f.local)));
    }
    if !known.iter().any(|k| want.starts_with(k)) {
        return Err("not a connected folder".into());
    }
    let p = want.display().to_string();
    if which("ewe-browser") {
        crate::tools::spawn_reaped("ewe-browser", &["open", &p]);
    } else {
        crate::tools::spawn_reaped("xdg-open", &[&p]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_names() {
        assert_eq!(
            conflict_original("report (conflicted copy 2026-09-02 153000).docx").as_deref(),
            Some("report.docx")
        );
        assert_eq!(
            conflict_original("notes (conflicted copy 2026-09-02 153000)").as_deref(),
            Some("notes")
        );
        assert_eq!(
            conflict_original("report_conflict-20260902-153000.docx").as_deref(),
            Some("report.docx")
        );
        assert_eq!(conflict_original("report.docx"), None);
        assert_eq!(conflict_original("my (conflicted copy).txt"), None);
        assert_eq!(conflict_original("x_conflict-abc.txt"), None);
    }

    #[test]
    fn pair_defaults_and_validation() {
        let p: Pair = serde_json::from_str(r#"{"remote":"/Docs/","local":"~/Docs"}"#).unwrap();
        assert_eq!(p.mode, "two-way");
        assert_eq!(p.trigger, "change");
        assert_eq!(p.interval, 10);
        assert!(!p.hidden);
        assert_eq!(p.key(), "Docs|two-way");
        assert!(p.validate().is_ok());
        // `hidden` stays out of ewe.conf unless it is on
        assert!(!serde_json::to_string(&p).unwrap().contains("hidden"));
        let bad: Pair = serde_json::from_str(r#"{"remote":"../x","local":"~/x"}"#).unwrap();
        assert!(bad.validate().is_err());
        let bad: Pair =
            serde_json::from_str(r#"{"remote":"/","local":"~/x","mode":"mirror"}"#).unwrap();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn remote_normalisation() {
        assert_eq!(norm_remote("/"), "");
        assert_eq!(norm_remote(" /Photos/2026/ "), "Photos/2026");
    }

    #[test]
    fn inside_rejects_escapes() {
        let d = std::env::temp_dir().join(format!("ewe-sync-test-{}", std::process::id()));
        std::fs::create_dir_all(d.join("sub")).unwrap();
        assert!(inside(&d, "sub/a (conflicted copy 2026-09-02 153000).txt").is_ok());
        assert!(inside(&d, "../a.txt").is_err());
        assert!(inside(&d, "/etc/passwd").is_err());
        assert!(inside(&d, "").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn layout_rules() {
        let h = Path::new("/home/u");
        let l = |v: &[&str]| -> Vec<(String, PathBuf)> {
            v.iter()
                .map(|s| (s.to_string(), PathBuf::from(s)))
                .collect()
        };
        assert!(check_layout(&l(&["/home/u/Documents", "/home/u/Pictures"]), h, &[]).is_ok());
        // the whole home folder, or above it
        assert!(check_layout(&l(&["/home/u"]), h, &[]).is_err());
        assert!(check_layout(&l(&["/home/u/"]), h, &[]).is_err());
        assert!(check_layout(&l(&["/home"]), h, &[]).is_err());
        assert!(check_layout(&l(&["/"]), h, &[]).is_err());
        // one inside another, either order, and the same folder twice
        assert!(
            check_layout(&l(&["/home/u/Documents", "/home/u/Documents/Work"]), h, &[]).is_err()
        );
        assert!(
            check_layout(&l(&["/home/u/Documents/Work", "/home/u/Documents"]), h, &[]).is_err()
        );
        assert!(check_layout(&l(&["/home/u/Docs", "/home/u/Docs/"]), h, &[]).is_err());
        // a name that merely starts the same is a different folder
        assert!(check_layout(&l(&["/home/u/Doc", "/home/u/Documents"]), h, &[]).is_ok());
        // somebody else's folder
        let blocked = vec![(PathBuf::from("/home/u/Nextcloud"), "mounted".to_string())];
        let e = check_layout(&l(&["/home/u/Nextcloud/Work"]), h, &blocked).unwrap_err();
        assert_eq!(e, "mounted");
        assert!(check_layout(&l(&["/home/u/Documents"]), h, &blocked).is_ok());
        assert!(check_layout(&l(&["relative/path"]), h, &[]).is_err());
    }

    #[test]
    fn engine_noise_is_ignored() {
        assert!(engine_noise(Path::new("/x/._sync_abc.db")));
        assert!(engine_noise(Path::new("/x/.sync_5f99ce78ffc9.db-wal")));
        assert!(engine_noise(Path::new("/x/.~lock.report.odt#")));
        assert!(!engine_noise(Path::new("/x/report.odt")));
    }

    #[test]
    fn bookmarks_add_and_drop_only_ours() {
        let d = std::env::temp_dir().join(format!("ewe-sync-bm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("gtk-3.0")).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &d);
        std::fs::write(d.join("gtk-3.0/bookmarks"), "file:///home/u/Mine Mine\n").unwrap();
        let work = PathBuf::from("/home/u/Work Notes");
        let owned = sync_bookmarks(std::slice::from_ref(&work), &[]);
        assert_eq!(owned, vec!["file:///home/u/Work%20Notes".to_string()]);
        let text = std::fs::read_to_string(d.join("gtk-3.0/bookmarks")).unwrap();
        assert!(text.contains("file:///home/u/Mine Mine"));
        assert!(text.contains("file:///home/u/Work%20Notes Work Notes"));
        // again: nothing doubles
        let owned = sync_bookmarks(&[work], &owned);
        assert_eq!(
            std::fs::read_to_string(d.join("gtk-3.0/bookmarks")).unwrap(),
            text
        );
        // the folder is gone: ours goes, the user's stays
        let owned = sync_bookmarks(&[], &owned);
        assert!(owned.is_empty());
        let text = std::fs::read_to_string(d.join("gtk-3.0/bookmarks")).unwrap();
        assert_eq!(text, "file:///home/u/Mine Mine\n");
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn offline_words() {
        assert!(looks_offline("Host not found"));
        assert!(looks_offline("Error transferring … Connection refused"));
        assert!(!looks_offline(
            "Server replied with HTTP 507 Insufficient Storage"
        ));
    }
}
