use crate::drivers::mysql;
use crate::models::ConnectionParams;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Executor, Row};
use std::collections::{BTreeMap, BTreeSet, HashSet};
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

pub struct RecoveryJournal {
    path: PathBuf,
    run: RecoveryRun,
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
        let path = directory.join(format!("{timestamp}-{run_id}.recovery.json"));
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
        let journal = Self { path, run };
        journal.persist()?;
        Ok(journal)
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
        self.run.statements.push(statement);
        if let Err(error) = self.persist() {
            self.run.statements.pop();
            return Err(error);
        }
        Ok(())
    }

    pub fn rewind_to(&mut self, checkpoint: usize) -> Result<(), String> {
        if checkpoint > self.run.statements.len() {
            return Err(format!(
                "Recovery journal checkpoint {checkpoint} exceeds {} recorded statements",
                self.run.statements.len()
            ));
        }
        self.run.statements.truncate(checkpoint);
        self.persist()
    }

    pub fn finalize(mut self) -> Result<PathBuf, String> {
        self.run.status = "complete".to_string();
        self.run.finished_at = Some(chrono::Utc::now().to_rfc3339());
        self.persist()?;
        Ok(self.path)
    }

    /// Marks the run as interrupted (e.g. a COMMIT whose outcome is unknown)
    /// instead of leaving it in "recording" forever. Interrupted runs stay
    /// visible and comparable — that is exactly the situation where the
    /// operator needs the recovery history most.
    pub fn interrupt(mut self, reason: &str) -> Result<PathBuf, String> {
        self.run.status = "interrupted".to_string();
        self.run.finished_at = Some(chrono::Utc::now().to_rfc3339());
        log::warn!(
            "Recovery run {} marked interrupted: {reason}",
            self.run.short_id
        );
        self.persist()?;
        Ok(self.path)
    }

    pub fn discard(self) -> Result<(), String> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "Could not discard recovery history {}: {error}",
                self.path.display()
            )),
        }
    }

    fn persist(&self) -> Result<(), String> {
        // fsync-backed writes run on the caller's thread while the pinned
        // session lock is held — hand them to a blocking-safe context so a
        // large batch cannot starve the async workers.
        crate::rollback_sql::run_blocking(|| {
            let payload = serde_json::to_vec_pretty(&self.run)
                .map_err(|error| format!("Could not serialize recovery history: {error}"))?;
            let temporary = self
                .path
                .with_extension(format!("json.tmp-{}", ulid::Ulid::new()));
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|error| {
                    format!("Could not create recovery history revision: {error}")
                })?;
            file.write_all(&payload)
                .map_err(|error| format!("Could not write recovery history revision: {error}"))?;
            file.flush()
                .map_err(|error| format!("Could not flush recovery history revision: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("Could not sync recovery history revision: {error}"))?;
            drop(file);

            if let Err(error) = fs::rename(&temporary, &self.path) {
                let _ = fs::remove_file(&temporary);
                return Err(format!("Could not publish recovery history: {error}"));
            }
            Ok(())
        })
    }
}

#[tauri::command]
pub fn list_recovery_runs(
    connection_id: Option<String>,
    started_after: Option<String>,
    started_before: Option<String>,
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
        summaries.push(run_summary(&run));
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
        return Err("Select at least one recovery Run All record".to_string());
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
    let base_database = &runs[0].database;

    for (sequence, (_, statement)) in statements.iter().copied().enumerate() {
        if !statement.exact {
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
        let backup_schema =
            mapped_backup_schema(&work.schema, base_database, &backup_probe.database);
        // One round-trip per batch instead of two per key: thousands of rows
        // would otherwise mean thousands of sequential single-row SELECTs.
        let keys: Vec<&Vec<String>> = work.keys.iter().collect();
        let current_rows = fetch_rows_batch(target, &work.schema, work, &keys).await?;
        let desired_rows = fetch_rows_batch(backup, &backup_schema, work, &keys).await?;
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

    for (_, statement) in statements {
        if !statement.exact {
            conflicts.push(format!(
                "{} was executed without an exact row/schema image",
                statement.id
            ));
        }
    }
    steps.sort_by(|left, right| right.order.cmp(&left.order));
    Ok((steps, unchanged_rows, conflicts))
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
    output.push_str("-- Recovery records are isolated from protected Run All rollback files.\n");
    output.push_str("-- No Tabularis session variables or prepared-statement wrappers are used.\n");
    output.push_str("-- Verify the target instance in this header before execution.\n");
    output.push_str(
        "-- DML groups end in ROLLBACK by default; replace it with COMMIT only after reviewing every affected-row result.\n",
    );
    output.push_str(
        "-- MySQL/MariaDB DDL implicitly commits, so DDL is commented and must be reviewed and executed separately.\n",
    );
    output.push_str(&format!(
        "-- Target connection ID: {}\n",
        single_line_comment(connection_id)
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
        "-- Selected Run All IDs: {}\n",
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

    let mut transaction_open = false;
    for (index, step) in steps.iter().enumerate() {
        let transactional = step.expected_affected_rows.is_some();
        if transactional && !transaction_open {
            output.push_str("START TRANSACTION;\n\n");
            transaction_open = true;
        } else if !transactional && transaction_open {
            append_transaction_finish(&mut output);
            transaction_open = false;
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
            output.push_str(
                "-- MANUAL DDL: implicit commit boundary; review and execute this statement separately.\n",
            );
            append_manual_ddl(&mut output, &step.sql);
        }
        output.push('\n');
    }
    if transaction_open {
        append_transaction_finish(&mut output);
    }
    if steps.is_empty() {
        output.push_str("-- No row or schema differences were found for the selection.\n\n");
    }
    output
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

fn append_manual_ddl(output: &mut String, sql: &str) {
    let sql = sql.trim_end();
    let lines = sql.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        output.push_str("-- TABULARIS_MANUAL_DDL: ;\n");
        return;
    }
    for (index, line) in lines.iter().enumerate() {
        output.push_str("-- TABULARIS_MANUAL_DDL: ");
        output.push_str(line);
        if index + 1 == lines.len() && !sql.ends_with(';') {
            output.push(';');
        }
        output.push('\n');
    }
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
                    .is_some_and(|name| name.ends_with(".recovery.json"))
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
    let run: RecoveryRun = serde_json::from_slice(&content).map_err(|error| {
        format!(
            "Could not parse recovery history {}: {error}",
            path.display()
        )
    })?;
    if run.version != HISTORY_VERSION {
        return Err(format!(
            "Unsupported recovery history version {} in {}",
            run.version,
            path.display()
        ));
    }
    Ok(run)
}

fn run_summary(run: &RecoveryRun) -> RecoveryRunSummary {
    RecoveryRunSummary {
        run_id: run.run_id.clone(),
        short_id: run.short_id.clone(),
        started_at: run.started_at.clone(),
        finished_at: run.finished_at.clone(),
        status: run.status.clone(),
        connection_id: run.connection_id.clone(),
        connection_name: run.connection_name.clone(),
        database: run.database.clone(),
        statement_count: run.statements.len(),
        statements: run
            .statements
            .iter()
            .map(|statement| {
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
            })
            .collect(),
    }
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
        assert!(sql.contains("-- Target instance: target:3306/app"));
        assert!(sql.contains("-- Backup instance: backup:3306/app"));
        assert!(!sql.to_ascii_lowercase().contains("password"));
        assert!(!sql.contains("@tabularis_"));
        assert!(!sql.contains("PREPARE "));
        assert!(!sql.contains("EXECUTE "));
        assert!(sql.contains("UPDATE `app`.`users` SET `name` = X'61' WHERE `id` = 1 LIMIT 1;"));
        assert!(sql.contains("SELECT ROW_COUNT() AS recovery_step_1_affected_rows"));
        assert!(
            sql.contains("-- TABULARIS_MANUAL_DDL: ALTER TABLE `app`.`users` DROP COLUMN `note`;")
        );
        assert_eq!(sql.matches("\nROLLBACK;\n").count(), 1);
        assert!(!sql.contains("\nCOMMIT;\n"));
    }
}
