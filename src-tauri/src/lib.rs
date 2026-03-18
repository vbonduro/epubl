pub mod config;
pub mod epub;
pub mod usb;
pub mod updater;

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
        ])
        .setup(|app| {
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
