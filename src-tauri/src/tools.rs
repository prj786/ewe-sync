//! The ewe tools ewe-sync drives — ewe-cloud, ewe-conf, ewe-auth — found the
//! way ewe-settings finds them and run as argv (python3 <tool> …), never
//! through a shell. Every tool prints one JSON object; the wrappers here
//! return it parsed, with the tool's stderr kept for the login-url side
//! channel and for error messages.

use std::path::PathBuf;
use std::process::Stdio;

use serde_json::Value;
use tokio::process::Command;

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}

/// `~/.local/state/ewe` — ewe's per-machine state (never synced).
pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("ewe")
}

/// Write a file the way every ewe tool does: a sibling tmp, then rename.
pub fn write_atomic(path: &std::path::Path, body: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)
}

/// The signed-in account as ewe-cloud recorded it (server, login) — the
/// non-secret half in `~/.config/ewe/cloud.json`; never the app password.
pub fn cloud_meta() -> Option<(String, String)> {
    let text = std::fs::read_to_string(config_home().join("ewe/cloud.json")).ok()?;
    let j: Value = serde_json::from_str(&text).ok()?;
    let server = j["server"].as_str()?.trim_end_matches('/').to_string();
    let user = j["login_name"]
        .as_str()
        .or_else(|| j["user"].as_str())?
        .to_string();
    if server.is_empty() || user.is_empty() {
        return None;
    }
    Some((server, user))
}

/// Is `bin` on PATH?
pub fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

/// `~` for the home folder, so a path written to ewe.conf means the same
/// folder on every machine (`/home/a/Documents` → `~/Documents`).
pub fn tilde(path: &std::path::Path) -> String {
    let h = home();
    match path.strip_prefix(&h) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Fire-and-forget a process, reaped off-thread (a child nobody waits for
/// stays a zombie for the life of this app — 2026-09-20).
pub fn spawn_reaped(bin: &str, args: &[&str]) {
    let child = std::process::Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if let Ok(mut child) = child {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

/// `$EWE_REPO/bin/<name>` (a developer checkout), the deployed payload
/// (`~/.local/share/ewe/bin`), the symlink farm behind `~/.config/quickshell`,
/// then `/usr/bin` (the packaged wrappers).
pub fn ewe_tool(name: &str) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(repo) = std::env::var_os("EWE_REPO") {
        candidates.push(PathBuf::from(repo).join("bin").join(name));
    }
    let h = home();
    candidates.push(h.join(".local/share/ewe/bin").join(name));
    candidates.push(h.join(".config/quickshell/../../bin").join(name));
    candidates.push(PathBuf::from("/usr/bin").join(name));
    candidates.into_iter().find(|p| p.is_file())
}

pub struct ToolOutput {
    pub json: Value,
    pub stderr: String,
    pub code: i32,
}

/// Run a tool with argv, optional stdin bytes, and an env overlay; parse the
/// last JSON line of stdout (the tools print exactly one, but a stray
/// warning line must not break the caller).
pub async fn run_tool(
    name: &str,
    args: &[&str],
    stdin: Option<&[u8]>,
    env: &[(&str, &str)],
) -> Result<ToolOutput, String> {
    let Some(path) = ewe_tool(name) else {
        return Err(format!(
            "{name} is not installed — is ewe deployed on this machine?"
        ));
    };
    let mut cmd = Command::new("python3");
    cmd.arg(&path)
        .args(args)
        .env("LANG", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| format!("{name}: {e}"))?;
    if let (Some(bytes), Some(mut si)) = (stdin, child.stdin.take()) {
        use tokio::io::AsyncWriteExt;
        let _ = si.write_all(bytes).await;
        drop(si);
    }
    let out = child
        .wait_with_output()
        .await
        .map_err(|e| format!("{name}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let json = stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
        .unwrap_or(Value::Null);
    Ok(ToolOutput {
        json,
        stderr,
        code: out.status.code().unwrap_or(-1),
    })
}

/// Run one of the shell-script tools (ewe-files) the same way: argv, found
/// like the others, stdout's last JSON line parsed when there is one.
pub async fn run_script(name: &str, args: &[&str]) -> Result<ToolOutput, String> {
    let Some(path) = ewe_tool(name) else {
        return Err(format!("{name} is not installed"));
    };
    let out = Command::new(&path)
        .args(args)
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("{name}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    Ok(ToolOutput {
        json: stdout
            .lines()
            .rev()
            .find_map(|l| serde_json::from_str::<Value>(l).ok())
            .unwrap_or(Value::Null),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    })
}

/// `ewe-conf get <key>` → the JSON value, or Null when the key is absent
/// (exit 1 with a `{"error":"no-key"}` line on stderr).
pub async fn conf_get(key: &str) -> Result<Value, String> {
    let o = run_tool("ewe-conf", &["get", key], None, &[]).await?;
    if o.code != 0 {
        return Ok(Value::Null);
    }
    Ok(o.json)
}

/// Keys the frontend may set: the sync switch, the browser switch and the
/// folder definitions.
pub fn conf_key_allowed(key: &str) -> bool {
    matches!(key, "sync.enabled" | "sync.browser" | "sync.folders")
}

/// The DE's version as the deployed payload records it (the shell shows the
/// same number) — for the machine registry.
pub fn ewe_version() -> String {
    for p in [
        home().join(".local/share/ewe/VERSION"),
        PathBuf::from("/usr/share/ewe/VERSION"),
    ] {
        if let Ok(s) = std::fs::read_to_string(&p) {
            return s.trim().to_string();
        }
    }
    String::new()
}

/// This computer in the account: the first 16 hex of
/// sha256("ewe:" + /etc/machine-id) — the SAME derivation as `ewe-remote`
/// (RFC-009), so the two registries name a computer alike. Not the hostname:
/// the owner's two machines are both called "emoh" and overwrote each
/// other's `machines/emoh.json`. Not the raw machine id either (systemd: never
/// publish it).
pub fn machine_id() -> String {
    use sha2::{Digest, Sha256};
    for p in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(s) = std::fs::read_to_string(p) {
            let mid = s.trim();
            if !mid.is_empty() {
                return id_from(mid);
            }
        }
    }
    let d = Sha256::digest(format!("ewe-host:{}", hostname()).as_bytes());
    d.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn id_from(mid: &str) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(format!("ewe:{mid}").as_bytes());
    d.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod machine_id_tests {
    // the vector ewe-remote computes in Python:
    // hashlib.sha256(b"ewe:" + mid).hexdigest()[:16]
    #[test]
    fn same_id_as_ewe_remote() {
        assert_eq!(super::id_from("0123456789abcdef0123456789abcdef"), "d019c617096def78");
    }
}

pub fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "ewe".into())
}

/// `qs ipc call cloud <verb>` — keep the shell's account state coherent
/// after ewe-sync acted. Verbs are an allowlist; nothing from the UI is
/// interpolated.
pub async fn shell_poke(verb: &str) {
    if !matches!(verb, "refresh" | "syncSoon" | "applyRestore") {
        return;
    }
    let _ = Command::new("qs")
        .args(["ipc", "call", "cloud", verb])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}
