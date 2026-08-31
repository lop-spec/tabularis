use log::{LevelFilter, Log, Metadata, Record};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

const DEFAULT_LOG_FILE_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_RETAINED_FILES: usize = 5;
const ACTIVE_LOG_FILE: &str = "tabularis.ndjson";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone)]
struct PersistentLogStore {
    directory: PathBuf,
    max_file_bytes: u64,
    retained_files: usize,
}

impl PersistentLogStore {
    fn new(directory: PathBuf, max_file_bytes: u64, retained_files: usize) -> Result<Self, String> {
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        Ok(Self {
            directory,
            max_file_bytes,
            retained_files,
        })
    }

    fn active_path(&self) -> PathBuf {
        self.directory.join(ACTIVE_LOG_FILE)
    }

    fn archive_path(&self, index: usize) -> PathBuf {
        self.directory.join(format!("{ACTIVE_LOG_FILE}.{index}"))
    }

    fn rotate_if_needed(&self, incoming_bytes: u64) -> Result<(), String> {
        let active = self.active_path();
        let current_bytes = active
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if current_bytes.saturating_add(incoming_bytes) <= self.max_file_bytes {
            return Ok(());
        }
        let oldest = self.archive_path(self.retained_files);
        if oldest.exists() {
            fs::remove_file(oldest).map_err(|error| error.to_string())?;
        }
        for index in (1..self.retained_files).rev() {
            let source = self.archive_path(index);
            if source.exists() {
                fs::rename(source, self.archive_path(index + 1))
                    .map_err(|error| error.to_string())?;
            }
        }
        if active.exists() {
            fs::rename(active, self.archive_path(1)).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn append(&self, entry: &LogEntry) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(entry).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        self.rotate_if_needed(bytes.len() as u64)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.active_path())
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.flush().map_err(|error| error.to_string())
    }

    fn load_tail(&self, max_entries: usize) -> VecDeque<LogEntry> {
        let mut entries = VecDeque::with_capacity(max_entries);
        let paths = (1..=self.retained_files)
            .rev()
            .map(|index| self.archive_path(index))
            .chain(std::iter::once(self.active_path()));
        for path in paths.filter(|path| path.exists()) {
            let Ok(file) = fs::File::open(path) else {
                continue;
            };
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                let Ok(entry) = serde_json::from_str::<LogEntry>(&line) else {
                    continue;
                };
                if entries.len() >= max_entries {
                    entries.pop_front();
                }
                entries.push_back(entry);
            }
        }
        entries
    }

    fn clear(&self) -> Result<(), String> {
        for path in std::iter::once(self.active_path())
            .chain((1..=self.retained_files).map(|index| self.archive_path(index)))
        {
            if path.exists() {
                fs::remove_file(path).map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LogBuffer {
    entries: VecDeque<LogEntry>,
    max_size: usize,
    enabled: bool,
    store: Option<PersistentLogStore>,
}

impl LogBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_size),
            max_size,
            enabled: true,
            store: None,
        }
    }

    fn persistent(
        max_size: usize,
        directory: PathBuf,
        max_file_bytes: u64,
        retained_files: usize,
    ) -> Result<Self, String> {
        let store = PersistentLogStore::new(directory, max_file_bytes, retained_files)?;
        Ok(Self {
            entries: store.load_tail(max_size),
            max_size,
            enabled: true,
            store: Some(store),
        })
    }

    pub fn push(&mut self, mut entry: LogEntry) {
        if !self.enabled {
            return;
        }
        entry.message = crate::redaction::redact_log_message(&entry.message);
        if self.entries.len() >= self.max_size {
            self.entries.pop_front();
        }
        self.entries.push_back(entry.clone());
        if let Some(store) = &self.store {
            if let Err(error) = store.append(&entry) {
                eprintln!("Failed to persist application log: {error}");
            }
        }
    }

    pub fn get_entries(&self, limit: Option<usize>, level_filter: Option<String>) -> Vec<LogEntry> {
        let filtered = self.entries.iter().filter(|entry| {
            level_filter
                .as_ref()
                .map_or(true, |filter| entry.level.eq_ignore_ascii_case(filter))
        });
        let entries = filtered.cloned().collect::<Vec<_>>();
        limit.map_or(entries.clone(), |limit| {
            entries
                .into_iter()
                .rev()
                .take(limit)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        })
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        if let Some(store) = &self.store {
            if let Err(error) = store.clear() {
                eprintln!("Failed to clear persistent application logs: {error}");
            }
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_max_size(&mut self, size: usize) {
        self.max_size = size;
        while self.entries.len() > self.max_size {
            self.entries.pop_front();
        }
    }

    pub fn get_max_size(&self) -> usize {
        self.max_size
    }
}

pub type SharedLogBuffer = Arc<Mutex<LogBuffer>>;

pub fn format_timestamp() -> String {
    let datetime = chrono::DateTime::<chrono::Local>::from(SystemTime::now());
    datetime.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

pub struct CapturingLogger {
    buffer: SharedLogBuffer,
    level: LevelFilter,
}

impl CapturingLogger {
    pub fn new(buffer: SharedLogBuffer, level: LevelFilter) -> Self {
        Self { buffer, level }
    }
}

impl Log for CapturingLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = crate::redaction::redact_log_message(&record.args().to_string());
        let timestamp = format_timestamp();
        let level = record.level().to_string();
        let target = record.target().to_string();
        eprintln!("[LOG] [{timestamp}] [{level}] {target} - {message}");
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(LogEntry {
                timestamp,
                level,
                message,
                target: Some(target),
            });
        }
    }

    fn flush(&self) {}
}

pub fn init_logger(buffer: SharedLogBuffer, level: LevelFilter) {
    let logger = CapturingLogger::new(buffer, level);
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(level);
    }
}

pub fn create_log_buffer(max_size: usize) -> SharedLogBuffer {
    Arc::new(Mutex::new(LogBuffer::new(max_size)))
}

pub fn create_persistent_log_buffer(
    max_size: usize,
    directory: impl AsRef<Path>,
) -> Result<SharedLogBuffer, String> {
    Ok(Arc::new(Mutex::new(LogBuffer::persistent(
        max_size,
        directory.as_ref().to_path_buf(),
        DEFAULT_LOG_FILE_BYTES,
        DEFAULT_RETAINED_FILES,
    )?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(message: impl Into<String>) -> LogEntry {
        LogEntry {
            timestamp: "2026-01-01 00:00:00.000".to_string(),
            level: "INFO".to_string(),
            message: message.into(),
            target: Some("test".to_string()),
        }
    }

    #[test]
    fn persistent_logs_survive_buffer_recreation_and_are_redacted() {
        let directory = tempfile::tempdir().unwrap();
        let public_address = [93, 184, 216, 34].map(|part| part.to_string()).join(".");
        {
            let buffer = create_persistent_log_buffer(10, directory.path()).unwrap();
            buffer
                .lock()
                .unwrap()
                .push(entry(format!("password=hunter2 host={public_address}")));
        }
        let reopened = create_persistent_log_buffer(10, directory.path()).unwrap();
        let logs = reopened.lock().unwrap().get_entries(None, None);
        assert_eq!(logs.len(), 1);
        assert!(!logs[0].message.contains("hunter2"));
        assert!(!logs[0].message.contains(&public_address));
    }

    #[test]
    fn persistent_log_rotation_retains_recent_entries() {
        let directory = tempfile::tempdir().unwrap();
        let mut buffer = LogBuffer::persistent(20, directory.path().to_path_buf(), 180, 2).unwrap();
        for index in 0..12 {
            buffer.push(entry(format!("entry-{index}-{}", "x".repeat(48))));
        }
        assert!(directory.path().join(ACTIVE_LOG_FILE).exists());
        assert!(directory
            .path()
            .join(format!("{ACTIVE_LOG_FILE}.1"))
            .exists());
        assert!(!buffer.get_entries(None, None).is_empty());
    }

    #[test]
    fn clearing_logs_removes_memory_and_disk_records() {
        let directory = tempfile::tempdir().unwrap();
        let mut buffer =
            LogBuffer::persistent(10, directory.path().to_path_buf(), 1024, 2).unwrap();
        buffer.push(entry("message"));
        buffer.clear();
        assert!(buffer.get_entries(None, None).is_empty());
        assert!(!directory.path().join(ACTIVE_LOG_FILE).exists());
    }
}
