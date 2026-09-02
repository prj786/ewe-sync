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
        return Err(format!("{name} is not installed — is ewe deployed on this machine?"));
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

/// `ewe-conf get <key>` → the JSON value, or Null when the key is absent
/// (exit 1 with a `{"error":"no-key"}` line on stderr).
pub async fn conf_get(key: &str) -> Result<Value, String> {
    let o = run_tool("ewe-conf", &["get", key], None, &[]).await?;
    if o.code != 0 {
        return Ok(Value::Null);
    }
    Ok(o.json)
}

/// Keys the frontend may set: the sync switch and the folder definitions.
pub fn conf_key_allowed(key: &str) -> bool {
    matches!(key, "sync.enabled" | "sync.folders")
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
