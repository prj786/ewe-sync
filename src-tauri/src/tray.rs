//! The tray icon: one StatusNotifierItem the ewe bar renders, with a menu the
//! shell themes. Left-click opens the window; the icon shows the account's
//! state (idle · syncing · conflict · offline · signed-out).

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub const TRAY_ID: &str = "ewe-sync";

fn icon_for(state: &str) -> Image<'static> {
    let bytes: &'static [u8] = match state {
        "syncing" => include_bytes!("../icons/tray-syncing.png"),
        "conflict" => include_bytes!("../icons/tray-conflict.png"),
        "offline" => include_bytes!("../icons/tray-offline.png"),
        "signed-out" => include_bytes!("../icons/tray-signed-out.png"),
        _ => include_bytes!("../icons/tray-idle.png"),
    };
    Image::from_bytes(bytes).expect("tray icons are valid PNGs")
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let sync_now = MenuItem::with_id(app, "sync-now", "Sync now", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause auto-sync", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open ewe-sync", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&sync_now, &pause, &open, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon_for("signed-out"))
        .tooltip("ewe-sync")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "sync-now" => {
                let _ = app.emit("tray-action", "sync-now");
            }
            "pause" => {
                let _ = app.emit("tray-action", "pause");
            }
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// The frontend owns the state machine; it tells the tray what to show.
#[tauri::command]
pub fn tray_state(app: AppHandle, state: String, tooltip: Option<String>) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Err("no tray".into());
    };
    tray.set_icon(Some(icon_for(&state)))
        .map_err(|e| e.to_string())?;
    let tip = tooltip.unwrap_or_else(|| "ewe-sync".into());
    tray.set_tooltip(Some(tip)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Rename the pause item as auto-sync flips, so the menu reads right.
#[tauri::command]
pub fn tray_pause_label(app: AppHandle, paused: bool) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Err("no tray".into());
    };
    let label = if paused {
        "Resume auto-sync"
    } else {
        "Pause auto-sync"
    };
    let sync_now = MenuItem::with_id(&app, "sync-now", "Sync now", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let pause =
        MenuItem::with_id(&app, "pause", label, true, None::<&str>).map_err(|e| e.to_string())?;
    let open = MenuItem::with_id(&app, "open", "Open ewe-sync", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit =
        MenuItem::with_id(&app, "quit", "Quit", true, None::<&str>).map_err(|e| e.to_string())?;
    let menu =
        Menu::with_items(&app, &[&sync_now, &pause, &open, &quit]).map_err(|e| e.to_string())?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())
}
