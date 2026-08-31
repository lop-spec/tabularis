pub mod askpass;
pub mod audit_outbox;
pub mod backup;
pub mod cli;
pub mod clipboard_import;
pub mod commands;
pub mod config;
pub mod connection_appearance;
#[cfg(test)]
pub mod connection_appearance_tests;
pub mod connection_cache;
#[cfg(test)]
pub mod connection_cache_tests;
pub mod connection_import;
pub mod connection_import_commands;
pub mod connection_window;
#[cfg(test)]
pub mod connection_window_tests;
pub mod credential_cache;
pub mod dump_commands; // Added
#[cfg(test)]
pub mod dump_commands_tests;
pub mod dump_utils;
pub mod explain_import;
#[cfg(test)]
pub mod explain_import_tests;
pub mod export;
pub mod export_crypto;
#[cfg(test)]
pub mod export_import_tests;
#[cfg(test)]
pub mod group_tree_tests;
pub mod health_check;
pub mod json_viewer;
pub mod k8s_tunnel;
pub mod keychain_utils;
pub mod log_commands;
pub mod logger;
pub mod models;
#[cfg(test)]
pub mod models_tests;
pub mod notebooks;
pub mod paths; // Added
pub mod persistence;
pub mod pool_manager;
#[cfg(test)]
pub mod pool_manager_tests;
pub mod preferences;
pub mod profile_crypto;
pub mod query_history;
#[cfg(test)]
pub mod query_history_tests;
pub mod recovery_history;
pub mod recovery_objects;
#[cfg(test)]
pub mod recovery_objects_tests;
pub mod redaction;
pub mod results_window;
pub mod rollback_sql;
pub mod saved_queries;
#[cfg(test)]
pub mod saved_queries_tests;
pub mod session_vars;
pub mod sql_database_statements;
pub mod sqlite_database;
#[cfg(test)]
pub mod sqlite_database_tests;
pub mod ssh_tunnel;
pub mod task_manager;
pub mod theme_commands;
pub mod theme_models;
pub mod updater;
pub mod drivers {
    pub mod common;
    pub mod driver_trait;
    pub mod mysql;
    pub mod postgres;
    pub mod registry;
    pub mod sqlite;
}

use logger::{create_log_buffer, create_persistent_log_buffer, init_logger, SharedLogBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

static DEBUG_MODE: AtomicBool = AtomicBool::new(false);

// Global log buffer for capturing logs
static LOG_BUFFER: std::sync::OnceLock<SharedLogBuffer> = std::sync::OnceLock::new();

pub fn get_log_buffer() -> SharedLogBuffer {
    LOG_BUFFER
        .get()
        .expect("Log buffer not initialized")
        .clone()
}

#[tauri::command]
fn is_debug_mode() -> bool {
    DEBUG_MODE.load(Ordering::Relaxed)
}

#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
    window.open_devtools();
    log::info!("DevTools opened");
}

#[tauri::command]
fn close_devtools(window: tauri::WebviewWindow) {
    window.close_devtools();
    log::info!("DevTools closed");
}

/// Real exit for close-to-hide mode: closing the window only hides it, so the
/// frontend offers Ctrl/Cmd+Q which lands here. Runs the normal RunEvent::Exit
/// path (exit backup, SSH tunnel shutdown) — nothing is skipped.
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    log::info!("Quit requested (Ctrl/Cmd+Q)");
    app.exit(0);
}

/// Suspends or resumes the hidden main window's WebView2 (close-to-hide mode).
///
/// Suspending asks WebView2 to shed most of its memory: the memory-usage
/// target drops to Low (same call wry exposes as `set_memory_usage_level`)
/// and `TrySuspend` pauses script/rendering. WebView2 resumes automatically
/// when the window becomes visible again; the explicit Resume + Normal on
/// focus is a belt-and-braces measure. Failures only cost memory savings,
/// never correctness, so they are logged and ignored.
#[cfg(windows)]
fn set_main_webview_suspended(window: &tauri::WebviewWindow, suspend: bool) {
    let outcome = window.with_webview(move |platform_webview| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, ICoreWebView2_3, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
        };
        use windows_core::Interface;

        let controller = platform_webview.controller();
        let core = match controller.CoreWebView2() {
            Ok(core) => core,
            Err(error) => {
                log::warn!("WebView suspend skipped, no CoreWebView2: {error}");
                return;
            }
        };

        // Tauri's window.hide() hides the Win32 window but leaves the
        // controller's IsVisible true, and TrySuspend rejects visible
        // webviews with ERROR_INVALID_STATE (0x8007139F). Follow the
        // documented sequence: IsVisible=false, then TrySuspend; and on the
        // way back IsVisible=true unconditionally so the window can never
        // come back as a blank surface.
        if let Err(error) = controller.SetIsVisible(!suspend) {
            log::warn!("Controller SetIsVisible failed: {error}");
        }

        // Memory target: 0 = Normal, 1 = Low (matches wry's mapping).
        if let Ok(webview19) = core.cast::<ICoreWebView2_19>() {
            let level = COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(i32::from(suspend));
            if let Err(error) = webview19.SetMemoryUsageTargetLevel(level) {
                log::warn!("SetMemoryUsageTargetLevel failed: {error}");
            }
        }

        if let Ok(webview3) = core.cast::<ICoreWebView2_3>() {
            if suspend {
                let handler = webview2_com::TrySuspendCompletedHandler::create(Box::new(
                    |result, suspended| {
                        log::info!(
                            "WebView TrySuspend completed: result={result:?} suspended={suspended:?}"
                        );
                        Ok(())
                    },
                ));
                if let Err(error) = webview3.TrySuspend(&handler) {
                    log::warn!("WebView TrySuspend failed: {error}");
                }
            } else if let Err(error) = webview3.Resume() {
                // Resuming a non-suspended webview reports an error; harmless.
                log::debug!("WebView Resume no-op/failed: {error}");
            }
        }
    });
    if let Err(error) = outcome {
        log::warn!("with_webview failed while toggling suspend: {error}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // When ssh re-executes this binary as its SSH_ASKPASS helper (see the
    // `askpass` module), serve the prompt and exit without booting the app.
    askpass::maybe_run_askpass_client();

    // Install the rustls `ring` crypto provider as the process-wide default.
    //
    // Both `sqlx` (via the `tls-rustls-ring-native-roots` feature) and the
    // workspace's direct `rustls` usage link against the same `rustls 0.23`
    // crate, but `rustls 0.23` enables both the `ring` and the `aws-lc-rs`
    // crypto providers when their respective feature flags are active in
    // the dependency graph. With two providers linked, rustls refuses to
    // pick one automatically and panics the first time someone tries a TLS
    // handshake ("Could not automatically determine the process-level
    // CryptoProvider"). We pin `ring` here because:
    //   * `sqlx` is configured to use the `ring` provider.
    //   * `ring` is pure-Rust and works on all our target platforms
    //     (macOS, Linux, Windows) without a C toolchain at runtime.
    // This must run before any sqlx pool is built.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // On Linux + Wayland, disable the DMA-BUF renderer in WebKitGTK to prevent
    // "Protocol error dispatching to Wayland display" crashes.
    // This targets the specific protocol causing the error while keeping GPU
    // compositing and rendering intact.
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE").map_or(false, |v| v == "wayland")
        {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    let args = cli::parse();

    // Default to Info level so users can see application logs.
    let log_level = log::LevelFilter::Info;
    DEBUG_MODE.store(args.debug, Ordering::Relaxed);

    // Initialize the durable log buffer before sqlx so startup diagnostics are captured.
    let log_directory = paths::get_app_config_dir().join("logs");
    let log_buffer = create_persistent_log_buffer(1000, log_directory).unwrap_or_else(|error| {
        eprintln!("Persistent logging is unavailable; using memory only: {error}");
        create_log_buffer(1000)
    });
    LOG_BUFFER
        .set(log_buffer.clone())
        .expect("Failed to initialize log buffer");

    // Initialize custom logger that captures logs to buffer and prints to stderr
    init_logger(log_buffer.clone(), log_level);

    // Log startup message
    log::info!("Tabularis application starting...");
    if args.debug {
        log::info!("Debug mode enabled - verbose logging active");
    } else {
        log::info!("Debug mode disabled - standard logging active");
    }

    // Install default drivers for sqlx::Any
    sqlx::any::install_default_drivers();

    tauri::Builder::default()
        // Keep a single application instance and focus the existing window.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            log::info!("Duplicate launch detected — forwarded to existing instance");
            if argv.iter().any(|argument| argument == "--background") {
                return;
            }
            if let Some(win) = tauri::Manager::get_webview_window(app, "main") {
                // This is also the warm-relaunch path for close-to-hide: the
                // launcher just starts a second instance and we bring the
                // window back HERE, in-process. External ShowWindowAsync pokes
                // are queued messages that can land after a later hide and
                // "resurrect" the window — so the launcher must never poke.
                let was_hidden = !win.is_visible().unwrap_or(true);
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
                if was_hidden {
                    log::info!("Warm relaunch: hidden main window shown again");
                    #[cfg(windows)]
                    set_main_webview_suspended(&win, false);
                }
            }
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(commands::QueryCancellationState::default())
        .manage(export::ExportCancellationState::default())
        .manage(dump_commands::DumpCancellationState::default())
        .manage(log_buffer)
        .manage(std::sync::Arc::new(
            credential_cache::CredentialCache::default(),
        ))
        .manage(std::sync::Arc::new(
            connection_cache::ConnectionCache::default(),
        ))
        .manage(connection_import_commands::ImportEnvelopeCache::default())
        .manage(explain_import::PendingExplainFile::default())
        .manage(json_viewer::JsonViewerStore::default())
        .manage(results_window::ResultsWindowStore::default())
        .manage(query_history::QueryHistoryState::default())
        .setup(move |app| {
            if !args.background {
                if let Some(window) = app.get_webview_window("main") {
                    window.show().map_err(std::io::Error::other)?;
                }
            }
            let audit_root =
                audit_outbox::audit_root(&app.handle()).map_err(std::io::Error::other)?;
            let audit_state =
                audit_outbox::AuditState::new(audit_root).map_err(std::io::Error::other)?;
            app.manage(std::sync::Arc::new(audit_state));
            audit_outbox::spawn_sync_worker(app.handle().clone());

            // Allow the SSH tunnel code (which runs without a Tauri context)
            // to bridge askpass prompts to the frontend.
            askpass::set_app_handle(app.handle().clone());

            // Register built-in drivers
            tauri::async_runtime::block_on(async {
                drivers::registry::register_driver(drivers::mysql::MysqlDriver::new()).await;
                drivers::registry::register_driver(drivers::postgres::PostgresDriver::new()).await;
                drivers::registry::register_driver(drivers::sqlite::SqliteDriver::new()).await;
            });

            // Start connection health-check ping loop.
            {
                let config = crate::config::load_config_internal(&app.handle());
                let interval = config
                    .ping_interval
                    .unwrap_or(health_check::DEFAULT_PING_INTERVAL);
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    health_check::start_ping_loop(handle, interval as u64).await;
                });
            }

            // Periodic encrypted backup of the connections, when enabled.
            backup::spawn_scheduler(app.handle().clone());

            // Maximize the window on startup if the user enabled it.
            if crate::config::load_config_internal(&app.handle())
                .start_maximized
                .unwrap_or(false)
            {
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(e) = window.maximize() {
                        log::warn!("Failed to maximize window on startup: {e}");
                    }
                }
            }

            // Open devtools automatically in debug mode
            if args.debug {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                    log::info!("DevTools opened (debug mode active)");
                }
            }

            // Opt-in warm-instance mode: closing the main window hides it
            // instead of exiting, so the next launch is a millisecond-level
            // window activation rather than a cold WebView2 + bundle load.
            // Gated on an env var set by the local dev launchers — official
            // builds and `--explain` runs (which close main deliberately)
            // keep the stock quit-on-close behaviour.
            //
            // While hidden the WebView is suspended (Windows), which releases
            // most of its memory; WebView2 auto-resumes when the window is
            // shown again, and the Focused handler resumes explicitly as a
            // belt-and-braces measure.
            if std::env::var("TABULARIS_CLOSE_TO_HIDE").as_deref() == Ok("1")
                && args.explain.is_none()
            {
                if let Some(window) = app.get_webview_window("main") {
                    let hide_target = window.clone();
                    window.on_window_event(move |event| match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            if let Err(e) = hide_target.hide() {
                                log::warn!("Close-to-hide failed, window stays open: {e}");
                            } else {
                                log::info!(
                                    "Main window hidden (close-to-hide); process stays warm for instant relaunch"
                                );
                                // TrySuspend rejects with ERROR_INVALID_STATE
                                // (0x8007139F) while the hide is still being
                                // processed — give it a beat, then skip if the
                                // user already brought the window back.
                                #[cfg(windows)]
                                {
                                    let suspend_target = hide_target.clone();
                                    tauri::async_runtime::spawn(async move {
                                        tokio::time::sleep(
                                            std::time::Duration::from_millis(800),
                                        )
                                        .await;
                                        if suspend_target.is_visible().unwrap_or(true) {
                                            return;
                                        }
                                        set_main_webview_suspended(&suspend_target, true);
                                    });
                                }
                            }
                        }
                        tauri::WindowEvent::Focused(true) => {
                            #[cfg(windows)]
                            set_main_webview_suspended(&hide_target, false);
                        }
                        _ => {}
                    });
                }
            }

            // If the user launched with `--explain <FILE>`, spawn the Visual
            // Explain window and hide the main app window: the CLI flag is
            // meant to be a dedicated plan viewer, not a full app launch.
            if let Some(path) = args.explain.clone() {
                log::info!("CLI --explain received: {path}");
                if let Err(e) = explain_import::spawn_visual_explain_window(app, Some(path)) {
                    log::error!("Failed to open Visual Explain window: {e}");
                }
                // Close the default main window only AFTER visual-explain is
                // built — closing the last window would terminate the app.
                if let Some(main) = app.get_webview_window("main") {
                    if let Err(e) = main.close() {
                        log::warn!("Failed to close main window: {e}");
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            is_debug_mode,
            open_devtools,
            close_devtools,
            quit_app,
            commands::get_registered_drivers,
            commands::get_driver_manifest,
            commands::get_keybindings,
            commands::save_keybindings,
            commands::test_connection,
            commands::test_saved_connection,
            commands::list_databases,
            commands::save_connection,
            sqlite_database::create_sqlite_file,
            sqlite_database::create_sqlite_database,
            commands::delete_connection,
            commands::update_connection,
            commands::duplicate_connection,
            commands::get_connections,
            commands::get_connection_by_id,
            commands::disconnect_connection,
            commands::register_active_connection,
            commands::get_active_connections,
            commands::get_data_types,
            commands::map_inferred_column_types,
            // SSH Connections
            commands::get_ssh_connections,
            commands::save_ssh_connection,
            commands::update_ssh_connection,
            commands::delete_ssh_connection,
            commands::test_ssh_connection,
            askpass::respond_ssh_askpass,
            // K8s Connections
            commands::get_k8s_connections,
            commands::save_k8s_connection,
            commands::update_k8s_connection,
            commands::delete_k8s_connection,
            commands::test_k8s_connection_cmd,
            commands::get_k8s_contexts_cmd,
            commands::get_k8s_namespaces_cmd,
            commands::get_k8s_resources_cmd,
            commands::get_k8s_resource_ports_cmd,
            commands::validate_k8s_path_cmd,
            // Connection Groups
            commands::get_connection_groups,
            commands::get_connections_with_groups,
            commands::create_connection_group,
            commands::create_group_path,
            commands::update_connection_group,
            commands::move_group_to_parent,
            commands::delete_connection_group,
            commands::move_connection_to_group,
            commands::reorder_groups,
            commands::reorder_connections_in_group,
            commands::export_connections_payload,
            commands::encrypt_export_payload,
            backup::get_connections_backup_status,
            backup::set_connections_backup_password,
            backup::set_connections_backup_target_password,
            backup::run_connections_backup,
            commands::decrypt_export_payload,
            commands::import_connections_payload,
            connection_import_commands::list_connection_import_sources,
            connection_import_commands::preview_connection_import,
            connection_import_commands::apply_connection_import,
            connection_import_commands::preview_tabularis_import,
            connection_import_commands::apply_tabularis_import,
            commands::get_schemas,
            commands::get_available_databases,
            commands::set_selected_databases,
            commands::get_tables,
            commands::get_columns,
            commands::get_foreign_keys,
            commands::get_indexes,
            commands::delete_record,
            commands::update_record,
            commands::insert_record,
            commands::save_blob_to_file,
            commands::fetch_blob_as_data_url,
            commands::load_blob_from_file,
            commands::detect_blob_mime,
            commands::detect_mime_type,
            commands::get_file_stats,
            commands::read_file_as_data_url,
            commands::execute_query,
            commands::execute_query_batch,
            commands::list_session_variables,
            commands::clear_session_variables,
            commands::rollback_transaction_context,
            commands::get_server_now,
            commands::explain_query_plan,
            commands::count_query,
            commands::cancel_query,
            commands::get_views,
            commands::get_view_definition,
            commands::create_view,
            commands::alter_view,
            commands::drop_view,
            commands::get_view_columns,
            commands::get_materialized_views,
            commands::get_materialized_view_columns,
            commands::get_materialized_view_definition,
            commands::refresh_materialized_view,
            commands::set_window_title,
            commands::open_er_diagram_window,
            explain_import::load_explain_from_file,
            explain_import::get_pending_explain_file,
            explain_import::open_visual_explain_window,
            export::export_query_to_file,
            export::cancel_export,
            saved_queries::get_saved_queries,
            saved_queries::save_query,
            saved_queries::update_saved_query,
            saved_queries::delete_saved_query,
            query_history::get_query_history,
            query_history::get_recent_query_history,
            query_history::search_query_history,
            query_history::get_recent_query_history_all,
            query_history::search_query_history_all,
            query_history::add_query_history_entry,
            query_history::delete_query_history_entry,
            query_history::clear_query_history,
            recovery_history::list_recovery_runs,
            commands::generate_recovery_sql,
            // Config
            config::get_schema_preference,
            config::set_schema_preference,
            config::get_last_active_connection,
            config::set_last_active_connection,
            config::get_last_open_connections,
            config::set_last_open_connections,
            config::get_selected_schemas,
            config::set_selected_schemas,
            config::get_config,
            config::save_config,
            config::get_config_json,
            config::save_config_json,
            config::relaunch_app,
            // Clipboard Import
            clipboard_import::execute_clipboard_import,
            commands::get_schema_snapshot,
            // DDL generation
            commands::get_create_table_sql,
            commands::get_add_column_sql,
            commands::get_alter_column_sql,
            commands::get_create_index_sql,
            commands::get_create_foreign_key_sql,
            commands::drop_index_action,
            commands::drop_foreign_key_action,
            // Routines
            commands::get_routines,
            commands::get_routine_parameters,
            commands::get_routine_definition,
            commands::build_routine_call_sql,
            commands::get_routine_create_template,
            commands::get_routine_edit_script,
            commands::drop_routine,
            // Triggers
            commands::get_triggers,
            commands::get_trigger_definition,
            commands::create_trigger,
            commands::drop_trigger,
            // Themes
            theme_commands::get_all_themes,
            theme_commands::get_theme,
            theme_commands::save_custom_theme,
            theme_commands::delete_custom_theme,
            theme_commands::import_theme,
            theme_commands::export_theme,
            // Dump & Import
            dump_commands::dump_database,
            dump_commands::cancel_dump,
            dump_commands::import_database,
            dump_commands::cancel_import,
            dump_commands::cancel_dump,
            // Updater
            updater::check_for_updates,
            updater::download_and_install_update,
            updater::get_installation_source,
            // Logs
            log_commands::get_logs,
            log_commands::log_frontend_event,
            log_commands::clear_logs,
            log_commands::get_log_settings,
            log_commands::set_log_enabled,
            log_commands::set_log_max_size,
            log_commands::export_logs,
            log_commands::test_log,
            // Preferences
            preferences::save_editor_preferences,
            preferences::load_editor_preferences,
            preferences::delete_editor_preferences,
            preferences::list_all_preferences,
            // Notebooks
            notebooks::create_notebook,
            notebooks::save_notebook,
            notebooks::load_notebook,
            notebooks::delete_notebook,
            notebooks::rename_notebook,
            notebooks::list_notebooks,
            // JSON Viewer
            json_viewer::open_json_viewer_window,
            json_viewer::get_json_viewer_session,
            json_viewer::complete_json_viewer_session,
            results_window::open_results_window,
            results_window::close_results_window,
            // Connection Window
            connection_window::open_connection_window,
            // Task Manager
            task_manager::get_system_stats,
            task_manager::get_tabularis_children,
            task_manager::open_task_manager_window,
            // Connection Appearance
            connection_appearance::save_connection_icon,
            connection_appearance::delete_connection_icon,
            commands::set_connection_appearance,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Back up the freshest state before the process ends (no-op
                // unless backups are enabled and due).
                backup::run_exit_backup(app_handle);
                log::info!("Application exiting, stopping all active SSH tunnels...");
                crate::ssh_tunnel::stop_all_tunnels();
            }
        });
}
