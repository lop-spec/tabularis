pub mod commands;
pub mod export;
pub mod keychain_utils;
pub mod models;
pub mod pool_manager;
pub mod saved_queries;
pub mod ssh_tunnel;
pub mod drivers {
    pub mod common;
    pub mod mysql;
    pub mod postgres;
    pub mod sqlite;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Install default drivers for sqlx::Any
    sqlx::any::install_default_drivers();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(commands::QueryCancellationState::default())
        .manage(export::ExportCancellationState::default())
        .invoke_handler(tauri::generate_handler![
            commands::test_connection,
            commands::save_connection,
            commands::delete_connection,
            commands::update_connection,
            commands::duplicate_connection,
            commands::get_connections,
            commands::get_tables,
            commands::get_columns,
            commands::get_foreign_keys,
            commands::get_indexes,
            commands::delete_record,
            commands::update_record,
            commands::insert_record,
            commands::execute_query,
            commands::cancel_query,
            commands::set_window_title,
            export::export_query_to_file,
            export::cancel_export,
            saved_queries::get_saved_queries,
            saved_queries::save_query,
            saved_queries::update_saved_query,
            saved_queries::delete_saved_query
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
