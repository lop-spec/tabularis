use crate::drivers::mysql;
use crate::models::ConnectionParams;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Executor, Row};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const HISTORY_VERSION: u32 = 1;
const MAX_HISTORY_FILES: usize = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryColumn {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryRow {
    /// Values are durable MySQL literals: either `NULL` or `X'...'`.
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryObject {
    /// `table` also covers views and table-owned indexes.
    pub kind: String,
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryStatement {
    pub id: String,
    pub index: usize,
    pub executed_at: String,
    pub sql: String,
    pub category: String,
    pub operation: String,
    pub objects: Vec<RecoveryObject>,
    pub affected_columns: Vec<String>,
    pub condition: Option<String>,
    pub columns: Vec<RecoveryColumn>,
    pub primary_key: Vec<String>,
    pub before_rows: Vec<RecoveryRow>,
    pub after_rows: Vec<RecoveryRow>,
    pub inverse_sql: Option<String>,
    pub exact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryRun {
    pub version: u32,
    pub run_id: String,
    pub short_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub connection_id: String,
    pub connection_name: String,
    pub database: String,
    pub target_identity: String,
    pub statements: Vec<RecoveryStatement>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryStatementSummary {
    pub id: String,
    pub index: usize,
    pub executed_at: String,
    pub sql: String,
    pub category: String,
    pub operation: String,
    pub schema: Option<String>,
    pub table: Option<String>,
    pub affected_columns: Vec<String>,
    pub condition: Option<String>,
    pub row_count: usize,
    pub exact: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryRunSummary {
    pub run_id: String,
    pub short_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub connection_id: String,
    pub connection_name: String,
    pub database: String,
    pub statement_count: usize,
    pub statements: Vec<RecoveryStatementSummary>,
}

/// One append-only line of a `.recovery.jsonl` run (after the header line,
/// which is the serialized [`RecoveryRun`] itself).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
enum RunRecord {
    Statement { statement: RecoveryStatement },
    Rewind { checkpoint: usize },
    Finish { status: String, finished_at: String },
}

pub struct RecoveryJournal {
    path: PathBuf,
    run: RecoveryRun,
    file: std::fs::File,
    synced_len: u64,
    poisoned: bool,
}

impl RecoveryJournal {
    pub fn create(
        connection_id: String,
        connection_name: String,
        database: String,
        target_identity: String,
    ) -> Result<Self, String> {
        let root = crate::paths::get_app_data_dir()
            .ok_or_else(|| "Could not resolve the Tabularis data directory".to_string())?;
        Self::create_in(
            &root,
            connection_id,
            connection_name,
            database,
            target_identity,
        )
    }

    fn create_in(
        root: &Path,
        connection_id: String,
        connection_name: String,
        database: String,
        target_identity: String,
    ) -> Result<Self, String> {
        let connection_segment = isolated_connection_segment(&connection_name, &connection_id);
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let directory = root
            .join("recovery-history")
            .join(connection_segment)
            .join(date);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Could not create recovery history directory: {error}"))?;

        let run_id = ulid::Ulid::new().to_string();
        let short_id = short_run_id(&run_id);
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        // Append-only JSONL: the header line is the run itself (statements
        // empty), each statement is one fsynced line, and `finalize` appends
        // a finish record. A run of `n` statements therefore writes O(n)
        // bytes instead of re-serializing every recorded row image per
        // statement. Legacy `.recovery.json` files remain readable.
        let path = directory.join(format!("{timestamp}-{run_id}.recovery.jsonl"));
        let run = RecoveryRun {
            version: HISTORY_VERSION,
            run_id,
            short_id,
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            status: "recording".to_string(),
            connection_id,
            connection_name,
            database,
            target_identity,
            statements: Vec::new(),
        };
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|error| format!("Could not create recovery history: {error}"))?;
        let mut journal = Self {
            path,
            run,
            file,
            synced_len: 0,
            poisoned: false,
        };
        let header = serde_json::to_vec(&journal.run)
            .map_err(|error| format!("Could not serialize recovery history header: {error}"))?;
        journal.append_line(header)?;
        Ok(journal)
    }

    /// Appends one JSON line with flush+fsync. On failure the file is
    /// truncated back to the last synced length so a torn line can never
    /// corrupt later appends.
    fn append_line(&mut self, mut line: Vec<u8>) -> Result<(), String> {
        if self.poisoned {
            return Err(
                "Recovery history is unusable after an earlier write failure".to_string(),
            );
        }
        line.push(b'\n');
        let outcome = crate::rollback_sql::run_blocking(|| {
            self.file
                .write_all(&line)
                .and_then(|()| self.file.flush())
                .and_then(|()| self.file.sync_all())
        });
        match outcome {
            Ok(()) => {
                self.synced_len += line.len() as u64;
                Ok(())
            }
            Err(error) => {
                if self.file.set_len(self.synced_len).is_err() {
                    self.poisoned = true;
                }
                Err(format!("Could not append to recovery history: {error}"))
            }
        }
    }

    fn append_record(&mut self, record: &RunRecord) -> Result<(), String> {
        let line = serde_json::to_vec(record)
            .map_err(|error| format!("Could not serialize recovery record: {error}"))?;
        self.append_line(line)
    }

    pub fn checkpoint(&self) -> usize {
        self.run.statements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.run.statements.is_empty()
    }

    pub fn add_statement(&mut self, mut statement: RecoveryStatement) -> Result<(), String> {
        statement.id = format!(
            "{}-{:03}",
            self.run.short_id,
            statement.index.saturating_add(1)
        );
        statement.executed_at = chrono::Utc::now().to_rfc3339();
        self.append_record(&RunRecord::Statement {
            statement: statement.clone(),
        })?;
        self.run.statements.push(statement);
        Ok(())
    }

    pub fn rewind_to(&mut self, checkpoint: usize) -> Result<(), String> {
        if checkpoint > self.run.statements.len() {
            return Err(format!(
                "Recovery journal checkpoint {checkpoint} exceeds {} recorded statements",
                self.run.statements.len()
            ));
        }
        if checkpoint == self.run.statements.len() {
            return Ok(());
        }
        self.append_record(&RunRecord::Rewind { checkpoint })?;
        self.run.statements.truncate(checkpoint);
        Ok(())
    }

    pub fn finalize(mut self) -> Result<PathBuf, String> {
        self.append_record(&RunRecord::Finish {
            status: "complete".to_string(),
            finished_at: chrono::Utc::now().to_rfc3339(),
        })?;
        Ok(self.path)
    }

    /// Marks the run as interrupted (e.g. a COMMIT whose outcome is unknown)
    /// instead of leaving it in "recording" forever. Interrupted runs stay
    /// visible and comparable — that is exactly the situation where the
    /// operator needs the recovery history most.
    pub fn interrupt(mut self, reason: &str) -> Result<PathBuf, String> {
        log::warn!(
            "Recovery run {} marked interrupted: {reason}",
            self.run.short_id
        );
        self.append_record(&RunRecord::Finish {
            status: "interrupted".to_string(),
            finished_at: chrono::Utc::now().to_rfc3339(),
        })?;
        Ok(self.path)
    }

    pub fn discard(self) -> Result<(), String> {
        let path = self.path.clone();
        drop(self.file);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "Could not discard recovery history {}: {error}",
                path.display()
            )),
        }
    }
}

/// Parses an append-only `.recovery.jsonl` run. Tolerates one trailing
/// partial line (a crash-torn append); earlier corruption is an error.
fn parse_jsonl_run(content: &str) -> Result<RecoveryRun, String> {
    let mut lines = content.lines().enumerate().peekable();
    let (_, header) = lines
        .next()
        .ok_or_else(|| "recovery history is empty".to_string())?;
    let mut run: RecoveryRun = serde_json::from_str(header)
        .map_err(|error| format!("unreadable recovery history header: {error}"))?;
    while let Some((_, line)) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<RunRecord>(line) {
            Ok(RunRecord::Statement { statement }) => run.statements.push(statement),
            Ok(RunRecord::Rewind { checkpoint }) => run.statements.truncate(checkpoint),
            Ok(RunRecord::Finish {
                status,
                finished_at,
            }) => {
                run.status = status;
                run.finished_at = Some(finished_at);
            }
            Err(error) => {
                if lines.peek().is_none() {
                    // The crash interrupted this very append; whatever it
                    // carried never finished committing to the journal.
                    break;
                }
                return Err(format!("corrupted recovery history line: {error}"));
            }
        }
    }
    Ok(run)
}

/// Closes crash-orphaned `.recovery.jsonl` runs (no finish record) by
/// appending an `interrupted` finish. Runs once at startup, before any new
/// batch can start recording, so it never races a live journal.
pub fn finalize_orphaned_recovery_runs(data_root: &Path) -> usize {
    let mut closed = 0usize;
    let Ok(files) = recovery_files(data_root) else {
        return 0;
    };
    for path in files {
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(run) = parse_jsonl_run(&content) else {
            continue;
        };
        if run.status != "recording" {
            continue;
        }
        let record = RunRecord::Finish {
            status: "interrupted".to_string(),
            finished_at: chrono::Utc::now().to_rfc3339(),
        };
        let Ok(mut line) = serde_json::to_vec(&record) else {
            continue;
        };
        line.push(b'\n');
        // A crash-torn partial tail line is dropped before appending: the
        // append it belonged to never finished, and leaving it mid-file would
        // read as corruption once the finish record follows it.
        let keep_len = if content.ends_with('\n') {
            content.len() as u64
        } else {
            content.rfind('\n').map(|idx| idx as u64 + 1).unwrap_or(0)
        };
        let outcome = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .and_then(|mut file| {
                file.set_len(keep_len)?;
                std::io::Seek::seek(&mut file, std::io::SeekFrom::End(0))?;
                file.write_all(&line)?;
                file.sync_all()
            });
        match outcome {
            Ok(()) => {
                log::warn!(
                    "Closed crash-orphaned recovery run as interrupted: {}",
                    path.display()
                );
                closed += 1;
            }
            Err(error) => {
                log::warn!(
                    "Could not close orphaned recovery run {}: {error}",
                    path.display()
                );
            }
        }
    }
    closed
}

/// Object extraction for a change statement, so that even SQL we cannot
/// invert (DROP/TRUNCATE/complex ALTER, or anything executed without
/// rollback protection) still lands in the journal with enough shape for the
/// backup-based restore to act on.
///
/// Lives in [`crate::recovery_objects`]; re-exported here because both this
/// module and the MySQL rollback guard call it by this path.
pub use crate::recovery_objects::parse_change_objects;

/// Journals change statements executed OUTSIDE rollback protection, so the
/// backup-based restore can still target the affected objects. Row images are
/// not captured (that is what protection is for) — these entries carry
/// `exact: false` and drive table-level restore from the backup connection.
pub async fn record_unprotected_changes(
    conn: &mut sqlx::MySqlConnection,
    connection_id: &str,
    connection_name: &str,
    database: &str,
    statements: &[(usize, &str)],
) -> Result<(), String> {
    if statements.is_empty() {
        return Ok(());
    }
    let probe = probe_instance(conn).await?;
    let mut journal = RecoveryJournal::create(
        connection_id.to_string(),
        connection_name.to_string(),
        database.to_string(),
        probe.server_key.clone(),
    )?;
    // `PREPARE s FROM @ddl` resolved here, so the later `EXECUTE s` inherits
    // the objects instead of journalling a statement with no target.
    let mut prepared: HashMap<String, Vec<RecoveryObject>> = HashMap::new();
    for (index, sql) in statements {
        let (operation, mut objects) = parse_change_objects(sql);
        // A dynamic statement names at most the routine it invokes, never the
        // tables that routine writes, so it is resolved even when the static
        // pass already produced something.
        if objects.is_empty() || crate::recovery_objects::dynamic_source(sql).is_some() {
            objects.extend(resolve_dynamic_objects(conn, database, sql, &mut prepared).await);
            objects.sort_by(|a, b| {
                (&a.kind, &a.schema, &a.name).cmp(&(&b.kind, &b.schema, &b.name))
            });
            objects.dedup();
        }
        if is_unprotected_non_recovery_operation(&operation) {
            continue;
        }
        journal.add_statement(RecoveryStatement {
            id: String::new(),
            index: *index,
            executed_at: String::new(),
            sql: sql.trim().to_string(),
            category: "unprotected".to_string(),
            operation,
            objects,
            affected_columns: Vec::new(),
            condition: None,
            columns: Vec::new(),
            primary_key: Vec::new(),
            before_rows: Vec::new(),
            after_rows: Vec::new(),
            inverse_sql: None,
            exact: false,
        })?;
    }
    if journal.is_empty() {
        journal.discard()?;
    } else {
        journal.finalize()?;
    }
    Ok(())
}

/// Names the objects behind a statement that carries none in its own text.
///
/// Runs on the same connection that just executed the batch
/// (`drivers/mysql/mod.rs`, `journal_unprotected_changes`), which is what
/// makes this possible at all: the user variable a `PREPARE` read from still
/// holds its value, and the routine a `CALL` invoked is still resolvable.
/// Everything issued here is read-only.
///
/// Best-effort by design — an empty result just leaves the statement flagged
/// as an unrecoverable conflict, exactly as before.
pub(crate) async fn resolve_dynamic_objects(
    conn: &mut sqlx::MySqlConnection,
    database: &str,
    sql: &str,
    prepared: &mut HashMap<String, Vec<RecoveryObject>>,
) -> Vec<RecoveryObject> {
    let Some(source) = crate::recovery_objects::dynamic_source(sql) else {
        return Vec::new();
    };

    match source {
        crate::recovery_objects::DynamicSource::Literal { statement, sql } => {
            let (_, objects) = parse_change_objects(&sql);
            prepared.insert(statement, objects.clone());
            objects
        }
        crate::recovery_objects::DynamicSource::UserVariable {
            statement,
            variable,
        } => {
            let objects = match read_user_variable(conn, &variable).await {
                Some(text) => parse_change_objects(&text).1,
                None => Vec::new(),
            };
            prepared.insert(statement, objects.clone());
            objects
        }
        crate::recovery_objects::DynamicSource::Execute { statement } => {
            prepared.get(&statement).cloned().unwrap_or_default()
        }
        crate::recovery_objects::DynamicSource::Call {
            schema,
            routine,
            string_args,
        } => {
            let routine_schema = if schema.is_empty() { database } else { &schema };
            let mut objects = Vec::new();
            // Helper routines take the target as a string argument
            // (`CALL add_column_if_missing('orders', …)`), so any argument
            // that names a real table is treated as touched.
            for arg in &string_args {
                if let Some(object) = match_existing_table(conn, database, arg).await {
                    objects.push(object);
                }
            }
            // A no-argument routine hides its targets in its body instead.
            if let Some(body) = read_routine_body(conn, routine_schema, &routine).await {
                objects.extend(objects_in_routine_body(&body, database));
            }
            objects.sort_by(|a, b| (&a.schema, &a.name).cmp(&(&b.schema, &b.name)));
            objects.dedup();
            objects
        }
    }
}

async fn read_user_variable(
    conn: &mut sqlx::MySqlConnection,
    variable: &str,
) -> Option<String> {
    // The name came out of the tokenizer as an identifier; re-quoting it
    // keeps a hostile name from breaking out of the SELECT.
    let row = conn
        .fetch_optional(sqlx::raw_sql(&format!(
            "SELECT @{}",
            quote_identifier(variable)
        )))
        .await
        .ok()??;
    mysql_text(&row, 0).ok().filter(|text| !text.is_empty())
}

/// Returns the argument as an object when it names a table that exists.
/// Accepts `db.table` as well as a bare name in the current database.
async fn match_existing_table(
    conn: &mut sqlx::MySqlConnection,
    database: &str,
    candidate: &str,
) -> Option<RecoveryObject> {
    let trimmed = candidate.trim().trim_matches('`');
    // Anything with whitespace or a quote is a DDL fragment, not a name.
    if trimmed.is_empty()
        || trimmed.len() > 64
        || trimmed.contains(|c: char| c.is_whitespace() || c == '`' || c == '(' || c == ',')
    {
        return None;
    }
    let (schema, table) = match trimmed.split_once('.') {
        Some((s, t)) => (s.to_string(), t.to_string()),
        None => (database.to_string(), trimmed.to_string()),
    };
    let row = conn
        .fetch_optional(
            sqlx::query(
                "SELECT 1 FROM information_schema.TABLES \
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? LIMIT 1",
            )
            .bind(&schema)
            .bind(&table),
        )
        .await
        .ok()??;
    let _ = row;
    Some(RecoveryObject {
        kind: crate::recovery_objects::KIND_TABLE.to_string(),
        schema,
        name: table,
    })
}

async fn read_routine_body(
    conn: &mut sqlx::MySqlConnection,
    schema: &str,
    routine: &str,
) -> Option<String> {
    let row = conn
        .fetch_optional(
            sqlx::query(
                "SELECT ROUTINE_DEFINITION FROM information_schema.ROUTINES \
                 WHERE ROUTINE_SCHEMA = ? AND ROUTINE_NAME = ? LIMIT 1",
            )
            .bind(schema)
            .bind(routine),
        )
        .await
        .ok()??;
    mysql_text(&row, 0).ok().filter(|body| !body.is_empty())
}

/// Objects named by the change statements inside a routine body.
///
/// The body is split on `;` rather than properly parsed: a routine that
/// builds its DDL with CONCAT still hides its target, but the statements
/// written out in full are recovered, which is better than nothing.
fn objects_in_routine_body(body: &str, database: &str) -> Vec<RecoveryObject> {
    let mut objects = Vec::new();
    for fragment in body.split(';') {
        let (_, mut found) = parse_change_objects(fragment);
        for object in &mut found {
            if object.schema.is_empty() && object.kind == crate::recovery_objects::KIND_TABLE {
                object.schema = database.to_string();
            }
        }
        objects.extend(found);
    }
    objects
}

#[tauri::command]
pub fn list_recovery_runs(
    connection_id: Option<String>,
    started_after: Option<String>,
    started_before: Option<String>,
    query: Option<String>,
) -> Result<Vec<RecoveryRunSummary>, String> {
    let Some(root) = crate::paths::get_app_data_dir() else {
        return Ok(Vec::new());
    };
    let started_after = parse_filter_time(started_after.as_deref(), "start")?;
    let started_before = parse_filter_time(started_before.as_deref(), "end")?;
    if matches!(
        (&started_after, &started_before),
        (Some(after), Some(before)) if after > before
    ) {
        return Err("Recovery start time must not be later than end time".to_string());
    }
    let mut summaries = Vec::new();
    for path in recovery_files(&root)? {
        let run = match read_run(&path) {
            Ok(run) => run,
            Err(error) => {
                log::warn!(
                    "Ignoring unreadable recovery history {}: {error}",
                    path.display()
                );
                continue;
            }
        };
        if connection_id
            .as_deref()
            .is_some_and(|expected| run.connection_id != expected)
        {
            continue;
        }
        let run_started =
            chrono::DateTime::parse_from_rfc3339(&run.started_at).map_err(|error| {
                format!("Invalid recovery run timestamp {}: {error}", run.started_at)
            })?;
        if started_after
            .as_ref()
            .is_some_and(|after| &run_started < after)
        {
            continue;
        }
        if started_before
            .as_ref()
            .is_some_and(|before| &run_started > before)
        {
            continue;
        }
        if let Some(summary) = run_summary_for_query(&run, query.as_deref()) {
            summaries.push(summary);
        }
    }
    summaries.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    Ok(summaries)
}

fn parse_filter_time(
    value: Option<&str>,
    label: &str,
) -> Result<Option<chrono::DateTime<chrono::FixedOffset>>, String> {
    value
        .map(|value| {
            chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|error| format!("Invalid recovery {label} time: {error}"))
        })
        .transpose()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySelection {
    pub run_ids: Vec<String>,
    #[serde(default)]
    pub statement_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCompareResponse {
    pub output_path: String,
    pub sql: String,
    pub generated_steps: usize,
    pub unchanged_rows: usize,
    pub conflicts: Vec<String>,
    pub exact: bool,
    pub target_instance: String,
    pub backup_instance: String,
}

#[derive(Debug, Clone)]
struct InstanceProbe {
    label: String,
    server_key: String,
    database: String,
}

#[derive(Debug, Clone)]
struct RecoverySqlStep {
    order: usize,
    sql: String,
    expected_affected_rows: Option<u64>,
    source: String,
}

#[derive(Debug, Clone)]
struct RowWork {
    schema: String,
    table: String,
    columns: Vec<RecoveryColumn>,
    primary_key: Vec<String>,
    affected_columns: BTreeSet<String>,
    compare_all_columns: bool,
    keys: BTreeSet<Vec<String>>,
    order: usize,
    source_ids: BTreeSet<String>,
}

pub async fn compare_and_generate(
    target_params: &ConnectionParams,
    backup_params: &ConnectionParams,
    connection_id: &str,
    selection: RecoverySelection,
) -> Result<RecoveryCompareResponse, String> {
    if selection.run_ids.is_empty() {
        return Err("Select at least one recovery record".to_string());
    }
    if !matches!(target_params.driver.as_str(), "mysql" | "mariadb")
        || !matches!(backup_params.driver.as_str(), "mysql" | "mariadb")
    {
        return Err(
            "Recovery comparison currently requires MySQL or MariaDB on both sides".to_string(),
        );
    }

    let root = crate::paths::get_app_data_dir()
        .ok_or_else(|| "Could not resolve the Tabularis data directory".to_string())?;
    let selected_run_ids: HashSet<_> = selection.run_ids.iter().cloned().collect();
    let selected_statement_ids: HashSet<_> = selection.statement_ids.iter().cloned().collect();
    let mut runs = Vec::new();
    for path in recovery_files(&root)? {
        let run = read_run(&path)?;
        if selected_run_ids.contains(&run.run_id) {
            if run.connection_id != connection_id {
                return Err(format!(
                    "Recovery run {} belongs to another connection",
                    run.short_id
                ));
            }
            // "interrupted" runs (COMMIT outcome unknown) are precisely the
            // ones an operator needs to compare; only a run still actively
            // "recording" is untrustworthy.
            if run.status != "complete" && run.status != "interrupted" {
                return Err(format!(
                    "Recovery run {} is still recording and cannot be compared",
                    run.short_id
                ));
            }
            runs.push(run);
        }
    }
    if runs.len() != selected_run_ids.len() {
        return Err("One or more selected recovery runs no longer exist".to_string());
    }
    runs.sort_by(|left, right| left.started_at.cmp(&right.started_at));
    let selected_databases = runs
        .iter()
        .map(|run| run.database.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    if selected_databases.len() != 1 {
        return Err("Select recovery runs from one target database at a time".to_string());
    }

    let mut statements = Vec::new();
    for run in &runs {
        for statement in &run.statements {
            if selected_statement_ids.is_empty() || selected_statement_ids.contains(&statement.id) {
                statements.push((run, statement));
            }
        }
    }
    if statements.is_empty() {
        return Err("The selection contains no DDL or DML statements".to_string());
    }
    if !selected_statement_ids.is_empty() && statements.len() != selected_statement_ids.len() {
        return Err(
            "One or more selected statement IDs do not belong to the selected runs".to_string(),
        );
    }

    let target_database = runs[0].database.clone();
    let mut target = mysql::acquire_mysql_conn(target_params, Some(&target_database)).await?;
    let mut backup = mysql::acquire_mysql_conn(backup_params, None).await?;
    let target_probe = probe_instance(&mut target).await?;
    let backup_probe = probe_instance(&mut backup).await?;
    for run in &runs {
        if !recorded_target_matches(&run.target_identity, &target_probe.server_key) {
            return Err(format!(
                "Recovery run {} was recorded on another target instance; the saved connection may have been repointed",
                run.short_id
            ));
        }
    }
    if target_probe.server_key == backup_probe.server_key
        && target_probe
            .database
            .eq_ignore_ascii_case(&backup_probe.database)
    {
        return Err(
            "Backup connection resolves to the same server and database as the target".to_string(),
        );
    }

    begin_read_only_snapshot(&mut target, "target").await?;
    if let Err(error) = begin_read_only_snapshot(&mut backup, "backup").await {
        let _ = (&mut *target).execute(sqlx::raw_sql("ROLLBACK")).await;
        return Err(error);
    }

    let comparison = compare_selected_statements(
        &mut target,
        &mut backup,
        &target_probe,
        &backup_probe,
        &runs,
        &statements,
    )
    .await;
    let target_rollback = (&mut *target).execute(sqlx::raw_sql("ROLLBACK")).await;
    let backup_rollback = (&mut *backup).execute(sqlx::raw_sql("ROLLBACK")).await;
    if let Err(error) = target_rollback {
        return Err(format!(
            "Could not close target read-only snapshot: {error}"
        ));
    }
    if let Err(error) = backup_rollback {
        return Err(format!(
            "Could not close backup read-only snapshot: {error}"
        ));
    }

    let (steps, unchanged_rows, conflicts) = comparison?;
    let sql = render_recovery_sql(
        connection_id,
        &runs,
        &target_probe,
        &backup_probe,
        &steps,
        &conflicts,
    );
    let output_path = write_recovery_sql(&root, connection_id, &runs[0].connection_name, &sql)?;
    Ok(RecoveryCompareResponse {
        output_path: output_path.to_string_lossy().to_string(),
        sql,
        generated_steps: steps.len(),
        unchanged_rows,
        exact: conflicts.is_empty(),
        conflicts,
        target_instance: target_probe.label,
        backup_instance: backup_probe.label,
    })
}

/// Normalizes a recorded target identity into the `uuid:`/`uid:`/`host:`
/// key format so the offline precheck stage can assert it.
fn normalized_identity_key(recorded: &str) -> String {
    if let Some(uuid) = recorded.strip_prefix("MySQL server UUID ") {
        return format!("uuid:{uuid}");
    }
    if let Some(uid) = recorded.strip_prefix("MariaDB server UID ") {
        return format!("uid:{uid}");
    }
    if recorded.starts_with("uuid:")
        || recorded.starts_with("uid:")
        || recorded.starts_with("host:")
    {
        return recorded.to_string();
    }
    format!("host:{recorded}")
}

fn offline_pk_positions(statement: &RecoveryStatement) -> Result<Vec<usize>, String> {
    if statement.primary_key.is_empty() {
        return Err("statement has no recorded primary key".to_string());
    }
    statement
        .primary_key
        .iter()
        .map(|key| {
            statement
                .columns
                .iter()
                .position(|column| column.name.eq_ignore_ascii_case(key))
                .ok_or_else(|| format!("primary-key column {key} is not in the recorded row image"))
        })
        .collect()
}

fn offline_row_check(statement: &RecoveryStatement, row: &RecoveryRow) -> Result<(), String> {
    if row.values.len() != statement.columns.len() {
        return Err(format!(
            "row image width {} does not match its {} recorded columns",
            row.values.len(),
            statement.columns.len()
        ));
    }
    Ok(())
}

fn offline_pk_condition(
    statement: &RecoveryStatement,
    positions: &[usize],
    row: &RecoveryRow,
) -> String {
    format!(
        "({})",
        positions
            .iter()
            .map(|position| {
                format!(
                    "CAST({} AS BINARY) <=> {}",
                    quote_identifier(&statement.columns[*position].name),
                    row.values[*position]
                )
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    )
}

fn offline_full_row_guard(statement: &RecoveryStatement, row: &RecoveryRow) -> String {
    statement
        .columns
        .iter()
        .zip(&row.values)
        .map(|(column, value)| {
            format!(
                "CAST({} AS BINARY) <=> {}",
                quote_identifier(&column.name),
                value
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn offline_table_name(statement: &RecoveryStatement) -> Result<String, String> {
    let object = statement
        .objects
        .iter()
        .find(|object| object.kind == "table")
        .ok_or_else(|| "statement has no recorded table object".to_string())?;
    Ok(format!(
        "{}.{}",
        quote_identifier(&object.schema),
        quote_identifier(&object.name)
    ))
}

fn offline_restore_update(
    statement: &RecoveryStatement,
    positions: &[usize],
    before: &RecoveryRow,
    after: &RecoveryRow,
) -> Result<String, String> {
    let assignments = statement
        .columns
        .iter()
        .enumerate()
        .filter(|(index, _)| !positions.contains(index))
        .map(|(index, column)| {
            format!(
                "{} = {}",
                quote_identifier(&column.name),
                restoration_literal(column, &before.values[index])
            )
        })
        .collect::<Vec<_>>();
    if assignments.is_empty() {
        return Err("recorded row image has no writable non-key columns".to_string());
    }
    Ok(format!(
        "UPDATE {} SET {} WHERE {} AND ({}) LIMIT 1",
        offline_table_name(statement)?,
        assignments.join(", "),
        offline_pk_condition(statement, positions, after),
        offline_full_row_guard(statement, after)
    ))
}

fn offline_restore_delete(
    statement: &RecoveryStatement,
    positions: &[usize],
    after: &RecoveryRow,
) -> Result<String, String> {
    Ok(format!(
        "DELETE FROM {} WHERE {} AND ({}) LIMIT 1",
        offline_table_name(statement)?,
        offline_pk_condition(statement, positions, after),
        offline_full_row_guard(statement, after)
    ))
}

fn offline_restore_insert(
    statement: &RecoveryStatement,
    before: &RecoveryRow,
) -> Result<String, String> {
    Ok(format!(
        "INSERT INTO {} ({}) VALUES ({})",
        offline_table_name(statement)?,
        statement
            .columns
            .iter()
            .map(|column| quote_identifier(&column.name))
            .collect::<Vec<_>>()
            .join(", "),
        statement
            .columns
            .iter()
            .zip(&before.values)
            .map(|(column, value)| restoration_literal(column, value))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Builds the inverse steps for one exact statement, newest-row-first, purely
/// from its recorded before/after images.
fn offline_statement_steps(
    statement: &RecoveryStatement,
    order: &mut usize,
) -> Result<(Vec<RecoverySqlStep>, usize), String> {
    let mut steps = Vec::new();
    let mut unchanged = 0usize;
    let mut push = |sql: String, expected: Option<u64>, order: &mut usize| {
        *order += 1;
        steps.push(RecoverySqlStep {
            order: *order,
            sql,
            expected_affected_rows: expected,
            source: statement.id.clone(),
        });
    };

    if statement.category.eq_ignore_ascii_case("ddl") {
        let inverse = statement
            .inverse_sql
            .as_deref()
            .ok_or_else(|| "DDL statement has no recorded inverse".to_string())?;
        push(inverse.to_string(), None, order);
        return Ok((steps, unchanged));
    }

    for row in statement.before_rows.iter().chain(&statement.after_rows) {
        offline_row_check(statement, row)?;
    }
    if statement.before_rows.is_empty() && statement.after_rows.is_empty() {
        // e.g. an INSERT ... SELECT that matched nothing: nothing to undo.
        return Ok((steps, unchanged));
    }
    let positions = offline_pk_positions(statement)?;
    let key_of = |row: &RecoveryRow| -> Vec<String> {
        positions
            .iter()
            .map(|position| row.values[*position].clone())
            .collect()
    };

    match statement.operation.as_str() {
        "insert" => {
            for row in statement.after_rows.iter().rev() {
                push(offline_restore_delete(statement, &positions, row)?, Some(1), order);
            }
        }
        "delete" => {
            for row in statement.before_rows.iter().rev() {
                push(offline_restore_insert(statement, row)?, Some(1), order);
            }
        }
        "update" | "upsert" => {
            let mut before_by_key: BTreeMap<Vec<String>, &RecoveryRow> = BTreeMap::new();
            for row in &statement.before_rows {
                if before_by_key.insert(key_of(row), row).is_some() {
                    return Err("recorded before-images contain a duplicate key".to_string());
                }
            }
            let mut matched = 0usize;
            for row in statement.after_rows.iter().rev() {
                match before_by_key.get(&key_of(row)) {
                    Some(before) => {
                        matched += 1;
                        if before.values == row.values {
                            unchanged += 1;
                        } else {
                            push(
                                offline_restore_update(statement, &positions, before, row)?,
                                Some(1),
                                order,
                            );
                        }
                    }
                    None if statement.operation == "upsert" => {
                        push(offline_restore_delete(statement, &positions, row)?, Some(1), order);
                    }
                    None => {
                        return Err(
                            "an after-image has no matching before-image".to_string()
                        );
                    }
                }
            }
            if matched != statement.before_rows.len() {
                return Err("a before-image has no matching after-image".to_string());
            }
        }
        other => {
            return Err(format!("operation {other} has no offline inverse"));
        }
    }
    Ok((steps, unchanged))
}

fn render_offline_recovery_sql(
    connection_id: &str,
    runs: &[RecoveryRun],
    steps: &[RecoverySqlStep],
    conflicts: &[String],
) -> String {
    let mut output = String::new();
    output.push_str("-- Tabularis offline rollback SQL\n");
    output.push_str(
        "-- Generated purely from the recovery journal's recorded before/after row images;\n",
    );
    output.push_str("-- no database instance was queried during generation.\n");
    output.push_str("-- Row guards assert the current row still matches the recorded after-image;\n");
    output.push_str("-- a later change to the same row makes its guarded statement affect 0 rows.\n");
    output.push_str("-- DML ends in ROLLBACK by default; replace it with COMMIT only after every\n");
    output.push_str("-- ROW_COUNT result equals its expected_affected_rows.\n");
    output.push_str(&format!(
        "-- Target connection alias: {}\n",
        single_line_comment(&runs[0].connection_name)
    ));
    output.push_str(&format!(
        "-- Target connection ID: {}\n",
        single_line_comment(connection_id)
    ));
    output.push_str(&format!(
        "-- Selected recovery Run IDs: {}\n",
        runs.iter()
            .map(|run| run.short_id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    for conflict in conflicts {
        output.push_str(&format!(
            "-- SKIPPED (no offline inverse): {}\n",
            single_line_comment(conflict)
        ));
    }
    output.push('\n');

    let probe = InstanceProbe {
        label: runs[0].connection_name.clone(),
        server_key: normalized_identity_key(&runs[0].target_identity),
        database: runs[0].database.clone(),
    };
    append_identity_stage(
        &mut output,
        "Stage A: identity precheck (read-only; run separately)",
        &probe,
    );

    let mut transaction_open = false;
    for step in steps {
        let transactional = step.expected_affected_rows.is_some();
        if transactional && !transaction_open {
            output.push_str("-- ===== Stage B: DML rollback (one transaction; run separately) =====\n");
            output.push_str("SET time_zone = '+00:00';\n");
            output.push_str("START TRANSACTION;\n\n");
            transaction_open = true;
        } else if !transactional && transaction_open {
            append_transaction_finish(&mut output);
            transaction_open = false;
        }
        output.push_str(&format!(
            "-- Rollback step {} (statement {})\n",
            step.order, step.source
        ));
        if let Some(expected) = step.expected_affected_rows {
            append_direct_statement(&mut output, &step.sql);
            output.push_str(&format!(
                "SELECT ROW_COUNT() AS rollback_step_{}_affected_rows, {expected} AS expected_affected_rows;\n",
                step.order
            ));
        } else {
            output.push_str(
                "-- MANUAL DDL: implicit commit boundary; review and execute separately.\n",
            );
            for line in step.sql.lines() {
                output.push_str("-- TABULARIS_MANUAL_DDL: ");
                output.push_str(line);
                output.push('\n');
            }
        }
        output.push('\n');
    }
    if transaction_open {
        append_transaction_finish(&mut output);
    }
    if steps.is_empty() {
        output.push_str("-- No reversible steps were generated.\n");
    }
    output
}

/// Generates rollback SQL for selected exact statements straight from their
/// recorded row images — no target probe, no backup instance. Statements
/// without an exact image are reported as conflicts and need the
/// backup-instance restore instead.
pub fn generate_offline_recovery_sql_in(
    root: &Path,
    connection_id: &str,
    selection: &RecoverySelection,
) -> Result<RecoveryCompareResponse, String> {
    if selection.run_ids.is_empty() {
        return Err("Select at least one recovery record".to_string());
    }
    let selected_run_ids: HashSet<_> = selection.run_ids.iter().cloned().collect();
    let selected_statement_ids: HashSet<_> = selection.statement_ids.iter().cloned().collect();
    let mut runs = Vec::new();
    for path in recovery_files(root)? {
        let run = read_run(&path)?;
        if !selected_run_ids.contains(&run.run_id) {
            continue;
        }
        if run.connection_id != connection_id {
            return Err(format!(
                "Recovery run {} belongs to another connection",
                run.short_id
            ));
        }
        if run.status != "complete" && run.status != "interrupted" {
            return Err(format!(
                "Recovery run {} is still recording and cannot be used",
                run.short_id
            ));
        }
        runs.push(run);
    }
    if runs.len() != selected_run_ids.len() {
        return Err("One or more selected recovery runs no longer exist".to_string());
    }
    runs.sort_by(|left, right| left.started_at.cmp(&right.started_at));

    let mut steps = Vec::new();
    let mut conflicts = Vec::new();
    let mut unchanged_rows = 0usize;
    let mut selected_count = 0usize;
    let mut order = 0usize;
    // Newest first: later changes must be undone before earlier ones.
    for run in runs.iter().rev() {
        for statement in run.statements.iter().rev() {
            if !selected_statement_ids.is_empty()
                && !selected_statement_ids.contains(&statement.id)
            {
                continue;
            }
            selected_count += 1;
            if !statement.exact {
                conflicts.push(format!(
                    "{}: executed without exact protection — use the backup-instance restore (SQL: {})",
                    statement.id,
                    statement.sql.chars().take(120).collect::<String>()
                ));
                continue;
            }
            match offline_statement_steps(statement, &mut order) {
                Ok((statement_steps, unchanged)) => {
                    unchanged_rows += unchanged;
                    steps.extend(statement_steps);
                }
                Err(reason) => {
                    conflicts.push(format!("{}: {reason}", statement.id));
                }
            }
        }
    }
    if selected_count == 0 {
        return Err("The selection contains no statements".to_string());
    }
    if !selected_statement_ids.is_empty() && selected_count != selected_statement_ids.len() {
        return Err(
            "One or more selected statement IDs do not belong to the selected runs".to_string(),
        );
    }
    if steps.is_empty() && conflicts.is_empty() && unchanged_rows == 0 {
        return Err("The selection contains no reversible row images".to_string());
    }

    let sql = render_offline_recovery_sql(connection_id, &runs, &steps, &conflicts);
    let output_path = write_recovery_sql(root, connection_id, &runs[0].connection_name, &sql)?;
    Ok(RecoveryCompareResponse {
        output_path: output_path.to_string_lossy().to_string(),
        sql,
        generated_steps: steps.len(),
        unchanged_rows,
        exact: conflicts.is_empty(),
        conflicts,
        target_instance: format!(
            "{} · {}",
            runs[0].connection_name, runs[0].target_identity
        ),
        backup_instance: "recorded row images (offline)".to_string(),
    })
}

/// Tauri command: offline rollback generation from recorded row images.
#[tauri::command]
pub fn generate_offline_recovery_sql(
    connection_id: String,
    selection: RecoverySelection,
) -> Result<RecoveryCompareResponse, String> {
    let root = crate::paths::get_app_data_dir()
        .ok_or_else(|| "Could not resolve the Tabularis data directory".to_string())?;
    generate_offline_recovery_sql_in(&root, &connection_id, &selection)
}

async fn compare_selected_statements(
    target: &mut sqlx::MySqlConnection,
    backup: &mut sqlx::MySqlConnection,
    _target_probe: &InstanceProbe,
    backup_probe: &InstanceProbe,
    runs: &[RecoveryRun],
    statements: &[(&RecoveryRun, &RecoveryStatement)],
) -> Result<(Vec<RecoverySqlStep>, usize, Vec<String>), String> {
    let mut row_work: BTreeMap<(String, String), RowWork> = BTreeMap::new();
    let mut ddl_statements = Vec::new();
    // Objects touched by statements with no exact image (DROP/TRUNCATE,
    // anything run without protection): restored table-by-table from the
    // backup connection instead of being silently skipped.
    // Per table: the statement ids that touched it, and the verbs they used.
    let mut table_restores: BTreeMap<(String, String), (BTreeSet<String>, BTreeSet<String>)> =
        BTreeMap::new();
    // Views, routines, triggers and events restore from their backup
    // definition (`SHOW CREATE …`) rather than by comparing rows.
    let mut routine_restores: BTreeMap<(String, String, String), BTreeSet<String>> =
        BTreeMap::new();
    // Databases that have to exist again before their tables can be restored.
    let mut database_recreates: BTreeSet<String> = BTreeSet::new();
    let mut restore_conflicts: Vec<String> = Vec::new();
    let base_database = &runs[0].database;

    for (sequence, (_, statement)) in statements.iter().copied().enumerate() {
        if !statement.exact {
            if statement.objects.is_empty() {
                restore_conflicts.push(format!(
                    "{}: no exact image and no recognizable object — restore its scope manually (SQL: {})",
                    statement.id,
                    statement.sql.chars().take(120).collect::<String>()
                ));
            }
            for object in &statement.objects {
                match object.kind.as_str() {
                    "table" => {
                        let schema = if object.schema.is_empty() {
                            runs[0].database.clone()
                        } else {
                            object.schema.clone()
                        };
                        let entry = table_restores.entry((schema, object.name.clone())).or_default();
                        entry.0.insert(statement.id.clone());
                        // Whether "missing from the backup" means "this
                        // statement created it" depends on the verb: for
                        // CREATE it does, for TRUNCATE/DELETE/ALTER it means
                        // the backup is incomplete, and dropping the table
                        // would destroy what we were asked to restore.
                        entry.1.insert(statement.operation.clone());
                    }
                    "database" => {
                        // A dropped database is restorable: recreate it, then
                        // put every table the backup still holds back into it.
                        // Creating one is not auto-reversed — emitting DROP
                        // DATABASE would be a far bigger action than the
                        // operator asked for.
                        if statement.operation.starts_with("drop ") {
                            let tables = backup_database_tables(
                                backup,
                                &mapped_backup_schema(
                                    &object.schema,
                                    base_database,
                                    &backup_probe.database,
                                ),
                            )
                            .await;
                            match tables {
                                Ok(tables) if !tables.is_empty() => {
                                    database_recreates.insert(object.schema.clone());
                                    for table in tables {
                                        let entry = table_restores
                                            .entry((object.schema.clone(), table))
                                            .or_default();
                                        entry.0.insert(statement.id.clone());
                                        entry.1.insert(statement.operation.clone());
                                    }
                                }
                                Ok(_) => restore_conflicts.push(format!(
                                    "{}: dropped database {} has no tables in the backup — nothing to restore beyond recreating it",
                                    statement.id, object.schema
                                )),
                                Err(error) => restore_conflicts.push(format!(
                                    "{}: could not list the backup copy of database {} ({error}) — restore it from a dump",
                                    statement.id, object.schema
                                )),
                            }
                        } else {
                            restore_conflicts.push(format!(
                                "{}: {} on database {} is not auto-reversed — review whether the database should be dropped or altered back",
                                statement.id, statement.operation, object.schema
                            ));
                        }
                    }
                    kind @ ("view" | "procedure" | "function" | "trigger" | "event") => {
                        // A CALL names the routine it invoked, not one it
                        // changed. Restoring it would DROP and recreate a
                        // live helper procedure that was never modified —
                        // the tables it wrote are the actual targets, and
                        // they arrive separately as "table" objects.
                        if statement.operation == "call" {
                            continue;
                        }
                        let schema = if object.schema.is_empty() {
                            runs[0].database.clone()
                        } else {
                            object.schema.clone()
                        };
                        routine_restores
                            .entry((kind.to_string(), schema, object.name.clone()))
                            .or_default()
                            .insert(statement.id.clone());
                    }
                    // Session-scoped: gone the moment the connection closed,
                    // so there is nothing to put back.
                    "temporary table" => {}
                    // A kind with no restore path must be reported, never
                    // dropped: silence here reads as "nothing to recover".
                    other => restore_conflicts.push(format!(
                        "{}: touches {} {}.{}, which has no automated restore path — recreate it manually",
                        statement.id, other, object.schema, object.name
                    )),
                }
            }
            let _ = sequence;
            continue;
        }
        if statement.category == "ddl" {
            ddl_statements.push((sequence, statement));
            continue;
        }
        if statement.category != "dml" {
            continue;
        }
        let object = statement
            .objects
            .first()
            .ok_or_else(|| format!("Statement {} has no target object", statement.id))?;
        let map_key = (object.schema.clone(), object.name.clone());
        let entry = row_work.entry(map_key).or_insert_with(|| RowWork {
            schema: object.schema.clone(),
            table: object.name.clone(),
            columns: statement.columns.clone(),
            primary_key: statement.primary_key.clone(),
            affected_columns: BTreeSet::new(),
            compare_all_columns: false,
            keys: BTreeSet::new(),
            order: sequence,
            source_ids: BTreeSet::new(),
        });
        if entry.columns != statement.columns || entry.primary_key != statement.primary_key {
            return Err(format!(
                "Table metadata changed across selected statements for {}.{}",
                object.schema, object.name
            ));
        }
        entry.order = entry.order.max(sequence);
        entry.source_ids.insert(statement.id.clone());
        entry.compare_all_columns |= statement.operation != "update";
        entry.affected_columns.extend(
            statement
                .affected_columns
                .iter()
                .map(|name| name.to_ascii_lowercase()),
        );
        for row in statement.before_rows.iter().chain(&statement.after_rows) {
            entry
                .keys
                .insert(row_key(&statement.columns, &statement.primary_key, row)?);
        }
    }

    let mut steps = Vec::new();
    let mut unchanged_rows = 0;
    let mut conflicts = Vec::new();
    for work in row_work.values() {
        // A table already scheduled for a full restore is rebuilt from the
        // backup wholesale. Row-level steps on top of it would run against
        // rows the restore just replaced, so their full-row guards match
        // nothing, report 0 affected rows against a non-zero expectation, and
        // read as a failed recovery.
        if table_restores.contains_key(&(work.schema.clone(), work.table.clone())) {
            conflicts.push(format!(
                "{}.{}: row-level steps skipped because the whole table is restored from the backup",
                work.schema, work.table
            ));
            continue;
        }
        let backup_schema =
            mapped_backup_schema(&work.schema, base_database, &backup_probe.database);
        // One round-trip per batch instead of two per key: thousands of rows
        // would otherwise mean thousands of sequential single-row SELECTs.
        let keys: Vec<&Vec<String>> = work.keys.iter().collect();
        // A table the backup cannot answer for (renamed column, missing
        // table) is reported and skipped. Propagating the error would throw
        // away every other table's recovery SQL and write no file at all.
        let rows = match (
            fetch_rows_batch(target, &work.schema, work, &keys).await,
            fetch_rows_batch(backup, &backup_schema, work, &keys).await,
        ) {
            (Ok(current), Ok(desired)) => Some((current, desired)),
            (Err(error), _) | (_, Err(error)) => {
                conflicts.push(format!(
                    "{}.{}: could not be compared against the backup ({error}) — restore this table from a dump",
                    work.schema, work.table
                ));
                None
            }
        };
        let Some((current_rows, desired_rows)) = rows else {
            continue;
        };
        for key in &work.keys {
            let current = current_rows.get(key).cloned();
            let desired = desired_rows.get(key).cloned();
            let source = work
                .source_ids
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            match (current, desired) {
                (None, None) => unchanged_rows += 1,
                (Some(current), Some(desired)) => {
                    let compare_indexes = comparable_indexes(work);
                    let changed = compare_indexes
                        .into_iter()
                        .filter(|index| current.values[*index] != desired.values[*index])
                        .collect::<Vec<_>>();
                    if changed.is_empty() {
                        unchanged_rows += 1;
                        continue;
                    }
                    let sql = build_update_sql(work, &current, &desired, &changed)?;
                    steps.push(RecoverySqlStep {
                        order: work.order,
                        sql,
                        expected_affected_rows: Some(1),
                        source,
                    });
                }
                (None, Some(desired)) => {
                    steps.push(RecoverySqlStep {
                        order: work.order,
                        sql: build_insert_sql(work, &desired)?,
                        expected_affected_rows: Some(1),
                        source,
                    });
                }
                (Some(current), None) => {
                    steps.push(RecoverySqlStep {
                        order: work.order,
                        sql: build_delete_sql(work, &current)?,
                        expected_affected_rows: Some(1),
                        source,
                    });
                }
            }
        }
    }

    for (sequence, statement) in ddl_statements {
        let mut differs = statement.objects.is_empty();
        for object in &statement.objects {
            let backup_object = mapped_backup_object(object, base_database, &backup_probe.database);
            let current_definition = read_object_definition(target, object).await?;
            let backup_definition = read_object_definition(backup, &backup_object).await?;
            if current_definition != backup_definition {
                differs = true;
            }
        }
        if !differs {
            continue;
        }
        match &statement.inverse_sql {
            Some(inverse) => steps.push(RecoverySqlStep {
                order: sequence,
                sql: inverse.clone(),
                expected_affected_rows: None,
                source: statement.id.clone(),
            }),
            None => conflicts.push(format!(
                "{} has a schema difference but no exact recorded inverse",
                statement.id
            )),
        }
    }

    // Table-level restore from the backup connection for everything without
    // an exact image. Emitted with the highest order so structure comes back
    // before any row-level fixes run (steps execute in descending order).
    conflicts.extend(restore_conflicts);
    let same_instance = _target_probe.server_key == backup_probe.server_key;
    let mut restore_order = usize::MAX;

    // Ordered ahead of every table restore, since the tables land inside it.
    for database in &database_recreates {
        steps.push(RecoverySqlStep {
            order: restore_order,
            sql: format!("CREATE DATABASE IF NOT EXISTS {}", quote_identifier(database)),
            expected_affected_rows: None,
            source: format!("recreate dropped database {database}"),
        });
        restore_order = restore_order.saturating_sub(1);
    }
    for ((schema, table), (source_ids, operations)) in &table_restores {
        let source = source_ids.iter().cloned().collect::<Vec<_>>().join(", ");
        // Only a statement that creates the table justifies restoring it by
        // dropping it.
        let created_here = operations
            .iter()
            .all(|op| op.starts_with("create table") || op.starts_with("create temporary"));
        match build_table_restore(
            target,
            backup,
            base_database,
            &backup_probe.database,
            same_instance,
            schema,
            table,
            restore_order,
            &source,
            created_here,
        )
        .await
        {
            Ok((mut restore_steps, mut restore_notes)) => {
                steps.append(&mut restore_steps);
                conflicts.append(&mut restore_notes);
            }
            Err(error) => conflicts.push(format!(
                "{}.{} could not be prepared for backup restore: {error} (source: {source})",
                schema, table
            )),
        }
    }

    for ((kind, schema, name), source_ids) in &routine_restores {
        let source = source_ids.iter().cloned().collect::<Vec<_>>().join(", ");
        match build_routine_restore(
            backup,
            base_database,
            &backup_probe.database,
            kind,
            schema,
            name,
            restore_order,
            &source,
        )
        .await
        {
            Ok(mut restore_steps) => steps.append(&mut restore_steps),
            Err(error) => conflicts.push(format!(
                "{} {}.{} could not be prepared for backup restore: {error} (source: {source})",
                kind, schema, name
            )),
        }
        restore_order = restore_order.saturating_sub(1);
    }

    steps.sort_by(|left, right| right.order.cmp(&left.order));
    Ok((steps, unchanged_rows, conflicts))
}

/// Base tables the backup still holds for a database, so a dropped database
/// can be rebuilt table by table. Views are excluded — they come back through
/// their own restore path.
async fn backup_database_tables(
    backup: &mut sqlx::MySqlConnection,
    backup_schema: &str,
) -> Result<Vec<String>, String> {
    let rows = backup
        .fetch_all(
            sqlx::query(
                "SELECT TABLE_NAME FROM information_schema.TABLES \
                 WHERE TABLE_SCHEMA = ? AND TABLE_TYPE = 'BASE TABLE' \
                 ORDER BY TABLE_NAME",
            )
            .bind(backup_schema),
        )
        .await
        .map_err(|error| error.to_string())?;
    rows.iter().map(|row| mysql_text(row, 0)).collect()
}

/// Rebuilds a view, routine, trigger or event from the backup's definition.
///
/// These carry no rows, so there is nothing to compare — the definition in
/// the backup either exists (recreate it) or does not (drop it on the
/// target). Definitions that require an internal statement delimiter are
/// reported as conflicts because recovery output intentionally avoids
/// `DELIMITER` directives.
#[allow(clippy::too_many_arguments)]
async fn build_routine_restore(
    backup: &mut sqlx::MySqlConnection,
    base_database: &str,
    backup_database: &str,
    kind: &str,
    schema: &str,
    name: &str,
    order: usize,
    source: &str,
) -> Result<Vec<RecoverySqlStep>, String> {
    let backup_schema = mapped_backup_schema(schema, base_database, backup_database);
    let qualified = format!(
        "{}.{}",
        quote_identifier(&backup_schema),
        quote_identifier(name)
    );
    // `SHOW CREATE` puts the statement in a different column per object kind.
    let (show, column) = match kind {
        "view" => (format!("SHOW CREATE VIEW {qualified}"), 1usize),
        "procedure" => (format!("SHOW CREATE PROCEDURE {qualified}"), 2),
        "function" => (format!("SHOW CREATE FUNCTION {qualified}"), 2),
        "trigger" => (format!("SHOW CREATE TRIGGER {qualified}"), 2),
        "event" => (format!("SHOW CREATE EVENT {qualified}"), 3),
        other => return Err(format!("unsupported object kind {other}")),
    };
    let drop_sql = format!(
        "DROP {} IF EXISTS {}.{}",
        kind.to_uppercase(),
        quote_identifier(schema),
        quote_identifier(name)
    );

    let mut steps = vec![RecoverySqlStep {
        order,
        sql: drop_sql,
        expected_affected_rows: None,
        source: source.to_string(),
    }];

    // Absent from the backup means it did not exist before the change, so
    // dropping it on the target is the whole restore.
    let Some(row) = backup
        .fetch_optional(sqlx::raw_sql(&show))
        .await
        .map_err(|error| format!("backup has no usable copy: {error}"))?
    else {
        return Ok(steps);
    };
    let create_sql = mysql_text(&row, column)?;
    if create_sql.trim().is_empty() {
        return Ok(steps);
    }
    let create_sql = strip_show_create_definer(&retarget_schema_qualifiers(
        &create_sql,
        &backup_schema,
        schema,
    ));
    let definition = create_sql.trim();
    let definition_without_trailing_delimiter = definition
        .strip_suffix(';')
        .unwrap_or(definition)
        .trim_end();
    if contains_executable_semicolon(definition_without_trailing_delimiter) {
        return Err(format!(
            "{} {}.{} has a compound definition; SQL policy forbids DELIMITER, so recreate it manually",
            kind,
            quote_identifier(schema),
            quote_identifier(name)
        ));
    }

    steps.push(RecoverySqlStep {
        order,
        sql: create_sql,
        expected_affected_rows: None,
        source: source.to_string(),
    });
    Ok(steps)
}

/// Rewrites qualified object references in a `SHOW CREATE` definition from
/// the backup schema to the target schema without touching string literals or
/// comments. Views and triggers commonly qualify both their own name and the
/// tables they reference, so replacing only the first occurrence is not
/// sufficient.
fn retarget_schema_qualifiers(sql: &str, backup_schema: &str, target_schema: &str) -> String {
    if backup_schema.eq_ignore_ascii_case(target_schema) {
        return sql.to_string();
    }
    let source = format!("{}.", quote_identifier(backup_schema));
    let replacement = format!("{}.", quote_identifier(target_schema));
    let mut output = String::with_capacity(sql.len());
    let mut index = 0usize;
    let mut quote: Option<char> = None;
    let mut line_comment = false;
    let mut block_comment = false;

    while index < sql.len() {
        let rest = &sql[index..];
        if line_comment {
            let ch = rest.chars().next().expect("index is inside SQL");
            output.push(ch);
            index += ch.len_utf8();
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            if rest.starts_with("*/") {
                output.push_str("*/");
                index += 2;
                block_comment = false;
            } else {
                let ch = rest.chars().next().expect("index is inside SQL");
                output.push(ch);
                index += ch.len_utf8();
            }
            continue;
        }
        if let Some(active_quote) = quote {
            let ch = rest.chars().next().expect("index is inside SQL");
            output.push(ch);
            index += ch.len_utf8();
            if ch == '\\' {
                if let Some(escaped) = sql[index..].chars().next() {
                    output.push(escaped);
                    index += escaped.len_utf8();
                }
            } else if ch == active_quote {
                if sql[index..].starts_with(active_quote) {
                    output.push(active_quote);
                    index += active_quote.len_utf8();
                } else {
                    quote = None;
                }
            }
            continue;
        }

        if rest.starts_with(&source) {
            output.push_str(&replacement);
            index += source.len();
        } else if rest.starts_with("--") || rest.starts_with('#') {
            let prefix_len = if rest.starts_with("--") { 2 } else { 1 };
            output.push_str(&rest[..prefix_len]);
            index += prefix_len;
            line_comment = true;
        } else if rest.starts_with("/*") {
            output.push_str("/*");
            index += 2;
            block_comment = true;
        } else {
            let ch = rest.chars().next().expect("index is inside SQL");
            output.push(ch);
            index += ch.len_utf8();
            if matches!(ch, '\'' | '"') {
                quote = Some(ch);
            }
        }
    }
    output
}

/// Whether a SQL fragment contains a real semicolon outside literals,
/// identifiers, or comments. A compound routine needs such separators and
/// cannot be emitted as one splitter-safe statement without `DELIMITER`.
fn contains_executable_semicolon(sql: &str) -> bool {
    let mut index = 0usize;
    let mut quote: Option<char> = None;
    let mut line_comment = false;
    let mut block_comment = false;

    while index < sql.len() {
        let rest = &sql[index..];
        if line_comment {
            let ch = rest.chars().next().expect("index is inside SQL");
            index += ch.len_utf8();
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            if rest.starts_with("*/") {
                index += 2;
                block_comment = false;
            } else {
                index += rest
                    .chars()
                    .next()
                    .expect("index is inside SQL")
                    .len_utf8();
            }
            continue;
        }
        if let Some(active_quote) = quote {
            let ch = rest.chars().next().expect("index is inside SQL");
            index += ch.len_utf8();
            if ch == '\\' {
                if let Some(escaped) = sql[index..].chars().next() {
                    index += escaped.len_utf8();
                }
            } else if ch == active_quote {
                if sql[index..].starts_with(active_quote) {
                    index += active_quote.len_utf8();
                } else {
                    quote = None;
                }
            }
            continue;
        }

        if rest.starts_with("--") || rest.starts_with('#') {
            index += if rest.starts_with("--") { 2 } else { 1 };
            line_comment = true;
        } else if rest.starts_with("/*") {
            index += 2;
            block_comment = true;
        } else {
            let ch = rest.chars().next().expect("index is inside SQL");
            index += ch.len_utf8();
            if matches!(ch, '\'' | '"' | '`') {
                quote = Some(ch);
            } else if ch == ';' {
                return true;
            }
        }
    }
    false
}

/// `SHOW CREATE` includes the account that originally owned views, routines,
/// triggers, and events. Replaying that `DEFINER=user@host` on a recovery
/// target commonly fails for an operator who cannot impersonate that account.
/// Removing only this metadata clause keeps `SQL SECURITY DEFINER` semantics
/// while letting MySQL assign the executing account as the new definer.
fn strip_show_create_definer(sql: &str) -> String {
    let lowercase = sql.to_ascii_lowercase();
    let Some(start) = lowercase.find("definer=") else {
        return sql.to_string();
    };
    if start > 0
        && lowercase.as_bytes()[start - 1]
            .is_ascii_alphanumeric()
    {
        return sql.to_string();
    }

    let mut index = start + "definer=".len();
    let mut in_backtick = false;
    let mut saw_at = false;
    while index < sql.len() {
        let rest = &sql[index..];
        let ch = rest.chars().next().expect("index is inside SQL");
        if ch == '`' {
            if in_backtick && sql[index + ch.len_utf8()..].starts_with('`') {
                index += ch.len_utf8() * 2;
                continue;
            }
            in_backtick = !in_backtick;
        } else if !in_backtick && ch == '@' {
            saw_at = true;
        } else if !in_backtick && saw_at && ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    if !saw_at || in_backtick {
        return sql.to_string();
    }

    let before = sql[..start].trim_end();
    let after = sql[index..].trim_start();
    if before.is_empty() {
        after.to_string()
    } else if after.is_empty() {
        before.to_string()
    } else {
        format!("{before} {after}")
    }
}

/// Rows above this are not inlined as INSERT literals when target and backup
/// are different servers; the operator is pointed at a dump tool instead.
const CROSS_INSTANCE_RESTORE_ROW_LIMIT: u64 = 50_000;

/// Batch size for inlined INSERT statements on cross-instance restore.
const RESTORE_INSERT_BATCH: usize = 500;

/// Builds a full table restore from the backup: recreate structure, then
/// repopulate. Same-instance backups use INSERT..SELECT; cross-instance
/// backups inline the rows as literals up to a row cap.
#[allow(clippy::too_many_arguments)]
async fn build_table_restore(
    target: &mut sqlx::MySqlConnection,
    backup: &mut sqlx::MySqlConnection,
    base_database: &str,
    backup_database: &str,
    same_instance: bool,
    schema: &str,
    table: &str,
    order: usize,
    source: &str,
    created_here: bool,
) -> Result<(Vec<RecoverySqlStep>, Vec<String>), String> {
    let mut steps = Vec::new();
    let mut notes = Vec::new();
    let backup_schema = mapped_backup_schema(schema, base_database, backup_database);

    // A table absent from the backup did not exist before the change — the
    // statement created it — so the restore is to drop it again. Checking
    // existence first matters because `SHOW CREATE TABLE` on a missing table
    // is an error, which used to surface as "backup has no usable copy" and
    // left every CREATE TABLE unrecoverable.
    let exists = (&mut *backup)
        .fetch_optional(
            sqlx::query(
                "SELECT 1 FROM information_schema.TABLES \
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? LIMIT 1",
            )
            .bind(&backup_schema)
            .bind(table),
        )
        .await
        .map_err(|error| format!("could not look up the backup copy: {error}"))?
        .is_some();
    if !exists {
        if !created_here {
            // The statement modified an existing table, so the backup should
            // have held a copy. Dropping the table here would delete the very
            // data the restore exists to bring back.
            return Err(format!(
                "the backup has no copy of this table, but the change was not a CREATE — the backup predates the table or is incomplete; restore it from a dump before running any recovery for {}.{}",
                schema, table
            ));
        }
        steps.push(RecoverySqlStep {
            order,
            sql: format!(
                "DROP TABLE IF EXISTS {}.{}",
                quote_identifier(schema),
                quote_identifier(table)
            ),
            expected_affected_rows: None,
            source: source.to_string(),
        });
        notes.push(format!(
            "{}.{}: created by this change and absent from the backup, so the restore drops it (source: {})",
            schema, table, source
        ));
        return Ok((steps, notes));
    }

    // The target's counter is read before the table is dropped: the backup's
    // AUTO_INCREMENT is from an earlier point in time, so restoring it as-is
    // would re-issue ids the target already handed out.
    let target_auto_increment = (&mut *target)
        .fetch_optional(
            sqlx::query(
                "SELECT AUTO_INCREMENT FROM information_schema.TABLES \
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND AUTO_INCREMENT IS NOT NULL",
            )
            .bind(schema)
            .bind(table),
        )
        .await
        .ok()
        .flatten()
        .and_then(|row| row.try_get::<u64, _>(0).ok());

    // 1) Structure from the backup.
    let create_row = (&mut *backup)
        .fetch_one(sqlx::raw_sql(&format!(
            "SHOW CREATE TABLE {}.{}",
            quote_identifier(&backup_schema),
            quote_identifier(table)
        )))
        .await
        .map_err(|error| format!("backup has no usable copy: {error}"))?;
    let create_sql = mysql_text(&create_row, 1)?;
    // SHOW CREATE emits an unqualified name; qualify it for the target.
    let qualified = format!(
        "CREATE TABLE {}.{}",
        quote_identifier(schema),
        quote_identifier(table)
    );
    let create_sql = create_sql.replacen(
        &format!("CREATE TABLE {}", quote_identifier(table)),
        &qualified,
        1,
    );

    steps.push(RecoverySqlStep {
        order,
        sql: format!(
            "DROP TABLE IF EXISTS {}.{}",
            quote_identifier(schema),
            quote_identifier(table)
        ),
        expected_affected_rows: None,
        source: source.to_string(),
    });
    steps.push(RecoverySqlStep {
        order,
        sql: create_sql,
        expected_affected_rows: None,
        source: source.to_string(),
    });

    let count_row = (&mut *backup)
        .fetch_one(sqlx::raw_sql(&format!(
            "SELECT COUNT(*) FROM {}.{}",
            quote_identifier(&backup_schema),
            quote_identifier(table)
        )))
        .await
        .map_err(|error| format!("could not count backup rows: {error}"))?;
    let row_count = count_row
        .try_get::<i64, _>(0)
        .map_err(|error| error.to_string())? as u64;

    // Column metadata drives both restore paths. `SHOW COLUMNS` was not
    // enough: it does not separate generated columns, and writing one
    // explicitly fails the whole INSERT with
    // ER_NON_DEFAULT_VALUE_FOR_GENERATED_COLUMN.
    let column_rows = (&mut *backup)
        .fetch_all(
            sqlx::query(
                "SELECT COLUMN_NAME, DATA_TYPE, GENERATION_EXPRESSION \
                 FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
            )
            .bind(&backup_schema)
            .bind(table),
        )
        .await
        .map_err(|error| format!("could not list backup columns: {error}"))?;
    let mut columns: Vec<(String, String)> = Vec::new();
    let mut generated: Vec<String> = Vec::new();
    for row in &column_rows {
        let name = mysql_text(row, 0)?;
        let data_type = mysql_text(row, 1)?;
        // Only a real generated column has an expression. EXTRA is not usable
        // here: `DEFAULT CURRENT_TIMESTAMP` sets it to `DEFAULT_GENERATED`,
        // which would drop every created_at/updated_at from the restore and
        // silently lose those values.
        let generation_expression = mysql_text(row, 2).unwrap_or_default();
        if !generation_expression.trim().is_empty() {
            generated.push(name);
            continue;
        }
        columns.push((name, data_type));
    }
    if columns.is_empty() {
        return Err("backup table reports no writable columns".to_string());
    }
    if !generated.is_empty() {
        notes.push(format!(
            "{}.{}: generated column(s) {} are recomputed by the server and not written back (source: {})",
            schema,
            table,
            generated.join(", "),
            source
        ));
    }
    let insertable: Vec<String> = columns
        .iter()
        .map(|(name, _)| quote_identifier(name))
        .collect();

    // 2) Data.
    if same_instance {
        steps.push(RecoverySqlStep {
            order,
            // Naming the columns keeps the copy correct when the backup has a
            // different column order, and skips generated columns, which
            // reject an explicit value (ER_NON_DEFAULT_VALUE_FOR_GENERATED_COLUMN).
            sql: format!(
                "INSERT INTO {}.{} ({}) SELECT {} FROM {}.{}",
                quote_identifier(schema),
                quote_identifier(table),
                insertable.join(", "),
                insertable.join(", "),
                quote_identifier(&backup_schema),
                quote_identifier(table)
            ),
            // Carrying the row count makes this an executable, verified step.
            // Left as None it rendered as commented-out manual DDL, so a
            // same-instance restore produced a file that did nothing at all.
            expected_affected_rows: Some(row_count),
            source: source.to_string(),
        });
        append_trigger_restores(
            backup,
            &backup_schema,
            schema,
            table,
            order,
            source,
            &mut steps,
        )
        .await?;
        append_auto_increment_reset(
            schema,
            table,
            target_auto_increment,
            order,
            source,
            &mut steps,
        );
        return Ok((steps, notes));
    }
    if row_count > CROSS_INSTANCE_RESTORE_ROW_LIMIT {
        // Emitting DROP + CREATE with no data would leave the operator
        // holding an empty table between running this file and finishing a
        // manual import. Better to hand back nothing executable and say so.
        steps.clear();
        notes.push(format!(
            "{}.{}: {} rows exceed the {}-row inline limit for a cross-instance restore — no SQL generated on purpose (dropping the table here would empty it until a manual import finished). Restore it with mysqldump from the backup instead (source: {})",
            schema, table, row_count, CROSS_INSTANCE_RESTORE_ROW_LIMIT, source
        ));
        return Ok((steps, notes));
    }

    // Durable literals via the same HEX technique the row comparison uses,
    // so binary data round-trips exactly.
    let projection = columns
        .iter()
        .enumerate()
        .map(|(index, (column, _))| {
            let quoted = quote_identifier(column);
            format!(
                "CASE WHEN {quoted} IS NULL THEN 'NULL' \
                 ELSE CONCAT('X''', HEX(CAST({quoted} AS BINARY)), '''') END AS `v{index}`"
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let data_rows = (&mut *backup)
        .fetch_all(sqlx::raw_sql(&format!(
            "SELECT {projection} FROM {}.{}",
            quote_identifier(&backup_schema),
            quote_identifier(table)
        )))
        .await
        .map_err(|error| format!("could not read backup rows: {error}"))?;

    let column_list = insertable.clone().join(", ");
    for chunk in data_rows.chunks(RESTORE_INSERT_BATCH) {
        let mut tuples = Vec::with_capacity(chunk.len());
        for row in chunk {
            let values = columns
                .iter()
                .enumerate()
                .map(|(index, (name, data_type))| {
                    let raw = mysql_text(row, index)?;
                    // Same typing rules the row-level path uses: a HEX blob
                    // assigned straight into a JSON column fails with
                    // "Cannot create a JSON value from a string in CHARACTER
                    // SET 'binary'", which used to sink the whole batch.
                    Ok::<String, String>(restoration_literal(
                        &RecoveryColumn {
                            name: name.clone(),
                            data_type: data_type.clone(),
                        },
                        &raw,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?;
            tuples.push(format!("({})", values.join(", ")));
        }
        steps.push(RecoverySqlStep {
            order,
            sql: format!(
                "INSERT INTO {}.{} ({}) VALUES\n{}",
                quote_identifier(schema),
                quote_identifier(table),
                column_list,
                tuples.join(",\n")
            ),
            expected_affected_rows: Some(chunk.len() as u64),
            source: source.to_string(),
        });
    }
    append_trigger_restores(
        backup,
        &backup_schema,
        schema,
        table,
        order,
        source,
        &mut steps,
    )
    .await?;
    append_auto_increment_reset(schema, table, target_auto_increment, order, source, &mut steps);
    Ok((steps, notes))
}

/// Restores the target's own AUTO_INCREMENT counter after a rebuild.
///
/// Skipped when the target had none (no auto-increment column, or the table
/// did not exist), in which case the backup's value is already correct.
fn append_auto_increment_reset(
    schema: &str,
    table: &str,
    target_auto_increment: Option<u64>,
    order: usize,
    source: &str,
    steps: &mut Vec<RecoverySqlStep>,
) {
    let Some(next_id) = target_auto_increment else {
        return;
    };
    steps.push(RecoverySqlStep {
        order,
        sql: format!(
            "ALTER TABLE {}.{} AUTO_INCREMENT = {}",
            quote_identifier(schema),
            quote_identifier(table),
            next_id
        ),
        expected_affected_rows: None,
        source: source.to_string(),
    });
}

/// Recreates the triggers a table carried in the backup.
///
/// `SHOW CREATE TABLE` does not include them, so rebuilding a table from it
/// silently drops every trigger it owned. Appended after the data so the
/// refill does not fire them.
#[allow(clippy::too_many_arguments)]
async fn append_trigger_restores(
    backup: &mut sqlx::MySqlConnection,
    backup_schema: &str,
    schema: &str,
    table: &str,
    order: usize,
    source: &str,
    steps: &mut Vec<RecoverySqlStep>,
) -> Result<(), String> {
    let rows = backup
        .fetch_all(
            sqlx::query(
                "SELECT TRIGGER_NAME FROM information_schema.TRIGGERS \
                 WHERE EVENT_OBJECT_SCHEMA = ? AND EVENT_OBJECT_TABLE = ? \
                 ORDER BY TRIGGER_NAME",
            )
            .bind(backup_schema)
            .bind(table),
        )
        .await
        .map_err(|error| format!("could not list backup triggers: {error}"))?;

    for row in &rows {
        let name = mysql_text(row, 0)?;
        let created = backup
            .fetch_optional(sqlx::raw_sql(&format!(
                "SHOW CREATE TRIGGER {}.{}",
                quote_identifier(backup_schema),
                quote_identifier(&name)
            )))
            .await
            .map_err(|error| format!("could not read backup trigger {name}: {error}"))?;
        let Some(created) = created else { continue };
        let create_sql = mysql_text(&created, 2)?;
        if create_sql.trim().is_empty() {
            continue;
        }
        let create_sql = strip_show_create_definer(&retarget_schema_qualifiers(
            &create_sql,
            backup_schema,
            schema,
        ));
        let definition = create_sql.trim();
        let definition_without_trailing_delimiter = definition
            .strip_suffix(';')
            .unwrap_or(definition)
            .trim_end();
        if contains_executable_semicolon(definition_without_trailing_delimiter) {
            return Err(format!(
                "trigger {}.{} has a compound definition; SQL policy forbids DELIMITER, so recreate it manually before rebuilding the table",
                quote_identifier(schema),
                quote_identifier(&name)
            ));
        }
        steps.push(RecoverySqlStep {
            order,
            sql: format!(
                "DROP TRIGGER IF EXISTS {}.{}",
                quote_identifier(schema),
                quote_identifier(&name)
            ),
            expected_affected_rows: None,
            source: source.to_string(),
        });
        steps.push(RecoverySqlStep {
            order,
            sql: create_sql,
            expected_affected_rows: None,
            source: source.to_string(),
        });
    }
    Ok(())
}

fn comparable_indexes(work: &RowWork) -> Vec<usize> {
    work.columns
        .iter()
        .enumerate()
        .filter(|(_, column)| {
            !work
                .primary_key
                .iter()
                .any(|key| key.eq_ignore_ascii_case(&column.name))
                && (work.compare_all_columns
                    || work
                        .affected_columns
                        .contains(&column.name.to_ascii_lowercase()))
        })
        .map(|(index, _)| index)
        .collect()
}

fn row_key(
    columns: &[RecoveryColumn],
    primary_key: &[String],
    row: &RecoveryRow,
) -> Result<Vec<String>, String> {
    primary_key
        .iter()
        .map(|key| {
            let index = columns
                .iter()
                .position(|column| column.name.eq_ignore_ascii_case(key))
                .ok_or_else(|| {
                    format!("Primary key column {key} is missing from recovery metadata")
                })?;
            row.values
                .get(index)
                .cloned()
                .ok_or_else(|| format!("Primary key column {key} is missing from a row image"))
        })
        .collect()
}

/// Keys per SELECT when comparing. Each key expands to an OR'd conjunction of
/// `CAST(col AS BINARY) <=> X'..'` terms, so the batch size bounds SQL length.
const COMPARE_FETCH_BATCH: usize = 200;

fn recovery_row_projection(work: &RowWork) -> String {
    work.columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let quoted = quote_identifier(&column.name);
            format!(
                "CASE WHEN {quoted} IS NULL THEN 'NULL' \
                 ELSE CONCAT('X''', HEX(CAST({quoted} AS BINARY)), '''') END AS `v{index}`"
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn key_condition(work: &RowWork, key: &[String]) -> String {
    work.primary_key
        .iter()
        .zip(key)
        .map(|(column, value)| {
            format!("CAST({} AS BINARY) <=> {}", quote_identifier(column), value)
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Fetches all requested rows in batches and returns them keyed by primary
/// key. A key that maps to more than one row (should be impossible for a PK
/// lookup) is reported as an error, matching the old single-row behaviour.
async fn fetch_rows_batch(
    conn: &mut sqlx::MySqlConnection,
    schema: &str,
    work: &RowWork,
    keys: &[&Vec<String>],
) -> Result<std::collections::HashMap<Vec<String>, RecoveryRow>, String> {
    for key in keys {
        if work.primary_key.len() != key.len() {
            return Err("Recovery primary-key arity mismatch".to_string());
        }
    }
    let projection = recovery_row_projection(work);
    let mut fetched = std::collections::HashMap::new();
    for chunk in keys.chunks(COMPARE_FETCH_BATCH) {
        let condition = chunk
            .iter()
            .map(|key| format!("({})", key_condition(work, key)))
            .collect::<Vec<_>>()
            .join(" OR ");
        let sql = format!(
            "SELECT {projection} FROM {}.{} WHERE {condition}",
            quote_identifier(schema),
            quote_identifier(&work.table)
        );
        let rows = (&mut *conn)
            .fetch_all(sqlx::raw_sql(&sql))
            .await
            .map_err(|error| {
                format!(
                    "Could not read {}.{} during recovery comparison: {error}",
                    schema, work.table
                )
            })?;
        for row in rows {
            let values = (0..work.columns.len())
                .map(|index| mysql_text(&row, index))
                .collect::<Result<Vec<_>, String>>()?;
            let row = RecoveryRow { values };
            let key = row_key(&work.columns, &work.primary_key, &row)?;
            if fetched.insert(key, row).is_some() {
                return Err(format!(
                    "Primary key lookup returned multiple rows for {}.{}",
                    schema, work.table
                ));
            }
        }
    }
    Ok(fetched)
}

fn build_update_sql(
    work: &RowWork,
    current: &RecoveryRow,
    desired: &RecoveryRow,
    changed_indexes: &[usize],
) -> Result<String, String> {
    let assignments = changed_indexes
        .iter()
        .map(|index| {
            let column = &work.columns[*index];
            let desired_value = desired
                .values
                .get(*index)
                .ok_or_else(|| format!("Desired value for {} is missing", column.name))?;
            Ok(format!(
                "{} = {}",
                quote_identifier(&column.name),
                restoration_literal(column, desired_value)
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!(
        "UPDATE {}.{} SET {} WHERE {} AND ({}) LIMIT 1",
        quote_identifier(&work.schema),
        quote_identifier(&work.table),
        assignments.join(", "),
        primary_key_condition(work, current)?,
        full_row_guard(work, current)?
    ))
}

fn build_insert_sql(work: &RowWork, desired: &RecoveryRow) -> Result<String, String> {
    let columns = work
        .columns
        .iter()
        .map(|column| quote_identifier(&column.name))
        .collect::<Vec<_>>()
        .join(", ");
    let values = work
        .columns
        .iter()
        .zip(&desired.values)
        .map(|(column, value)| restoration_literal(column, value))
        .collect::<Vec<_>>()
        .join(", ");
    let qualified_table = format!(
        "{}.{}",
        quote_identifier(&work.schema),
        quote_identifier(&work.table)
    );
    Ok(format!(
        "INSERT INTO {qualified_table} ({columns}) SELECT {values} FROM DUAL \
         WHERE NOT EXISTS (SELECT 1 FROM {qualified_table} WHERE {})",
        primary_key_condition(work, desired)?
    ))
}

fn build_delete_sql(work: &RowWork, current: &RecoveryRow) -> Result<String, String> {
    Ok(format!(
        "DELETE FROM {}.{} WHERE {} AND ({}) LIMIT 1",
        quote_identifier(&work.schema),
        quote_identifier(&work.table),
        primary_key_condition(work, current)?,
        full_row_guard(work, current)?
    ))
}

fn primary_key_condition(work: &RowWork, row: &RecoveryRow) -> Result<String, String> {
    let key = row_key(&work.columns, &work.primary_key, row)?;
    Ok(work
        .primary_key
        .iter()
        .zip(key)
        .map(|(column, value)| {
            format!("CAST({} AS BINARY) <=> {}", quote_identifier(column), value)
        })
        .collect::<Vec<_>>()
        .join(" AND "))
}

fn full_row_guard(work: &RowWork, row: &RecoveryRow) -> Result<String, String> {
    if work.columns.len() != row.values.len() {
        return Err("Recovery row image width does not match its column metadata".to_string());
    }
    Ok(work
        .columns
        .iter()
        .zip(&row.values)
        .map(|(column, value)| {
            format!(
                "CAST({} AS BINARY) <=> {}",
                quote_identifier(&column.name),
                value
            )
        })
        .collect::<Vec<_>>()
        .join(" AND "))
}

fn restoration_literal(column: &RecoveryColumn, value: &str) -> String {
    if value == "NULL" {
        value.to_string()
    } else if column.data_type.eq_ignore_ascii_case("json") {
        format!("CONVERT({value} USING utf8mb4)")
    } else if column.data_type.eq_ignore_ascii_case("bit") {
        value.to_string()
    } else {
        format!("CAST({value} AS BINARY)")
    }
}

async fn begin_read_only_snapshot(
    conn: &mut sqlx::MySqlConnection,
    label: &str,
) -> Result<(), String> {
    // Target and backup are separate sessions, possibly on servers with
    // different @@time_zone. TIMESTAMP values render in session time, so
    // without a shared zone identical rows compare as different and the
    // "restored" value is written back shifted. UTC on both sides removes it.
    (&mut *conn)
        .execute(sqlx::raw_sql("SET time_zone = '+00:00'"))
        .await
        .map_err(|error| format!("Could not pin the {label} session time zone: {error}"))?;
    (&mut *conn)
        .execute(sqlx::raw_sql(
            "START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY",
        ))
        .await
        .map(|_| ())
        .map_err(|error| format!("Could not start {label} read-only snapshot: {error}"))
}

async fn probe_instance(conn: &mut sqlx::MySqlConnection) -> Result<InstanceProbe, String> {
    let row = (&mut *conn)
        .fetch_one(sqlx::raw_sql(
            "SELECT @@hostname, @@port, DATABASE(), CURRENT_USER(), VERSION()",
        ))
        .await
        .map_err(|error| format!("Could not identify recovery comparison instance: {error}"))?;
    let hostname = mysql_text(&row, 0)?;
    let port = row
        .try_get::<u32, _>(1)
        .map_err(|error| format!("Could not decode instance port: {error}"))?;
    let database = row
        .try_get::<Option<String>, _>(2)
        .map_err(|error| format!("Could not decode current database: {error}"))?
        .ok_or_else(|| "Recovery comparison requires a selected database".to_string())?;
    let current_user = mysql_text(&row, 3)?;
    let version = mysql_text(&row, 4)?;

    let server_uuid = optional_text_scalar(conn, "SELECT @@server_uuid").await;
    let server_uid = if server_uuid.is_none() {
        optional_text_scalar(conn, "SELECT @@server_uid").await
    } else {
        None
    };
    let server_key = if let Some(uuid) = server_uuid {
        format!("uuid:{uuid}")
    } else if let Some(uid) = server_uid {
        format!("uid:{uid}")
    } else {
        format!("host:{hostname}:{port}")
    };
    Ok(InstanceProbe {
        label: format!("{hostname}:{port}/{database} · {current_user} · {version}"),
        server_key,
        database,
    })
}

async fn optional_text_scalar(conn: &mut sqlx::MySqlConnection, sql: &str) -> Option<String> {
    (&mut *conn)
        .fetch_optional(sqlx::raw_sql(sql))
        .await
        .ok()
        .flatten()
        .and_then(|row| mysql_text(&row, 0).ok())
}

async fn read_object_definition(
    conn: &mut sqlx::MySqlConnection,
    object: &RecoveryObject,
) -> Result<Option<String>, String> {
    if object.kind == "database" {
        let exists_sql = format!(
            "SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = {}",
            sql_hex(object.schema.as_bytes())
        );
        if scalar_count(conn, &exists_sql).await? == 0 {
            return Ok(None);
        }
        let sql = format!("SHOW CREATE DATABASE {}", quote_identifier(&object.schema));
        let row = (&mut *conn)
            .fetch_one(sqlx::raw_sql(&sql))
            .await
            .map_err(|error| format!("Could not read database definition: {error}"))?;
        return mysql_text(&row, 1).map(Some);
    }

    let exists_sql = format!(
        "SELECT COUNT(*) FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {}",
        sql_hex(object.schema.as_bytes()),
        sql_hex(object.name.as_bytes())
    );
    if scalar_count(conn, &exists_sql).await? == 0 {
        return Ok(None);
    }
    let sql = format!(
        "SHOW CREATE TABLE {}.{}",
        quote_identifier(&object.schema),
        quote_identifier(&object.name)
    );
    let row = (&mut *conn)
        .fetch_one(sqlx::raw_sql(&sql))
        .await
        .map_err(|error| {
            format!(
                "Could not read definition for {}.{}: {error}",
                object.schema, object.name
            )
        })?;
    mysql_text(&row, 1).map(Some)
}

async fn scalar_count(conn: &mut sqlx::MySqlConnection, sql: &str) -> Result<u64, String> {
    let count = (&mut *conn)
        .fetch_one(sqlx::raw_sql(sql))
        .await
        .map_err(|error| error.to_string())?
        .try_get::<i64, _>(0)
        .map_err(|error| error.to_string())?;
    u64::try_from(count).map_err(|_| "COUNT(*) returned a negative value".to_string())
}

fn mapped_backup_schema(source: &str, base_database: &str, backup_database: &str) -> String {
    if source.eq_ignore_ascii_case(base_database) {
        backup_database.to_string()
    } else {
        source.to_string()
    }
}

fn recorded_target_matches(recorded: &str, current: &str) -> bool {
    if recorded.eq_ignore_ascii_case(current) {
        return true;
    }
    if let Some(uuid) = current.strip_prefix("uuid:") {
        return recorded.eq_ignore_ascii_case(&format!("MySQL server UUID {uuid}"));
    }
    if let Some(uid) = current.strip_prefix("uid:") {
        return recorded.eq_ignore_ascii_case(&format!("MariaDB server UID {uid}"));
    }
    if let Some(host_port) = current.strip_prefix("host:") {
        return recorded.eq_ignore_ascii_case(host_port);
    }
    false
}

fn mapped_backup_object(
    object: &RecoveryObject,
    base_database: &str,
    backup_database: &str,
) -> RecoveryObject {
    let mut mapped = object.clone();
    mapped.schema = mapped_backup_schema(&object.schema, base_database, backup_database);
    if object.kind == "database" && object.name.eq_ignore_ascii_case(base_database) {
        mapped.name = backup_database.to_string();
    }
    mapped
}

fn render_recovery_sql(
    connection_id: &str,
    runs: &[RecoveryRun],
    target: &InstanceProbe,
    backup: &InstanceProbe,
    steps: &[RecoverySqlStep],
    conflicts: &[String],
) -> String {
    let mut output = String::new();
    output.push_str("-- Tabularis backup-instance recovery SQL\n");
    output.push_str(
        "-- Generated by read-only row/schema comparison; never executed automatically.\n",
    );
    output.push_str("-- No Tabularis session variables or prepared-statement wrappers are used.\n");
    output
        .push_str("-- IMPORTANT: DO NOT USE RUN ALL. Select and execute each stage separately.\n");
    output.push_str("-- Run Stage A immediately before each chosen Stage B group and stop on any identity mismatch.\n");
    output.push_str("-- DDL implicitly commits; execute one DDL statement at a time. DML ends in ROLLBACK by default.\n");
    output.push_str(&format!(
        "-- Target connection alias: {}\n",
        single_line_comment(&runs[0].connection_name)
    ));
    output.push_str(&format!(
        "-- Target connection ID: {}\n",
        single_line_comment(connection_id)
    ));
    output.push_str(&format!(
        "-- Expected target database: {}\n",
        single_line_comment(&target.database)
    ));
    output.push_str(&format!(
        "-- Expected target identity: {}\n",
        single_line_comment(&target.server_key)
    ));
    output.push_str(&format!(
        "-- Target instance: {}\n",
        single_line_comment(&target.label)
    ));
    output.push_str(&format!(
        "-- Backup instance: {}\n",
        single_line_comment(&backup.label)
    ));
    output.push_str(&format!(
        "-- Selected recovery Run IDs: {}\n",
        runs.iter()
            .map(|run| run.short_id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    for conflict in conflicts {
        output.push_str(&format!(
            "-- CONFLICT (no SQL generated): {}\n",
            single_line_comment(conflict)
        ));
    }
    output.push('\n');

    append_identity_stage(
        &mut output,
        "Stage A: identity precheck (read-only; run separately)",
        target,
    );

    let mut current_group = None;
    let mut stage_number = 1usize;
    let mut transaction_open = false;
    for (index, step) in steps.iter().enumerate() {
        let transactional = step.expected_affected_rows.is_some();
        if current_group != Some(transactional) {
            if transaction_open {
                append_transaction_finish(&mut output);
                transaction_open = false;
            }
            if transactional {
                output.push_str(&format!(
                    "-- ===== Stage B{stage_number}: DML recovery (write; one transaction; run separately) =====\n"
                ));
                output.push_str("-- Row guards are the precheck; each adjacent ROW_COUNT result is the postcheck.\n");
                output.push_str(
                    "-- Replace the final ROLLBACK with COMMIT only after every result matches.\n",
                );
                output.push_str("SET time_zone = '+00:00';\n");
                output.push_str("START TRANSACTION;\n\n");
                transaction_open = true;
            } else {
                output.push_str(&format!(
                    "-- ===== Stage B{stage_number}: DDL recovery (write; implicit commit; run separately) =====\n"
                ));
                output.push_str("-- Execute exactly one statement, then run Stage C before continuing. Never select this whole stage.\n\n");
            }
            current_group = Some(transactional);
            stage_number += 1;
        }
        output.push_str(&format!(
            "-- Recovery step {} from {}\n",
            index + 1,
            single_line_comment(&step.source)
        ));
        if let Some(expected) = step.expected_affected_rows {
            append_direct_statement(&mut output, &step.sql);
            output.push_str(&format!(
                "SELECT ROW_COUNT() AS recovery_step_{}_affected_rows, {expected} AS expected_affected_rows;\n",
                index + 1
            ));
        } else {
            output.push_str("-- PRECHECK: Stage A must show both match columns = 1.\n");
            append_direct_statement(&mut output, &step.sql);
            output.push_str(
                "-- POSTCHECK: run Stage C now, then inspect this object's definition/data.\n",
            );
        }
        output.push('\n');
    }
    if transaction_open {
        append_transaction_finish(&mut output);
    }
    if steps.is_empty() {
        output.push_str("-- ===== Stage B: no write required =====\n");
        output.push_str("-- No row or schema differences were found for the selection.\n\n");
    }
    append_identity_stage(
        &mut output,
        "Stage C: postcheck (read-only; run separately after each write)",
        target,
    );
    output.push_str(
        "-- DML: every adjacent affected_rows value must equal expected_affected_rows.\n",
    );
    output.push_str("-- DDL: inspect SHOW CREATE / information_schema for the changed object before the next step.\n");
    output.push_str("-- Re-run Tabularis read-only comparison; completion means it reports no remaining differences.\n");
    output
}

fn append_identity_stage(output: &mut String, title: &str, target: &InstanceProbe) {
    let actual_identity = if target.server_key.starts_with("uuid:") {
        "CONCAT('uuid:', @@server_uuid)"
    } else if target.server_key.starts_with("uid:") {
        "CONCAT('uid:', @@server_uid)"
    } else {
        "CONCAT('host:', @@hostname, ':', @@port)"
    };
    let expected_identity = sql_text_literal(&target.server_key);
    let expected_database = sql_text_literal(&target.database);
    output.push_str(&format!("-- ===== {title} =====\n"));
    output.push_str("SELECT\n");
    output.push_str("  @@hostname AS actual_hostname,\n");
    output.push_str("  @@port AS actual_port,\n");
    output.push_str(&format!("  {actual_identity} AS actual_server_identity,\n"));
    output.push_str(&format!(
        "  {expected_identity} AS expected_server_identity,\n"
    ));
    output.push_str(&format!(
        "  LOWER({actual_identity}) = LOWER({expected_identity}) AS server_identity_matches,\n"
    ));
    output.push_str("  DATABASE() AS actual_database,\n");
    output.push_str(&format!("  {expected_database} AS expected_database,\n"));
    output.push_str(&format!(
        "  LOWER(COALESCE(DATABASE(), '')) = LOWER({expected_database}) AS database_matches,\n"
    ));
    output.push_str("  CURRENT_USER() AS actual_user,\n");
    output.push_str("  VERSION() AS actual_version,\n");
    output.push_str("  @@read_only AS target_read_only;\n");
    output.push_str("-- STOP unless server_identity_matches = 1 and database_matches = 1.\n\n");
}

fn sql_text_literal(value: &str) -> String {
    format!("CONVERT({} USING utf8mb4)", sql_hex(value.as_bytes()))
}

fn append_transaction_finish(output: &mut String) {
    output.push_str(
        "-- TABULARIS_REVIEW_REQUIRED: every affected-row result above must equal expected_affected_rows.\n",
    );
    output.push_str(
        "-- Replace the ROLLBACK below with COMMIT only after review in this same session.\n",
    );
    output.push_str("ROLLBACK;\n\n");
}

fn append_direct_statement(output: &mut String, sql: &str) {
    let sql = sql.trim_end();
    output.push_str(sql);
    if !sql.ends_with(';') {
        output.push(';');
    }
    output.push('\n');
}

fn write_recovery_sql(
    root: &Path,
    connection_id: &str,
    connection_name: &str,
    sql: &str,
) -> Result<PathBuf, String> {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let directory = root
        .join("recovery-sql")
        .join(isolated_connection_segment(connection_name, connection_id))
        .join(date);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create recovery SQL directory: {error}"))?;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let path = directory.join(format!("{timestamp}-{}.recovery.sql", ulid::Ulid::new()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("Could not create recovery SQL file: {error}"))?;
    file.write_all(sql.as_bytes())
        .map_err(|error| format!("Could not write recovery SQL file: {error}"))?;
    file.flush()
        .map_err(|error| format!("Could not flush recovery SQL file: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("Could not sync recovery SQL file: {error}"))?;
    Ok(path)
}

fn recovery_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let history_root = root.join("recovery-history");
    if !history_root.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for connection in fs::read_dir(&history_root)
        .map_err(|error| format!("Could not read recovery history: {error}"))?
        .flatten()
    {
        if !connection.path().is_dir() {
            continue;
        }
        for date in fs::read_dir(connection.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if !date.path().is_dir() {
                continue;
            }
            for entry in fs::read_dir(date.path()).into_iter().flatten().flatten() {
                let path = entry.path();
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.ends_with(".recovery.json") || name.ends_with(".recovery.jsonl")
                    })
                {
                    files.push(path);
                }
            }
        }
    }
    // Cap by NEWEST first: truncating in directory-walk order could hide the
    // most recent runs — the ones an operator actually needs.
    if files.len() > MAX_HISTORY_FILES {
        files.sort_by_cached_key(|path| {
            std::cmp::Reverse(
                fs::metadata(path)
                    .and_then(|meta| meta.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
        });
        files.truncate(MAX_HISTORY_FILES);
    }
    Ok(files)
}

fn read_run(path: &Path) -> Result<RecoveryRun, String> {
    let content = fs::read(path).map_err(|error| {
        format!(
            "Could not read recovery history {}: {error}",
            path.display()
        )
    })?;
    let is_jsonl = path.extension().and_then(|ext| ext.to_str()) == Some("jsonl");
    let run: RecoveryRun = if is_jsonl {
        let text = String::from_utf8_lossy(&content);
        parse_jsonl_run(&text).map_err(|error| {
            format!(
                "Could not parse recovery history {}: {error}",
                path.display()
            )
        })?
    } else {
        serde_json::from_slice(&content).map_err(|error| {
            format!(
                "Could not parse recovery history {}: {error}",
                path.display()
            )
        })?
    };
    if run.version != HISTORY_VERSION {
        return Err(format!(
            "Unsupported recovery history version {} in {}",
            run.version,
            path.display()
        ));
    }
    Ok(run)
}

pub(crate) fn is_unprotected_non_recovery_operation(operation: &str) -> bool {
    matches!(
        operation.to_ascii_lowercase().as_str(),
        "select" | "set" | "analyze table"
    )
}

fn is_legacy_read_only_noise(statement: &RecoveryStatement) -> bool {
    statement.category.eq_ignore_ascii_case("unprotected")
        && is_unprotected_non_recovery_operation(&statement.operation)
}

fn statement_matches_query(statement: &RecoveryStatement, needle: &str) -> bool {
    statement.id.to_ascii_lowercase().contains(needle)
        || statement.sql.to_ascii_lowercase().contains(needle)
        || statement.category.to_ascii_lowercase().contains(needle)
        || statement.operation.to_ascii_lowercase().contains(needle)
        || statement.objects.iter().any(|object| {
            object.kind.to_ascii_lowercase().contains(needle)
                || object.schema.to_ascii_lowercase().contains(needle)
                || object.name.to_ascii_lowercase().contains(needle)
        })
        || statement
            .affected_columns
            .iter()
            .any(|column| column.to_ascii_lowercase().contains(needle))
        || statement
            .condition
            .as_deref()
            .is_some_and(|condition| condition.to_ascii_lowercase().contains(needle))
}

fn statement_summary(statement: &RecoveryStatement) -> RecoveryStatementSummary {
    let object = statement.objects.first();
    RecoveryStatementSummary {
        id: statement.id.clone(),
        index: statement.index,
        executed_at: statement.executed_at.clone(),
        sql: statement.sql.clone(),
        category: statement.category.clone(),
        operation: statement.operation.clone(),
        schema: object.map(|object| object.schema.clone()),
        table: object.map(|object| object.name.clone()),
        affected_columns: statement.affected_columns.clone(),
        condition: statement.condition.clone(),
        row_count: statement.before_rows.len().max(statement.after_rows.len()),
        exact: statement.exact,
    }
}

fn run_summary_for_query(run: &RecoveryRun, query: Option<&str>) -> Option<RecoveryRunSummary> {
    let visible = run
        .statements
        .iter()
        .filter(|statement| !is_legacy_read_only_noise(statement))
        .collect::<Vec<_>>();
    if visible.is_empty() {
        return None;
    }

    let needle = query.map(str::trim).filter(|value| !value.is_empty());
    let matched = if let Some(needle) = needle {
        let needle = needle.to_ascii_lowercase();
        let run_matches = run.run_id.to_ascii_lowercase().contains(&needle)
            || run.short_id.to_ascii_lowercase().contains(&needle)
            || run.connection_id.to_ascii_lowercase().contains(&needle)
            || run.connection_name.to_ascii_lowercase().contains(&needle)
            || run.database.to_ascii_lowercase().contains(&needle)
            || run.status.to_ascii_lowercase().contains(&needle);
        if run_matches {
            visible
        } else {
            visible
                .into_iter()
                .filter(|statement| statement_matches_query(statement, &needle))
                .collect()
        }
    } else {
        visible
    };
    if matched.is_empty() {
        return None;
    }

    let statements = matched
        .into_iter()
        .map(statement_summary)
        .collect::<Vec<_>>();
    Some(RecoveryRunSummary {
        run_id: run.run_id.clone(),
        short_id: run.short_id.clone(),
        started_at: run.started_at.clone(),
        finished_at: run.finished_at.clone(),
        status: run.status.clone(),
        connection_id: run.connection_id.clone(),
        connection_name: run.connection_name.clone(),
        database: run.database.clone(),
        statement_count: statements.len(),
        statements,
    })
}

fn isolated_connection_segment(connection_name: &str, connection_id: &str) -> String {
    let readable = safe_connection_name_segment(connection_name);
    let digest = Sha256::digest(connection_id.as_bytes());
    let suffix = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{readable}-{suffix}")
}

fn short_run_id(run_id: &str) -> String {
    run_id
        .chars()
        .rev()
        .take(10)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn safe_connection_name_segment(connection_name: &str) -> String {
    let mut readable = String::new();
    for ch in connection_name.trim().chars().take(64) {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            if !readable.ends_with('_') && !readable.is_empty() {
                readable.push('_');
            }
        } else {
            readable.push(ch);
        }
    }
    let mut readable = readable
        .trim_matches(|ch| matches!(ch, ' ' | '.' | '_'))
        .to_string();
    if readable.is_empty() {
        readable = "connection".to_string();
    }
    readable
}

fn quote_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', "``"))
}

fn sql_hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(2 + bytes.len() * 2);
    value.push_str("0x");
    for byte in bytes {
        value.push_str(&format!("{byte:02X}"));
    }
    value
}

fn single_line_comment(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn mysql_text(row: &sqlx::mysql::MySqlRow, index: usize) -> Result<String, String> {
    match row.try_get::<String, _>(index) {
        Ok(value) => Ok(value),
        Err(text_error) => {
            let bytes = row.try_get::<Vec<u8>, _>(index).map_err(|bytes_error| {
                format!(
                    "Could not decode MySQL text column {index}: {text_error}; binary fallback failed: {bytes_error}"
                )
            })?;
            String::from_utf8(bytes)
                .map_err(|error| format!("MySQL text column {index} is not UTF-8: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statement(index: usize) -> RecoveryStatement {
        RecoveryStatement {
            id: String::new(),
            index,
            executed_at: String::new(),
            sql: "UPDATE users SET name = 'b' WHERE id = 1".to_string(),
            category: "dml".to_string(),
            operation: "update".to_string(),
            objects: vec![RecoveryObject {
                kind: "table".to_string(),
                schema: "app".to_string(),
                name: "users".to_string(),
            }],
            affected_columns: vec!["name".to_string()],
            condition: Some("id = 1".to_string()),
            columns: vec![
                RecoveryColumn {
                    name: "id".to_string(),
                    data_type: "bigint".to_string(),
                },
                RecoveryColumn {
                    name: "name".to_string(),
                    data_type: "varchar".to_string(),
                },
            ],
            primary_key: vec!["id".to_string()],
            before_rows: vec![RecoveryRow {
                values: vec!["X'31'".to_string(), "X'61'".to_string()],
            }],
            after_rows: vec![RecoveryRow {
                values: vec!["X'31'".to_string(), "X'62'".to_string()],
            }],
            inverse_sql: None,
            exact: true,
        }
    }

    #[test]
    fn journal_is_isolated_and_assigns_short_statement_ids() {
        let root = tempfile::tempdir().unwrap();
        let mut journal = RecoveryJournal::create_in(
            root.path(),
            "connection-a".to_string(),
            "生产主库".to_string(),
            "app".to_string(),
            "server-a".to_string(),
        )
        .unwrap();
        journal.add_statement(statement(0)).unwrap();
        let path = journal.finalize().unwrap();
        assert!(path.starts_with(root.path().join("recovery-history")));
        let run = read_run(&path).unwrap();
        assert_eq!(run.status, "complete");
        assert_eq!(run.statements.len(), 1);
        assert_eq!(run.statements[0].id, format!("{}-001", run.short_id));
        assert!(!path.to_string_lossy().contains("rollback-sql"));
    }

    #[test]
    fn rewind_removes_statements_from_rolled_back_transaction() {
        let root = tempfile::tempdir().unwrap();
        let mut journal = RecoveryJournal::create_in(
            root.path(),
            "connection-a".to_string(),
            "prod".to_string(),
            "app".to_string(),
            "server-a".to_string(),
        )
        .unwrap();
        journal.add_statement(statement(0)).unwrap();
        let checkpoint = journal.checkpoint();
        journal.add_statement(statement(1)).unwrap();
        journal.rewind_to(checkpoint).unwrap();
        let run = read_run(&journal.finalize().unwrap()).unwrap();
        assert_eq!(run.statements.len(), 1);
        assert_eq!(run.statements[0].index, 0);
    }

    #[test]
    fn discard_removes_an_uncommitted_recovery_run() {
        let root = tempfile::tempdir().unwrap();
        let mut journal = RecoveryJournal::create_in(
            root.path(),
            "connection-a".to_string(),
            "prod".to_string(),
            "app".to_string(),
            "server-a".to_string(),
        )
        .unwrap();
        journal.add_statement(statement(0)).unwrap();
        let path = journal.path.clone();

        journal.discard().unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn offline_rollback_generates_guarded_inverse_sql_without_any_connection() {
        let root = tempfile::tempdir().unwrap();
        let mut journal = RecoveryJournal::create_in(
            root.path(),
            "connection-a".to_string(),
            "prod".to_string(),
            "app".to_string(),
            "MySQL server UUID feed-beef".to_string(),
        )
        .unwrap();
        // Statement 0: exact update (id=1: name 'a' -> 'b').
        journal.add_statement(statement(0)).unwrap();
        // Statement 1: exact insert of id=2.
        let mut insert = statement(1);
        insert.operation = "insert".to_string();
        insert.before_rows = Vec::new();
        insert.after_rows = vec![RecoveryRow {
            values: vec!["X'32'".to_string(), "X'63'".to_string()],
        }];
        journal.add_statement(insert).unwrap();
        // Statement 2: exact delete of id=3.
        let mut delete = statement(2);
        delete.operation = "delete".to_string();
        delete.before_rows = vec![RecoveryRow {
            values: vec!["X'33'".to_string(), "X'64'".to_string()],
        }];
        delete.after_rows = Vec::new();
        journal.add_statement(delete).unwrap();
        // Statement 3: unprotected — must surface as a conflict, not SQL.
        let mut unprotected = statement(3);
        unprotected.exact = false;
        unprotected.category = "unprotected".to_string();
        unprotected.operation = "replace".to_string();
        unprotected.before_rows = Vec::new();
        unprotected.after_rows = Vec::new();
        journal.add_statement(unprotected).unwrap();
        let run = read_run(&journal.finalize().unwrap()).unwrap();

        let response = generate_offline_recovery_sql_in(
            root.path(),
            "connection-a",
            &RecoverySelection {
                run_ids: vec![run.run_id.clone()],
                statement_ids: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(response.generated_steps, 3);
        assert!(!response.exact, "the unprotected statement is a conflict");
        assert_eq!(response.conflicts.len(), 1);
        assert_eq!(response.backup_instance, "recorded row images (offline)");
        let sql = &response.sql;
        // Identity precheck derives from the recorded label, offline; the
        // expected identity is hex-encoded by sql_text_literal.
        assert!(
            sql.contains(&sql_text_literal("uuid:feed-beef")),
            "{sql}"
        );
        assert!(sql.contains("@@server_uuid"), "{sql}");
        // Inverses are guarded and reverse-ordered: delete-inverse (INSERT)
        // first, then insert-inverse (DELETE), then update-inverse (UPDATE).
        let insert_back = sql.find("INSERT INTO `app`.`users`").expect("insert-back");
        let delete_back = sql.find("DELETE FROM `app`.`users`").expect("delete-back");
        let update_back = sql.find("UPDATE `app`.`users` SET").expect("update-back");
        assert!(insert_back < delete_back && delete_back < update_back, "{sql}");
        assert!(sql.contains("CAST(`id` AS BINARY) <=> X'31'"), "{sql}");
        assert!(sql.contains("`name` = CAST(X'61' AS BINARY)"), "{sql}");
        assert!(sql.contains("\nROLLBACK;\n"), "{sql}");
        assert!(std::path::Path::new(&response.output_path).exists());

        // Selecting only the exact statements yields a clean, exact result.
        let exact_only: Vec<String> = run
            .statements
            .iter()
            .filter(|statement| statement.exact)
            .map(|statement| statement.id.clone())
            .collect();
        let clean = generate_offline_recovery_sql_in(
            root.path(),
            "connection-a",
            &RecoverySelection {
                run_ids: vec![run.run_id],
                statement_ids: exact_only,
            },
        )
        .unwrap();
        assert!(clean.exact);
        assert_eq!(clean.generated_steps, 3);
    }

    #[test]
    fn legacy_pretty_json_runs_remain_readable() {
        // G8 (2026-09-01): histories written before the JSONL migration must
        // still load for RecoveryPage listing and comparison.
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("recovery-history").join("conn").join("d");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("20260101T000000.000Z-run.recovery.json");
        let legacy = RecoveryRun {
            version: HISTORY_VERSION,
            run_id: "0123456789ABCDEFGHIJ123456".to_string(),
            short_id: "GHIJ123456".to_string(),
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: Some("2026-01-01T00:00:01Z".to_string()),
            status: "complete".to_string(),
            connection_id: "connection-a".to_string(),
            connection_name: "prod".to_string(),
            database: "app".to_string(),
            target_identity: "server-a".to_string(),
            statements: vec![statement(0)],
        };
        fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();

        let run = read_run(&path).unwrap();
        assert_eq!(run.status, "complete");
        assert_eq!(run.statements.len(), 1);
    }

    #[test]
    fn orphaned_jsonl_run_is_closed_as_interrupted_and_tolerates_a_torn_tail() {
        let root = tempfile::tempdir().unwrap();
        let mut journal = RecoveryJournal::create_in(
            root.path(),
            "connection-a".to_string(),
            "prod".to_string(),
            "app".to_string(),
            "server-a".to_string(),
        )
        .unwrap();
        journal.add_statement(statement(0)).unwrap();
        let path = journal.path.clone();
        // Simulate a crash: drop without finalize, leaving a torn append.
        drop(journal);
        {
            use std::io::Write;
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            file.write_all(b"{\"record\":\"stat").unwrap();
        }

        assert_eq!(finalize_orphaned_recovery_runs(root.path()), 1);

        let run = read_run(&path).unwrap();
        assert_eq!(run.status, "interrupted");
        assert!(run.finished_at.is_some());
        assert_eq!(run.statements.len(), 1);
        // Idempotent: a second pass finds nothing left to close.
        assert_eq!(finalize_orphaned_recovery_runs(root.path()), 0);
    }

    #[test]
    fn short_id_uses_the_random_ulid_suffix() {
        assert_eq!(short_run_id("0123456789ABCDEFGHIJ123456"), "GHIJ123456");
    }

    #[test]
    fn recorded_target_identity_accepts_machine_and_legacy_labels() {
        assert!(recorded_target_matches("uuid:ABC", "uuid:abc"));
        assert!(recorded_target_matches("MySQL server UUID abc", "uuid:abc"));
        assert!(recorded_target_matches("db-1:3306", "host:db-1:3306"));
        assert!(!recorded_target_matches("uuid:other", "uuid:abc"));
    }

    #[test]
    fn update_sql_restores_only_changed_fields_and_guards_full_row() {
        let work = RowWork {
            schema: "app".to_string(),
            table: "users".to_string(),
            columns: statement(0).columns,
            primary_key: vec!["id".to_string()],
            affected_columns: BTreeSet::from(["name".to_string()]),
            compare_all_columns: false,
            keys: BTreeSet::new(),
            order: 0,
            source_ids: BTreeSet::new(),
        };
        let current = RecoveryRow {
            values: vec!["X'31'".to_string(), "X'62'".to_string()],
        };
        let desired = RecoveryRow {
            values: vec!["X'31'".to_string(), "X'61'".to_string()],
        };
        let sql = build_update_sql(&work, &current, &desired, &[1]).unwrap();
        assert!(sql.contains("UPDATE `app`.`users` SET `name` = CAST(X'61' AS BINARY)"));
        assert!(sql.contains("CAST(`id` AS BINARY) <=> X'31'"));
        assert!(sql.contains("CAST(`name` AS BINARY) <=> X'62'"));
        assert!(sql.ends_with("LIMIT 1"));
    }

    #[test]
    fn insert_sql_is_guarded_by_primary_key_absence() {
        let work = RowWork {
            schema: "app".to_string(),
            table: "users".to_string(),
            columns: statement(0).columns,
            primary_key: vec!["id".to_string()],
            affected_columns: BTreeSet::new(),
            compare_all_columns: true,
            keys: BTreeSet::new(),
            order: 0,
            source_ids: BTreeSet::new(),
        };
        let desired = RecoveryRow {
            values: vec!["X'31'".to_string(), "X'61'".to_string()],
        };
        let sql = build_insert_sql(&work, &desired).unwrap();
        assert!(sql.contains("INSERT INTO `app`.`users`"));
        assert!(sql.contains("WHERE NOT EXISTS (SELECT 1 FROM `app`.`users`"));
        assert!(sql.contains("CAST(`id` AS BINARY) <=> X'31'"));
    }

    #[test]
    fn generated_sql_without_session_variables_identifies_both_instances() {
        let target = InstanceProbe {
            label: "target:3306/app".to_string(),
            server_key: "uuid:a".to_string(),
            database: "app".to_string(),
        };
        let backup = InstanceProbe {
            label: "backup:3306/app".to_string(),
            server_key: "uuid:b".to_string(),
            database: "app".to_string(),
        };
        let run = RecoveryRun {
            version: HISTORY_VERSION,
            run_id: "01FULLID".to_string(),
            short_id: "01SHORT".to_string(),
            started_at: "2026-07-30T00:00:00Z".to_string(),
            finished_at: Some("2026-07-30T00:00:01Z".to_string()),
            status: "complete".to_string(),
            connection_id: "connection-a".to_string(),
            connection_name: "prod".to_string(),
            database: "app".to_string(),
            target_identity: "target".to_string(),
            statements: vec![],
        };
        let steps = vec![
            RecoverySqlStep {
                order: 0,
                sql: "UPDATE `app`.`users` SET `name` = X'61' WHERE `id` = 1 LIMIT 1".to_string(),
                expected_affected_rows: Some(1),
                source: "01SHORT:1".to_string(),
            },
            RecoverySqlStep {
                order: 1,
                sql: "ALTER TABLE `app`.`users` DROP COLUMN `note`".to_string(),
                expected_affected_rows: None,
                source: "01SHORT:2".to_string(),
            },
        ];
        let sql = render_recovery_sql("connection-a", &[run], &target, &backup, &steps, &[]);
        assert!(sql.contains("-- Target connection alias: prod"));
        assert!(sql.contains("-- Expected target database: app"));
        assert!(sql.contains("-- Target instance: target:3306/app"));
        assert!(sql.contains("-- Backup instance: backup:3306/app"));
        assert!(sql.contains("-- IMPORTANT: DO NOT USE RUN ALL."));
        assert!(
            sql.contains("-- ===== Stage A: identity precheck (read-only; run separately) =====")
        );
        assert!(sql.contains("CONCAT('uuid:', @@server_uuid) AS actual_server_identity"));
        assert!(sql.contains("-- ===== Stage B1: DML recovery"));
        assert!(sql.contains("-- ===== Stage B2: DDL recovery"));
        assert!(sql.contains("-- ===== Stage C: postcheck"));
        assert!(!sql.to_ascii_lowercase().contains("password"));
        assert!(!sql.contains("@tabularis_"));
        assert!(!sql.contains("PREPARE "));
        assert!(!sql.contains("EXECUTE "));
        assert!(!sql.contains("DELIMITER "));
        assert!(!sql.contains("\nUSE "));
        assert!(sql.contains("UPDATE `app`.`users` SET `name` = X'61' WHERE `id` = 1 LIMIT 1;"));
        assert!(sql.contains("SELECT ROW_COUNT() AS recovery_step_1_affected_rows"));
        assert!(sql.contains("ALTER TABLE `app`.`users` DROP COLUMN `note`;"));
        assert!(!sql.contains("-- TABULARIS_MANUAL_DDL:"));
        assert_eq!(sql.matches("\nROLLBACK;\n").count(), 1);
        let executable_semicolons = sql
            .lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .map(|line| line.matches(';').count())
            .sum::<usize>();
        assert_eq!(executable_semicolons, 8);
        assert!(!sql.contains("\nCOMMIT;\n"));
    }

    #[test]
    fn recovery_search_filters_legacy_non_recovery_noise_and_matches_sql() {
        let mut change = statement(0);
        change.id = "RUN-001".to_string();
        change.sql = "UPDATE app.users SET name = 'old name' WHERE id = 1".to_string();
        let mut noise = statement(1);
        noise.id = "RUN-002".to_string();
        noise.sql = "SELECT * FROM app.users".to_string();
        noise.category = "unprotected".to_string();
        noise.operation = "select".to_string();
        noise.objects.clear();
        noise.exact = false;
        let mut analyze = statement(2);
        analyze.id = "RUN-003".to_string();
        analyze.sql = "ANALYZE TABLE app.users".to_string();
        analyze.category = "unprotected".to_string();
        analyze.operation = "analyze table".to_string();
        analyze.exact = false;
        let run = RecoveryRun {
            version: HISTORY_VERSION,
            run_id: "run-id".to_string(),
            short_id: "RUN".to_string(),
            started_at: "2026-08-20T00:00:00Z".to_string(),
            finished_at: Some("2026-08-20T00:00:01Z".to_string()),
            status: "complete".to_string(),
            connection_id: "connection-a".to_string(),
            connection_name: "prod".to_string(),
            database: "app".to_string(),
            target_identity: "uuid:a".to_string(),
            statements: vec![change, noise, analyze],
        };

        let all = run_summary_for_query(&run, None).unwrap();
        assert_eq!(all.statement_count, 1);
        assert_eq!(all.statements[0].id, "RUN-001");

        let hit = run_summary_for_query(&run, Some("old name")).unwrap();
        assert_eq!(hit.statement_count, 1);
        assert_eq!(hit.statements[0].id, "RUN-001");
        assert!(run_summary_for_query(&run, Some("definitely missing")).is_none());
    }

    #[test]
    fn routine_definitions_are_retargeted_to_the_target_schema() {
        let sql = "CREATE VIEW `backup_app`.`active_users` AS SELECT '`backup_app`.`literal`' AS note FROM `backup_app`.`users`";
        let retargeted = retarget_schema_qualifiers(sql, "backup_app", "app");
        assert_eq!(
            retargeted,
            "CREATE VIEW `app`.`active_users` AS SELECT '`backup_app`.`literal`' AS note FROM `app`.`users`"
        );
    }

    #[test]
    fn compound_routine_delimiters_are_detected_without_false_positives() {
        assert!(contains_executable_semicolon(
            "CREATE PROCEDURE `app`.`p`() BEGIN SELECT 1; SELECT 2; END"
        ));
        assert!(!contains_executable_semicolon(
            "CREATE VIEW `app`.`v` AS SELECT ';' AS marker /* ; */"
        ));
    }

    #[test]
    fn show_create_definer_is_removed_without_changing_security_or_body_text() {
        let sql = "CREATE ALGORITHM=UNDEFINED DEFINER=`backup``user`@`%` SQL SECURITY DEFINER VIEW `app`.`v` AS SELECT 'DEFINER=x@y' AS note";
        assert_eq!(
            strip_show_create_definer(sql),
            "CREATE ALGORITHM=UNDEFINED SQL SECURITY DEFINER VIEW `app`.`v` AS SELECT 'DEFINER=x@y' AS note"
        );
    }
}
