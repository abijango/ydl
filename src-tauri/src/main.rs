// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod sink;

use commands::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::resolve_output_dir,
            commands::classify_url,
            commands::deps_status,
            commands::install_deps,
            commands::update_dep,
            commands::open_download_path,
            commands::start_download,
            commands::cancel_download,
            commands::clear_busy,
            commands::reveal_path,
            commands::app_version,
            commands::get_history,
            commands::add_history,
            commands::remove_history,
            commands::clear_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ydl-gui");
}
