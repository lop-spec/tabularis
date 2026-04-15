use crate::config::load_config_internal;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

const DEFAULT_MAX_HISTORY_ENTRIES: u32 = 500;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueryHistoryEntry {
    pub id: String,
    pub sql: String,
    pub executed_at: String,
    pub execution_time_ms: Option<f64>,
    pub status: String,
    pub rows_affected: Option<i64>,
    pub error: Option<String>,
}

fn get_history_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let history_dir = config_dir.join("query_history");
    if !history_dir.exists() {
        fs::create_dir_all(&history_dir).map_err(|e| e.to_string())?;
    }
    Ok(history_dir)
}

fn get_history_path<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
) -> Result<PathBuf, String> {
    let dir = get_history_dir(app)?;
    Ok(dir.join(format!("{}.json", connection_id)))
}

fn read_history<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
) -> Result<Vec<QueryHistoryEntry>, String> {
    let path = get_history_path(app, connection_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn write_history<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
    entries: &[QueryHistoryEntry],
) -> Result<(), String> {
    let path = get_history_path(app, connection_id)?;
    let content = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_query_history<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
) -> Result<Vec<QueryHistoryEntry>, String> {
    read_history(&app, &connection_id)
}

#[tauri::command]
pub async fn add_query_history_entry<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
    sql: String,
    executed_at: String,
    execution_time_ms: Option<f64>,
    status: String,
    rows_affected: Option<i64>,
    error: Option<String>,
) -> Result<QueryHistoryEntry, String> {
    let mut entries = read_history(&app, &connection_id)?;

    let config = load_config_internal(&app);
    let max_entries = config
        .query_history_max_entries
        .unwrap_or(DEFAULT_MAX_HISTORY_ENTRIES) as usize;

    // Deduplicate: if the most recent entry has the same SQL, update it instead
    if let Some(first) = entries.first_mut() {
        if first.sql == sql {
            first.executed_at = executed_at.clone();
            first.execution_time_ms = execution_time_ms;
            first.status = status.clone();
            first.rows_affected = rows_affected;
            first.error = error.clone();
            let updated = first.clone();
            write_history(&app, &connection_id, &entries)?;
            return Ok(updated);
        }
    }

    let entry = QueryHistoryEntry {
        id: Uuid::new_v4().to_string(),
        sql,
        executed_at,
        execution_time_ms,
        status,
        rows_affected,
        error,
    };

    // Insert at the beginning (newest first)
    entries.insert(0, entry.clone());

    // Evict oldest entries if over the limit
    if entries.len() > max_entries {
        entries.truncate(max_entries);
    }

    write_history(&app, &connection_id, &entries)?;
    Ok(entry)
}

#[tauri::command]
pub async fn delete_query_history_entry<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
    id: String,
) -> Result<(), String> {
    let mut entries = read_history(&app, &connection_id)?;
    let original_len = entries.len();
    entries.retain(|e| e.id != id);

    if entries.len() == original_len {
        return Err("History entry not found".to_string());
    }

    write_history(&app, &connection_id, &entries)
}

#[tauri::command]
pub async fn clear_query_history<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
) -> Result<(), String> {
    let path = get_history_path(&app, &connection_id)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Remove history file for a connection (called during connection deletion).
pub fn remove_history_for_connection<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
) -> Result<(), String> {
    let path = get_history_path(app, connection_id)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
