use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerIdentity {
    Uuid(String),
    Uid(String),
    HostPort { hostname: String, port: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackEnvironment {
    pub connection_id: String,
    pub connection_name: String,
    pub database: String,
    pub current_user: String,
    pub server: ServerIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackStep {
    pub statement_index: usize,
    pub sql: String,
    pub expected_affected_rows: Option<u64>,
}

/// Durable, connection-isolated rollback journal.
///
/// Every revision is written to a new file and synced before the previous
/// revision is removed. A crash therefore leaves at least one executable
/// recovery file. The latest revision is renamed to `.rollback.sql` only when
/// the protected batch finishes.
pub struct RollbackJournal {
    directory: PathBuf,
    stem: String,
    final_path: PathBuf,
    environment: RollbackEnvironment,
    steps: Vec<RollbackStep>,
    revision: u32,
    current_revision_path: PathBuf,
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
        let current_revision_path = directory.join(format!("{stem}.recovery.0000.sql"));
        let mut journal = Self {
            directory,
            stem,
            final_path,
            environment,
            steps: Vec::new(),
            revision: 0,
            current_revision_path,
        };
        journal.persist_current_revision(None)?;
        Ok(journal)
    }

    pub fn planned_final_path(&self) -> &Path {
        &self.final_path
    }

    pub fn current_recovery_path(&self) -> &Path {
        &self.current_revision_path
    }

    pub fn checkpoint(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn add_step(&mut self, step: RollbackStep) -> Result<(), String> {
        self.add_steps(vec![step])
    }

    pub fn add_steps(&mut self, steps: Vec<RollbackStep>) -> Result<(), String> {
        let original_len = self.steps.len();
        self.steps.extend(steps);
        if let Err(error) = self.persist_next_revision() {
            self.steps.truncate(original_len);
            return Err(error);
        }
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

        let removed = self.steps.split_off(checkpoint);
        if let Err(error) = self.persist_next_revision() {
            self.steps.extend(removed);
            return Err(error);
        }
        Ok(())
    }

    pub fn finalize(self) -> Result<PathBuf, String> {
        if self.final_path.exists() {
            return Err(format!(
                "Rollback destination already exists: {}",
                self.final_path.display()
            ));
        }
        fs::rename(&self.current_revision_path, &self.final_path)
            .map_err(|error| format!("Could not finalize rollback SQL: {error}"))?;
        self.remove_stale_revisions(None);
        Ok(self.final_path)
    }

    /// Removes only this unfinished journal's isolated recovery revisions.
    /// Finalized rollback files are never deleted by this method.
    pub fn discard(self) -> Result<(), String> {
        if self.final_path.exists() {
            return Err(format!(
                "Refusing to discard finalized rollback SQL: {}",
                self.final_path.display()
            ));
        }
        self.remove_stale_revisions(None);
        match fs::remove_file(&self.current_revision_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "Could not discard rollback SQL revision {}: {error}",
                self.current_revision_path.display()
            )),
        }
    }

    fn persist_next_revision(&mut self) -> Result<(), String> {
        let previous = self.current_revision_path.clone();
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| "Rollback journal revision overflow".to_string())?;
        self.current_revision_path = self
            .directory
            .join(format!("{}.recovery.{:04}.sql", self.stem, self.revision));
        if let Err(error) = self.persist_current_revision(Some(&previous)) {
            self.current_revision_path = previous;
            self.revision -= 1;
            return Err(error);
        }
        Ok(())
    }

    fn persist_current_revision(&mut self, previous: Option<&Path>) -> Result<(), String> {
        run_blocking(|| {
            let rendered = render_rollback_sql(&self.environment, &self.steps);
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&self.current_revision_path)
                .map_err(|error| format!("Could not create rollback SQL revision: {error}"))?;
            file.write_all(rendered.as_bytes())
                .map_err(|error| format!("Could not write rollback SQL revision: {error}"))?;
            file.flush()
                .map_err(|error| format!("Could not flush rollback SQL revision: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("Could not sync rollback SQL revision: {error}"))?;
            drop(file);
            self.remove_stale_revisions(previous);
            Ok(())
        })
    }

    fn remove_stale_revisions(&self, keep_previous: Option<&Path>) {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return;
        };
        let prefix = format!("{}.recovery.", self.stem);
        for entry in entries.flatten() {
            let path = entry.path();
            if path == self.current_revision_path
                || keep_previous.is_some_and(|previous| previous == path)
            {
                continue;
            }
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(prefix.as_str())
            {
                let _ = fs::remove_file(path);
            }
        }
        if let Some(previous) = keep_previous {
            let _ = fs::remove_file(previous);
        }
    }
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
    use std::fs::File;
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
    fn keeps_a_durable_recovery_revision_before_finalize() {
        let root = tempfile::tempdir().unwrap();
        let mut journal =
            RollbackJournal::create_in(root.path(), environment("connection-a")).unwrap();
        assert!(journal.current_revision_path.exists());
        journal
            .add_step(RollbackStep {
                statement_index: 0,
                sql: "DROP TABLE `app`.`new_table`".to_string(),
                expected_affected_rows: None,
            })
            .unwrap();
        assert!(journal.current_revision_path.exists());
        assert!(!journal.planned_final_path().exists());
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
    fn discard_removes_only_the_unfinished_recovery_revision() {
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
