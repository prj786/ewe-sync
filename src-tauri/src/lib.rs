mod accounts;
mod cloud;
mod folders;
mod tools;
mod tray;

use tauri::{Manager, WindowEvent};

/// `ewe-sync --hidden` (the autostart unit): tray only, no window until the
/// user asks. `ewe-sync --check`: print the self-check JSON and exit.
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
    let hidden = args.iter().any(|a| a == "--hidden");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // a second launch (the launcher, the shell) brings the window back
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            tray::build(app.handle())?;
            // the folder runner: its triggers live for the app's lifetime;
            // "login" pairs run once, shortly after start
            let runner = folders::Runner::new(app.handle().clone());
            app.manage(runner.clone());
            tauri::async_runtime::spawn(async move { runner.reload(true).await });
            if !hidden {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // hide, don't quit: the tray is the app; Quit lives in its menu
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            cloud::de_prefs,
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
