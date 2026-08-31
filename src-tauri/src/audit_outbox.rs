use crate::drivers::common::strip_leading_sql_comments;
use crate::models::SavedConnection;
use chrono::Utc;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use sha2::Digest;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

const AUDIT_URL_ENV: &str = "TABULARIS_AUDIT_URL";
const AUDIT_TOKEN_ENV: &str = "TABULARIS_AUDIT_TOKEN";
const AUDIT_TOKEN_KEYCHAIN_ENTRY: &str = "audit-ingest-token-v1";
const DEFAULT_SYNC_INTERVAL_SECS: u64 = 15;
const DEFAULT_BATCH_SIZE: usize = 500;
const DEFAULT_MAX_SEGMENT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 1_000_000;
const MANUAL_FLUSH_ACK_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditExecutionStatus {
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEvent {
    pub event_id: String,
    pub instance_id: String,
    pub connection_id: String,
    pub connection_name: String,
    pub database_account: String,
    pub database_name: String,
    pub sql_text: String,
    pub operation: String,
    pub sql_kind: String,
    pub started_epoch_us: i64,
    pub finished_epoch_us: i64,
    pub execution_time_ms: u64,
    pub execution_status: AuditExecutionStatus,
    pub affected_rows: u64,
    pub error_message: String,
    pub batch_id: String,
    pub statement_index: i64,
    pub transaction_context_id: String,
}

impl AuditEvent {
    pub fn new(
        target: &AuditTarget,
        sql_text: &str,
        started_epoch_us: i64,
        finished_epoch_us: i64,
        execution_status: AuditExecutionStatus,
        affected_rows: u64,
        error_message: impl Into<String>,
        batch_id: Option<&str>,
        statement_index: Option<usize>,
        transaction_context_id: Option<&str>,
    ) -> Self {
        Self::with_planned_id(
            Uuid::new_v4().to_string(),
            target,
            sql_text,
            started_epoch_us,
            finished_epoch_us,
            execution_status,
            affected_rows,
            error_message,
            batch_id,
            statement_index,
            transaction_context_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_planned_id(
        event_id: impl Into<String>,
        target: &AuditTarget,
        sql_text: &str,
        started_epoch_us: i64,
        finished_epoch_us: i64,
        execution_status: AuditExecutionStatus,
        affected_rows: u64,
        error_message: impl Into<String>,
        batch_id: Option<&str>,
        statement_index: Option<usize>,
        transaction_context_id: Option<&str>,
    ) -> Self {
        let elapsed_us = finished_epoch_us.saturating_sub(started_epoch_us);
        let operation = sql_operation(sql_text);
        Self {
            event_id: event_id.into(),
            instance_id: target.instance_id.clone(),
            connection_id: target.connection_id.clone(),
            connection_name: target.connection_name.clone(),
            database_account: target.database_account.clone(),
            database_name: target.database_name.clone(),
            sql_text: sql_text.to_string(),
            operation: operation.clone(),
            sql_kind: operation,
            started_epoch_us,
            finished_epoch_us,
            execution_time_ms: elapsed_us as u64 / 1_000,
            execution_status,
            affected_rows,
            error_message: error_message.into(),
            batch_id: batch_id.unwrap_or_default().to_string(),
            statement_index: statement_index.map(|value| value as i64).unwrap_or(-1),
            transaction_context_id: transaction_context_id.unwrap_or_default().to_string(),
        }
    }

    pub fn from_statement(
        event_id: &str,
        target: &AuditTarget,
        sql_text: &str,
        finished_epoch_us: i64,
        statement: &crate::models::BatchStatementResult,
        batch_id: Option<&str>,
        statement_index: usize,
        transaction_context_id: Option<&str>,
    ) -> Option<Self> {
        if !statement_was_executed(statement) {
            return None;
        }
        let (status, affected_rows, error_message) = match (&statement.result, &statement.error) {
            (Some(result), _) => (AuditExecutionStatus::Success, result.affected_rows, ""),
            (None, Some(error)) => (AuditExecutionStatus::Failed, 0, error.as_str()),
            (None, None) => return None,
        };
        let execution_time_ms = statement.execution_time_ms.unwrap_or_default().max(0.0);
        let elapsed_us = (execution_time_ms * 1_000.0).round() as i64;
        let mut event = Self::with_planned_id(
            event_id.to_string(),
            target,
            sql_text,
            finished_epoch_us.saturating_sub(elapsed_us),
            finished_epoch_us,
            status,
            affected_rows,
            error_message,
            batch_id,
            Some(statement_index),
            transaction_context_id,
        );
        event.execution_time_ms = execution_time_ms as u64;
        Some(event)
    }

    fn redacted_for_transport_with<F>(&self, mut pseudonym: F) -> Result<Self, String>
    where
        F: FnMut(&str, &str) -> Result<String, String>,
    {
        let mut event = self.clone();
        event.connection_id = pseudonym("connection", &self.connection_id)?;
        event.connection_name = pseudonym("connection_name", &self.connection_name)?;
        event.database_account = pseudonym("account", &self.database_account)?;
        event.database_name = pseudonym("database", &self.database_name)?;
        event.sql_text = crate::redaction::redact_sql_literals(&self.sql_text);
        event.error_message = crate::redaction::redact_sql_literals(&self.error_message);
        if !self.batch_id.is_empty() {
            event.batch_id = pseudonym("batch", &self.batch_id)?;
        }
        if !self.transaction_context_id.is_empty() {
            event.transaction_context_id = pseudonym("transaction", &self.transaction_context_id)?;
        }
        Ok(event)
    }

    fn redacted_for_transport(&self) -> Result<Self, String> {
        #[cfg(not(test))]
        {
            self.redacted_for_transport_with(crate::profile_crypto::pseudonymize)
        }
        #[cfg(test)]
        {
            self.redacted_for_transport_with(|label, value| {
                Ok(format!(
                    "{label}_{}",
                    hex::encode(sha2::Sha256::digest(value.as_bytes()))
                ))
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditTarget {
    pub instance_id: String,
    pub connection_id: String,
    pub connection_name: String,
    pub database_account: String,
    pub database_name: String,
}

impl AuditTarget {
    pub fn from_connection(connection: &SavedConnection, database: Option<&str>) -> Option<Self> {
        if !matches!(connection.params.driver.as_str(), "mysql" | "mariadb") {
            return None;
        }
        let instance_id = classify_instance(connection)?;
        Some(Self {
            instance_id: instance_id.to_string(),
            connection_id: connection.id.clone(),
            connection_name: connection.name.clone(),
            database_account: connection.params.username.clone().unwrap_or_default(),
            database_name: database
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| connection.params.database.primary())
                .to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AuditCursor {
    segment: u64,
    offset: u64,
}

#[derive(Debug, Deserialize)]
struct AuditIngestEnvelope {
    ok: bool,
    data: AuditIngestResult,
}

#[derive(Debug, Deserialize)]
struct AuditIngestResult {
    acknowledged: bool,
    #[serde(default)]
    acknowledged_event_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct PendingBatch {
    events: Vec<AuditEvent>,
    next_cursor: AuditCursor,
}

#[derive(Debug)]
pub struct AuditOutbox {
    root: PathBuf,
    max_segment_bytes: u64,
}

impl AuditOutbox {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        Self::with_segment_limit(root, DEFAULT_MAX_SEGMENT_BYTES)
    }

    fn with_segment_limit(root: PathBuf, max_segment_bytes: u64) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(Self {
            root,
            max_segment_bytes,
        })
    }

    fn cursor_path(&self) -> PathBuf {
        self.root.join("cursor.json")
    }

    fn segment_path(&self, segment: u64) -> PathBuf {
        self.root.join(format!("events-{segment:020}.ndjson"))
    }

    fn segment_numbers(&self) -> Result<Vec<u64>, String> {
        let mut segments = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(value) = name
                .strip_prefix("events-")
                .and_then(|value| value.strip_suffix(".ndjson"))
            else {
                continue;
            };
            if let Ok(segment) = value.parse::<u64>() {
                segments.push(segment);
            }
        }
        segments.sort_unstable();
        Ok(segments)
    }

    fn read_cursor(&self) -> Result<AuditCursor, String> {
        let path = self.cursor_path();
        if !path.exists() {
            return Ok(AuditCursor::default());
        }
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    }

    /// Writes the cursor atomically: a fresh temporary file is fsynced and then
    /// renamed over `cursor.json`, so a crash leaves either the complete old
    /// value or the complete new one. Truncating `cursor.json` in place would
    /// open a window where it is 0 bytes; `read_cursor` cannot deserialize that,
    /// which fails `append` as well as `pending` and stalls auditing for good.
    fn write_cursor(&self, cursor: &AuditCursor) -> Result<(), String> {
        let bytes = serde_json::to_vec(cursor).map_err(|error| error.to_string())?;
        let path = self.cursor_path();
        let temporary = self
            .root
            .join(format!(".cursor.tmp.{}", Uuid::new_v4().simple()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        fs::rename(&temporary, &path).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            error.to_string()
        })
    }

    pub fn append(&self, event: &AuditEvent) -> Result<(), String> {
        let mut record = serde_json::to_vec(event).map_err(|error| error.to_string())?;
        record.push(b'\n');
        let segments = self.segment_numbers()?;
        let cursor = self.read_cursor()?;
        let mut segment = segments
            .last()
            .copied()
            .unwrap_or(cursor.segment)
            .max(cursor.segment);
        let mut path = self.segment_path(segment);
        let current_size = path.metadata().map(|value| value.len()).unwrap_or(0);
        if current_size > 0 && current_size + record.len() as u64 > self.max_segment_bytes {
            segment += 1;
            path = self.segment_path(segment);
        }
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        file.write_all(&record).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())
    }

    fn seal_active_segment(&self) -> Result<(), String> {
        let segments = self.segment_numbers()?;
        let cursor = self.read_cursor()?;
        let segment = segments
            .last()
            .copied()
            .unwrap_or(cursor.segment)
            .max(cursor.segment);
        let path = self.segment_path(segment);
        if path.metadata().map(|value| value.len()).unwrap_or(0) == 0 {
            return Ok(());
        }
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.segment_path(segment.saturating_add(1)))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn pending(&self, limit: usize) -> Result<PendingBatch, String> {
        let cursor = self.read_cursor()?;
        let segments = self.segment_numbers()?;
        let last_segment = segments.last().copied();
        let mut events = Vec::new();
        let mut next = cursor.clone();
        for segment in segments
            .into_iter()
            .filter(|value| *value >= cursor.segment)
        {
            let path = self.segment_path(segment);
            let mut file = File::open(&path).map_err(|error| error.to_string())?;
            let file_length = file.metadata().map_err(|error| error.to_string())?.len();
            let start = if segment == cursor.segment {
                cursor.offset.min(file_length)
            } else {
                0
            };
            if file_length == 0 && Some(segment) == last_segment {
                next = AuditCursor { segment, offset: 0 };
                break;
            }
            file.seek(SeekFrom::Start(start))
                .map_err(|error| error.to_string())?;
            let mut reader = BufReader::new(file);
            let mut offset = start;
            loop {
                let mut line = String::new();
                let bytes = reader
                    .read_line(&mut line)
                    .map_err(|error| error.to_string())?;
                if bytes == 0 {
                    break;
                }
                offset += bytes as u64;
                if line.trim().is_empty() {
                    continue;
                }
                let event = serde_json::from_str::<AuditEvent>(&line)
                    .map_err(|error| format!("Audit outbox record is invalid: {error}"))?;
                events.push(event);
                next = AuditCursor { segment, offset };
                if events.len() >= limit {
                    return Ok(PendingBatch {
                        events,
                        next_cursor: next,
                    });
                }
            }
            next = AuditCursor {
                segment: segment.saturating_add(1),
                offset: 0,
            };
        }
        Ok(PendingBatch {
            events,
            next_cursor: next,
        })
    }

    fn acknowledge(&self, cursor: &AuditCursor) -> Result<(), String> {
        self.write_cursor(cursor)?;
        for segment in self.segment_numbers()? {
            if segment < cursor.segment {
                fs::remove_file(self.segment_path(segment)).map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct AuditState {
    outbox: AuditOutbox,
    gate: Mutex<()>,
}

impl AuditState {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        Ok(Self {
            outbox: AuditOutbox::new(root)?,
            gate: Mutex::new(()),
        })
    }

    pub fn append_blocking(&self, event: &AuditEvent) -> Result<(), String> {
        let _guard = self.gate.lock().map_err(|error| error.to_string())?;
        self.outbox.append(event)
    }

    async fn sync_once(
        &self,
        client: &reqwest::Client,
        url: &str,
        token: &str,
    ) -> Result<usize, String> {
        let batch = {
            let _guard = self.gate.lock().map_err(|error| error.to_string())?;
            self.outbox.seal_active_segment()?;
            self.outbox.pending(DEFAULT_BATCH_SIZE)?
        };
        if batch.events.is_empty() {
            return Ok(0);
        }
        let event_ids: HashSet<String> = batch
            .events
            .iter()
            .map(|event| event.event_id.clone())
            .collect();
        let transport_events = batch
            .events
            .iter()
            .map(AuditEvent::redacted_for_transport)
            .collect::<Result<Vec<_>, _>>()?;
        let response = client
            .post(url)
            .bearer_auth(token)
            .json(&serde_json::json!({ "events": transport_events }))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("Audit ingest returned HTTP {}", response.status()));
        }
        let content_length = response.content_length();
        if content_length.is_some_and(|value| value > MAX_RESPONSE_BYTES) {
            return Err("Audit ingest response exceeded the maximum size".to_string());
        }
        let body = response.bytes().await.map_err(|error| error.to_string())?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err("Audit ingest response exceeded the maximum size".to_string());
        }
        let envelope = serde_json::from_slice::<AuditIngestEnvelope>(&body)
            .map_err(|error| error.to_string())?;
        let acknowledged: HashSet<String> =
            envelope.data.acknowledged_event_ids.into_iter().collect();
        if !envelope.ok || !envelope.data.acknowledged || acknowledged != event_ids {
            return Err("Audit ingest did not acknowledge the complete batch".to_string());
        }
        let acknowledged_count = event_ids.len();
        let _guard = self.gate.lock().map_err(|error| error.to_string())?;
        self.outbox.acknowledge(&batch.next_cursor)?;
        Ok(acknowledged_count)
    }

    async fn sync_until_empty(
        &self,
        client: &reqwest::Client,
        url: &str,
        token: &str,
    ) -> Result<usize, String> {
        let mut acknowledged = 0;
        loop {
            let count = self.sync_once(client, url, token).await?;
            if count == 0 {
                return Ok(acknowledged);
            }
            acknowledged += count;
        }
    }
}

pub fn now_epoch_us() -> i64 {
    Utc::now().timestamp_micros()
}

pub fn new_event_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn audit_target(connection: &SavedConnection, database: Option<&str>) -> Option<AuditTarget> {
    AuditTarget::from_connection(connection, database)
}

pub fn append_required<R: Runtime>(app: &AppHandle<R>, event: &AuditEvent) -> Result<(), String> {
    let state = app.state::<Arc<AuditState>>();
    state.append_blocking(event)
}

pub fn statement_was_executed(statement: &crate::models::BatchStatementResult) -> bool {
    !statement.skipped.unwrap_or(false)
        && !statement
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with("Skipped because "))
        && (statement.result.is_some() || statement.error.is_some())
}

pub fn persist_statement_outcome<R: Runtime>(
    app: &AppHandle<R>,
    event_id: &str,
    target: &AuditTarget,
    sql_text: &str,
    statement: &crate::models::BatchStatementResult,
    batch_id: Option<&str>,
    statement_index: usize,
    transaction_context_id: Option<&str>,
) -> Result<bool, String> {
    let Some(event) = AuditEvent::from_statement(
        event_id,
        target,
        sql_text,
        now_epoch_us(),
        statement,
        batch_id,
        statement_index,
        transaction_context_id,
    ) else {
        return Ok(false);
    };
    append_required(app, &event)?;
    Ok(true)
}

fn configured_audit_token() -> Result<Option<String>, String> {
    if let Some(token) = crate::keychain_utils::get_private_value(AUDIT_TOKEN_KEYCHAIN_ENTRY)? {
        if !token.trim().is_empty() {
            return Ok(Some(token));
        }
    }
    let token = std::env::var(AUDIT_TOKEN_ENV).unwrap_or_default();
    if token.trim().is_empty() {
        return Ok(None);
    }
    crate::keychain_utils::set_private_value(AUDIT_TOKEN_KEYCHAIN_ENTRY, &token)?;
    Ok(Some(token))
}

pub async fn flush_pending<R: Runtime>(app: &AppHandle<R>) -> Result<usize, String> {
    let url = std::env::var(AUDIT_URL_ENV)
        .map_err(|_| format!("{AUDIT_URL_ENV} is required to flush the audit outbox"))?;
    let token = configured_audit_token()?
        .ok_or_else(|| "The audit token is not configured in the OS keychain".to_string())?;
    if url.trim().is_empty() {
        return Err("Audit endpoint must not be empty".to_string());
    }
    let state = app.state::<Arc<AuditState>>().inner().clone();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?;
    tokio::time::timeout(
        MANUAL_FLUSH_ACK_TIMEOUT,
        state.sync_until_empty(&client, &url, &token),
    )
    .await
    .map_err(|_| {
        format!(
            "Audit outbox was not fully acknowledged within {} seconds",
            MANUAL_FLUSH_ACK_TIMEOUT.as_secs()
        )
    })?
}

pub fn spawn_sync_worker<R: Runtime>(app: AppHandle<R>) {
    let url = std::env::var(AUDIT_URL_ENV).unwrap_or_default();
    if url.trim().is_empty() {
        log::info!("Tabularis audit sync is disabled because its endpoint is not configured");
        return;
    }
    let token = match configured_audit_token() {
        Ok(Some(token)) => token,
        Ok(None) => {
            log::info!(
                "Tabularis audit sync is disabled because its endpoint or token is not configured"
            );
            return;
        }
        Err(error) => {
            log::warn!(
                "Tabularis audit sync is disabled because the OS keychain is unavailable: {error}"
            );
            return;
        }
    };
    let state = app.state::<Arc<AuditState>>().inner().clone();
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            log::error!("Failed to create Tabularis audit HTTP client: {error}");
            return;
        }
    };
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(DEFAULT_SYNC_INTERVAL_SECS));
        loop {
            interval.tick().await;
            match state.sync_once(&client, &url, &token).await {
                Ok(0) => {}
                Ok(count) => log::info!("Tabularis audit acknowledged {count} event(s)"),
                Err(error) => log::warn!("Tabularis audit sync deferred: {error}"),
            }
        }
    });
}

fn classify_instance(connection: &SavedConnection) -> Option<&'static str> {
    match connection.params.audit_profile.as_deref() {
        Some("mysql-test") => return Some("mysql-test"),
        Some("mysql-prod") => return Some("mysql-prod"),
        Some(_) => return None,
        None => {}
    }
    audit_profile_from_name(&connection.name)
}

pub(crate) fn audit_profile_from_name(name: &str) -> Option<&'static str> {
    let normalized_name = normalize_identity(name);
    let mentions_mysql = normalized_name.contains("mysql");
    let matches_test =
        mentions_mysql && (normalized_name.contains("test") || normalized_name.contains("测试"));
    let matches_prod = mentions_mysql
        && (normalized_name.contains("prod")
            || normalized_name.contains("production")
            || normalized_name.contains("生产"));
    match (matches_test, matches_prod) {
        (true, false) => Some("mysql-test"),
        (false, true) => Some("mysql-prod"),
        _ => None,
    }
}

fn normalize_identity(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|character| !matches!(character, '_' | '-' | ' ' | '（' | '）' | '(' | ')'))
        .collect()
}

fn sql_operation(sql: &str) -> String {
    strip_leading_sql_comments(sql)
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("OTHER")
        .to_ascii_uppercase()
}

pub fn audit_root<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|path| path.join("audit_outbox"))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
