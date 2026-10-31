mod accounts;
mod cloud;
mod folders;
mod ncapp;
mod overview;
mod remote;
mod tools;
mod tray;

use std::sync::Mutex;

use tauri::{Manager, WindowEvent};

/// `ewe-sync --hidden` (the autostart unit): tray only, no window until the
/// user asks. `ewe-sync --check`: print the self-check JSON and exit.
/// `ewe-sync --status`: the folder snapshot other apps read, one JSON line.
/// `--pane <id>` / `--add-folder <path>`: open the window there (a second
/// launch forwards them to the running app).
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--check") {
        let v = tauri::async_runtime::block_on(cloud::self_check());
        println!("{v}");
        std::process::exit(if v["ok"].as_bool().unwrap_or(false) {
            0
        } else {
            1
        });
    }
    if args.iter().any(|a| a == "--status") {
        println!("{}", overview::status_line());
        std::process::exit(0);
    }
    let link = overview::parse_deep_link(&args);
    let hidden = args.iter().any(|a| a == "--hidden") && link.is_none();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // a second launch (the launcher, the shell, the file manager's
            // "Keep in your account…") brings the window back, on its pane
            let rest: Vec<String> = argv.into_iter().skip(1).collect();
            if let Some(l) = overview::parse_deep_link(&rest) {
                overview::deliver(app, l);
            }
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(overview::DeepLink(Mutex::new(link)))
        .setup(move |app| {
            // the folder runner: its triggers live for the app's lifetime;
            // every folder catches up shortly after start
            let runner = folders::Runner::new(app.handle().clone());
            app.manage(runner.clone());
            tauri::async_runtime::spawn(async move { runner.start().await });
            if !hidden {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // hide, don't quit: the app lives for the session (the folder
                // runner's triggers); the bar's sync glyph / Settings bring it back
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            cloud::de_prefs,
            cloud::theme_tokens,
            cloud::cloud_status,
            cloud::cloud_login,
            cloud::cloud_logout,
            cloud::cloud_avatar,
            cloud::keyring_reset,
            cloud::session_logout,
            cloud::conf_sync_status,
            cloud::conf_push,
            cloud::conf_restore,
            cloud::conf_get_cmd,
            cloud::conf_set_cmd,
            cloud::shell_poke_cmd,
            cloud::machines_list,
            cloud::machines_write,
            cloud::this_machine,
            folders::folders_list,
            folders::folders_set,
            folders::folders_run,
            folders::folders_run_all,
            folders::folders_local_override,
            folders::folders_resolve,
            folders::folders_pause,
            folders::folders_takeover,
            folders::folders_suggest,
            folders::folder_size,
            folders::remote_list,
            folders::folder_open,
            overview::take_deep_link,
            overview::browser_status,
            overview::safe_status,
            overview::open_safe,
            overview::sync_everything,
            overview::restore_preview,
            overview::restore_everything,
            overview::after_sign_in,
            overview::after_sign_out,
            accounts::mail_status,
            accounts::mail_login,
            accounts::mail_logout,
            accounts::mail_unseen,
            accounts::google_client_info,
            accounts::google_status,
            accounts::google_login,
            accounts::google_logout,
            tray::tray_state,
            tray::tray_pause_label,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ewe-sync");
}
