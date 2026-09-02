//! Folder sync (RFC-006 F2): pairs of a local folder and a folder in the
//! account. Two-way pairs run the Nextcloud sync engine headless
//! (`nextcloudcmd`); one-way pairs copy with rclone over WebDAV. Runs are
//! serialised, triggered on change (inotify), on an interval, or once at
//! login; conflict files the engine leaves behind are listed and resolved
//! here. Everything is the user's own files — no privileges.

use std::collections::HashMap;
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

use crate::tools::{conf_get, home, run_tool};

/// `nextcloudcmd`'s interface, in ONE place. Written against the documented
/// command line client (docs.nextcloud.com → "Nextcloud command line client":
/// `nextcloudcmd [OPTION] <source_dir> <server_url>`); no binary was
/// available on the development host to verify against — if the real one
/// disagrees, correct it here and nowhere else.
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
}

/// rclone for the one-way modes (`nextcloudcmd` has none): `copy` adds and
/// updates, never deletes — the honest one-way.
mod rc {
    pub const BIN: &str = "rclone";
    pub const REMOTE: &str = "EWESYNC";
}

const DEBOUNCE: Duration = Duration::from_secs(5);
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
}

/// `/Documents/` → `Documents`; `/` → `` (the files root).
pub fn norm_remote(r: &str) -> String {
    r.trim().trim_matches('/').to_string()
}

fn expand_local(p: &str) -> PathBuf {
    let p = p.trim();
    if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
    } else if p == "~" {
        home()
    } else {
        PathBuf::from(p)
    }
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
    /// live only — never persisted
    #[serde(skip)]
    pub running: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct StateFile {
    #[serde(default)]
    pairs: HashMap<String, PairState>,
}

fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("ewe")
}

fn state_path() -> PathBuf {
    state_dir().join("ewe-sync.json")
}

fn load_state() -> HashMap<String, PairState> {
    std::fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str::<StateFile>(&s).ok())
        .map(|f| f.pairs)
        .unwrap_or_default()
}

fn save_state(pairs: &HashMap<String, PairState>) {
    let _ = std::fs::create_dir_all(state_dir());
    let f = StateFile {
        pairs: pairs.clone(),
    };
    if let Ok(s) = serde_json::to_string_pretty(&f) {
        let tmp = state_path().with_extension("json.tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, state_path());
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

// ── the runner ──────────────────────────────────────────────────────────────

pub struct Runner {
    app: AppHandle,
    run_lock: Mutex<()>,
    state: Mutex<HashMap<String, PairState>>,
    pairs: Mutex<Vec<Pair>>,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    watchers: Mutex<Vec<notify::RecommendedWatcher>>,
}

impl Runner {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            run_lock: Mutex::new(()),
            state: Mutex::new(load_state()),
            pairs: Mutex::new(Vec::new()),
            tasks: Mutex::new(Vec::new()),
            watchers: Mutex::new(Vec::new()),
        })
    }

    async fn snapshot(&self) -> Value {
        let pairs = self.pairs.lock().await.clone();
        let state = self.state.lock().await;
        let mut items: Vec<Value> = Vec::new();
        for p in &pairs {
            let st = state.get(&p.key()).cloned().unwrap_or_default();
            let local = st.local_override.clone().unwrap_or_else(|| p.local.clone());
            items.push(json!({
                "key": p.key(),
                "pair": p,
                "local_effective": expand_local(&local).display().to_string(),
                "running": st.running,
                "last_run": st.last_run,
                "last_ok": st.last_ok,
                "last_error": st.last_error,
                "conflicts": st.conflicts,
            }));
        }
        json!({
            "pairs": items,
            "syncing": state.values().any(|s| s.running),
            "conflicts": state.values().map(|s| s.conflicts.len()).sum::<usize>(),
            "engines": { "nextcloudcmd": which(nccmd::BIN), "rclone": which(rc::BIN) },
        })
    }

    async fn emit(&self) {
        let v = self.snapshot().await;
        let _ = self.app.emit("folders-status", v);
    }

    /// Read `[[sync.folders]]`, (re)build every trigger. Called at start and
    /// after the UI saved the list.
    pub async fn reload(self: &Arc<Self>, run_login: bool) {
        let v = conf_get("sync.folders").await.unwrap_or(Value::Null);
        let pairs: Vec<Pair> = serde_json::from_value(v).unwrap_or_default();
        let pairs: Vec<Pair> = pairs.into_iter().filter(|p| p.validate().is_ok()).collect();
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
            let mut st = self.state.lock().await;
            st.retain(|k, _| keys.contains(k));
            save_state(&st);
        }
        for p in pairs {
            let key = p.key();
            let local = self.effective_local(&p).await;
            match p.trigger.as_str() {
                "change" => self.watch(key.clone(), local).await,
                "interval" => {
                    let me = self.clone();
                    let mins = p.interval.max(1);
                    let k = key.clone();
                    self.tasks.lock().await.push(tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(mins * 60)).await;
                            me.run_auto(&k).await;
                        }
                    }));
                }
                _ => {
                    if run_login {
                        let me = self.clone();
                        let k = key.clone();
                        self.tasks.lock().await.push(tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_secs(20)).await;
                            me.run_auto(&k).await;
                        }));
                    }
                }
            }
        }
        self.emit().await;
    }

    async fn effective_local(&self, p: &Pair) -> PathBuf {
        let st = self.state.lock().await;
        let l = st
            .get(&p.key())
            .and_then(|s| s.local_override.clone())
            .unwrap_or_else(|| p.local.clone());
        expand_local(&l)
    }

    /// inotify on the local folder, debounced: a burst of writes becomes one
    /// run once the folder has been quiet for DEBOUNCE.
    async fn watch(self: &Arc<Self>, key: String, local: PathBuf) {
        let _ = std::fs::create_dir_all(&local);
        let (tx, mut rx) = mpsc::channel::<()>(64);
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                // the engine's own journal churns during a run — not a change
                let ours = ev.paths.iter().all(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("._sync_"))
                        .unwrap_or(false)
                });
                if !ours {
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
                me.run_auto(&key).await;
            }
        }));
    }

    /// An automatic run honours the one auto-sync switch; a manual one does not.
    async fn run_auto(&self, key: &str) {
        let enabled = conf_get("sync.enabled")
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if enabled {
            let _ = self.run(key).await;
        }
    }

    pub async fn run(&self, key: &str) -> Result<Value, String> {
        let pair = self
            .pairs
            .lock()
            .await
            .iter()
            .find(|p| p.key() == key)
            .cloned()
            .ok_or("no such folder pair")?;
        let _guard = self.run_lock.lock().await;
        {
            let mut st = self.state.lock().await;
            st.entry(key.to_string()).or_default().running = true;
        }
        self.emit().await;
        let local = self.effective_local(&pair).await;
        let result = run_pair(&pair, &local).await;
        let conflicts = if pair.mode == "two-way" && local.is_dir() {
            scan_conflicts(&local)
        } else {
            Vec::new()
        };
        {
            let mut st = self.state.lock().await;
            let s = st.entry(key.to_string()).or_default();
            s.running = false;
            s.last_run = now();
            s.conflicts = conflicts;
            match &result {
                Ok(()) => {
                    s.last_ok = true;
                    s.last_error.clear();
                }
                Err(e) => {
                    s.last_ok = false;
                    s.last_error = e.clone();
                }
            }
            save_state(&st);
        }
        self.emit().await;
        result.map(|_| json!({ "ok": true }))
    }

    pub async fn run_all(&self) -> Value {
        let keys: Vec<String> = self.pairs.lock().await.iter().map(Pair::key).collect();
        let mut failed: Vec<String> = Vec::new();
        for k in keys {
            if let Err(e) = self.run(&k).await {
                failed.push(format!("{k}: {e}"));
            }
        }
        json!({ "ok": failed.is_empty(), "failed": failed })
    }
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

struct Creds {
    server: String,
    host: String,
    user: String,
    token: String,
}

async fn creds() -> Result<Creds, String> {
    let t = run_tool("ewe-cloud", &["token", "--json"], None, &[])
        .await?
        .json;
    if t.get("ok").and_then(|b| b.as_bool()) != Some(true) {
        return Err("not signed in".into());
    }
    let server = t["server"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    let user = t["user"].as_str().unwrap_or("").to_string();
    let token = t["token"].as_str().unwrap_or("").to_string();
    if server.is_empty() || user.is_empty() || token.is_empty() {
        return Err("not signed in".into());
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
    Ok(Creds {
        server,
        host,
        user,
        token,
    })
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

async fn run_pair(pair: &Pair, local: &Path) -> Result<(), String> {
    std::fs::create_dir_all(local).map_err(|e| format!("local folder: {e}"))?;
    let c = creds().await?;
    match pair.mode.as_str() {
        "two-way" => run_nextcloudcmd(pair, local, &c).await,
        "upload" | "download" => run_rclone(pair, local, &c).await,
        m => Err(format!("unknown mode {m}")),
    }
}

async fn run_nextcloudcmd(pair: &Pair, local: &Path, c: &Creds) -> Result<(), String> {
    if !which(nccmd::BIN) {
        return Err(format!(
            "{} is not installed — it comes with the nextcloud-client package",
            nccmd::BIN
        ));
    }
    let phome = private_home(c)?;
    let excl = exclude_file(pair)?;
    let remote = norm_remote(&pair.remote);
    let mut cmd = Command::new(nccmd::BIN);
    cmd.arg(nccmd::NON_INTERACTIVE)
        .arg(nccmd::SILENT)
        .arg(nccmd::NETRC);
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
        .stderr(Stdio::piped());
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
    let url = format!("{}/remote.php/dav/files/{}/", c.server, percent(&c.user));
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
        .env(format!("{pfx}URL"), url)
        .env(format!("{pfx}VENDOR"), "nextcloud")
        .env(format!("{pfx}USER"), &c.user)
        .env(format!("{pfx}PASS"), obscured)
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = cmd.output().await.map_err(|e| format!("rclone: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(tail(&out.stdout, &out.stderr))
    }
}

fn percent(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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

#[tauri::command]
pub async fn folders_list(app: AppHandle) -> Result<Value, String> {
    let r = app.state::<Arc<Runner>>();
    Ok(r.snapshot().await)
}

/// Save the whole list through ewe-conf (the one file syncs it), then
/// rebuild the triggers.
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
    let v = serde_json::to_string(&pairs).map_err(|e| e.to_string())?;
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
    let r = app.state::<Arc<Runner>>().inner().clone();
    r.reload(false).await;
    crate::tools::shell_poke("refresh").await;
    Ok(r.snapshot().await)
}

#[tauri::command]
pub async fn folders_run(app: AppHandle, key: String) -> Result<Value, String> {
    let r = app.state::<Arc<Runner>>().inner().clone();
    r.run(&key).await
}

#[tauri::command]
pub async fn folders_run_all(app: AppHandle) -> Result<Value, String> {
    let r = app.state::<Arc<Runner>>().inner().clone();
    Ok(r.run_all().await)
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
    let r = app.state::<Arc<Runner>>().inner().clone();
    {
        let mut st = r.state.lock().await;
        st.entry(key).or_default().local_override = local.map(|s| s.trim().to_string());
        save_state(&st);
    }
    r.reload(false).await;
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
    let r = app.state::<Arc<Runner>>().inner().clone();
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
        save_state(&st);
    }
    r.emit().await;
    Ok(r.snapshot().await)
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
        assert_eq!(p.key(), "Docs|two-way");
        assert!(p.validate().is_ok());
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
}
