use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Runs fsync-heavy journal IO without starving the async runtime.
///
/// Journal writes happen while the pinned-session mutex is held, on a tokio
/// worker thread. `block_in_place` moves the worker out of the scheduler for
/// the duration; outside a multi-thread runtime (unit tests) the closure just
/// runs inline.
pub(crate) fn run_blocking<T>(work: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread =>
        {
            tokio::task::block_in_place(work)
        }
        _ => work(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ServerIdentity {
    Uuid(String),
    Uid(String),
    HostPort { hostname: String, port: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackEnvironment {
    pub connection_id: String,
    pub connection_name: String,
    pub database: String,
    pub current_user: String,
    pub server: ServerIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackStep {
    pub statement_index: usize,
    pub sql: String,
    pub expected_affected_rows: Option<u64>,
}

/// One line of the append-only steps journal.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
enum JournalRecord {
    Header {
        version: u32,
        environment: RollbackEnvironment,
    },
    Step {
        step: RollbackStep,
    },
    /// A rolled-back explicit transaction truncates the recorded steps back
    /// to `checkpoint`. Appended instead of rewriting the file.
    Rewind {
        checkpoint: usize,
    },
}

const STEPS_VERSION: u32 = 1;
const STEPS_SUFFIX: &str = ".rollback-steps.ndjson";
const CRASH_SUFFIX: &str = ".crash.rollback.sql";

/// Durable, connection-isolated rollback journal.
///
/// Append-only: every step is one fsynced NDJSON line, so a batch of `n`
/// statements writes O(n) bytes instead of rewriting the whole journal per
/// statement (the previous design cost ~n²/2 — ~673 MB for a real 3,429-step
/// batch). The executable `.rollback.sql` is rendered once at `finalize`.
/// A crash leaves the steps journal on disk; `render_orphaned_step_journals`
/// turns it into an executable crash-recovery SQL file at next startup.
pub struct RollbackJournal {
    directory: PathBuf,
    stem: String,
    final_path: PathBuf,
    steps_path: PathBuf,
    file: File,
    environment: RollbackEnvironment,
    steps: Vec<RollbackStep>,
    synced_len: u64,
    bytes_written: u64,
    poisoned: bool,
}

impl RollbackJournal {
    pub fn create(environment: RollbackEnvironment) -> Result<Self, String> {
        let data_dir = crate::paths::get_app_data_dir()
            .ok_or_else(|| "Could not resolve the Tabularis data directory".to_string())?;
        Self::create_in(&data_dir, environment)
    }

    fn create_in(root: &Path, environment: RollbackEnvironment) -> Result<Self, String> {
        let connection_segment =
            isolated_connection_segment(&environment.connection_name, &environment.connection_id);
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let directory = root
            .join("rollback-sql")
            .join(connection_segment)
            .join(date);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Could not create rollback directory: {error}"))?;

        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        let stem = format!("{timestamp}-{}", ulid::Ulid::new());
        let final_path = directory.join(format!("{stem}.rollback.sql"));
        let steps_path = directory.join(format!("{stem}{STEPS_SUFFIX}"));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&steps_path)
            .map_err(|error| format!("Could not create rollback steps journal: {error}"))?;
        let mut journal = Self {
            directory,
            stem,
            final_path,
            steps_path,
            file,
            environment,
            steps: Vec::new(),
            synced_len: 0,
            bytes_written: 0,
            poisoned: false,
        };
        let header = JournalRecord::Header {
            version: STEPS_VERSION,
            environment: journal.environment.clone(),
        };
        journal.append_records(std::slice::from_ref(&header))?;
        Ok(journal)
    }

    pub fn planned_final_path(&self) -> &Path {
        &self.final_path
    }

    /// The durable on-disk artifact that survives a crash before `finalize`.
    pub fn current_recovery_path(&self) -> &Path {
        &self.steps_path
    }

    pub fn checkpoint(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub fn add_step(&mut self, step: RollbackStep) -> Result<(), String> {
        self.add_steps(vec![step])
    }

    pub fn add_steps(&mut self, steps: Vec<RollbackStep>) -> Result<(), String> {
        if steps.is_empty() {
            return Ok(());
        }
        let records: Vec<JournalRecord> = steps
            .iter()
            .cloned()
            .map(|step| JournalRecord::Step { step })
            .collect();
        self.append_records(&records)?;
        self.steps.extend(steps);
        Ok(())
    }

    pub fn rewind_to(&mut self, checkpoint: usize) -> Result<(), String> {
        if checkpoint > self.steps.len() {
            return Err(format!(
                "Rollback journal checkpoint {checkpoint} exceeds {} recorded steps",
                self.steps.len()
            ));
        }
        if checkpoint == self.steps.len() {
            return Ok(());
        }
        self.append_records(&[JournalRecord::Rewind { checkpoint }])?;
        self.steps.truncate(checkpoint);
        Ok(())
    }

    /// Appends records as NDJSON lines with one flush+fsync for the batch.
    /// On a write error the file is truncated back to the last synced length
    /// so a partial line can never corrupt later appends.
    fn append_records(&mut self, records: &[JournalRecord]) -> Result<(), String> {
        if self.poisoned {
            return Err(
                "Rollback steps journal is unusable after an earlier write failure".to_string(),
            );
        }
        let mut buffer = Vec::new();
        for record in records {
            serde_json::to_writer(&mut buffer, record)
                .map_err(|error| format!("Could not serialize rollback step: {error}"))?;
            buffer.push(b'\n');
        }
        let outcome = run_blocking(|| {
            self.file
                .write_all(&buffer)
                .and_then(|()| self.file.flush())
                .and_then(|()| self.file.sync_all())
        });
        match outcome {
            Ok(()) => {
                self.synced_len += buffer.len() as u64;
                self.bytes_written += buffer.len() as u64;
                Ok(())
            }
            Err(error) => {
                if self.file.set_len(self.synced_len).is_err() {
                    self.poisoned = true;
                }
                Err(format!("Could not append to rollback steps journal: {error}"))
            }
        }
    }

    pub fn finalize(self) -> Result<PathBuf, String> {
        if self.final_path.exists() {
            return Err(format!(
                "Rollback destination already exists: {}",
                self.final_path.display()
            ));
        }
        let Self {
            directory,
            stem,
            final_path,
            steps_path,
            file,
            environment,
            steps,
            ..
        } = self;
        drop(file);
        run_blocking(|| {
            let rendered = render_rollback_sql(&environment, &steps);
            let temporary = directory.join(format!("{stem}.rollback.tmp"));
            let mut out = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|error| format!("Could not create rollback SQL file: {error}"))?;
            let write = out
                .write_all(rendered.as_bytes())
                .and_then(|()| out.flush())
                .and_then(|()| out.sync_all());
            drop(out);
            if let Err(error) = write {
                let _ = fs::remove_file(&temporary);
                return Err(format!("Could not write rollback SQL file: {error}"));
            }
            if let Err(error) = fs::rename(&temporary, &final_path) {
                let _ = fs::remove_file(&temporary);
                return Err(format!("Could not finalize rollback SQL: {error}"));
            }
            let _ = fs::remove_file(&steps_path);
            Ok(final_path)
        })
    }

    /// Renders the recorded steps to an executable crash-recovery SQL file
    /// immediately, for paths that must walk away from the journal (a COMMIT
    /// whose outcome is unknown). Returns the path the operator should open;
    /// on a render failure the durable steps journal is kept and returned —
    /// the next startup renders it.
    pub fn abandon(self) -> PathBuf {
        let Self {
            stem,
            steps_path,
            file,
            environment,
            steps,
            ..
        } = self;
        drop(file);
        if steps.is_empty() {
            let _ = fs::remove_file(&steps_path);
            return steps_path;
        }
        let crash_path = steps_path.with_file_name(format!("{stem}{CRASH_SUFFIX}"));
        let rendered = render_rollback_sql(&environment, &steps);
        let outcome = run_blocking(|| {
            let mut out = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&crash_path)?;
            out.write_all(rendered.as_bytes())
                .and_then(|()| out.flush())
                .and_then(|()| out.sync_all())
        });
        match outcome {
            Ok(()) => {
                let _ = fs::remove_file(&steps_path);
                crash_path
            }
            Err(error) => {
                log::warn!(
                    "Could not render abandoned rollback journal {}: {error}",
                    crash_path.display()
                );
                steps_path
            }
        }
    }

    /// Removes only this unfinished journal's steps file. Finalized rollback
    /// files are never deleted by this method.
    pub fn discard(self) -> Result<(), String> {
        if self.final_path.exists() {
            return Err(format!(
                "Refusing to discard finalized rollback SQL: {}",
                self.final_path.display()
            ));
        }
        let steps_path = self.steps_path.clone();
        drop(self.file);
        match fs::remove_file(&steps_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "Could not discard rollback steps journal {}: {error}",
                steps_path.display()
            )),
        }
    }
}

/// Parses a steps journal. Tolerates one trailing partial line (a write cut
/// short by a crash); any earlier unparseable line aborts with an error so a
/// corrupted journal is never silently mis-rendered.
fn parse_step_journal(content: &str) -> Result<(RollbackEnvironment, Vec<RollbackStep>), String> {
    let mut lines = content.lines().enumerate().peekable();
    let (_, first) = lines
        .next()
        .ok_or_else(|| "steps journal is empty".to_string())?;
    let JournalRecord::Header { version, environment } = serde_json::from_str(first)
        .map_err(|error| format!("unreadable steps journal header: {error}"))?
    else {
        return Err("steps journal does not start with a header".to_string());
    };
    if version != STEPS_VERSION {
        return Err(format!("unsupported steps journal version {version}"));
    }
    let mut steps = Vec::new();
    while let Some((_, line)) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<JournalRecord>(line) {
            Ok(JournalRecord::Step { step }) => steps.push(step),
            Ok(JournalRecord::Rewind { checkpoint }) => {
                steps.truncate(checkpoint);
            }
            Ok(JournalRecord::Header { .. }) => {
                return Err("unexpected second header in steps journal".to_string());
            }
            Err(error) => {
                if lines.peek().is_none() {
                    // Trailing partial line: the crash interrupted this very
                    // write, so the step it carried never committed.
                    break;
                }
                return Err(format!("corrupted steps journal line: {error}"));
            }
        }
    }
    Ok((environment, steps))
}

/// Renders every crash-orphaned steps journal under `<root>/rollback-sql`
/// into an executable `.crash.rollback.sql` and removes the journal. Called
/// once at startup, before any new batch can run, so it never races a live
/// journal. Unreadable journals are left in place and logged.
pub fn render_orphaned_step_journals(data_root: &Path) -> usize {
    let rollback_root = data_root.join("rollback-sql");
    let mut rendered = 0usize;
    let connections = match fs::read_dir(&rollback_root) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    for connection in connections.flatten() {
        let Ok(dates) = fs::read_dir(connection.path()) else {
            continue;
        };
        for date in dates.flatten() {
            let Ok(entries) = fs::read_dir(date.path()) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if !name.ends_with(STEPS_SUFFIX) {
                    continue;
                }
                match render_orphaned_journal(&path, name) {
                    Ok(true) => rendered += 1,
                    Ok(false) => {}
                    Err(error) => {
                        log::warn!(
                            "Could not render orphaned rollback journal {}: {error}",
                            path.display()
                        );
                    }
                }
            }
        }
    }
    rendered
}

fn render_orphaned_journal(path: &Path, file_name: &str) -> Result<bool, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("could not read steps journal: {error}"))?;
    let (environment, steps) = parse_step_journal(&content)?;
    if steps.is_empty() {
        fs::remove_file(path)
            .map_err(|error| format!("could not remove empty steps journal: {error}"))?;
        return Ok(false);
    }
    let stem = file_name
        .strip_suffix(STEPS_SUFFIX)
        .expect("caller matched the suffix");
    let out_path = path.with_file_name(format!("{stem}{CRASH_SUFFIX}"));
    if !out_path.exists() {
        let rendered = render_rollback_sql(&environment, &steps);
        let mut out = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&out_path)
            .map_err(|error| format!("could not create crash rollback SQL: {error}"))?;
        out.write_all(rendered.as_bytes())
            .and_then(|()| out.flush())
            .and_then(|()| out.sync_all())
            .map_err(|error| format!("could not write crash rollback SQL: {error}"))?;
    }
    fs::remove_file(path)
        .map_err(|error| format!("could not remove rendered steps journal: {error}"))?;
    log::warn!(
        "Rendered crash-orphaned rollback journal to {}",
        out_path.display()
    );
    Ok(true)
}

fn isolated_connection_segment(connection_name: &str, connection_id: &str) -> String {
    let readable = safe_connection_name_segment(connection_name);
    let digest = Sha256::digest(connection_id.as_bytes());
    format!("{readable}-{}", hex_prefix(&digest, 8))
}

fn safe_connection_name_segment(connection_name: &str) -> String {
    let mut readable = String::new();
    let mut replaced_invalid = false;
    for ch in connection_name.trim().chars().take(64) {
        let invalid =
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            if !replaced_invalid && !readable.is_empty() {
                readable.push('_');
            }
            replaced_invalid = true;
        } else {
            readable.push(ch);
            replaced_invalid = false;
        }
    }

    let mut readable = readable
        .trim_matches(|ch| matches!(ch, ' ' | '.' | '_'))
        .to_string();
    if readable.is_empty() {
        readable = "connection".to_string();
    }
    if is_windows_reserved_segment(&readable) {
        readable.insert(0, '_');
    }
    readable
}

fn is_windows_reserved_segment(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_uppercase();
    let stem_bytes = stem.as_bytes();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem_bytes.len() == 4
            && matches!(&stem_bytes[..3], b"COM" | b"LPT")
            && matches!(stem_bytes[3], b'1'..=b'9'))
}

fn render_rollback_sql(environment: &RollbackEnvironment, steps: &[RollbackStep]) -> String {
    let connection_label = single_line_comment(&environment.connection_id);
    let connection_name = single_line_comment(&environment.connection_name);
    let server_identity = single_line_comment(&server_identity_label(&environment.server));

    let mut output = String::new();
    output.push_str("-- Tabularis protected rollback SQL\n");
    output.push_str("-- Generated before each protected write was committed.\n");
    output.push_str("-- No Tabularis session variables or prepared-statement wrappers are used.\n");
    output.push_str("-- Verify the target identity in this header before execution.\n");
    output.push_str("-- Guarded row predicates prevent nonmatching rows from being changed.\n");
    output.push_str(
        "-- DML groups end in ROLLBACK by default; replace it with COMMIT only after reviewing every affected-row result.\n",
    );
    output.push_str(
        "-- MySQL/MariaDB DDL implicitly commits, so DDL is commented and must be reviewed and executed separately.\n",
    );
    output.push_str(&format!("-- Connection name: {connection_name}\n"));
    output.push_str(&format!("-- Connection ID: {connection_label}\n"));
    output.push_str(&format!("-- Expected server identity: {server_identity}\n"));
    output.push_str(&format!(
        "-- Expected database: {}\n",
        single_line_comment(&environment.database)
    ));
    output.push_str(&format!(
        "-- Expected authenticated user: {}\n\n",
        single_line_comment(&environment.current_user)
    ));

    let mut transaction_open = false;
    for (rollback_order, step) in steps.iter().rev().enumerate() {
        let transactional = step.expected_affected_rows.is_some();
        if transactional && !transaction_open {
            output.push_str("START TRANSACTION;\n\n");
            transaction_open = true;
        } else if !transactional && transaction_open {
            append_transaction_finish(&mut output);
            transaction_open = false;
        }

        output.push_str(&format!(
            "-- Rollback step {} for original statement {}\n",
            rollback_order + 1,
            step.statement_index + 1
        ));
        if let Some(expected) = step.expected_affected_rows {
            append_direct_statement(&mut output, &step.sql);
            output.push_str(&format!(
                "SELECT ROW_COUNT() AS rollback_step_{}_affected_rows, {expected} AS expected_affected_rows;\n",
                rollback_order + 1
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
        output.push_str("-- No rollback steps were recorded.\n");
    }
    output
}

fn server_identity_label(server: &ServerIdentity) -> String {
    match server {
        ServerIdentity::Uuid(uuid) => format!("MySQL server UUID {uuid}"),
        ServerIdentity::Uid(uid) => format!("MariaDB server UID {uid}"),
        ServerIdentity::HostPort { hostname, port } => {
            format!("server host {hostname}:{port}")
        }
    }
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

fn hex_prefix(bytes: &[u8], byte_count: usize) -> String {
    bytes
        .iter()
        .take(byte_count)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn single_line_comment(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn environment(connection_id: &str) -> RollbackEnvironment {
        RollbackEnvironment {
            connection_id: connection_id.to_string(),
            connection_name: "生产主库".to_string(),
            database: "app".to_string(),
            current_user: "writer@%".to_string(),
            server: ServerIdentity::Uuid("server-uuid-1".to_string()),
        }
    }

    #[test]
    fn names_the_directory_after_the_connection_alias() {
        let root = tempfile::tempdir().unwrap();
        let mut environment = environment("7aaee341-e9f4-4fd7-ad19-e5848cb07eeb");
        environment.connection_name = "mysql-yy".to_string();
        let journal = RollbackJournal::create_in(root.path(), environment).unwrap();
        let relative = journal
            .planned_final_path()
            .strip_prefix(root.path().join("rollback-sql"))
            .unwrap();
        let directory = relative
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy();

        assert_eq!(directory, "mysql-yy-259e10e712db51ec");
        assert!(!directory.contains("7aaee341"), "{directory}");
        let sql = fs::read_to_string(journal.finalize().unwrap()).unwrap();
        assert!(sql.contains("-- Connection name: mysql-yy"));
        assert!(sql.contains("-- Connection ID: 7aaee341-e9f4-4fd7-ad19-e5848cb07eeb"));
        assert!(sql.contains("-- Expected server identity: MySQL server UUID server-uuid-1"));
    }

    #[test]
    fn safely_handles_unicode_and_windows_reserved_connection_names() {
        assert_eq!(safe_connection_name_segment("😀"), "😀");
        assert_eq!(safe_connection_name_segment("生产/主库:*?"), "生产_主库");
        assert_eq!(safe_connection_name_segment("COM1.txt"), "_COM1.txt");
    }

    #[test]
    fn keeps_same_named_connections_in_distinct_directories() {
        let root = tempfile::tempdir().unwrap();
        let first = RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        let second = RollbackJournal::create_in(root.path(), environment("connection-b")).unwrap();
        let first_directory = first
            .planned_final_path()
            .strip_prefix(root.path().join("rollback-sql"))
            .unwrap()
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_owned();
        let second_directory = second
            .planned_final_path()
            .strip_prefix(root.path().join("rollback-sql"))
            .unwrap()
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_owned();

        assert_ne!(first_directory, second_directory);
    }

    #[test]
    fn renders_reverse_order_without_session_variables_or_prepared_statements() {
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("prod/../../escape")).unwrap();
        journal
            .add_step(RollbackStep {
                statement_index: 0,
                sql: "DELETE FROM `app`.`users` WHERE `id` = 1".to_string(),
                expected_affected_rows: Some(1),
            })
            .unwrap();
        journal
            .add_step(RollbackStep {
                statement_index: 1,
                sql: "UPDATE `app`.`users` SET `name` = X'416461' WHERE `id` = 1".to_string(),
                expected_affected_rows: Some(1),
            })
            .unwrap();
        let final_path = journal.finalize().unwrap();
        let sql = fs::read_to_string(&final_path).unwrap();

        assert!(final_path.starts_with(root.path().join("rollback-sql")));
        assert!(!final_path.to_string_lossy().contains(".."));
        assert!(!sql.contains("@tabularis_"));
        assert!(!sql.contains("PREPARE "));
        assert!(!sql.contains("EXECUTE "));
        assert!(!sql.contains("@@SESSION."));
        assert!(sql.contains("-- Verify the target identity in this header before execution."));
        assert_eq!(sql.matches("START TRANSACTION;").count(), 1);
        assert_eq!(
            sql.matches("SELECT ROW_COUNT() AS rollback_step_").count(),
            2
        );
        assert_eq!(sql.matches("\nROLLBACK;\n").count(), 1);
        assert!(!sql.contains("\nCOMMIT;\n"));
        let update = "UPDATE `app`.`users` SET `name` = X'416461' WHERE `id` = 1;";
        let delete = "DELETE FROM `app`.`users` WHERE `id` = 1;";
        assert!(sql.contains(update));
        assert!(sql.contains(delete));
        assert!(sql.find(update).unwrap() < sql.find(delete).unwrap());
    }

    #[test]
    fn starts_a_fresh_transaction_after_each_ddl_boundary() {
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        journal
            .add_steps(vec![
                RollbackStep {
                    statement_index: 0,
                    sql: "UPDATE `app`.`users` SET `name` = X'41' WHERE `id` = 1".to_string(),
                    expected_affected_rows: Some(1),
                },
                RollbackStep {
                    statement_index: 1,
                    sql: "ALTER TABLE `app`.`users` DROP COLUMN `note`".to_string(),
                    expected_affected_rows: None,
                },
                RollbackStep {
                    statement_index: 2,
                    sql: "DELETE FROM `app`.`users` WHERE `id` = 2".to_string(),
                    expected_affected_rows: Some(1),
                },
            ])
            .unwrap();
        let sql = fs::read_to_string(journal.finalize().unwrap()).unwrap();
        let ddl = "ALTER TABLE `app`.`users` DROP COLUMN `note`";

        assert_eq!(sql.matches("START TRANSACTION;").count(), 2);
        assert_eq!(sql.matches("\nROLLBACK;\n").count(), 2);
        assert!(sql.contains(&format!("-- TABULARIS_MANUAL_DDL: {ddl};")));
        assert!(!sql.lines().any(|line| line == format!("{ddl};")));
        assert!(sql.find("ROLLBACK;").unwrap() < sql.find(ddl).unwrap());
        assert!(sql.find(ddl).unwrap() < sql.rfind("START TRANSACTION;").unwrap());
    }

    #[test]
    fn keeps_a_durable_steps_journal_before_finalize() {
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        assert!(journal.current_recovery_path().exists());
        journal
            .add_step(RollbackStep {
                statement_index: 0,
                sql: "DROP TABLE `app`.`new_table`".to_string(),
                expected_affected_rows: None,
            })
            .unwrap();
        assert!(journal.current_recovery_path().exists());
        assert!(!journal.planned_final_path().exists());
    }

    #[test]
    fn crash_orphaned_steps_journal_renders_to_executable_sql() {
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        journal
            .add_step(RollbackStep {
                statement_index: 0,
                sql: "DELETE FROM `app`.`users` WHERE `id` = 7".to_string(),
                expected_affected_rows: Some(1),
            })
            .unwrap();
        let steps_path = journal.current_recovery_path().to_path_buf();
        // Simulate a crash: the journal object goes away without finalize or
        // discard; the steps file stays behind.
        drop(journal);
        assert!(steps_path.exists());

        let rendered = render_orphaned_step_journals(root.path());

        assert_eq!(rendered, 1);
        assert!(!steps_path.exists());
        let crash_path = steps_path
            .to_string_lossy()
            .replace(STEPS_SUFFIX, CRASH_SUFFIX);
        let sql = fs::read_to_string(&crash_path).unwrap();
        assert!(sql.contains("DELETE FROM `app`.`users` WHERE `id` = 7;"));
        assert!(sql.contains("ROLLBACK;\n"));
    }

    #[test]
    fn orphan_renderer_honors_rewinds_and_tolerates_a_partial_trailing_line() {
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        let retained = "DELETE FROM `app`.`users` WHERE `id` = 1";
        let rolled_back = "DELETE FROM `app`.`users` WHERE `id` = 2";
        journal
            .add_step(RollbackStep {
                statement_index: 0,
                sql: retained.to_string(),
                expected_affected_rows: Some(1),
            })
            .unwrap();
        let checkpoint = journal.checkpoint();
        journal
            .add_step(RollbackStep {
                statement_index: 1,
                sql: rolled_back.to_string(),
                expected_affected_rows: Some(1),
            })
            .unwrap();
        journal.rewind_to(checkpoint).unwrap();
        let steps_path = journal.current_recovery_path().to_path_buf();
        drop(journal);
        // A crash mid-append leaves a torn line at the tail.
        {
            let mut file = OpenOptions::new().append(true).open(&steps_path).unwrap();
            file.write_all(b"{\"record\":\"step\",\"st").unwrap();
        }

        assert_eq!(render_orphaned_step_journals(root.path()), 1);
        let crash_path = steps_path
            .to_string_lossy()
            .replace(STEPS_SUFFIX, CRASH_SUFFIX);
        let sql = fs::read_to_string(&crash_path).unwrap();
        assert!(sql.contains(retained));
        assert!(!sql.contains(rolled_back));
    }

    #[test]
    fn journal_write_amplification_stays_linear() {
        // G6 (2026-09-01): the previous design re-rendered the whole journal
        // per statement (~n²/2 bytes; ~673 MB for the real 3,429-step batch).
        // Append-only must stay O(n).
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        for index in 0..3_429usize {
            journal
                .add_step(RollbackStep {
                    statement_index: index,
                    sql: format!(
                        "DELETE FROM `app`.`orders` WHERE `id` = {index} AND `state` = X'6e6577' LIMIT 1"
                    ),
                    expected_affected_rows: Some(1),
                })
                .unwrap();
        }
        let bytes_written = journal.bytes_written();
        let steps_len = fs::metadata(journal.current_recovery_path()).unwrap().len();
        assert_eq!(bytes_written, steps_len, "append-only: no rewrites");
        assert!(
            bytes_written < 5 * 1024 * 1024,
            "3,429 steps must stay under 5 MB of journal writes, got {bytes_written}"
        );
        let final_path = journal.finalize().unwrap();
        assert!(final_path.exists());
    }

    #[test]
    fn rewinds_steps_from_a_rolled_back_explicit_transaction() {
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        let retained_sql = "DROP TABLE `app`.`new_table`";
        journal
            .add_step(RollbackStep {
                statement_index: 0,
                sql: retained_sql.to_string(),
                expected_affected_rows: None,
            })
            .unwrap();
        let checkpoint = journal.checkpoint();
        let rolled_back_sql = "UPDATE `app`.`users` SET `name` = X'41' WHERE `id` = 1";
        journal
            .add_step(RollbackStep {
                statement_index: 2,
                sql: rolled_back_sql.to_string(),
                expected_affected_rows: Some(1),
            })
            .unwrap();

        journal.rewind_to(checkpoint).unwrap();
        let path = journal.finalize().unwrap();
        let sql = fs::read_to_string(path).unwrap();
        assert!(sql.contains(retained_sql));
        assert!(!sql.contains(rolled_back_sql));
    }

    #[test]
    fn discard_removes_only_the_unfinished_steps_journal() {
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        journal
            .add_step(RollbackStep {
                statement_index: 0,
                sql: "DELETE FROM `app`.`users` WHERE `id` = 1".to_string(),
                expected_affected_rows: Some(1),
            })
            .unwrap();
        let recovery_path = journal.current_recovery_path().to_path_buf();
        let final_path = journal.planned_final_path().to_path_buf();

        journal.discard().unwrap();

        assert!(!recovery_path.exists());
        assert!(!final_path.exists());
    }

    #[test]
    fn renders_mariadb_server_uid_when_available() {
        let root = tempfile::tempdir().unwrap();
        let mut environment = environment("mariadb-connection");
        environment.server = ServerIdentity::Uid("mariadb-server-uid".to_string());
        let path = RollbackJournal::create_in(root.path(), environment)
            .unwrap()
            .finalize()
            .unwrap();
        let sql = fs::read_to_string(path).unwrap();
        assert!(sql.contains("-- Expected server identity: MariaDB server UID mariadb-server-uid"));
        assert!(!sql.contains("@@server_uid"));
        assert!(!sql.contains("@@server_uuid"));
    }

    #[test]
    fn concurrent_batches_get_distinct_connection_isolated_files() {
        let root = tempfile::tempdir().unwrap();
        let root = Arc::new(root.path().to_path_buf());
        let barrier = Arc::new(Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let root = Arc::clone(&root);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    RollbackJournal::create_in(&root, environment("connection-a"))
                        .unwrap()
                        .finalize()
                        .unwrap()
                })
            })
            .collect();
        let mut paths: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), 8);
    }

    #[test]
    fn refuses_to_write_when_the_data_root_is_a_file() {
        let root = tempfile::tempdir().unwrap();
        let invalid_root = root.path().join("not-a-directory");
        File::create(&invalid_root).unwrap();
        let error = RollbackJournal::create_in(&invalid_root, environment("connection-a"))
            .err()
            .expect("must fail closed");
        assert!(error.contains("Could not create rollback directory"));
    }
}
