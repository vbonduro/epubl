pub mod config;
pub mod copy;
pub mod epub;
pub mod log;
pub mod usb;
pub mod updater;

/// Returns true if the app was launched with `--setup`, forcing the config wizard open.
#[tauri::command]
fn get_force_setup() -> bool {
    std::env::args().any(|a| a == "--setup")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            usb::get_connected_ereaders,
            usb::eject,
            config::get_config,
            config::set_config,
            epub::list_epubs,
            epub::diff_epubs,
            copy::copy_epubs,
            get_force_setup,
        ])
        .setup(|app| {
            // Initialise the log file in the app config directory.
            use tauri::Manager;
            if let Ok(config_dir) = app.handle().path().app_config_dir() {
                let _ = std::fs::create_dir_all(&config_dir);
                log::init(&config_dir);
                crate::log!("epubl started");
            }

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                usb::watch_ereader(handle).await;
            });
            let handle2 = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                updater::check_for_update(handle2).await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
