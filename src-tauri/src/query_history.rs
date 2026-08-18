use crate::config::load_config_internal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime, State};
use tokio::sync::Mutex;
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
    #[serde(default)]
    pub database: Option<String>,
}

/// Response shape for `get_query_history`. `recovered_backup_path` is set
/// when the on-disk JSON could not be parsed and was renamed aside so the
/// app could start fresh; the UI surfaces a banner with the backup location
/// so the user can recover entries manually if needed.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryHistoryResponse {
    pub entries: Vec<QueryHistoryEntry>,
    pub recovered_backup_path: Option<String>,
}

/// Serializes read-modify-write sequences against each connection's history
/// file so concurrent `addEntry` calls (e.g. one per statement in a
/// multi-statement batch) can't interleave and overwrite each other.
#[derive(Default)]
pub struct QueryHistoryState {
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

async fn acquire_lock(state: &QueryHistoryState, connection_id: &str) -> Arc<Mutex<()>> {
    let mut map = state.locks.lock().await;
    map.entry(connection_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
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

/// Read history, recovering from corruption by renaming the bad file aside.
///
/// Returns the parsed entries and, when recovery happened, the path of the
/// backup file. Callers that don't need to surface recovery info can use
/// the [`read_history`] wrapper.
fn read_history_with_recovery<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
) -> Result<(Vec<QueryHistoryEntry>, Option<PathBuf>), String> {
    let path = get_history_path(app, connection_id)?;
    if !path.exists() {
        return Ok((Vec::new(), None));
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    match serde_json::from_str::<Vec<QueryHistoryEntry>>(&content) {
        Ok(entries) => Ok((entries, None)),
        Err(parse_err) => {
            let backup = backup_corrupt_file(&path).map_err(|e| {
                format!(
                    "Query history JSON parse failed and backup also failed: {} (parse error: {})",
                    e, parse_err
                )
            })?;
            log::warn!(
                "Query history file for connection '{}' was corrupt ({}); moved to {}",
                connection_id,
                parse_err,
                backup.display()
            );
            Ok((Vec::new(), Some(backup)))
        }
    }
}

fn read_history<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
) -> Result<Vec<QueryHistoryEntry>, String> {
    read_history_with_recovery(app, connection_id).map(|(entries, _)| entries)
}

/// Rename a corrupt history file aside using a UTC timestamp suffix. If a
/// file already exists at the target backup path (unlikely but possible on
/// the same-millisecond retry) a short uuid suffix is appended.
pub(crate) fn backup_corrupt_file(path: &Path) -> Result<PathBuf, String> {
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S%3f");
    let base = path.file_name().and_then(|n| n.to_str()).unwrap_or("history.json");
    let mut backup = path.with_file_name(format!("{}.corrupt-{}", base, timestamp));
    if backup.exists() {
        backup = path.with_file_name(format!(
            "{}.corrupt-{}-{}",
            base,
            timestamp,
            Uuid::new_v4().simple()
        ));
    }
    fs::rename(path, &backup).map_err(|e| e.to_string())?;
    Ok(backup)
}

/// Atomic write: serialise to a per-write temp file in the same directory,
/// then `rename` onto the target. `rename` within a filesystem is atomic on
/// POSIX/APFS, so a concurrent or crashed write can never leave the target
/// file half-written (which is the failure mode that produced the original
/// "extra data after array" corruption).
fn write_history<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
    entries: &[QueryHistoryEntry],
) -> Result<(), String> {
    let path = get_history_path(app, connection_id)?;
    let content = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    atomic_write(&path, content.as_bytes())
}

// --- Append-only log -----------------------------------------------------
//
// The original store rewrote the entire JSON array on every executed
// statement, so cost grew with history size — at a few thousand entries that
// is a visible stall on each run, and 100k would be unusable. The log below
// is JSONL, appended once per statement (O(1)), ordered oldest-first.
// Readers either walk a bounded tail (the UI) or stream the file (search).

/// Hard cap per connection. Reaching it compacts the log down to the newest
/// `HISTORY_COMPACT_TO` entries — a rare O(n) event instead of a per-write one.
const HISTORY_LOG_MAX: usize = 100_000;
const HISTORY_COMPACT_TO: usize = 80_000;

/// Bytes read per backward step when walking the tail. One entry is well
/// under 1 KB, so this covers the default page in a single read.
const TAIL_CHUNK: usize = 256 * 1024;

fn log_path<R: Runtime>(app: &AppHandle<R>, connection_id: &str) -> Result<PathBuf, String> {
    let dir = get_history_dir(app)?;
    Ok(dir.join(format!("{}.jsonl", connection_id)))
}

/// Moves a legacy `<id>.json` array into the JSONL log, once.
///
/// The old file is renamed rather than deleted so a failed migration cannot
/// lose history.
fn migrate_legacy_json<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
) -> Result<(), String> {
    let legacy = get_history_path(app, connection_id)?;
    if !legacy.exists() {
        return Ok(());
    }
    let log = log_path(app, connection_id)?;
    let mut existing = read_history(app, connection_id).unwrap_or_default();
    // Legacy order is newest-first; the log is oldest-first.
    existing.reverse();
    let mut buffer = String::new();
    for entry in &existing {
        if let Ok(line) = serde_json::to_string(entry) {
            buffer.push_str(&line);
            buffer.push('\n');
        }
    }
    if !buffer.is_empty() {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .map_err(|e| e.to_string())?;
        file.write_all(buffer.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let archived = legacy.with_extension("json.migrated");
    let _ = fs::rename(&legacy, &archived);
    log::info!(
        "Query history for '{}' migrated to append-only log ({} entries)",
        connection_id,
        existing.len()
    );
    Ok(())
}

fn append_log_entry<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
    entry: &QueryHistoryEntry,
) -> Result<(), String> {
    migrate_legacy_json(app, connection_id)?;
    let path = log_path(app, connection_id)?;
    let mut line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    line.push('\n');
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

/// Returns the newest `limit` entries (newest first) by reading only the tail.
fn read_log_tail<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
    limit: usize,
) -> Result<Vec<QueryHistoryEntry>, String> {
    migrate_legacy_json(app, connection_id)?;
    let path = log_path(app, connection_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    use std::io::{Read, Seek, SeekFrom};
    let mut file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let mut end = size;
    let mut collected: Vec<QueryHistoryEntry> = Vec::new();
    let mut carry = String::new();

    while end > 0 && collected.len() < limit {
        let step = TAIL_CHUNK.min(end as usize);
        let start = end - step as u64;
        file.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; step];
        file.read_exact(&mut buf).map_err(|e| e.to_string())?;
        let mut chunk = String::from_utf8_lossy(&buf).into_owned();
        chunk.push_str(&carry);

        // The first line may be truncated unless we are at the file start.
        let mut lines: Vec<&str> = chunk.split('\n').collect();
        if start > 0 {
            carry = lines.remove(0).to_string();
        } else {
            carry.clear();
        }
        for line in lines.iter().rev() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<QueryHistoryEntry>(line) {
                collected.push(entry);
                if collected.len() >= limit {
                    break;
                }
            }
        }
        end = start;
    }
    Ok(collected)
}

fn count_log_lines(path: &Path) -> usize {
    let Ok(file) = fs::File::open(path) else {
        return 0;
    };
    std::io::BufRead::lines(std::io::BufReader::new(file))
        .filter(|line| line.as_ref().is_ok_and(|l| !l.trim().is_empty()))
        .count()
}

/// Trims the log to `HISTORY_COMPACT_TO` newest entries once it exceeds
/// `HISTORY_LOG_MAX`.
fn compact_log_if_needed<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
) -> Result<(), String> {
    let path = log_path(app, connection_id)?;
    if !path.exists() || count_log_lines(&path) <= HISTORY_LOG_MAX {
        return Ok(());
    }
    let mut newest = read_log_tail(app, connection_id, HISTORY_COMPACT_TO)?;
    newest.reverse(); // back to oldest-first for the log
    let mut buffer = String::new();
    for entry in &newest {
        if let Ok(line) = serde_json::to_string(entry) {
            buffer.push_str(&line);
            buffer.push('\n');
        }
    }
    atomic_write(&path, buffer.as_bytes())?;
    log::info!(
        "Compacted query history log for '{}' to {} entries",
        connection_id,
        newest.len()
    );
    Ok(())
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path.parent().ok_or_else(|| "history path has no parent".to_string())?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("history.json");
    let tmp = dir.join(format!(".{}.tmp.{}", file_name, Uuid::new_v4().simple()));
    if let Err(e) = fs::write(&tmp, bytes) {
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_query_history<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, QueryHistoryState>,
    connection_id: String,
) -> Result<QueryHistoryResponse, String> {
    let lock = acquire_lock(&state, &connection_id).await;
    let _guard = lock.lock().await;
    let (entries, backup) = read_history_with_recovery(&app, &connection_id)?;
    Ok(QueryHistoryResponse {
        entries,
        recovered_backup_path: backup.map(|p| p.to_string_lossy().into_owned()),
    })
}

/// How many entries the UI loads by default. The full log stays on disk and
/// is reachable through `search_query_history`; rendering more than this in
/// the sidebar is what made history feel slow.
const UI_HISTORY_PAGE: usize = 200;

/// Reads the newest `limit` entries without materializing the whole log.
///
/// The log is append-only JSONL (newest last), so this seeks to the end and
/// walks backwards over a bounded tail rather than parsing every line.
#[tauri::command]
pub async fn get_recent_query_history<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, QueryHistoryState>,
    connection_id: String,
    limit: Option<usize>,
) -> Result<QueryHistoryResponse, String> {
    let lock = acquire_lock(&state, &connection_id).await;
    let _guard = lock.lock().await;
    let limit = limit.unwrap_or(UI_HISTORY_PAGE).clamp(1, 5_000);
    let entries = read_log_tail(&app, &connection_id, limit)?;
    Ok(QueryHistoryResponse {
        entries,
        recovered_backup_path: None,
    })
}

/// Substring search across the full log (case-insensitive), newest first.
///
/// Streams the file line by line so a 100k-entry log costs bounded memory.
#[tauri::command]
pub async fn search_query_history<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, QueryHistoryState>,
    connection_id: String,
    query: String,
    limit: Option<usize>,
) -> Result<QueryHistoryResponse, String> {
    let lock = acquire_lock(&state, &connection_id).await;
    let _guard = lock.lock().await;
    let limit = limit.unwrap_or(UI_HISTORY_PAGE).clamp(1, 5_000);
    let needle = query.trim().to_lowercase();

    let path = log_path(&app, &connection_id)?;
    if !path.exists() {
        return Ok(QueryHistoryResponse::default());
    }
    let file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let reader = std::io::BufReader::new(file);
    let mut hits: Vec<QueryHistoryEntry> = Vec::new();
    for line in std::io::BufRead::lines(reader) {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        // Cheap pre-filter on the raw line before paying for JSON parsing.
        if !needle.is_empty() && !line.to_lowercase().contains(&needle) {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<QueryHistoryEntry>(&line) else {
            continue;
        };
        if needle.is_empty()
            || entry.sql.to_lowercase().contains(&needle)
            || entry
                .database
                .as_deref()
                .is_some_and(|db| db.to_lowercase().contains(&needle))
            || entry
                .error
                .as_deref()
                .is_some_and(|err| err.to_lowercase().contains(&needle))
        {
            hits.push(entry);
            // Keep only the newest `limit`: drop from the front as we go so
            // memory stays bounded regardless of how many rows match.
            if hits.len() > limit {
                hits.remove(0);
            }
        }
    }
    hits.reverse();
    Ok(QueryHistoryResponse {
        entries: hits,
        recovered_backup_path: None,
    })
}

#[tauri::command]
pub async fn add_query_history_entry<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, QueryHistoryState>,
    connection_id: String,
    sql: String,
    executed_at: String,
    execution_time_ms: Option<f64>,
    status: String,
    rows_affected: Option<i64>,
    error: Option<String>,
    database: Option<String>,
) -> Result<QueryHistoryEntry, String> {
    let lock = acquire_lock(&state, &connection_id).await;
    let _guard = lock.lock().await;

    let entry = QueryHistoryEntry {
        id: Uuid::new_v4().to_string(),
        sql,
        executed_at,
        execution_time_ms,
        status,
        rows_affected,
        error,
        database,
    };

    // Append-only: cost is independent of how much history exists. The old
    // implementation re-read and rewrote the whole array per statement, which
    // is what made executing SQL slow as history grew.
    //
    // Consecutive-duplicate collapsing is deliberately dropped: it required
    // reading the file back on every write. The full log is the point now —
    // the UI shows the newest page and search covers the rest.
    append_log_entry(&app, &connection_id, &entry)?;
    compact_log_if_needed(&app, &connection_id)?;
    Ok(entry)
}

#[tauri::command]
pub async fn delete_query_history_entry<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, QueryHistoryState>,
    connection_id: String,
    id: String,
) -> Result<(), String> {
    let lock = acquire_lock(&state, &connection_id).await;
    let _guard = lock.lock().await;

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
    state: State<'_, QueryHistoryState>,
    connection_id: String,
) -> Result<(), String> {
    let lock = acquire_lock(&state, &connection_id).await;
    let _guard = lock.lock().await;

    let path = get_history_path(&app, &connection_id)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Set `database = Some(database)` on any history entry whose `database` is currently
/// `None`. Returns the count of entries updated.
pub fn backfill_missing_database(entries: &mut [QueryHistoryEntry], database: &str) -> usize {
    let mut updated = 0usize;
    for entry in entries.iter_mut() {
        if entry.database.is_none() {
            entry.database = Some(database.to_string());
            updated += 1;
        }
    }
    updated
}

/// Backfill `database` on history entries for a connection where it is currently `None`.
/// Used when a connection transitions from single-db to multi-db: existing entries
/// without an explicit database get associated with the original single database.
///
/// Acquires the per-connection [`QueryHistoryState`] lock so concurrent
/// `add_query_history_entry` calls can't race the read-modify-write sequence
/// and lose entries.
pub async fn backfill_missing_database_for_connection<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
    database: &str,
) -> Result<usize, String> {
    let state = app.state::<QueryHistoryState>();
    let lock = acquire_lock(&state, connection_id).await;
    let _guard = lock.lock().await;

    let mut entries = read_history(app, connection_id)?;
    let updated = backfill_missing_database(&mut entries, database);
    if updated > 0 {
        write_history(app, connection_id, &entries)?;
    }
    Ok(updated)
}

/// Remove history file for a connection (called during connection deletion).
///
/// Acquires the per-connection [`QueryHistoryState`] lock so an in-flight
/// `add_query_history_entry` (started before the connection was deleted) can't
/// recreate the file after we remove it.
pub async fn remove_history_for_connection<R: Runtime>(
    app: &AppHandle<R>,
    connection_id: &str,
) -> Result<(), String> {
    let state = app.state::<QueryHistoryState>();
    let lock = acquire_lock(&state, connection_id).await;
    let _guard = lock.lock().await;

    // Remove every on-disk form: the legacy array, its migrated archive, and
    // the append-only log. Missing any of them would resurrect history for a
    // connection id that gets reused.
    let legacy = get_history_path(app, connection_id)?;
    for path in [
        legacy.clone(),
        legacy.with_extension("json.migrated"),
        log_path(app, connection_id)?,
    ] {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod log_tests {
    use super::*;

    fn entry(sql: &str, at: &str) -> QueryHistoryEntry {
        QueryHistoryEntry {
            id: Uuid::new_v4().to_string(),
            sql: sql.to_string(),
            executed_at: at.to_string(),
            execution_time_ms: Some(1.0),
            status: "success".to_string(),
            rows_affected: Some(1),
            error: None,
            database: Some("testdb".to_string()),
        }
    }

    fn write_log(path: &Path, entries: &[QueryHistoryEntry]) {
        let mut buffer = String::new();
        for e in entries {
            buffer.push_str(&serde_json::to_string(e).unwrap());
            buffer.push('\n');
        }
        fs::write(path, buffer).unwrap();
    }

    /// Mirrors `read_log_tail`'s backward walk without the Tauri AppHandle.
    fn tail_of(path: &Path, limit: usize) -> Vec<QueryHistoryEntry> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = fs::File::open(path).unwrap();
        let size = file.metadata().unwrap().len();
        let mut end = size;
        let mut collected = Vec::new();
        let mut carry = String::new();
        while end > 0 && collected.len() < limit {
            let step = TAIL_CHUNK.min(end as usize);
            let start = end - step as u64;
            file.seek(SeekFrom::Start(start)).unwrap();
            let mut buf = vec![0u8; step];
            file.read_exact(&mut buf).unwrap();
            let mut chunk = String::from_utf8_lossy(&buf).into_owned();
            chunk.push_str(&carry);
            let mut lines: Vec<&str> = chunk.split('\n').collect();
            if start > 0 {
                carry = lines.remove(0).to_string();
            } else {
                carry.clear();
            }
            for line in lines.iter().rev() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(e) = serde_json::from_str::<QueryHistoryEntry>(line) {
                    collected.push(e);
                    if collected.len() >= limit {
                        break;
                    }
                }
            }
            end = start;
        }
        collected
    }

    #[test]
    fn tail_returns_newest_first_and_respects_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let entries: Vec<_> = (0..1000)
            .map(|i| entry(&format!("SELECT {i}"), "2026-01-01T00:00:00Z"))
            .collect();
        write_log(&path, &entries);

        let tail = tail_of(&path, 200);
        assert_eq!(tail.len(), 200, "limit must bound the result");
        assert_eq!(tail[0].sql, "SELECT 999", "newest entry must come first");
        assert_eq!(tail[199].sql, "SELECT 800");
    }

    #[test]
    fn tail_handles_logs_smaller_than_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        write_log(&path, &[entry("SELECT 1", "2026-01-01T00:00:00Z")]);
        let tail = tail_of(&path, 200);
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0].sql, "SELECT 1");
    }

    /// A log larger than one TAIL_CHUNK must still stitch lines across reads
    /// rather than dropping or corrupting the entry split by the boundary.
    #[test]
    fn tail_spans_chunk_boundaries_without_losing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        // ~300 bytes/entry * 4000 = ~1.2 MB, several TAIL_CHUNK steps.
        let entries: Vec<_> = (0..4000)
            .map(|i| entry(&format!("SELECT {i} /* {} */", "x".repeat(200)), "2026-01-01T00:00:00Z"))
            .collect();
        write_log(&path, &entries);

        let tail = tail_of(&path, 1000);
        assert_eq!(tail.len(), 1000);
        assert!(tail[0].sql.starts_with("SELECT 3999"));
        assert!(tail[999].sql.starts_with("SELECT 3000"));
        // Every entry must parse: a mis-stitched boundary would yield fewer.
        assert_eq!(
            tail.iter().filter(|e| e.status == "success").count(),
            1000
        );
    }

    #[test]
    fn count_log_lines_ignores_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let mut buffer = String::new();
        for i in 0..5 {
            buffer.push_str(&serde_json::to_string(&entry(&format!("S{i}"), "t")).unwrap());
            buffer.push('\n');
        }
        buffer.push('\n');
        fs::write(&path, buffer).unwrap();
        assert_eq!(count_log_lines(&path), 5);
    }
}

