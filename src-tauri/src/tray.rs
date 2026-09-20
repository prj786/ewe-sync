//! No tray icon since 0.13: the ewe bar renders the account's state itself
//! (syncing · conflict · offline — click opens this app), fed by `poke_shell`
//! below, and a cloud in the system tray only ever read as "the Nextcloud
//! desktop client is running". Sync now / Pause live in the window. The
//! `tray_state` / `tray_pause_label` commands stay so the frontend's state
//! machine is unchanged; the first one now only talks to the shell.

use tauri::{AppHandle, Manager};

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The frontend owns the state machine; it tells the tray what to show.
#[tauri::command]
pub fn tray_state(_app: AppHandle, state: String, tooltip: Option<String>) -> Result<(), String> {
    let tip = tooltip.unwrap_or_else(|| "ewe-sync".into());
    poke_shell(&state, &tip);
    Ok(())
}

/// Tell the ewe bar what we are doing, so "is my stuff safe" is answerable
/// without opening this app. Same out-of-process contract ewe-conf and
/// ewe-settings use: poke a named IPC target, never touch the shell's state
/// directly. Best-effort by design — outside the ewe desktop `qs` is simply
/// not there, and the tray above is still the whole story.
fn poke_shell(state: &str, detail: &str) {
    let child = std::process::Command::new("qs")
        .args(["ipc", "call", "sync", "state", state, detail])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    // Reap it. A spawned child nobody waits for stays a zombie for the life
    // of this process: 39 `[qs] <defunct>` under a 20-minute-old ewe-sync
    // on 2026-09-20, one per state poke, forever. The wait happens off the
    // command thread so a wedged shell cannot stall the tray.
    if let Ok(mut child) = child {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

/// Kept for the frontend; there is no menu to relabel any more.
#[tauri::command]
pub fn tray_pause_label(_app: AppHandle, _paused: bool) -> Result<(), String> {
    Ok(())
}
