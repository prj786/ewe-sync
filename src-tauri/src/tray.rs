//! The window, on demand. No tray icon since 0.13: the ewe bar renders the
//! account's state itself (bar.rs feeds it), and a cloud in the system tray
//! only ever read as "the Nextcloud desktop client is running".
//!
//! The window is not declared in tauri.conf.json: it is built here the first
//! time something asks for it (the bar, a second launch, a conflict
//! notification) and DESTROYED when it is closed, so a WebKit page exists
//! only while someone can see it. `ewe-sync --hidden` therefore starts with
//! no page at all, and the Rust side (the folder runner, bar.rs) is all that
//! stays resident — lib.rs keeps the process alive when the last window goes.

use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent};

pub const MAIN: &str = "main";

/// Show the window, creating it when there is none. The page reads its pane
/// from `take_deep_link` once mounted, so callers store a link first
/// (overview::deliver) and then call this.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    // the same window tauri.conf.json declared until 0.16.2. No minimum size:
    // Hyprland gives a tiled window the size of its tile whatever the window
    // asks for, and a window that insists on more is drawn squeezed into the
    // tile (cropped on a scale-1 screen) while the pointer is not, so every
    // hover and click lands beside what is drawn. The page takes any size.
    let built = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
        .title("ewe Sync")
        .inner_size(960.0, 640.0)
        .center()
        .decorations(false)
        .build();
    // every build is a new web view, so every build is hooked
    match built {
        Ok(w) => crate::pointer::forget_hover_on_leave(&w),
        Err(e) => eprintln!("ewe-sync: could not open the window: {e}"),
    }
}

/// Close = destroy, and the WebKit web process with it.
///
/// Destroying the window alone is not enough: WebKit keeps a page-less web
/// process in its process cache for up to 30 minutes (~90 MB here, measured
/// 2026-11-19), and a reopened window gets a fresh process anyway, so the
/// cached one is pure cost. `terminate_web_process` while the view still
/// exists ends it with the page — this runs synchronously on the main thread
/// from inside the close request, before the window is torn down. The
/// network process stays (~13 MB: Tauri keeps the web context for the app's
/// lifetime on Linux, tauri#14626). Once the window is gone, the heap glibc
/// kept from the page's lifetime (~20 MB) is handed back to the kernel.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN {
        return;
    }
    match event {
        WindowEvent::CloseRequested { .. } => {
            if let Some(wv) = window.webviews().into_iter().next() {
                let _ = wv.with_webview(|pw| {
                    use webkit2gtk::WebViewExt;
                    pw.inner().terminate_web_process();
                });
            }
        }
        WindowEvent::Destroyed => {
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(3));
                extern "C" {
                    fn malloc_trim(pad: usize) -> i32;
                }
                // glibc: give freed arenas back; Arch only, so glibc always
                unsafe { malloc_trim(0) };
            });
        }
        _ => {}
    }
}
