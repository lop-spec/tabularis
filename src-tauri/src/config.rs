use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;
use std::sync::RwLock;

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub theme: Option<String>,
    pub language: Option<String>,
    pub result_page_size: Option<u32>,
    pub font_family: Option<String>,
    pub font_size: Option<u32>,
    /// Colorize query result cell values by their data type (number, string,
    /// date, boolean). Default: false — values render in the primary text color.
    pub result_color_by_type: Option<bool>,
    /// Per-type hex color overrides for result cell values. Keys: "number",
    /// "string", "date", "boolean". Missing keys fall back to the active theme's
    /// semantic colors.
    pub result_type_colors: Option<HashMap<String, String>>,
    pub check_for_updates: Option<bool>,
    pub auto_check_updates_on_startup: Option<bool>,
    pub last_dismissed_version: Option<String>,
    pub er_diagram_default_layout: Option<String>,
    pub schema_preferences: Option<HashMap<String, String>>,
    pub selected_schemas: Option<HashMap<String, Vec<String>>>,
    pub max_blob_size: Option<u64>,
    pub copy_format: Option<String>,
    pub csv_delimiter: Option<String>,
    /// Whether copied CSV output includes a header row. Default: true.
    pub csv_include_headers: Option<bool>,
    /// Update channel: "stable" (default) or "nightly". None ⇒ stable.
    pub release_channel: Option<String>,
    pub editor_theme: Option<String>,
    pub editor_font_family: Option<String>,
    pub editor_font_size: Option<u32>,
    pub editor_line_height: Option<f32>,
    pub editor_tab_size: Option<u32>,
    pub editor_word_wrap: Option<bool>,
    pub editor_show_line_numbers: Option<bool>,
    /// Whether the Enter key accepts the active autocomplete suggestion in the
    /// SQL editor. Maps to Monaco's `acceptSuggestionOnEnter` setting: `true`
    /// becomes `"smart"` (the safer variant), `false` becomes `"off"`.
    /// Default: `true` — matches the behaviour users expect from most editors.
    pub editor_accept_suggestion_on_enter: Option<bool>,
    pub run_statement_under_cursor: Option<bool>,
    // ----- SQL Formatter -----
    pub formatter_keyword_case: Option<String>,
    pub formatter_indent_style: Option<String>,
    pub formatter_tab_width: Option<u32>,
    pub formatter_use_tabs: Option<bool>,
    pub formatter_function_case: Option<String>,
    pub formatter_lines_between_queries: Option<u32>,
    pub formatter_dense_operators: Option<bool>,
    /// Connection health check interval in seconds. 0 = disabled. Default: 30.
    pub ping_interval: Option<u32>,
    /// Maximum number of query history entries per connection. Default: 500.
    pub query_history_max_entries: Option<u32>,
    /// Whether to show the welcome screen on startup. Default: true (first launch).
    pub show_welcome: Option<bool>,
    /// Maximize the window on startup. Default: false.
    pub start_maximized: Option<bool>,
    /// IANA timezone name (e.g. `Asia/Tokyo`) used to render timestamps in the
    /// UI and exports. `None` or `"auto"` follows the OS local timezone.
    pub display_timezone: Option<String>,

    // ----- Automatic connections backup -----
    /// When backups run: `"manual"` (default), `"interval"`, `"onClose"`
    /// or `"onLaunch"`.
    pub backup_mode: Option<String>,
    /// Directory the backup files are written to.
    pub backup_directory: Option<String>,
    /// Minutes between automatic backups in interval mode. Default: 1440.
    pub backup_interval_minutes: Option<u32>,
    /// Number of backup files kept before rotation. Default: 10.
    pub backup_retention: Option<u32>,
    /// Backup destination: `"local"` (default) or `"webdav"`.
    pub backup_target: Option<String>,
    /// WebDAV collection URL the backups are uploaded into.
    pub backup_webdav_url: Option<String>,
    /// WebDAV username; the password lives in the OS keychain.
    pub backup_webdav_username: Option<String>,

    // ----- Session restore -----
    /// Reconnect to the last active connection on startup. Default: true.
    pub auto_connect_last_connection: Option<bool>,
    /// Id of the connection that was active when the app was last closed.
    pub last_active_connection_id: Option<String>,
    /// Ids of all connections that were open when the app was last closed.
    pub last_open_connection_ids: Option<Vec<String>>,
}

static CONFIG_CACHE: Lazy<RwLock<AppConfig>> = Lazy::new(|| RwLock::new(AppConfig::default()));

pub fn get_config_dir<R: tauri::Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_config_dir().ok()
}

fn cache_config(config: &AppConfig) {
    if let Ok(mut cached) = CONFIG_CACHE.write() {
        *cached = config.clone();
    }
}

pub fn get_cached_config() -> AppConfig {
    CONFIG_CACHE
        .read()
        .map(|cached| cached.clone())
        .unwrap_or_default()
}

// Internal load
pub fn load_config_internal<R: tauri::Runtime>(app: &AppHandle<R>) -> AppConfig {
    if let Some(config_dir) = get_config_dir(app) {
        let config_path = config_dir.join("config.json");
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(config_path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    cache_config(&config);
                    return config;
                }
            }
        }
    }
    let default_config = AppConfig::default();
    cache_config(&default_config);
    default_config
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> AppConfig {
    load_config_internal(&app)
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    if let Some(config_dir) = get_config_dir(&app) {
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
        }
        let config_path = config_dir.join("config.json");

        // Load existing config and merge with new values
        let mut existing_config = load_config_internal(&app);

        // Merge: only update fields that are Some in the new config
        if config.theme.is_some() {
            existing_config.theme = config.theme;
        }
        if config.language.is_some() {
            existing_config.language = config.language;
        }
        if config.result_page_size.is_some() {
            existing_config.result_page_size = config.result_page_size;
        }
        if config.font_family.is_some() {
            existing_config.font_family = config.font_family;
        }
        if config.font_size.is_some() {
            existing_config.font_size = config.font_size;
        }
        if config.result_color_by_type.is_some() {
            existing_config.result_color_by_type = config.result_color_by_type;
        }
        if config.result_type_colors.is_some() {
            existing_config.result_type_colors = config.result_type_colors;
        }
        if config.check_for_updates.is_some() {
            existing_config.check_for_updates = config.check_for_updates;
        }
        if config.auto_check_updates_on_startup.is_some() {
            existing_config.auto_check_updates_on_startup = config.auto_check_updates_on_startup;
        }
        if config.last_dismissed_version.is_some() {
            existing_config.last_dismissed_version = config.last_dismissed_version;
        }
        if config.er_diagram_default_layout.is_some() {
            existing_config.er_diagram_default_layout = config.er_diagram_default_layout;
        }
        if config.schema_preferences.is_some() {
            existing_config.schema_preferences = config.schema_preferences;
        }
        if config.selected_schemas.is_some() {
            existing_config.selected_schemas = config.selected_schemas;
        }
        if config.max_blob_size.is_some() {
            existing_config.max_blob_size = config.max_blob_size;
        }
        if config.copy_format.is_some() {
            existing_config.copy_format = config.copy_format;
        }
        if config.csv_delimiter.is_some() {
            existing_config.csv_delimiter = config.csv_delimiter;
        }
        if config.csv_include_headers.is_some() {
            existing_config.csv_include_headers = config.csv_include_headers;
        }
        if config.release_channel.is_some() {
            existing_config.release_channel = config.release_channel;
        }
        if config.editor_theme.is_some() {
            existing_config.editor_theme = config.editor_theme;
        }
        if config.editor_font_family.is_some() {
            existing_config.editor_font_family = config.editor_font_family;
        }
        if config.editor_font_size.is_some() {
            existing_config.editor_font_size = config.editor_font_size;
        }
        if config.editor_line_height.is_some() {
            existing_config.editor_line_height = config.editor_line_height;
        }
        if config.editor_tab_size.is_some() {
            existing_config.editor_tab_size = config.editor_tab_size;
        }
        if config.editor_word_wrap.is_some() {
            existing_config.editor_word_wrap = config.editor_word_wrap;
        }
        if config.editor_show_line_numbers.is_some() {
            existing_config.editor_show_line_numbers = config.editor_show_line_numbers;
        }
        if config.editor_accept_suggestion_on_enter.is_some() {
            existing_config.editor_accept_suggestion_on_enter =
                config.editor_accept_suggestion_on_enter;
        }
        if config.run_statement_under_cursor.is_some() {
            existing_config.run_statement_under_cursor = config.run_statement_under_cursor;
        }
        if config.ping_interval.is_some() {
            let old_interval = existing_config.ping_interval;
            existing_config.ping_interval = config.ping_interval;
            // Restart the ping loop if the interval changed.
            if existing_config.ping_interval != old_interval {
                let interval = existing_config
                    .ping_interval
                    .unwrap_or(crate::health_check::DEFAULT_PING_INTERVAL);
                tauri::async_runtime::spawn(crate::health_check::restart_ping_loop(
                    app.clone(),
                    interval as u64,
                ));
            }
        }
        if config.query_history_max_entries.is_some() {
            existing_config.query_history_max_entries = config.query_history_max_entries;
        }
        if config.show_welcome.is_some() {
            existing_config.show_welcome = config.show_welcome;
        }
        if config.start_maximized.is_some() {
            existing_config.start_maximized = config.start_maximized;
        }
        if config.display_timezone.is_some() {
            existing_config.display_timezone = config.display_timezone;
        }
        if config.backup_mode.is_some() {
            existing_config.backup_mode = config.backup_mode;
        }
        if config.backup_directory.is_some() {
            existing_config.backup_directory = config.backup_directory;
        }
        if config.backup_interval_minutes.is_some() {
            existing_config.backup_interval_minutes = config.backup_interval_minutes;
        }
        if config.backup_retention.is_some() {
            existing_config.backup_retention = config.backup_retention;
        }
        if config.backup_target.is_some() {
            existing_config.backup_target = config.backup_target;
        }
        if config.backup_webdav_url.is_some() {
            existing_config.backup_webdav_url = config.backup_webdav_url;
        }
        if config.backup_webdav_username.is_some() {
            existing_config.backup_webdav_username = config.backup_webdav_username;
        }
        if config.auto_connect_last_connection.is_some() {
            existing_config.auto_connect_last_connection = config.auto_connect_last_connection;
        }
        if config.last_active_connection_id.is_some() {
            existing_config.last_active_connection_id = config.last_active_connection_id;
        }
        if config.last_open_connection_ids.is_some() {
            existing_config.last_open_connection_ids = config.last_open_connection_ids;
        }

        let content = serde_json::to_string_pretty(&existing_config).map_err(|e| e.to_string())?;
        fs::write(config_path, content).map_err(|e| e.to_string())?;
        cache_config(&existing_config);
        Ok(())
    } else {
        Err("Could not resolve config directory".to_string())
    }
}

#[tauri::command]
pub fn get_schema_preference(app: AppHandle, connection_id: String) -> Option<String> {
    let config = load_config_internal(&app);
    config
        .schema_preferences
        .and_then(|prefs| prefs.get(&connection_id).cloned())
}

#[tauri::command]
pub fn set_schema_preference(
    app: AppHandle,
    connection_id: String,
    schema: String,
) -> Result<(), String> {
    if let Some(config_dir) = get_config_dir(&app) {
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
        }
        let config_path = config_dir.join("config.json");
        let mut config = load_config_internal(&app);
        let prefs = config.schema_preferences.get_or_insert_with(HashMap::new);
        prefs.insert(connection_id, schema);
        let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
        fs::write(config_path, content).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Could not resolve config directory".to_string())
    }
}

#[tauri::command]
pub fn get_last_active_connection(app: AppHandle) -> Option<String> {
    load_config_internal(&app).last_active_connection_id
}

#[tauri::command]
pub fn set_last_active_connection(
    app: AppHandle,
    connection_id: Option<String>,
) -> Result<(), String> {
    if let Some(config_dir) = get_config_dir(&app) {
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
        }
        let config_path = config_dir.join("config.json");
        let mut config = load_config_internal(&app);
        config.last_active_connection_id = connection_id;
        let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
        fs::write(config_path, content).map_err(|e| e.to_string())?;
        cache_config(&config);
        Ok(())
    } else {
        Err("Could not resolve config directory".to_string())
    }
}

#[tauri::command]
pub fn get_last_open_connections(app: AppHandle) -> Vec<String> {
    load_config_internal(&app)
        .last_open_connection_ids
        .unwrap_or_default()
}

#[tauri::command]
pub fn set_last_open_connections(
    app: AppHandle,
    connection_ids: Vec<String>,
) -> Result<(), String> {
    if let Some(config_dir) = get_config_dir(&app) {
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
        }
        let config_path = config_dir.join("config.json");
        let mut config = load_config_internal(&app);
        config.last_open_connection_ids = Some(connection_ids);
        let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
        fs::write(config_path, content).map_err(|e| e.to_string())?;
        cache_config(&config);
        Ok(())
    } else {
        Err("Could not resolve config directory".to_string())
    }
}

#[tauri::command]
pub fn get_selected_schemas(app: AppHandle, connection_id: String) -> Vec<String> {
    let config = load_config_internal(&app);
    config
        .selected_schemas
        .and_then(|map| map.get(&connection_id).cloned())
        .unwrap_or_default()
}

#[tauri::command]
pub fn set_selected_schemas(
    app: AppHandle,
    connection_id: String,
    schemas: Vec<String>,
) -> Result<(), String> {
    if let Some(config_dir) = get_config_dir(&app) {
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
        }
        let config_path = config_dir.join("config.json");
        let mut config = load_config_internal(&app);
        let map = config.selected_schemas.get_or_insert_with(HashMap::new);
        if schemas.is_empty() {
            map.remove(&connection_id);
        } else {
            map.insert(connection_id, schemas);
        }
        let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
        fs::write(config_path, content).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Could not resolve config directory".to_string())
    }
}

/// Get the configured maximum BLOB size in bytes, or DEFAULT_MAX_BLOB_SIZE if not set
pub fn get_max_blob_size<R: tauri::Runtime>(app: &AppHandle<R>) -> u64 {
    let config = load_config_internal(app);
    config
        .max_blob_size
        .unwrap_or(crate::drivers::common::DEFAULT_MAX_BLOB_SIZE)
}

#[tauri::command]
pub fn get_config_json(app: AppHandle) -> Result<String, String> {
    if let Some(config_dir) = get_config_dir(&app) {
        let config_path = config_dir.join("config.json");
        if config_path.exists() {
            return fs::read_to_string(config_path).map_err(|e| e.to_string());
        }
    }
    // Return empty JSON object if no config file exists yet
    Ok("{}".to_string())
}

#[tauri::command]
pub fn relaunch_app(app: AppHandle) {
    app.restart();
}

#[tauri::command]
pub fn save_config_json(app: AppHandle, json: String) -> Result<(), String> {
    // Validate the JSON parses as a valid AppConfig
    serde_json::from_str::<AppConfig>(&json)
        .map_err(|e| format!("Invalid configuration JSON: {}", e))?;

    if let Some(config_dir) = get_config_dir(&app) {
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
        }
        let config_path = config_dir.join("config.json");
        // Re-serialize with pretty-printing for consistency
        let value: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        let pretty = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
        fs::write(config_path, pretty).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Could not resolve config directory".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_schemas_default_is_none() {
        let config = AppConfig::default();
        assert!(config.selected_schemas.is_none());
    }

    #[test]
    fn selected_schemas_serialization_round_trip() {
        let mut config = AppConfig::default();
        let mut map = HashMap::new();
        map.insert(
            "conn-1".to_string(),
            vec!["public".to_string(), "analytics".to_string()],
        );
        config.selected_schemas = Some(map);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

        let schemas = deserialized.selected_schemas.unwrap();
        let conn1 = schemas.get("conn-1").unwrap();
        assert_eq!(conn1, &vec!["public".to_string(), "analytics".to_string()]);
    }

    #[test]
    fn selected_schemas_camel_case_in_json() {
        let mut config = AppConfig::default();
        let mut map = HashMap::new();
        map.insert("conn-1".to_string(), vec!["public".to_string()]);
        config.selected_schemas = Some(map);

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("selectedSchemas"));
        assert!(!json.contains("selected_schemas"));
    }

    #[test]
    fn multiple_connections_independent_selected_schemas() {
        let mut config = AppConfig::default();
        let mut map = HashMap::new();
        map.insert("conn-1".to_string(), vec!["public".to_string()]);
        map.insert(
            "conn-2".to_string(),
            vec!["staging".to_string(), "prod".to_string()],
        );
        config.selected_schemas = Some(map);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

        let schemas = deserialized.selected_schemas.unwrap();
        assert_eq!(schemas.get("conn-1").unwrap(), &vec!["public".to_string()]);
        assert_eq!(
            schemas.get("conn-2").unwrap(),
            &vec!["staging".to_string(), "prod".to_string()]
        );
    }

    #[test]
    fn old_hidden_schemas_json_deserializes_without_error() {
        // Ensure old config files with hiddenSchemas don't break deserialization
        let json = r#"{"hiddenSchemas":{"conn-1":["secret"]}}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        // hiddenSchemas is no longer a field, so it's ignored; selectedSchemas is None
        assert!(config.selected_schemas.is_none());
    }

    #[test]
    fn editor_fields_default_to_none() {
        let config = AppConfig::default();
        assert!(config.editor_theme.is_none());
        assert!(config.editor_font_family.is_none());
        assert!(config.editor_font_size.is_none());
        assert!(config.editor_line_height.is_none());
        assert!(config.editor_tab_size.is_none());
        assert!(config.editor_word_wrap.is_none());
        assert!(config.editor_show_line_numbers.is_none());
        assert!(config.editor_accept_suggestion_on_enter.is_none());
    }

    #[test]
    fn editor_fields_serialize_with_camel_case() {
        let mut config = AppConfig::default();
        config.editor_font_family = Some("JetBrains Mono".to_string());
        config.editor_font_size = Some(16);
        config.editor_line_height = Some(1.5);
        config.editor_tab_size = Some(4);
        config.editor_word_wrap = Some(false);
        config.editor_show_line_numbers = Some(true);
        config.editor_theme = Some("tabularis-light".to_string());
        config.editor_accept_suggestion_on_enter = Some(true);

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("editorFontFamily"));
        assert!(json.contains("editorFontSize"));
        assert!(json.contains("editorLineHeight"));
        assert!(json.contains("editorTabSize"));
        assert!(json.contains("editorWordWrap"));
        assert!(json.contains("editorShowLineNumbers"));
        assert!(json.contains("editorTheme"));
        assert!(json.contains("editorAcceptSuggestionOnEnter"));
        // snake_case must not appear
        assert!(!json.contains("editor_font_family"));
        assert!(!json.contains("editor_accept_suggestion_on_enter"));
    }

    #[test]
    fn editor_fields_round_trip() {
        let json = r#"{
            "editorFontFamily": "Hack",
            "editorFontSize": 14,
            "editorLineHeight": 1.8,
            "editorTabSize": 2,
            "editorWordWrap": true,
            "editorShowLineNumbers": true,
            "editorTheme": "tabularis-dark",
            "editorAcceptSuggestionOnEnter": true
        }"#;

        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.editor_font_family.as_deref(), Some("Hack"));
        assert_eq!(config.editor_font_size, Some(14));
        assert_eq!(config.editor_tab_size, Some(2));
        assert_eq!(config.editor_word_wrap, Some(true));
        assert_eq!(config.editor_show_line_numbers, Some(true));
        assert_eq!(config.editor_theme.as_deref(), Some("tabularis-dark"));
        assert_eq!(config.editor_accept_suggestion_on_enter, Some(true));
    }

    #[test]
    fn save_config_json_rejects_invalid_json() {
        // Test that the validation logic catches malformed AppConfig JSON
        let invalid = r#"{"editorFontSize": "not-a-number"}"#;
        let result = serde_json::from_str::<AppConfig>(invalid);
        assert!(result.is_err());
    }

    #[test]
    fn display_timezone_serializes_with_camel_case_and_round_trips() {
        let mut config = AppConfig::default();
        assert!(config.display_timezone.is_none());
        config.display_timezone = Some("Asia/Tokyo".into());
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("displayTimezone"));
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.display_timezone.as_deref(), Some("Asia/Tokyo"));
    }

}
