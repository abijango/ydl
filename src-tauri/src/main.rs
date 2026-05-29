// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod sink;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::classify_url,
            commands::deps_status,
            commands::install_deps,
            commands::update_dep,
            commands::start_download,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ydl-gui");
}
