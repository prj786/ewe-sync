//! The official Nextcloud desktop app, seen from outside. People install it
//! (it is how a Nextcloud account usually syncs), and two engines on one
//! folder corrupt each other — so ewe-sync reads the app's own folder list
//! (`~/.config/Nextcloud/nextcloud.cfg`), shows those folders as connected,
//! refuses a pair on top of one, and, when the user asks, takes them over:
//! the app quits, stops starting at login (its autostart entry is moved
//! aside, not deleted), and the same folders sync through ewe-sync. The app
//! is never started, configured or written to here.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use crate::tools::home;

#[derive(Debug, Clone, PartialEq)]
pub struct AppFolder {
    pub local: String,
    pub remote: String,
    pub paused: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppAccount {
    pub url: String,
    pub folders: Vec<AppFolder>,
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}

pub fn cfg_path() -> PathBuf {
    config_home().join("Nextcloud").join("nextcloud.cfg")
}

/// `[Accounts]` of the app's INI: `0\url=…`, `0\Folders\1\localPath=…`,
/// `0\Folders\1\targetPath=/`, `…\paused=`, `…\ignoreHiddenFiles=`. Virtual-
/// file folders live under `FoldersWithPlaceholders`, older ones under
/// `Multifolders`; all three are folders.
pub fn parse_cfg(text: &str) -> Vec<AppAccount> {
    use std::collections::BTreeMap;
    let mut accounts: BTreeMap<String, AppAccount> = BTreeMap::new();
    let mut folders: BTreeMap<(String, String, String), AppFolder> = BTreeMap::new();
    let mut in_accounts = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_accounts = line == "[Accounts]";
            continue;
        }
        if !in_accounts {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let parts: Vec<&str> = key.split('\\').collect();
        match parts.as_slice() {
            [acc, "url"] => {
                accounts.entry(acc.to_string()).or_default().url =
                    val.trim().trim_end_matches('/').to_string();
            }
            [acc, kind, n, field]
                if matches!(
                    *kind,
                    "Folders" | "FoldersWithPlaceholders" | "Multifolders"
                ) =>
            {
                let f = folders
                    .entry((acc.to_string(), kind.to_string(), n.to_string()))
                    .or_insert(AppFolder {
                        local: String::new(),
                        remote: String::new(),
                        paused: false,
                        hidden: false,
                    });
                match *field {
                    "localPath" => f.local = val.trim().trim_end_matches('/').to_string(),
                    "targetPath" => f.remote = val.trim().trim_matches('/').to_string(),
                    "paused" => f.paused = val.trim() == "true",
                    // the app's own switch is "ignore hidden files"
                    "ignoreHiddenFiles" => f.hidden = val.trim() == "false",
                    _ => {}
                }
            }
            _ => {}
        }
    }
    for ((acc, _, _), f) in folders {
        if f.local.is_empty() {
            continue;
        }
        accounts.entry(acc).or_default().folders.push(f);
    }
    accounts
        .into_values()
        .filter(|a| !a.folders.is_empty())
        .collect()
}

fn same_server(a: &str, b: &str) -> bool {
    let n = |s: &str| {
        s.trim()
            .trim_end_matches('/')
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_lowercase()
    };
    !a.is_empty() && n(a) == n(b)
}

/// The app's folders for THIS account's server (another account's folders
/// are the app's business, and not ewe-sync's to show or take).
pub fn app_folders(server: &str) -> Vec<AppFolder> {
    let Ok(text) = std::fs::read_to_string(cfg_path()) else {
        return Vec::new();
    };
    parse_cfg(&text)
        .into_iter()
        .filter(|a| same_server(&a.url, server))
        .flat_map(|a| a.folders)
        .collect()
}

/// Is the app running right now (its process is `nextcloud`)?
pub fn running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "nextcloud"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Ask the running app to quit (`nextcloud --quit` signals the instance
/// that is running), then wait for it to go — a sync in flight finishes
/// its file first.
pub async fn quit() -> Result<(), String> {
    if !running() {
        return Ok(());
    }
    let _ = tokio::process::Command::new("nextcloud")
        .arg("--quit")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    for _ in 0..40 {
        if !running() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err("The Nextcloud app is still running. Quit it from its tray icon, then try again.".into())
}

fn autostart_entries() -> Vec<PathBuf> {
    let dir = config_home().join("autostart");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            name.ends_with(".desktop") && name.contains("nextcloud") && is_app_entry(p)
        })
        .collect()
}

/// Does the app start at login?
pub fn autostarts() -> bool {
    !autostart_entries().is_empty()
}

/// An autostart entry that starts the desktop app (not, say, a user's own
/// script that happens to mention it).
fn is_app_entry(p: &Path) -> bool {
    std::fs::read_to_string(p)
        .map(|t| {
            t.lines().any(|l| {
                let l = l.trim();
                l.starts_with("Exec=") && (l.contains("/nextcloud") || l.contains("Exec=nextcloud"))
            })
        })
        .unwrap_or(false)
}

/// Move the app's autostart entries aside (into ewe's state folder, so the
/// user can put them back). Returns where they went.
pub fn disable_autostart(backup_dir: &Path) -> Result<Vec<String>, String> {
    let mut moved = Vec::new();
    for p in autostart_entries() {
        std::fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;
        let dest = backup_dir.join(p.file_name().unwrap_or_default());
        std::fs::rename(&p, &dest)
            .or_else(|_| std::fs::copy(&p, &dest).and_then(|_| std::fs::remove_file(&p)))
            .map_err(|e| format!("autostart entry {}: {e}", p.display()))?;
        moved.push(dest.display().to_string());
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = r#"[General]
clientVersion=35.0.0
[Accounts]
0\Folders\1\ignoreHiddenFiles=false
0\Folders\1\journalPath=.sync_5f99ce78ffc9.db
0\Folders\1\localPath=/home/u/Nextcloud/
0\Folders\1\paused=false
0\Folders\1\targetPath=/
0\FoldersWithPlaceholders\2\localPath=/home/u/Photos
0\FoldersWithPlaceholders\2\targetPath=/Photos
0\FoldersWithPlaceholders\2\paused=true
0\url=https://cloud.example.org/
1\Folders\1\localPath=/home/u/Work
1\Folders\1\targetPath=/
1\url=https://work.example.com
[Nextcloud]
0\Folders\9\localPath=/not/an/account
"#;

    #[test]
    fn parses_folders_per_account() {
        let a = parse_cfg(CFG);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].url, "https://cloud.example.org");
        assert_eq!(a[0].folders.len(), 2);
        let nc = &a[0].folders[0];
        assert_eq!(nc.local, "/home/u/Nextcloud");
        assert_eq!(nc.remote, "");
        assert!(!nc.paused);
        assert!(nc.hidden);
        let photos = a[0].folders.iter().find(|f| f.remote == "Photos").unwrap();
        assert!(photos.paused);
        assert_eq!(a[1].url, "https://work.example.com");
    }

    #[test]
    fn server_match() {
        assert!(same_server(
            "https://cloud.example.org",
            "https://cloud.example.org/"
        ));
        assert!(same_server(
            "https://Cloud.Example.org",
            "cloud.example.org"
        ));
        assert!(!same_server(
            "https://cloud.example.org",
            "https://work.example.com"
        ));
        assert!(!same_server("", "https://x"));
    }
}
