#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum ProtectionClass {
    ReadOnly,
    SessionOnly,
    SupportedDml,
    SupportedDdl,
    BlockedDestructive,
    BlockedUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct ClassifiedStatement {
    pub class: ProtectionClass,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ObjectName {
    pub schema: Option<String>,
    pub name: String,
}

impl ObjectName {
    pub fn quoted(&self) -> String {
        let name = quote_identifier(&self.name);
        match &self.schema {
            Some(schema) => format!("{}.{}", quote_identifier(schema), name),
            None => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InsertPlan {
    pub table: ObjectName,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UpdatePlan {
    pub table: ObjectName,
    pub assigned_columns: Vec<String>,
    pub where_sql: Option<String>,
    pub statement_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DeletePlan {
    pub table: ObjectName,
    pub where_sql: Option<String>,
    pub statement_prefix: String,
}

/// Where the rows of an extended INSERT come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InsertSource {
    /// Raw VALUES row expressions, taken verbatim from the statement.
    Values(Vec<Vec<String>>),
    /// Raw SELECT text. Executed once inside the protected transaction to
    /// materialize the exact rows, which then insert as literal VALUES —
    /// `INSERT … SELECT` semantics with a provable row set.
    Select(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UpsertTail {
    /// Raw tail starting at `ON DUPLICATE KEY UPDATE`, appended verbatim when
    /// a materialized SELECT source is re-executed as VALUES.
    pub tail_sql: String,
    pub assigned_columns: Vec<String>,
}

/// `INSERT [IGNORE] INTO t [(cols)] (VALUES … | SELECT …) [ON DUPLICATE KEY
/// UPDATE …]` — the shapes the plain exact planner refuses. Normalized at
/// execution time into known-row inserts/updates so they stay in the
/// pre-commit exact-rollback channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InsertFamilyPlan {
    pub table: ObjectName,
    /// `None` = implicit full column list.
    pub columns: Option<Vec<String>>,
    pub source: InsertSource,
    pub ignore: bool,
    pub upsert: Option<UpsertTail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MultiTableTarget {
    pub table: ObjectName,
    /// How the statement refers to the table inside `refs_sql` (its alias, or
    /// its own name when unaliased).
    pub alias: String,
    /// Columns this statement assigns on this target (empty for DELETE).
    pub assigned_columns: Vec<String>,
}

/// Multi-table / aliased UPDATE and DELETE. The original statement executes
/// verbatim; the affected primary keys are materialized first with the same
/// table references and WHERE clause under `FOR UPDATE`, so before/after
/// images are exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MultiTablePlan {
    pub targets: Vec<MultiTableTarget>,
    /// Raw table references, verbatim between UPDATE/USING-FROM and SET/WHERE.
    pub refs_sql: String,
    pub where_sql: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DmlPlan {
    Insert(InsertPlan),
    Update(UpdatePlan),
    Delete(DeletePlan),
    InsertFamily(InsertFamilyPlan),
    MultiUpdate(MultiTablePlan),
    MultiDelete(MultiTablePlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DdlPlan {
    CreateTable(ObjectName),
    CreateDatabase(String),
    CreateView(ObjectName),
    CreateIndex {
        table: ObjectName,
        index: String,
    },
    RenameTable {
        from: ObjectName,
        to: ObjectName,
    },
    AlterAddColumn {
        table: ObjectName,
        column: String,
    },
    AlterAddIndex {
        table: ObjectName,
        index: String,
    },
    AlterRenameColumn {
        table: ObjectName,
        from: String,
        to: String,
    },
    AlterRenameTable {
        from: ObjectName,
        to: ObjectName,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionPlan {
    UserVariable,
    /// `USE db` — changes which database unqualified names resolve to, so a
    /// rollback file written before it would target the wrong schema. Batches
    /// mixing this with writes are refused.
    ScopeChange,
    /// `SET SESSION ...` / `SET @@session....` / `SET sql_mode = ...` and the
    /// rest of what `session_vars` replays. These change how statements behave
    /// but not which schema they resolve against, so unlike `ScopeChange` they
    /// are safe to mix with writes.
    Setting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransactionPlan {
    Start,
    Commit,
    Rollback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TemporaryPlan {
    Create(ObjectName),
    Drop(Vec<ObjectName>),
    Statement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProtectedStatement {
    ReadOnly,
    Session(SessionPlan),
    Transaction(TransactionPlan),
    Temporary(TemporaryPlan),
    Dml(DmlPlan),
    Ddl(DdlPlan),
    Unsupported(BlockedStatement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BlockedStatement {
    pub class: ProtectionClass,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct RollbackRiskStatement {
    pub index: usize,
    pub sql: String,
    pub reason: String,
    pub destructive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct RollbackRiskReview {
    pub kind: RollbackRiskKind,
    pub statements: Vec<RollbackRiskStatement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RollbackRiskKind {
    Unsupported,
    ImplicitCommit,
}

const ROLLBACK_RISK_REVIEW_PREFIX: &str = "TABULARIS_ROLLBACK_RISK_REVIEW:";

impl BlockedStatement {
    fn destructive(reason: impl Into<String>) -> Self {
        Self {
            class: ProtectionClass::BlockedDestructive,
            reason: reason.into(),
        }
    }

    fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            class: ProtectionClass::BlockedUnsupported,
            reason: reason.into(),
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn classify_for_rollback(sql: &str) -> ClassifiedStatement {
    match plan_for_rollback(sql) {
        Ok(ProtectedStatement::ReadOnly) => ClassifiedStatement {
            class: ProtectionClass::ReadOnly,
            reason: None,
        },
        Ok(ProtectedStatement::Session(_)) => ClassifiedStatement {
            class: ProtectionClass::SessionOnly,
            reason: None,
        },
        Ok(ProtectedStatement::Transaction(_)) => ClassifiedStatement {
            class: ProtectionClass::SessionOnly,
            reason: None,
        },
        Ok(ProtectedStatement::Temporary(_)) => ClassifiedStatement {
            class: ProtectionClass::SessionOnly,
            reason: None,
        },
        Ok(ProtectedStatement::Dml(_)) => ClassifiedStatement {
            class: ProtectionClass::SupportedDml,
            reason: None,
        },
        Ok(ProtectedStatement::Ddl(_)) => ClassifiedStatement {
            class: ProtectionClass::SupportedDdl,
            reason: None,
        },
        Ok(ProtectedStatement::Unsupported(blocked)) => ClassifiedStatement {
            class: blocked.class,
            reason: Some(blocked.reason),
        },
        Err(blocked) => ClassifiedStatement {
            class: blocked.class,
            reason: Some(blocked.reason),
        },
    }
}

pub(super) fn plan_for_rollback(sql: &str) -> Result<ProtectedStatement, BlockedStatement> {
    let tokens = tokenize(sql)?;
    let tokens = trim_trailing_semicolon(&tokens);
    if has_top_level_symbol(tokens, 0, ";") {
        return Err(BlockedStatement::unsupported(
            "multiple SQL statements must be split before rollback planning",
        ));
    }
    let Some(first) = tokens.first().map(Token::upper) else {
        return Ok(ProtectedStatement::ReadOnly);
    };

    match first {
        "SELECT" => classify_select(tokens),
        "WITH" => classify_with(tokens),
        "SHOW" | "DESCRIBE" | "DESC" => Ok(ProtectedStatement::ReadOnly),
        "EXPLAIN" => {
            if contains_word(tokens, "DELETE")
                || contains_word(tokens, "UPDATE")
                || contains_word(tokens, "INSERT")
                || contains_word(tokens, "REPLACE")
            {
                Err(BlockedStatement::unsupported(
                    "EXPLAIN for a write statement is not allowed in rollback protection mode",
                ))
            } else {
                Ok(ProtectedStatement::ReadOnly)
            }
        }
        "SET" => classify_set(sql, tokens),
        "USE" => Ok(ProtectedStatement::Session(SessionPlan::ScopeChange)),
        "START" | "BEGIN" | "COMMIT" | "ROLLBACK" => classify_transaction_control(tokens),
        "INSERT" => match parse_insert(sql, tokens) {
            Ok(plan) => Ok(ProtectedStatement::Dml(DmlPlan::Insert(plan))),
            // The strict planner covers plain `INSERT INTO t (cols) VALUES`;
            // everything else (IGNORE, missing column list, SELECT source,
            // ON DUPLICATE KEY UPDATE) goes through the normalizing family
            // planner before it may fail closed.
            Err(_) => parse_insert_family(sql, tokens)
                .map(|plan| ProtectedStatement::Dml(DmlPlan::InsertFamily(plan))),
        },
        "UPDATE" => match parse_update(sql, tokens) {
            Ok(plan) => Ok(ProtectedStatement::Dml(DmlPlan::Update(plan))),
            Err(_) => parse_multi_update(sql, tokens)
                .map(|plan| ProtectedStatement::Dml(DmlPlan::MultiUpdate(plan))),
        },
        "DELETE" => match parse_delete(sql, tokens) {
            Ok(plan) => Ok(ProtectedStatement::Dml(DmlPlan::Delete(plan))),
            Err(_) => parse_multi_delete(sql, tokens)
                .map(|plan| ProtectedStatement::Dml(DmlPlan::MultiDelete(plan))),
        },
        "CREATE"
            if tokens
                .get(1)
                .is_some_and(|token| token.upper() == "TEMPORARY") =>
        {
            parse_create_temporary(tokens).map(ProtectedStatement::Temporary)
        }
        "CREATE" => parse_create(tokens).map(ProtectedStatement::Ddl),
        "DROP"
            if tokens
                .get(1)
                .is_some_and(|token| token.upper() == "TEMPORARY") =>
        {
            parse_drop_temporary(tokens).map(ProtectedStatement::Temporary)
        }
        "DROP" => parse_drop(tokens).map(ProtectedStatement::Ddl),
        "RENAME" => parse_rename(tokens).map(ProtectedStatement::Ddl),
        "ALTER" => parse_alter(tokens).map(ProtectedStatement::Ddl),
        "TRUNCATE" => Err(BlockedStatement::destructive(
            "TRUNCATE TABLE is forbidden because deleted rows cannot be reconstructed without a table snapshot",
        )),
        "REPLACE" | "LOAD" | "MERGE" => Err(BlockedStatement::unsupported(
            "this DML family cannot be mapped to an exact row set before execution",
        )),
        "CALL" | "DO" | "PREPARE" | "EXECUTE" | "DEALLOCATE" => Err(BlockedStatement::unsupported(
            "stored or dynamic SQL is forbidden because its write set is not statically provable",
        )),
        "GRANT" | "REVOKE" | "ANALYZE" | "OPTIMIZE" | "REPAIR" | "INSTALL" | "UNINSTALL"
        | "IMPORT" | "FLUSH" | "RESET" | "LOCK" | "UNLOCK" | "SAVEPOINT" | "RELEASE" | "XA"
        | "KILL" | "SHUTDOWN" | "CLONE" => Err(BlockedStatement::unsupported(
            "administrative, transaction-control, or server-state statements are forbidden in rollback protection mode",
        )),
        _ => Err(BlockedStatement::unsupported(format!(
            "unrecognized statement family {first}; rollback protection fails closed"
        ))),
    }
}

#[cfg(test)]
pub(super) fn plan_batch_for_rollback(
    queries: &[String],
) -> Result<Vec<ProtectedStatement>, String> {
    let (plans, review) = plan_batch_collecting_risks(queries);
    if let Some(blocked) = review.statements.first() {
        return Err(format!(
            "Rollback protection blocked statement {}: {}",
            blocked.index, blocked.reason
        ));
    }
    Ok(plans)
}

#[cfg(test)]
pub(super) fn review_batch_for_rollback(queries: &[String]) -> Option<RollbackRiskReview> {
    let (_, review) = plan_batch_collecting_risks(queries);
    (!review.statements.is_empty()).then_some(review)
}

#[cfg(test)]
pub(super) fn plan_batch_for_rollback_with_policy(
    queries: &[String],
    _policy: RollbackUnsupportedPolicy,
) -> Result<Vec<ProtectedStatement>, String> {
    Ok(plan_batch_collecting_risks(queries).0)
}

fn plan_batch_collecting_risks(
    queries: &[String],
) -> (Vec<ProtectedStatement>, RollbackRiskReview) {
    let mut temporary_tables = Vec::new();
    let mut plans = Vec::with_capacity(queries.len());
    let mut statements = Vec::new();
    for (index, query) in queries.iter().enumerate() {
        let planned = match plan_temporary_table_write(query, &temporary_tables) {
            Ok(Some(plan)) => Ok(plan),
            Ok(None) => plan_for_rollback(query),
            Err(blocked) => Err(blocked),
        };
        let plan = match planned {
            Ok(plan) => plan,
            Err(blocked) => {
                statements.push(RollbackRiskStatement {
                    index: index + 1,
                    sql: query.trim().to_string(),
                    reason: blocked.reason.clone(),
                    destructive: blocked.class == ProtectionClass::BlockedDestructive,
                });
                ProtectedStatement::Unsupported(blocked)
            }
        };

        match &plan {
            ProtectedStatement::Temporary(TemporaryPlan::Create(table)) => {
                if !temporary_tables.contains(table) {
                    temporary_tables.push(table.clone());
                }
            }
            ProtectedStatement::Temporary(TemporaryPlan::Drop(tables)) => {
                temporary_tables.retain(|table| !tables.contains(table));
            }
            _ => {}
        }
        plans.push(plan);
    }
    (
        plans,
        RollbackRiskReview {
            kind: RollbackRiskKind::Unsupported,
            statements,
        },
    )
}

fn rollback_risk_review_error(review: &RollbackRiskReview) -> String {
    let payload = serde_json::to_string(review)
        .expect("rollback risk review only contains serializable strings");
    format!("{ROLLBACK_RISK_REVIEW_PREFIX}{payload}")
}

pub(super) fn validate_transaction_structure(plans: &[ProtectedStatement]) -> Result<(), String> {
    let mut open_transaction = None;
    for (index, plan) in plans.iter().enumerate() {
        match plan {
            ProtectedStatement::Transaction(TransactionPlan::Start) => {
                if let Some(start_index) = open_transaction {
                    return Err(format!(
                        "Rollback protection blocked statement {}: nested explicit transactions are not supported; statement {} already opened the transaction",
                        index + 1,
                        start_index + 1
                    ));
                }
                open_transaction = Some(index);
            }
            ProtectedStatement::Transaction(
                TransactionPlan::Commit | TransactionPlan::Rollback,
            ) => {
                if open_transaction.take().is_none() {
                    return Err(format!(
                        "Rollback protection blocked statement {}: COMMIT/ROLLBACK has no matching START TRANSACTION or BEGIN in this Run All batch",
                        index + 1
                    ));
                }
            }
            ProtectedStatement::Ddl(_) if open_transaction.is_some() => {
                return Err(format!(
                    "Rollback protection blocked statement {}: MySQL/MariaDB DDL implicitly commits and cannot run inside an explicit protected transaction",
                    index + 1
                ));
            }
            _ => {}
        }
    }
    if let Some(start_index) = open_transaction {
        return Err(format!(
            "Rollback protection blocked statement {}: explicit transaction is not closed by COMMIT or ROLLBACK in the same Run All batch",
            start_index + 1
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PinnedTransactionLifecycle {
    pub ends_active: bool,
    pub implicit_commit: bool,
}

pub(super) fn validate_pinned_transaction_structure(
    plans: &[ProtectedStatement],
    starts_active: bool,
    allow_implicit_commit: bool,
    queries: &[String],
) -> Result<PinnedTransactionLifecycle, String> {
    let mut active = starts_active;
    let mut implicit_commit = false;
    for (index, plan) in plans.iter().enumerate() {
        match plan {
            ProtectedStatement::Transaction(TransactionPlan::Start) => {
                if active {
                    let origin = if starts_active {
                        "a previous Run All"
                    } else {
                        "an earlier statement"
                    };
                    return Err(format!(
                        "Rollback protection blocked statement {}: nested explicit transactions are not supported; {origin} already opened this editor transaction",
                        index + 1
                    ));
                }
                active = true;
            }
            ProtectedStatement::Transaction(
                TransactionPlan::Commit | TransactionPlan::Rollback,
            ) => {
                if !active {
                    return Err(format!(
                        "Rollback protection blocked statement {}: COMMIT/ROLLBACK has no active START TRANSACTION or BEGIN for this editor tab",
                        index + 1
                    ));
                }
                active = false;
            }
            ProtectedStatement::Ddl(_) if active => {
                if !allow_implicit_commit {
                    let review = RollbackRiskReview {
                        kind: RollbackRiskKind::ImplicitCommit,
                        statements: vec![RollbackRiskStatement {
                            index: index + 1,
                            sql: queries
                                .get(index)
                                .map_or_else(String::new, |query| query.trim().to_string()),
                            reason: "MySQL/MariaDB DDL commits the active transaction before the DDL runs. Continuing will commit that boundary, protect the DDL, and finalize their combined rollback SQL when this Run All finishes.".to_string(),
                            destructive: false,
                        }],
                    };
                    return Err(rollback_risk_review_error(&review));
                }
                active = false;
                implicit_commit = true;
            }
            _ => {}
        }
    }
    Ok(PinnedTransactionLifecycle {
        ends_active: active,
        implicit_commit,
    })
}

const PINNED_TRANSACTION_IDLE_TIMEOUT_SECONDS: u64 = 15 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TransactionContextKey {
    connection_id: String,
    context_id: String,
}

struct PinnedTransactionSession {
    session_id: String,
    conn: Option<sqlx::pool::PoolConnection<sqlx::MySql>>,
    journal: Option<RollbackJournal>,
    recovery_journal: Option<RecoveryJournal>,
    explicit_transaction_checkpoint: Option<(usize, usize)>,
    boundary_in_flight: Option<TransactionPlan>,
    execution_in_flight: bool,
    statement_offset: usize,
    last_activity: std::time::Instant,
}

type PinnedTransactionSlot = std::sync::Arc<tokio::sync::Mutex<PinnedTransactionSession>>;
type TransactionContextLock = std::sync::Arc<tokio::sync::Mutex<()>>;

fn pinned_transaction_sessions(
) -> &'static std::sync::Mutex<HashMap<TransactionContextKey, PinnedTransactionSlot>> {
    static SESSIONS: std::sync::OnceLock<
        std::sync::Mutex<HashMap<TransactionContextKey, PinnedTransactionSlot>>,
    > = std::sync::OnceLock::new();
    SESSIONS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn pinned_transaction_context_locks(
) -> &'static std::sync::Mutex<HashMap<TransactionContextKey, TransactionContextLock>> {
    static LOCKS: std::sync::OnceLock<
        std::sync::Mutex<HashMap<TransactionContextKey, TransactionContextLock>>,
    > = std::sync::OnceLock::new();
    LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn lock_unpoisoned<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn transaction_context_lock(key: &TransactionContextKey) -> TransactionContextLock {
    let mut locks = lock_unpoisoned(pinned_transaction_context_locks());
    locks
        .entry(key.clone())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

fn transaction_context_key(params: &ConnectionParams) -> Result<TransactionContextKey, String> {
    let connection_id = params
        .connection_id
        .clone()
        .ok_or_else(|| "Rollback protection requires a stable connection ID".to_string())?;
    let context_id = params
        .transaction_context_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Pinned transactions require an editor transaction context".to_string())?;
    Ok(TransactionContextKey {
        connection_id,
        context_id,
    })
}

fn get_pinned_transaction_slot(key: &TransactionContextKey) -> Option<PinnedTransactionSlot> {
    lock_unpoisoned(pinned_transaction_sessions())
        .get(key)
        .cloned()
}

fn remove_pinned_transaction_slot(key: &TransactionContextKey) {
    lock_unpoisoned(pinned_transaction_sessions()).remove(key);
}

fn insert_pinned_transaction_slot(key: TransactionContextKey, slot: PinnedTransactionSlot) {
    lock_unpoisoned(pinned_transaction_sessions()).insert(key, slot);
}

fn spawn_pinned_transaction_watchdog(key: TransactionContextKey, session_id: String) {
    tokio::spawn(async move {
        let timeout = std::time::Duration::from_secs(PINNED_TRANSACTION_IDLE_TIMEOUT_SECONDS);
        let mut delay = timeout;
        loop {
            tokio::time::sleep(delay).await;
            let Some(slot) = get_pinned_transaction_slot(&key) else {
                return;
            };
            // try_lock, never lock().await: a batch stuck mid-execution holds
            // this mutex, and a blocked watchdog can reap nothing. The batch
            // timeout in execute_batch guarantees the lock frees eventually;
            // until then just re-check on a short interval.
            let remaining = match slot.try_lock() {
                Ok(session) => {
                    if session.session_id != session_id {
                        return;
                    }
                    timeout.saturating_sub(session.last_activity.elapsed())
                }
                Err(_) => {
                    delay = std::time::Duration::from_secs(60);
                    continue;
                }
            };
            if !remaining.is_zero() {
                delay = remaining;
                continue;
            }
            if let Err(error) = close_pinned_transaction_context(&key, "idle timeout").await {
                log::error!(
                    "Could not close idle pinned transaction {}/{}: {}",
                    key.connection_id,
                    key.context_id,
                    error
                );
            }
            return;
        }
    });
}

fn finish_pinned_journals(
    journal: Option<RollbackJournal>,
    recovery_journal: Option<RecoveryJournal>,
) -> Result<Option<String>, String> {
    match (journal, recovery_journal) {
        (None, None) => Ok(None),
        (Some(journal), Some(recovery_journal)) if journal.is_empty() => {
            journal.discard()?;
            // The rollback journal being empty does not mean nothing changed:
            // unsupported statements run without rollback steps but are still
            // journaled for the backup-based restore. Keep that record.
            if recovery_journal.is_empty() {
                recovery_journal.discard()?;
            } else {
                let history_path = recovery_journal.finalize().map_err(|error| {
                    format!(
                        "Unprotected changes completed, but the recovery history could not be finalized: {error}"
                    )
                })?;
                log::info!("Recovery history finalized at {}", history_path.display());
            }
            Ok(None)
        }
        (Some(journal), Some(recovery_journal)) => {
            let recovery_path = journal.current_recovery_path().to_path_buf();
            let final_path = journal.finalize().map_err(|error| {
                format!(
                    "{error}. The synced recovery SQL remains at {}",
                    recovery_path.display()
                )
            })?;
            let history_path = recovery_journal.finalize().map_err(|error| {
                format!(
                    "Protected writes completed, but the independent recovery history could not be finalized: {error}"
                )
            })?;
            log::info!("Recovery history finalized at {}", history_path.display());
            Ok(Some(final_path.to_string_lossy().to_string()))
        }
        _ => Err(
            "Internal rollback protection error: rollback and recovery journals diverged"
                .to_string(),
        ),
    }
}

async fn close_pinned_transaction_context(
    key: &TransactionContextKey,
    reason: &str,
) -> Result<bool, String> {
    let context_lock = transaction_context_lock(key);
    // Bounded wait: this runs from disconnect/cleanup paths, and a batch that
    // is stuck inside MySQL holds the context lock. Waiting forever would
    // make "disconnect" hang alongside the stuck batch (hang contagion).
    let _context_guard = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        context_lock.lock(),
    )
    .await
    {
        Ok(guard) => guard,
        Err(_) => {
            return Err(format!(
                "Pinned transaction {}/{} is still executing; close it again \
                 after the running batch finishes or times out ({reason})",
                key.connection_id, key.context_id
            ));
        }
    };
    let Some(slot) = get_pinned_transaction_slot(key) else {
        return Ok(false);
    };
    close_pinned_transaction_slot(key, slot, reason).await
}

async fn close_pinned_transaction_slot(
    key: &TransactionContextKey,
    slot: PinnedTransactionSlot,
    reason: &str,
) -> Result<bool, String> {
    let mut session = slot.lock().await;
    let text = super::TextProto::protocol_only(true);

    if session.boundary_in_flight == Some(TransactionPlan::Commit) {
        let conn = session.conn.take();
        let rollback_file =
            finish_pinned_journals(session.journal.take(), session.recovery_journal.take())?;
        remove_pinned_transaction_slot(key);
        drop(session);
        if let Some(conn) = conn {
            let _ = conn.close().await;
        }
        log::warn!(
            "Closed pinned transaction context {}/{} during {} while COMMIT outcome was unknown; retained rollback file: {}",
            key.connection_id,
            key.context_id,
            reason,
            rollback_file.as_deref().unwrap_or("none")
        );
        return Ok(true);
    }

    let rollback = {
        let conn = session
            .conn
            .as_mut()
            .ok_or_else(|| "Pinned transaction lost its physical connection".to_string())?;
        super::exec_on_mysql_conn(conn, "ROLLBACK", None, 1, text).await
    };
    if let Err(error) = rollback {
        let conn = session.conn.take();
        remove_pinned_transaction_slot(key);
        drop(session);
        if let Some(conn) = conn {
            let _ = conn.close().await;
        }
        return Err(format!(
            "Automatic transaction rollback failed during {reason}: {error}; the physical connection was closed and the durable recovery files were retained"
        ));
    }
    if let Some((rollback_checkpoint, recovery_checkpoint)) =
        session.explicit_transaction_checkpoint
    {
        let PinnedTransactionSession {
            journal,
            recovery_journal,
            ..
        } = &mut *session;
        rewind_journals(
            journal.as_mut(),
            rollback_checkpoint,
            recovery_journal.as_mut(),
            recovery_checkpoint,
        )?;
        session.explicit_transaction_checkpoint = None;
    }
    session.boundary_in_flight = None;

    let rollback_file =
        finish_pinned_journals(session.journal.take(), session.recovery_journal.take())?;
    session.conn.take();
    remove_pinned_transaction_slot(key);
    log::info!(
        "Closed pinned transaction context {}/{} during {}; rollback file: {}",
        key.connection_id,
        key.context_id,
        reason,
        rollback_file.as_deref().unwrap_or("none")
    );
    Ok(true)
}

pub(super) async fn rollback_transaction_context(
    connection_id: &str,
    context_id: &str,
) -> Result<bool, String> {
    close_pinned_transaction_context(
        &TransactionContextKey {
            connection_id: connection_id.to_string(),
            context_id: context_id.to_string(),
        },
        "editor tab close",
    )
    .await
}

pub(super) async fn rollback_connection_transactions(connection_id: &str) -> Result<usize, String> {
    let keys = lock_unpoisoned(pinned_transaction_sessions())
        .keys()
        .filter(|key| key.connection_id == connection_id)
        .cloned()
        .collect::<Vec<_>>();
    let mut closed = 0;
    let mut errors = Vec::new();
    for key in keys {
        match close_pinned_transaction_context(&key, "connection disconnect").await {
            Ok(true) => closed += 1,
            Ok(false) => {}
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(closed)
    } else {
        Err(errors.join("; "))
    }
}

pub(super) fn complete_statement_without_database(
    index: usize,
    plan: &ProtectedStatement,
    stopped: bool,
    unsupported_policy: Option<RollbackUnsupportedPolicy>,
    rollback_file: Option<&str>,
    on_progress: Option<&crate::drivers::driver_trait::BatchProgressFn>,
) -> Result<Option<BatchStatementResult>, String> {
    let mut result = match plan {
        ProtectedStatement::Unsupported(blocked)
            if unsupported_policy == Some(RollbackUnsupportedPolicy::Skip) =>
        {
            BatchStatementResult::skipped(format!(
                "Skipped by user because no exact rollback SQL can be generated: {}",
                blocked.reason
            ))
        }
        _ if stopped => BatchStatementResult::from_outcome(
            std::time::Instant::now(),
            Err("Skipped because an earlier protected statement failed".to_string()),
        ),
        _ => return Ok(None),
    };
    result.rollback_file = rollback_file.map(str::to_string);
    if let Some(callback) = on_progress {
        callback(index, &result)?;
    }
    Ok(Some(result))
}

/// Journals a statement that executed OUTSIDE exact protection into the
/// batch's recovery journal (`exact: false`), mirroring
/// `recovery_history::record_unprotected_changes` but reusing the journals
/// the protected batch already holds. Best-effort: a journaling failure is
/// logged, never surfaced — it must not fail a statement that already ran.
async fn journal_unsupported_statement(
    conn: &mut sqlx::MySqlConnection,
    recovery_journal: Option<&mut RecoveryJournal>,
    prepared: &mut HashMap<String, Vec<RecoveryObject>>,
    statement_index: usize,
    sql: &str,
) {
    let Some(recovery_journal) = recovery_journal else {
        return;
    };
    let (operation, mut objects) = crate::recovery_history::parse_change_objects(sql);
    if crate::recovery_history::is_unprotected_non_recovery_operation(&operation) {
        return;
    }
    let database = current_database(conn).await.unwrap_or_default();
    if objects.is_empty() || crate::recovery_objects::dynamic_source(sql).is_some() {
        objects.extend(
            crate::recovery_history::resolve_dynamic_objects(conn, &database, sql, prepared)
                .await,
        );
        objects
            .sort_by(|a, b| (&a.kind, &a.schema, &a.name).cmp(&(&b.kind, &b.schema, &b.name)));
        objects.dedup();
    }
    if let Err(error) = recovery_journal.add_statement(RecoveryStatement {
        id: String::new(),
        index: statement_index,
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
    }) {
        log::warn!("Could not journal an unprotected statement: {error}");
    }
}

pub(super) async fn execute_protected_batch(
    params: &ConnectionParams,
    queries: &[String],
    limit: Option<u32>,
    page: u32,
    schema: Option<&str>,
    on_progress: Option<&crate::drivers::driver_trait::BatchProgressFn>,
) -> Result<Vec<BatchStatementResult>, String> {
    if params.transaction_context_id.is_some() {
        execute_pinned_protected_batch(params, queries, limit, page, schema, on_progress).await
    } else {
        execute_single_run_protected_batch(params, queries, limit, page, schema, on_progress).await
    }
}

/// Creates the rollback + recovery journals for a pinned session that is
/// about to run its first write. Split out so the caller can reset
/// `execution_in_flight` on failure (see call site).
async fn init_pinned_journals(
    session: &mut PinnedTransactionSession,
    params: &ConnectionParams,
) -> Result<(), String> {
    let connection_id = params
        .connection_id
        .clone()
        .ok_or_else(|| "Rollback protection requires a stable connection ID".to_string())?;
    let connection_name = params
        .connection_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "Rollback protection requires a connection name".to_string())?;
    let environment = {
        let conn = session
            .conn
            .as_mut()
            .ok_or_else(|| "Pinned transaction lost its physical connection".to_string())?;
        read_rollback_environment(conn, connection_id, connection_name).await?
    };
    session.recovery_journal = Some(RecoveryJournal::create(
        environment.connection_id.clone(),
        environment.connection_name.clone(),
        environment.database.clone(),
        recovery_server_identity_label(&environment.server),
    )?);
    session.journal = Some(RollbackJournal::create(environment)?);
    Ok(())
}

async fn execute_pinned_protected_batch(
    params: &ConnectionParams,
    queries: &[String],
    limit: Option<u32>,
    page: u32,
    schema: Option<&str>,
    on_progress: Option<&crate::drivers::driver_trait::BatchProgressFn>,
) -> Result<Vec<BatchStatementResult>, String> {
    // No-prompt policy (2026-09-01): a missing policy no longer raises
    // TABULARIS_ROLLBACK_RISK_REVIEW. Unsupported statements execute
    // unprotected, are flagged in their result, and are journaled below so
    // the backup-based restore can still reach their objects. An explicit
    // `Skip` policy is still honored.
    let (plans, _review) = plan_batch_collecting_risks(queries);

    let key = transaction_context_key(params)?;
    let context_lock = transaction_context_lock(&key);
    let _context_guard = context_lock.lock().await;
    let existing_slot = get_pinned_transaction_slot(&key);
    let (starts_active, interrupted) = if let Some(slot) = &existing_slot {
        let session = slot.lock().await;
        (
            session.explicit_transaction_checkpoint.is_some(),
            session.execution_in_flight,
        )
    } else {
        (false, false)
    };
    if interrupted {
        close_pinned_transaction_slot(
            &key,
            existing_slot
                .as_ref()
                .expect("interrupted state requires an existing slot")
                .clone(),
            "interrupted previous Run All",
        )
        .await?;
        return Err(
            "The previous Run All was interrupted; its pinned transaction was closed safely. Run the statement again."
                .to_string(),
        );
    }
    // No-prompt policy: a DDL implicit commit inside an explicit transaction
    // is auto-allowed (the boundary is committed, the DDL protected, and the
    // combined rollback SQL finalized) instead of raising a review error.
    let lifecycle =
        validate_pinned_transaction_structure(&plans, starts_active, true, queries)?;

    // `USE db` mixed with writes used to be refused here, on the theory that an
    // unqualified table name in the rollback file would resolve against whatever
    // database happened to be current at restore time.
    //
    // That is not how the rollback file is built. Both paths qualify the name at
    // execution time, against the connection's database *as of that statement*:
    //   * DML  — `load_table_metadata` fills `TableMetadata.schema` (a `String`,
    //     not an `Option`) via `current_database(conn)`, and the inverse SQL uses
    //     `metadata.qualified_name()`.
    //   * DDL  — `resolve_object` does the same before `ObjectName::quoted()`.
    // So a batch that switches database mid-way produces rollback SQL pointing at
    // the right schema for each statement, which is exactly what USE + Run All
    // needs. A failed USE cannot desync this either: the batch sets `stopped` and
    // every later statement is skipped rather than executed against the old one.
    //
    // lop asked for USE + Run All twice; the refusal accounted for 53 blocked
    // statements in his execution history, each of which failed its whole batch.

    // Unsupported statements count as writes: they may change data, and the
    // no-prompt policy journals them into the recovery history, so the
    // journals must exist even for a batch that contains nothing protectable.
    let has_writes = plans.iter().any(|plan| {
        matches!(
            plan,
            ProtectedStatement::Dml(_)
                | ProtectedStatement::Ddl(_)
                | ProtectedStatement::Unsupported(_)
        )
    });

    let slot = if let Some(slot) = existing_slot {
        slot
    } else {
        let conn = super::acquire_mysql_conn(params, schema).await?;
        let session_id = ulid::Ulid::new().to_string();
        let slot = std::sync::Arc::new(tokio::sync::Mutex::new(PinnedTransactionSession {
            session_id: session_id.clone(),
            conn: Some(conn),
            journal: None,
            recovery_journal: None,
            explicit_transaction_checkpoint: None,
            boundary_in_flight: None,
            execution_in_flight: false,
            statement_offset: 0,
            last_activity: std::time::Instant::now(),
        }));
        insert_pinned_transaction_slot(key.clone(), slot.clone());
        spawn_pinned_transaction_watchdog(key.clone(), session_id);
        slot
    };

    let text = super::TextProto::protocol_only(super::force_text_protocol(params));
    let mut session = slot.lock().await;
    session.last_activity = std::time::Instant::now();
    session.execution_in_flight = true;

    if has_writes && session.journal.is_none() {
        // A failure here (missing permission, journal IO error) must not
        // leave execution_in_flight=true behind: the next run would then hit
        // the "interrupted previous Run All" recovery instead of just seeing
        // this error once.
        if let Err(error) = init_pinned_journals(&mut session, params).await {
            session.execution_in_flight = false;
            return Err(error);
        }
    }

    let mut results = Vec::with_capacity(queries.len());
    let mut stopped = false;
    let mut transaction_outcome: Option<String> = None;
    let mut uncertain_boundary = false;
    let mut prepared_dynamic_objects: HashMap<String, Vec<RecoveryObject>> = HashMap::new();
    let statement_offset = session.statement_offset;

    {
        let PinnedTransactionSession {
            conn,
            journal,
            recovery_journal,
            explicit_transaction_checkpoint,
            boundary_in_flight,
            execution_in_flight,
            last_activity,
            ..
        } = &mut *session;
        let conn = conn
            .as_mut()
            .ok_or_else(|| "Pinned transaction lost its physical connection".to_string())?;

        for (index, (query, plan)) in queries.iter().zip(plans.iter()).enumerate() {
            *last_activity = std::time::Instant::now();
            let start = std::time::Instant::now();
            match complete_statement_without_database(
                index,
                plan,
                stopped,
                params.rollback_unsupported_policy,
                None,
                on_progress,
            ) {
                Ok(Some(result)) => {
                    results.push(result);
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    *execution_in_flight = false;
                    *last_activity = std::time::Instant::now();
                    return Err(error);
                }
            }

            // Set when a DML statement had to run without an exact inverse.
            let mut degraded: Option<String> = None;
            let mut outcome = match plan {
                ProtectedStatement::ReadOnly
                | ProtectedStatement::Session(_)
                | ProtectedStatement::Temporary(_) => {
                    super::exec_on_mysql_conn(conn, query, limit, page, text).await
                }
                ProtectedStatement::Transaction(TransactionPlan::Start) => {
                    *boundary_in_flight = Some(TransactionPlan::Start);
                    let outcome = super::exec_on_mysql_conn(conn, query, None, 1, text).await;
                    *boundary_in_flight = None;
                    if outcome.is_ok() {
                        *explicit_transaction_checkpoint = Some((
                            journal.as_ref().map_or(0, RollbackJournal::checkpoint),
                            recovery_journal
                                .as_ref()
                                .map_or(0, RecoveryJournal::checkpoint),
                        ));
                        transaction_outcome = Some("opened".to_string());
                    }
                    outcome
                }
                ProtectedStatement::Transaction(TransactionPlan::Commit) => {
                    *boundary_in_flight = Some(TransactionPlan::Commit);
                    match super::exec_on_mysql_conn(conn, query, None, 1, text).await {
                        Ok(result) => {
                            *boundary_in_flight = None;
                            *explicit_transaction_checkpoint = None;
                            transaction_outcome = Some("committed".to_string());
                            Ok(result)
                        }
                        Err(error) => {
                            uncertain_boundary = true;
                            transaction_outcome = Some("unknown".to_string());
                            Err(format!(
                                    "COMMIT outcome is unknown ({error}); the physical connection will be closed and the durable rollback file retained"
                                ))
                        }
                    }
                }
                ProtectedStatement::Transaction(TransactionPlan::Rollback) => {
                    let (rollback_checkpoint, recovery_checkpoint) =
                        explicit_transaction_checkpoint.expect(
                            "pinned preflight requires ROLLBACK to match an active transaction",
                        );
                    *boundary_in_flight = Some(TransactionPlan::Rollback);
                    match super::exec_on_mysql_conn(conn, query, None, 1, text).await {
                        Ok(result) => {
                            *boundary_in_flight = None;
                            *explicit_transaction_checkpoint = None;
                            transaction_outcome = Some("rolled_back".to_string());
                            match rewind_journals(
                                    journal.as_mut(),
                                    rollback_checkpoint,
                                    recovery_journal.as_mut(),
                                    recovery_checkpoint,
                                ) {
                                    Ok(()) => Ok(result),
                                    Err(error) => Err(format!(
                                        "ROLLBACK succeeded, but obsolete rollback/recovery records could not be removed: {error}"
                                    )),
                                }
                        }
                        Err(error) => {
                            uncertain_boundary = true;
                            transaction_outcome = Some("unknown".to_string());
                            Err(format!(
                                    "ROLLBACK outcome is unknown ({error}); the physical connection will be closed"
                                ))
                        }
                    }
                }
                ProtectedStatement::Dml(plan) => {
                    let statement_index = statement_offset + index;
                    if explicit_transaction_checkpoint.is_some() {
                        execute_protected_dml_body(
                            conn,
                            query,
                            plan,
                            statement_index,
                            journal
                                .as_mut()
                                .expect("write batches always have a rollback journal"),
                            recovery_journal
                                .as_mut()
                                .expect("write batches always have a recovery journal"),
                            text,
                            &mut degraded,
                        )
                        .await
                    } else {
                        execute_protected_dml(
                            conn,
                            query,
                            plan,
                            statement_index,
                            journal
                                .as_mut()
                                .expect("write batches always have a rollback journal"),
                            recovery_journal
                                .as_mut()
                                .expect("write batches always have a recovery journal"),
                            text,
                            &mut degraded,
                        )
                        .await
                    }
                }
                ProtectedStatement::Ddl(plan) => {
                    let commit_error = if explicit_transaction_checkpoint.is_some() {
                        *boundary_in_flight = Some(TransactionPlan::Commit);
                        match super::exec_on_mysql_conn(conn, "COMMIT", None, 1, text).await {
                            Ok(_) => {
                                *boundary_in_flight = None;
                                *explicit_transaction_checkpoint = None;
                                transaction_outcome = Some("ddl_implicit_commit".to_string());
                                None
                            }
                            Err(error) => {
                                uncertain_boundary = true;
                                transaction_outcome = Some("unknown".to_string());
                                Some(format!(
                                        "COMMIT before DDL has an unknown outcome ({error}); the DDL was not executed"
                                    ))
                            }
                        }
                    } else {
                        None
                    };
                    if let Some(error) = commit_error {
                        Err(error)
                    } else {
                        execute_protected_ddl(
                            conn,
                            query,
                            plan,
                            statement_offset + index,
                            journal
                                .as_mut()
                                .expect("write batches always have a rollback journal"),
                            recovery_journal
                                .as_mut()
                                .expect("write batches always have a recovery journal"),
                            text,
                        )
                        .await
                    }
                }
                ProtectedStatement::Unsupported(_) => {
                    // MySQL commits implicitly around DDL. The Ddl branch
                    // above clears the checkpoint for that reason; an
                    // Unsupported statement that is also DDL — DROP TABLE,
                    // TRUNCATE, the destructive ones a user accepts the
                    // risk on — commits identically but left the
                    // checkpoint standing, so a later ROLLBACK rewound the
                    // journal past changes that were already permanent and
                    // erased the only record of them.
                    if explicit_transaction_checkpoint.is_some()
                        && causes_implicit_commit(query)
                    {
                        *explicit_transaction_checkpoint = None;
                        transaction_outcome =
                            Some("unsupported_implicit_commit".to_string());
                    }
                    let outcome =
                        super::exec_on_mysql_conn(conn, query, limit, page, text).await;
                    if outcome.is_ok() {
                        journal_unsupported_statement(
                            conn,
                            recovery_journal.as_mut(),
                            &mut prepared_dynamic_objects,
                            statement_offset + index,
                            query,
                        )
                        .await;
                    }
                    outcome
                }
            };

            if outcome.is_err()
                && explicit_transaction_checkpoint.is_some()
                && !matches!(
                    plan,
                    ProtectedStatement::Transaction(
                        TransactionPlan::Commit | TransactionPlan::Rollback
                    )
                )
                && !uncertain_boundary
            {
                let error = outcome
                    .err()
                    .expect("the explicit transaction error branch requires an error");
                let (rollback_checkpoint, recovery_checkpoint) = explicit_transaction_checkpoint
                    .take()
                    .expect("the explicit transaction error branch requires a checkpoint");
                *boundary_in_flight = Some(TransactionPlan::Rollback);
                outcome = match rollback_explicit_transaction(
                    conn,
                    rollback_checkpoint,
                    journal.as_mut(),
                    recovery_checkpoint,
                    recovery_journal.as_mut(),
                    text,
                )
                .await
                {
                    Ok(()) => {
                        *boundary_in_flight = None;
                        transaction_outcome = Some("auto_rolled_back".to_string());
                        Err(format!(
                            "{error}; the active explicit transaction was rolled back"
                        ))
                    }
                    Err(rollback_error) => {
                        uncertain_boundary = true;
                        transaction_outcome = Some("unknown".to_string());
                        Err(format!(
                            "{error}; automatic explicit transaction rollback failed: {rollback_error}; the physical connection will be closed and the durable rollback file retained"
                        ))
                    }
                };
            }
            if outcome.is_err() {
                stopped = true;
            }
            let mut result = BatchStatementResult::from_outcome(start, outcome);
            if degraded.take().is_some() {
                result.rollback_unprotected = Some(true);
            }
            if matches!(plan, ProtectedStatement::Unsupported(_)) {
                result.rollback_unprotected = Some(true);
            }
            if let Some(callback) = on_progress {
                if let Err(error) = callback(index, &result) {
                    *execution_in_flight = false;
                    *last_activity = std::time::Instant::now();
                    return Err(error);
                }
            }
            results.push(result);
        }
    }

    session.statement_offset = session.statement_offset.saturating_add(queries.len());
    session.last_activity = std::time::Instant::now();
    session.execution_in_flight = false;
    let ends_active = session.explicit_transaction_checkpoint.is_some() && !uncertain_boundary;
    if !stopped {
        debug_assert_eq!(ends_active, lifecycle.ends_active);
    }

    if uncertain_boundary {
        let conn = session.conn.take();
        // Render the abandoned rollback journal to executable SQL right now —
        // the unknown-COMMIT case is exactly when the operator needs it. The
        // recovery history must not be dropped mid-"recording": mark it
        // interrupted so RecoveryPage can still show and compare it.
        let recovery_file = session
            .journal
            .take()
            .map(|journal| journal.abandon().to_string_lossy().to_string());
        if let Some(recovery_journal) = session.recovery_journal.take() {
            if let Err(error) =
                recovery_journal.interrupt("transaction boundary outcome unknown")
            {
                log::warn!("Could not mark recovery run interrupted: {error}");
            }
        }
        remove_pinned_transaction_slot(&key);
        drop(session);
        if let Some(conn) = conn {
            let _ = conn.close().await;
        }
        for result in &mut results {
            result.transaction_active = Some(false);
            result.transaction_outcome = Some("unknown".to_string());
            result.transaction_recovery_file = recovery_file.clone();
        }
        return Ok(results);
    }

    if ends_active {
        let recovery_file = session.journal.as_ref().map(|journal| {
            journal
                .current_recovery_path()
                .to_string_lossy()
                .to_string()
        });
        let outcome = transaction_outcome.unwrap_or_else(|| "active".to_string());
        for result in &mut results {
            result.transaction_active = Some(true);
            result.transaction_outcome = Some(outcome.clone());
            result.transaction_recovery_file = recovery_file.clone();
            result.transaction_idle_timeout_seconds = Some(PINNED_TRANSACTION_IDLE_TIMEOUT_SECONDS);
        }
        return Ok(results);
    }

    let rollback_file =
        finish_pinned_journals(session.journal.take(), session.recovery_journal.take())?;
    session.conn.take();
    remove_pinned_transaction_slot(&key);
    drop(session);
    for result in &mut results {
        result.rollback_file = rollback_file.clone();
        result.transaction_active = Some(false);
        result.transaction_outcome = transaction_outcome.clone();
    }
    Ok(results)
}

async fn execute_single_run_protected_batch(
    params: &ConnectionParams,
    queries: &[String],
    limit: Option<u32>,
    page: u32,
    schema: Option<&str>,
    on_progress: Option<&crate::drivers::driver_trait::BatchProgressFn>,
) -> Result<Vec<BatchStatementResult>, String> {
    // No-prompt policy (2026-09-01): a missing policy no longer raises
    // TABULARIS_ROLLBACK_RISK_REVIEW; unsupported statements execute
    // unprotected, are flagged, and are journaled. `Skip` is still honored.
    let (plans, _review) = plan_batch_collecting_risks(queries);
    if params.rollback_unsupported_policy == Some(RollbackUnsupportedPolicy::ExecuteUnprotected) {
        return Err(
            "Internal rollback protection error: unprotected execution must use the normal batch path"
                .to_string(),
        );
    }
    validate_transaction_structure(&plans)?;

    // Unsupported statements count as writes: they may change data, and the
    // no-prompt policy journals them into the recovery history, so the
    // journals must exist even for a batch that contains nothing protectable.
    let has_writes = plans.iter().any(|plan| {
        matches!(
            plan,
            ProtectedStatement::Dml(_)
                | ProtectedStatement::Ddl(_)
                | ProtectedStatement::Unsupported(_)
        )
    });
    // USE + writes is allowed here for the same reason as in the pinned path:
    // the rollback file is qualified at execution time via `current_database`,
    // so switching database mid-batch cannot make it target the wrong schema.

    let mut conn = super::acquire_mysql_conn(params, schema).await?;
    let text = super::TextProto::protocol_only(super::force_text_protocol(params));
    let (mut journal, mut recovery_journal) = if has_writes {
        let connection_id = params
            .connection_id
            .clone()
            .ok_or_else(|| "Rollback protection requires a stable connection ID".to_string())?;
        let connection_name = params
            .connection_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| "Rollback protection requires a connection name".to_string())?;
        let environment =
            read_rollback_environment(&mut conn, connection_id, connection_name).await?;
        let recovery_journal = RecoveryJournal::create(
            environment.connection_id.clone(),
            environment.connection_name.clone(),
            environment.database.clone(),
            recovery_server_identity_label(&environment.server),
        )?;
        (
            Some(RollbackJournal::create(environment)?),
            Some(recovery_journal),
        )
    } else {
        (None, None)
    };
    let rollback_path = journal
        .as_ref()
        .map(|journal| journal.planned_final_path().to_string_lossy().to_string());

    let mut results = Vec::with_capacity(queries.len());
    let mut stopped = false;
    let mut explicit_transaction_checkpoint = None;
    let mut prepared_dynamic_objects: HashMap<String, Vec<RecoveryObject>> = HashMap::new();
    for (index, (query, plan)) in queries.iter().zip(plans.iter()).enumerate() {
        let start = std::time::Instant::now();
        if let Some(result) = complete_statement_without_database(
            index,
            plan,
            stopped,
            params.rollback_unsupported_policy,
            rollback_path.as_deref(),
            on_progress,
        )? {
            results.push(result);
            continue;
        }
        // Set when a DML statement had to run without an exact inverse.
        let mut degraded: Option<String> = None;
        let mut outcome = match plan {
            ProtectedStatement::ReadOnly
                | ProtectedStatement::Session(_)
                | ProtectedStatement::Temporary(_) => {
                    super::exec_on_mysql_conn(&mut conn, query, limit, page, text).await
                }
                ProtectedStatement::Transaction(TransactionPlan::Start) => {
                    let outcome = super::exec_on_mysql_conn(&mut conn, query, None, 1, text).await;
                    if outcome.is_ok() {
                        explicit_transaction_checkpoint = Some((
                            journal.as_ref().map_or(0, RollbackJournal::checkpoint),
                            recovery_journal
                                .as_ref()
                                .map_or(0, RecoveryJournal::checkpoint),
                        ));
                    }
                    outcome
                }
                ProtectedStatement::Transaction(TransactionPlan::Commit) => {
                    match super::exec_on_mysql_conn(&mut conn, query, None, 1, text).await {
                        Ok(result) => {
                            explicit_transaction_checkpoint = None;
                            Ok(result)
                        }
                        Err(error) => Err(format!(
                            "COMMIT outcome is unknown ({error}); the durable rollback file was retained"
                        )),
                    }
                }
                ProtectedStatement::Transaction(TransactionPlan::Rollback) => {
                    let (rollback_checkpoint, recovery_checkpoint) =
                        explicit_transaction_checkpoint
                            .expect("preflight requires ROLLBACK to match an open transaction");
                    match super::exec_on_mysql_conn(&mut conn, query, None, 1, text).await {
                        Ok(result) => {
                            explicit_transaction_checkpoint = None;
                            let rewind = rewind_journals(
                                journal.as_mut(),
                                rollback_checkpoint,
                                recovery_journal.as_mut(),
                                recovery_checkpoint,
                            );
                            match rewind {
                                Ok(()) => Ok(result),
                                Err(error) => Err(format!(
                                    "ROLLBACK succeeded, but obsolete rollback/recovery records could not be removed: {error}"
                                )),
                            }
                        }
                        Err(error) => Err(format!(
                            "ROLLBACK outcome is unknown ({error}); the durable rollback file was retained"
                        )),
                    }
                }
                ProtectedStatement::Dml(plan) => {
                    if explicit_transaction_checkpoint.is_some() {
                        execute_protected_dml_body(
                            &mut conn,
                            query,
                            plan,
                            index,
                            journal
                                .as_mut()
                                .expect("write batches always have a rollback journal"),
                            recovery_journal
                                .as_mut()
                                .expect("write batches always have a recovery journal"),
                            text,
                            &mut degraded,
                        )
                        .await
                    } else {
                        execute_protected_dml(
                            &mut conn,
                            query,
                            plan,
                            index,
                            journal
                                .as_mut()
                                .expect("write batches always have a rollback journal"),
                            recovery_journal
                                .as_mut()
                                .expect("write batches always have a recovery journal"),
                            text,
                            &mut degraded,
                        )
                        .await
                    }
                }
                ProtectedStatement::Ddl(plan) => {
                    execute_protected_ddl(
                        &mut conn,
                        query,
                        plan,
                        index,
                        journal
                            .as_mut()
                            .expect("write batches always have a rollback journal"),
                        recovery_journal
                            .as_mut()
                            .expect("write batches always have a recovery journal"),
                        text,
                    )
                    .await
                }
            ProtectedStatement::Unsupported(_) => {
                // No-prompt policy: run it, flag the result, and journal the
                // statement so the backup-based restore can reach its objects.
                let outcome =
                    super::exec_on_mysql_conn(&mut conn, query, limit, page, text).await;
                if outcome.is_ok() {
                    journal_unsupported_statement(
                        &mut conn,
                        recovery_journal.as_mut(),
                        &mut prepared_dynamic_objects,
                        index,
                        query,
                    )
                    .await;
                }
                outcome
            }
        };
        if outcome.is_err()
            && explicit_transaction_checkpoint.is_some()
            && !matches!(
                plan,
                ProtectedStatement::Transaction(
                    TransactionPlan::Commit | TransactionPlan::Rollback
                )
            )
        {
            let error = outcome
                .err()
                .expect("the explicit transaction error branch requires an error");
            let (rollback_checkpoint, recovery_checkpoint) = explicit_transaction_checkpoint
                .take()
                .expect("the explicit transaction error branch requires a checkpoint");
            outcome = match rollback_explicit_transaction(
                &mut conn,
                rollback_checkpoint,
                journal.as_mut(),
                recovery_checkpoint,
                recovery_journal.as_mut(),
                text,
            )
            .await
            {
                Ok(()) => Err(format!(
                    "{error}; the active explicit transaction was rolled back"
                )),
                Err(rollback_error) => Err(format!(
                    "{error}; automatic explicit transaction rollback failed: {rollback_error}; the durable rollback file was retained"
                )),
            };
        }
        if outcome.is_err() {
            stopped = true;
        }
        let mut result = BatchStatementResult::from_outcome(start, outcome);
        if degraded.take().is_some() || matches!(plan, ProtectedStatement::Unsupported(_)) {
            result.rollback_unprotected = Some(true);
        }
        result.rollback_file = rollback_path.clone();
        if let Some(callback) = on_progress {
            callback(index, &result)?;
        }
        results.push(result);
    }

    // Same finish semantics as the pinned path: an empty rollback journal is
    // discarded (no noise file), while a recovery history that recorded
    // unprotected changes is still finalized.
    let final_rollback = finish_pinned_journals(journal, recovery_journal)?;
    for result in &mut results {
        result.rollback_file = final_rollback.clone();
    }
    Ok(results)
}

async fn rollback_explicit_transaction(
    conn: &mut sqlx::MySqlConnection,
    rollback_checkpoint: usize,
    journal: Option<&mut RollbackJournal>,
    recovery_checkpoint: usize,
    recovery_journal: Option<&mut RecoveryJournal>,
    text: super::TextProto,
) -> Result<(), String> {
    super::exec_on_mysql_conn(conn, "ROLLBACK", None, 1, text)
        .await
        .map_err(|error| format!("ROLLBACK outcome is unknown ({error})"))?;
    rewind_journals(
        journal,
        rollback_checkpoint,
        recovery_journal,
        recovery_checkpoint,
    )
    .map_err(|error| {
        format!("ROLLBACK succeeded, but obsolete rollback/recovery records could not be removed: {error}")
    })
}

fn rewind_journals(
    rollback_journal: Option<&mut RollbackJournal>,
    rollback_checkpoint: usize,
    recovery_journal: Option<&mut RecoveryJournal>,
    recovery_checkpoint: usize,
) -> Result<(), String> {
    if let Some(journal) = rollback_journal {
        journal.rewind_to(rollback_checkpoint)?;
    }
    if let Some(journal) = recovery_journal {
        journal.rewind_to(recovery_checkpoint)?;
    }
    Ok(())
}

async fn execute_protected_dml(
    conn: &mut sqlx::MySqlConnection,
    query: &str,
    plan: &DmlPlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
    degraded: &mut Option<String>,
) -> Result<QueryResult, String> {
    let rollback_checkpoint = rollback_journal.checkpoint();
    let recovery_checkpoint = recovery_journal.checkpoint();
    let mut transaction = conn
        .begin()
        .await
        .map_err(|error| format!("Could not start protected transaction: {error}"))?;
    let outcome = execute_protected_dml_body(
        &mut transaction,
        query,
        plan,
        statement_index,
        rollback_journal,
        recovery_journal,
        text,
        degraded,
    )
    .await;

    match outcome {
        Ok(result) => {
            if let Err(error) = transaction.commit().await {
                return Err(format!(
                    "COMMIT outcome is unknown ({error}); the durable rollback file was retained"
                ));
            }
            Ok(result)
        }
        Err(error) => {
            let rollback_error = transaction.rollback().await.err();
            if let Some(rollback_error) = rollback_error {
                return Err(format!(
                    "{error}; transaction rollback also failed: {rollback_error}"
                ));
            }
            rewind_journals(
                Some(rollback_journal),
                rollback_checkpoint,
                Some(recovery_journal),
                recovery_checkpoint,
            )
            .map_err(|rewind_error| {
                format!(
                    "{error}; transaction rolled back, but obsolete rollback/recovery records could not be removed: {rewind_error}"
                )
            })?;
            Err(error)
        }
    }
}

/// Marks an error as "this statement is real, but no exact inverse can be
/// built for it" — as opposed to a syntax error, a lock timeout, or a dead
/// connection, which must still abort the batch.
///
/// Only column-shape checks use it. Resource limits (the capture row cap) stay
/// fail-closed: degrading there would mean silently running a write we
/// deliberately refused to size.
pub(super) const UNPROTECTABLE: &str = "TABULARIS_UNPROTECTABLE:";

pub(super) fn unprotectable_reason(error: &str) -> Option<&str> {
    error.strip_prefix(UNPROTECTABLE)
}

async fn execute_protected_dml_body(
    conn: &mut sqlx::MySqlConnection,
    query: &str,
    plan: &DmlPlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
    degraded: &mut Option<String>,
) -> Result<QueryResult, String> {
    let outcome = match plan {
        DmlPlan::Insert(plan) => {
            execute_insert(
                conn,
                query,
                plan,
                statement_index,
                rollback_journal,
                recovery_journal,
                text,
            )
            .await
        }
        DmlPlan::Update(plan) => {
            execute_update(
                conn,
                query,
                plan,
                statement_index,
                rollback_journal,
                recovery_journal,
                text,
            )
            .await
        }
        DmlPlan::Delete(plan) => {
            execute_delete(
                conn,
                query,
                plan,
                statement_index,
                rollback_journal,
                recovery_journal,
                text,
            )
            .await
        }
        DmlPlan::InsertFamily(plan) => {
            execute_insert_family(
                conn,
                query,
                plan,
                statement_index,
                rollback_journal,
                recovery_journal,
                text,
            )
            .await
        }
        DmlPlan::MultiUpdate(plan) => {
            execute_multi_update(
                conn,
                query,
                plan,
                statement_index,
                rollback_journal,
                recovery_journal,
                text,
            )
            .await
        }
        DmlPlan::MultiDelete(plan) => {
            execute_multi_delete(
                conn,
                query,
                plan,
                statement_index,
                rollback_journal,
                recovery_journal,
                text,
            )
            .await
        }
    };

    // The statement is legitimate, only its inverse is not derivable. Refusing
    // it means a valid UPDATE simply cannot run while protection is on, which
    // is worse than running it with the loss of exactness recorded. The row
    // images are gone either way; what we keep is the record that the table
    // was touched, so the backup-based restore can still reach it.
    let Err(error) = &outcome else {
        return outcome;
    };
    let Some(reason) = unprotectable_reason(error) else {
        return outcome;
    };
    let reason = reason.to_string();
    log::warn!("Degrading to an unprotected run: {reason}");

    let result = super::exec_on_mysql_conn(conn, query, None, 1, text).await?;
    let (operation, objects) = crate::recovery_history::parse_change_objects(query);
    recovery_journal.add_statement(crate::recovery_history::RecoveryStatement {
        id: String::new(),
        index: statement_index,
        executed_at: String::new(),
        sql: query.trim().to_string(),
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
    *degraded = Some(reason);
    Ok(result)
}

async fn execute_insert(
    conn: &mut sqlx::MySqlConnection,
    query: &str,
    plan: &InsertPlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
) -> Result<QueryResult, String> {
    let metadata = load_locked_dml_metadata(conn, &plan.table, false).await?;
    validate_insert_columns(plan, &metadata)?;

    let explicit_key_condition = explicit_insert_key_condition(plan, &metadata)?;
    if let Some(condition) = &explicit_key_condition {
        let existing = capture_rows(conn, &metadata, Some(condition)).await?;
        if !existing.is_empty() {
            return Err(
                "INSERT primary key already exists; rollback protection will not execute an ambiguous insert"
                    .to_string(),
            );
        }
    } else if metadata.auto_increment_primary_key().is_none() {
        return Err(format!(
            "{UNPROTECTABLE}INSERT requires literal values for every primary-key column, or one omitted AUTO_INCREMENT primary key"
        ));
    }

    let result = super::exec_on_mysql_conn(conn, query, None, 1, text).await?;
    let expected_rows = plan.rows.len() as u64;
    if result.affected_rows != expected_rows {
        return Err(format!(
            "INSERT changed {} rows but the protected VALUES plan expected {}; transaction was rolled back",
            result.affected_rows, expected_rows
        ));
    }

    let key_condition = match explicit_key_condition {
        Some(condition) => condition,
        None => auto_increment_insert_condition(conn, &metadata, plan.rows.len()).await?,
    };
    let inserted = capture_rows(conn, &metadata, Some(&key_condition)).await?;
    if inserted.len() != plan.rows.len() {
        return Err(format!(
            "INSERT after-image contains {} rows but {} were expected; transaction was rolled back",
            inserted.len(),
            plan.rows.len()
        ));
    }

    let mut rollback_steps = Vec::new();
    if let (Some(column), Some(next_value)) = (
        metadata.auto_increment_primary_key(),
        metadata.auto_increment_next,
    ) {
        let _ = column;
        rollback_steps.push(RollbackStep {
            statement_index,
            sql: format!(
                "ALTER TABLE {} AUTO_INCREMENT = {}",
                metadata.qualified_name(),
                next_value
            ),
            expected_affected_rows: None,
        });
    }
    for row in &inserted {
        rollback_steps.push(RollbackStep {
            statement_index,
            sql: build_insert_rollback_delete(&metadata, row)?,
            expected_affected_rows: Some(1),
        });
    }
    rollback_journal.add_steps(rollback_steps)?;
    recovery_journal.add_statement(recovery_dml_statement(
        query,
        statement_index,
        "insert",
        &metadata,
        metadata
            .writable_columns()
            .map(|column| column.name.clone())
            .collect(),
        Some(key_condition),
        Vec::new(),
        inserted,
    ))?;
    Ok(result)
}

async fn execute_update(
    conn: &mut sqlx::MySqlConnection,
    _query: &str,
    plan: &UpdatePlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
) -> Result<QueryResult, String> {
    let metadata = load_locked_dml_metadata(conn, &plan.table, true).await?;
    if metadata.primary_key.is_empty() {
        return Err(format!(
            "{UNPROTECTABLE}UPDATE rollback requires a declared primary key"
        ));
    }
    for assigned in &plan.assigned_columns {
        let column = metadata
            .column(assigned)
            .ok_or_else(|| {
                format!("{UNPROTECTABLE}UPDATE references unknown column {assigned}")
            })?;
        if column.generated {
            return Err(format!(
                "{UNPROTECTABLE}UPDATE of generated column {} cannot be rollback-protected",
                column.name
            ));
        }
        if metadata.is_primary_key(&column.name) {
            return Err(format!(
                "{UNPROTECTABLE}UPDATE of primary-key column {} cannot be rollback-protected because row identity would change",
                column.name
            ));
        }
    }

    let before = capture_rows(conn, &metadata, plan.where_sql.as_deref()).await?;
    let key_filter = captured_primary_key_filter(&metadata, &before)?;
    let guarded_query = locked_write_sql(
        &plan.statement_prefix,
        plan.where_sql.as_deref(),
        &key_filter,
    );
    let result = super::exec_on_mysql_conn(conn, &guarded_query, None, 1, text).await?;
    let after = capture_rows_by_primary_keys(conn, &metadata, &before).await?;
    let before_by_key = rows_by_primary_key(&metadata, before)?;
    let after_by_key = rows_by_primary_key(&metadata, after)?;
    if before_by_key.len() != after_by_key.len() || before_by_key.keys().ne(after_by_key.keys()) {
        return Err(
            "UPDATE changed row identity or caused rows to disappear; transaction was rolled back"
                .to_string(),
        );
    }

    let mut rollback_steps = Vec::new();
    let mut changed_before = Vec::new();
    let mut changed_after = Vec::new();
    let writable_column_names = metadata
        .writable_columns()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let mut actual_changed_columns = BTreeSet::new();
    let mut changed_count = 0_u64;
    for (key, before_row) in before_by_key {
        let after_row = after_by_key
            .get(&key)
            .expect("key sets were verified as equal");
        if before_row.values == after_row.values {
            continue;
        }
        if before_row.values.len() != writable_column_names.len()
            || after_row.values.len() != writable_column_names.len()
        {
            return Err(
                "UPDATE row image width does not match writable column metadata".to_string(),
            );
        }
        for (index, column) in writable_column_names.iter().enumerate() {
            if before_row.values[index] != after_row.values[index] {
                actual_changed_columns.insert(column.clone());
            }
        }
        changed_count += 1;
        changed_before.push(before_row.clone());
        changed_after.push(after_row.clone());
        rollback_steps.push(RollbackStep {
            statement_index,
            sql: build_update_rollback(&metadata, &before_row, after_row)?,
            expected_affected_rows: Some(1),
        });
    }
    if result.affected_rows != changed_count && result.affected_rows != after_by_key.len() as u64 {
        return Err(format!(
            "UPDATE reported {} affected rows but row diff found {}; transaction was rolled back",
            result.affected_rows, changed_count
        ));
    }
    if !rollback_steps.is_empty() {
        rollback_journal.add_steps(rollback_steps)?;
    }
    recovery_journal.add_statement(recovery_dml_statement(
        _query,
        statement_index,
        "update",
        &metadata,
        actual_changed_columns.into_iter().collect(),
        plan.where_sql.clone(),
        changed_before,
        changed_after,
    ))?;
    Ok(result)
}

async fn execute_delete(
    conn: &mut sqlx::MySqlConnection,
    _query: &str,
    plan: &DeletePlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
) -> Result<QueryResult, String> {
    let metadata = load_locked_dml_metadata(conn, &plan.table, true).await?;
    if metadata.primary_key.is_empty() {
        return Err(format!(
            "{UNPROTECTABLE}DELETE rollback requires a declared primary key"
        ));
    }
    let before = capture_rows(conn, &metadata, plan.where_sql.as_deref()).await?;
    let key_filter = captured_primary_key_filter(&metadata, &before)?;
    let guarded_query = locked_write_sql(
        &plan.statement_prefix,
        plan.where_sql.as_deref(),
        &key_filter,
    );
    let result = super::exec_on_mysql_conn(conn, &guarded_query, None, 1, text).await?;
    if result.affected_rows != before.len() as u64 {
        return Err(format!(
            "DELETE reported {} affected rows but {} before-images were locked; transaction was rolled back",
            result.affected_rows,
            before.len()
        ));
    }
    let rollback_steps = before
        .iter()
        .map(|row| RollbackStep {
            statement_index,
            sql: build_delete_rollback_insert(&metadata, row),
            expected_affected_rows: Some(1),
        })
        .collect();
    if !before.is_empty() {
        rollback_journal.add_steps(rollback_steps)?;
    }
    recovery_journal.add_statement(recovery_dml_statement(
        _query,
        statement_index,
        "delete",
        &metadata,
        metadata
            .writable_columns()
            .map(|column| column.name.clone())
            .collect(),
        plan.where_sql.clone(),
        before,
        Vec::new(),
    ))?;
    Ok(result)
}

/// Row-chunk size for synthesized VALUES statements built from a
/// materialized SELECT source. Bounds single-statement SQL size (and thus
/// max_allowed_packet exposure) while keeping AUTO_INCREMENT allocation in
/// the "simple insert" contiguous regime per chunk.
const FAMILY_CHUNK_ROWS: usize = 500;

fn empty_write_result(affected_rows: u64) -> QueryResult {
    QueryResult {
        columns: Vec::new(),
        rows: Vec::new(),
        affected_rows,
        truncated: false,
        pagination: None,
        additional_results: None,
    }
}

fn validate_family_columns(
    columns: &[String],
    metadata: &TableMetadata,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for column in columns {
        if !seen.insert(column.to_ascii_lowercase()) {
            return Err(format!("INSERT column {column} is repeated"));
        }
        let metadata_column = metadata
            .column(column)
            .ok_or_else(|| format!("{UNPROTECTABLE}INSERT references unknown column {column}"))?;
        if metadata_column.generated {
            return Err(format!(
                "{UNPROTECTABLE}INSERT into generated column {} cannot be rollback-protected",
                metadata_column.name
            ));
        }
    }
    Ok(())
}

fn key_positions(key: &[String], columns: &[String]) -> Option<Vec<usize>> {
    key.iter()
        .map(|k| columns.iter().position(|c| c.eq_ignore_ascii_case(k)))
        .collect()
}

/// Builds an OR filter that locates every planned row by at least one of the
/// given key column sets (all of them when `require_all`, for upserts where a
/// conflict can arise on any unique key). Values are either client-encoded
/// `X'..'`/`NULL` literals or raw statement literals.
fn family_key_filter(
    columns: &[String],
    rows: &[Vec<String>],
    encoded: bool,
    key_sets: &[Vec<String>],
    require_all: bool,
) -> Result<String, String> {
    let mut row_conditions = Vec::with_capacity(rows.len());
    for row in rows {
        let mut located = Vec::new();
        for key in key_sets {
            let Some(positions) = key_positions(key, columns) else {
                if require_all {
                    return Err(format!(
                        "{UNPROTECTABLE}every unique-key column must appear in the INSERT column list for exact upsert rollback"
                    ));
                }
                continue;
            };
            let mut conditions = Vec::with_capacity(key.len());
            let mut locatable = true;
            for (column, position) in key.iter().zip(&positions) {
                let literal = row
                    .get(*position)
                    .ok_or_else(|| "INSERT key position is out of bounds".to_string())?;
                if encoded {
                    conditions.push(format!(
                        "CAST({} AS BINARY) <=> {}",
                        quote_identifier(column),
                        literal
                    ));
                } else if is_safe_key_literal(literal) {
                    conditions.push(format!(
                        "{} <=> {}",
                        quote_identifier(column),
                        literal.trim()
                    ));
                } else {
                    locatable = false;
                    break;
                }
            }
            if locatable {
                located.push(format!("({})", conditions.join(" AND ")));
            } else if require_all {
                return Err(format!(
                    "{UNPROTECTABLE}INSERT key expression is not a deterministic literal"
                ));
            }
        }
        if located.is_empty() {
            return Err(format!(
                "{UNPROTECTABLE}INSERT rows cannot be located by any key for exact rollback"
            ));
        }
        row_conditions.push(if located.len() == 1 {
            located.pop().expect("checked non-empty")
        } else {
            format!("({})", located.join(" OR "))
        });
    }
    Ok(row_conditions.join(" OR "))
}

/// Runs the raw SELECT source once and returns its rows as durable
/// `X'..'`/`NULL` literals. The single evaluation IS the statement's
/// semantics: the rows read here are exactly the rows inserted.
///
/// Values are hex-encoded server-side (the same projection `capture_rows`
/// uses) because the text protocol's typed decoding cannot hand back
/// arbitrary columns as bytes.
async fn materialize_select_rows(
    conn: &mut sqlx::MySqlConnection,
    select_sql: &str,
    expected_width: usize,
) -> Result<Vec<Vec<String>>, String> {
    let description = conn
        .describe(select_sql)
        .await
        .map_err(|error| format!("Could not inspect the INSERT source SELECT: {error}"))?;
    let names: Vec<String> = description
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect();
    if names.len() != expected_width {
        return Err(format!(
            "{UNPROTECTABLE}INSERT column list expects {expected_width} columns but its SELECT produces {}",
            names.len()
        ));
    }
    let mut seen = HashSet::new();
    for name in &names {
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(format!(
                "{UNPROTECTABLE}INSERT source SELECT has duplicate output column {name}; its rows cannot be materialized"
            ));
        }
    }

    let cap = rollback_capture_row_limit();
    let projection = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let quoted = quote_identifier(name);
            format!(
                "CASE WHEN {quoted} IS NULL THEN 'NULL' \
                 ELSE CONCAT('X''', HEX(CAST({quoted} AS BINARY)), '''') END AS `v{index}`"
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT {projection} FROM ({select_sql}) AS `__tabularis_src` LIMIT {}",
        cap + 1
    );
    let fetched = conn
        .fetch_all(sqlx::raw_sql(&sql))
        .await
        .map_err(|error| format!("Could not materialize INSERT source rows: {error}"))?;
    if fetched.len() > cap {
        return Err(format!(
            "Rollback protection refused to materialize more than {cap} INSERT source rows \
             (set TABULARIS_ROLLBACK_CAPTURE_LIMIT to raise the cap or split the statement)"
        ));
    }
    let mut rows = Vec::with_capacity(fetched.len());
    for row in fetched {
        let values = (0..expected_width)
            .map(|index| mysql_text(&row, index))
            .collect::<Result<Vec<_>, _>>()?;
        rows.push(values);
    }
    Ok(rows)
}

/// Unique index column sets (excluding PRIMARY). `None` when the table has a
/// functional/expression unique index whose conflicts we cannot pre-locate.
async fn load_unique_index_column_sets(
    conn: &mut sqlx::MySqlConnection,
    metadata: &TableMetadata,
) -> Result<Option<Vec<Vec<String>>>, String> {
    let sql = format!(
        "SELECT INDEX_NAME, COLUMN_NAME FROM information_schema.STATISTICS \
         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {} AND NON_UNIQUE = 0 \
           AND INDEX_NAME <> 'PRIMARY' \
         ORDER BY INDEX_NAME, SEQ_IN_INDEX",
        sql_hex(metadata.schema.as_bytes()),
        sql_hex(metadata.name.as_bytes())
    );
    let rows = conn
        .fetch_all(sqlx::raw_sql(&sql))
        .await
        .map_err(|error| format!("Could not inspect unique indexes: {error}"))?;
    let mut sets: Vec<(String, Vec<String>)> = Vec::new();
    for row in rows {
        let index_name = mysql_text(&row, 0)?;
        let column = match row.try_get::<Option<String>, _>(1) {
            Ok(Some(column)) => column,
            Ok(None) => return Ok(None),
            Err(_) => match row.try_get::<Option<Vec<u8>>, _>(1) {
                Ok(Some(bytes)) => String::from_utf8(bytes)
                    .map_err(|error| format!("unique index column is not UTF-8: {error}"))?,
                _ => return Ok(None),
            },
        };
        match sets.last_mut() {
            Some((name, columns)) if *name == index_name => columns.push(column),
            _ => sets.push((index_name, vec![column])),
        }
    }
    Ok(Some(sets.into_iter().map(|(_, columns)| columns).collect()))
}

fn synthesize_family_insert_sql(
    metadata: &TableMetadata,
    columns: &[String],
    chunk: &[Vec<String>],
    ignore: bool,
    upsert_tail: Option<&str>,
) -> Result<String, String> {
    let column_list = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let mut rows_sql = Vec::with_capacity(chunk.len());
    for row in chunk {
        let mut literals = Vec::with_capacity(row.len());
        for (value, column) in row.iter().zip(columns) {
            let metadata_column = metadata
                .column(column)
                .ok_or_else(|| format!("unknown INSERT column {column}"))?;
            literals.push(restoration_literal(metadata_column, value));
        }
        rows_sql.push(format!("({})", literals.join(", ")));
    }
    Ok(format!(
        "INSERT {}INTO {} ({column_list}) VALUES\n{}{}",
        if ignore { "IGNORE " } else { "" },
        metadata.qualified_name(),
        rows_sql.join(",\n"),
        upsert_tail
            .map(|tail| format!("\n{tail}"))
            .unwrap_or_default()
    ))
}

/// Executes the extended INSERT family with exact pre-commit rollback:
/// IGNORE and ON DUPLICATE KEY UPDATE via key-located before/after diffs, and
/// SELECT sources via one-shot materialization into literal VALUES chunks.
async fn execute_insert_family(
    conn: &mut sqlx::MySqlConnection,
    query: &str,
    plan: &InsertFamilyPlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
) -> Result<QueryResult, String> {
    let metadata =
        load_locked_dml_metadata(conn, &plan.table, plan.upsert.is_some()).await?;
    let columns: Vec<String> = match &plan.columns {
        Some(columns) => columns.clone(),
        None => {
            if metadata.columns.iter().any(|column| column.generated) {
                return Err(format!(
                    "{UNPROTECTABLE}INSERT without an explicit column list cannot be rollback-protected on a table with generated columns"
                ));
            }
            metadata
                .columns
                .iter()
                .map(|column| column.name.clone())
                .collect()
        }
    };
    validate_family_columns(&columns, &metadata)?;
    if metadata.primary_key.is_empty() {
        return Err(format!(
            "{UNPROTECTABLE}this INSERT form requires a declared primary key for exact rollback"
        ));
    }
    if let Some(upsert) = &plan.upsert {
        for assigned in &upsert.assigned_columns {
            let column = metadata.column(assigned).ok_or_else(|| {
                format!("{UNPROTECTABLE}ON DUPLICATE KEY UPDATE references unknown column {assigned}")
            })?;
            if column.generated {
                return Err(format!(
                    "{UNPROTECTABLE}ON DUPLICATE KEY UPDATE of generated column {} cannot be rollback-protected",
                    column.name
                ));
            }
            if metadata.is_primary_key(&column.name) {
                return Err(format!(
                    "{UNPROTECTABLE}ON DUPLICATE KEY UPDATE of primary-key column {} would change row identity",
                    column.name
                ));
            }
        }
    }

    let (rows, encoded) = match &plan.source {
        InsertSource::Values(rows) => {
            for row in rows {
                if row.len() != columns.len() {
                    return Err(format!(
                        "INSERT row value count {} does not match its column list {}",
                        row.len(),
                        columns.len()
                    ));
                }
            }
            (rows.clone(), false)
        }
        InsertSource::Select(select_sql) => (
            materialize_select_rows(conn, select_sql, columns.len()).await?,
            true,
        ),
    };
    if rows.is_empty() {
        // The SELECT matched nothing: the statement inserts zero rows.
        recovery_journal.add_statement(recovery_dml_statement(
            query,
            statement_index,
            "insert",
            &metadata,
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
        ))?;
        return Ok(empty_write_result(0));
    }

    // Key sets that identify planned rows. Upserts must be locatable through
    // every unique key (a conflict can arise on any of them); plain and
    // IGNORE inserts only need the primary key.
    let mut key_sets = vec![metadata.primary_key.clone()];
    if plan.upsert.is_some() {
        match load_unique_index_column_sets(conn, &metadata).await? {
            Some(sets) => key_sets.extend(sets),
            None => {
                return Err(format!(
                    "{UNPROTECTABLE}the target table has a functional unique index; upsert conflicts cannot be pre-located"
                ));
            }
        }
    }
    let pk_locatable = key_positions(&metadata.primary_key, &columns).is_some();
    let needs_key_identity = encoded || plan.ignore || plan.upsert.is_some();
    if needs_key_identity && !pk_locatable && metadata.auto_increment_primary_key().is_none() {
        return Err(format!(
            "{UNPROTECTABLE}this INSERT form requires its primary-key columns in the column list, or an omitted AUTO_INCREMENT primary key"
        ));
    }
    if (plan.ignore || plan.upsert.is_some()) && !pk_locatable {
        return Err(format!(
            "{UNPROTECTABLE}IGNORE/ON DUPLICATE KEY UPDATE rollback requires the primary-key columns in the column list"
        ));
    }

    let auto_increment_reset =
        if let (Some(_), Some(next_value)) = (
            metadata.auto_increment_primary_key(),
            metadata.auto_increment_next,
        ) {
            Some(RollbackStep {
                statement_index,
                sql: format!(
                    "ALTER TABLE {} AUTO_INCREMENT = {}",
                    metadata.qualified_name(),
                    next_value
                ),
                expected_affected_rows: None,
            })
        } else {
            None
        };

    let upsert_tail = plan.upsert.as_ref().map(|tail| tail.tail_sql.as_str());
    let chunks: Vec<&[Vec<String>]> = if encoded {
        rows.chunks(FAMILY_CHUNK_ROWS).collect()
    } else {
        vec![rows.as_slice()]
    };

    let mut total_affected = 0_u64;
    let mut inserted_total: Vec<CapturedRow> = Vec::new();
    let mut changed_before: Vec<CapturedRow> = Vec::new();
    let mut changed_after: Vec<CapturedRow> = Vec::new();
    let mut matched_total = 0_u64;
    let mut updated_total = 0_u64;
    let mut rollback_steps: Vec<RollbackStep> = Vec::new();
    let mut actual_changed_columns = BTreeSet::new();

    for chunk in chunks {
        let filter = if pk_locatable {
            Some(family_key_filter(
                &columns,
                chunk,
                encoded,
                &key_sets,
                plan.upsert.is_some(),
            )?)
        } else {
            None
        };

        let before_by_key = if plan.ignore || plan.upsert.is_some() {
            let filter = filter.as_deref().expect("checked pk_locatable above");
            let before = capture_rows(conn, &metadata, Some(filter)).await?;
            matched_total += before.len() as u64;
            rows_by_primary_key(&metadata, before)?
        } else {
            BTreeMap::new()
        };

        let chunk_sql;
        let executed_sql = if encoded {
            chunk_sql = synthesize_family_insert_sql(
                &metadata,
                &columns,
                chunk,
                plan.ignore,
                upsert_tail,
            )?;
            chunk_sql.as_str()
        } else {
            query
        };
        let result = super::exec_on_mysql_conn(conn, executed_sql, None, 1, text).await?;
        total_affected += result.affected_rows;

        let after_condition = match &filter {
            Some(filter) => filter.clone(),
            None => auto_increment_insert_condition(conn, &metadata, chunk.len()).await?,
        };
        let after = capture_rows(conn, &metadata, Some(&after_condition)).await?;
        let after_by_key = rows_by_primary_key(&metadata, after)?;

        if plan.ignore || plan.upsert.is_some() {
            for (key, after_row) in after_by_key {
                match before_by_key.get(&key) {
                    None => {
                        rollback_steps.push(RollbackStep {
                            statement_index,
                            sql: build_insert_rollback_delete(&metadata, &after_row)?,
                            expected_affected_rows: Some(1),
                        });
                        inserted_total.push(after_row);
                    }
                    Some(before_row) if before_row.values != after_row.values => {
                        for (index, column) in metadata.writable_columns().enumerate() {
                            if before_row.values.get(index) != after_row.values.get(index) {
                                actual_changed_columns.insert(column.name.clone());
                            }
                        }
                        rollback_steps.push(RollbackStep {
                            statement_index,
                            sql: build_update_rollback(&metadata, before_row, &after_row)?,
                            expected_affected_rows: Some(1),
                        });
                        updated_total += 1;
                        changed_before.push(before_row.clone());
                        changed_after.push(after_row);
                    }
                    Some(_) => {}
                }
            }
        } else {
            if after_by_key.len() != chunk.len() {
                return Err(format!(
                    "INSERT after-image contains {} rows but {} were expected; transaction was rolled back",
                    after_by_key.len(),
                    chunk.len()
                ));
            }
            if result.affected_rows != chunk.len() as u64 {
                return Err(format!(
                    "INSERT changed {} rows but the protected plan expected {}; transaction was rolled back",
                    result.affected_rows,
                    chunk.len()
                ));
            }
            for (_, after_row) in after_by_key {
                rollback_steps.push(RollbackStep {
                    statement_index,
                    sql: build_insert_rollback_delete(&metadata, &after_row)?,
                    expected_affected_rows: Some(1),
                });
                inserted_total.push(after_row);
            }
        }
    }

    // Affected-rows tripwire for the conflict-handling forms. MySQL counts an
    // upserted row as 2 and an IGNOREd duplicate as 0; with CLIENT_FOUND_ROWS
    // every matched row counts 1. The row diffs above are the ground truth —
    // this only catches a diverging execution.
    if plan.upsert.is_some() {
        let inserted = inserted_total.len() as u64;
        let standard = inserted + 2 * updated_total;
        let found_rows = inserted + matched_total;
        if total_affected != standard && total_affected != found_rows {
            return Err(format!(
                "upsert reported {total_affected} affected rows but the row diff found {inserted} inserts and {updated_total} updates; transaction was rolled back"
            ));
        }
    } else if plan.ignore && total_affected != inserted_total.len() as u64 {
        return Err(format!(
            "INSERT IGNORE reported {total_affected} affected rows but {} rows were inserted; transaction was rolled back",
            inserted_total.len()
        ));
    }

    if !rollback_steps.is_empty() {
        if !inserted_total.is_empty() {
            if let Some(reset) = auto_increment_reset {
                rollback_steps.insert(0, reset);
            }
        }
        rollback_journal.add_steps(rollback_steps)?;
    }

    let operation = if plan.upsert.is_some() { "upsert" } else { "insert" };
    let mut before_rows = changed_before;
    let mut after_rows = changed_after;
    after_rows.extend(inserted_total);
    if operation == "insert" {
        before_rows.clear();
    }
    let affected_columns = if actual_changed_columns.is_empty() {
        metadata
            .writable_columns()
            .map(|column| column.name.clone())
            .collect()
    } else {
        actual_changed_columns.into_iter().collect()
    };
    recovery_journal.add_statement(recovery_dml_statement(
        query,
        statement_index,
        operation,
        &metadata,
        affected_columns,
        None,
        before_rows,
        after_rows,
    ))?;
    Ok(empty_write_result(total_affected))
}

/// Materializes the primary keys a multi-table statement touches on one
/// target, using the statement's own table references and WHERE clause under
/// `FOR UPDATE` so the set cannot change before the write executes.
async fn materialize_target_keys(
    conn: &mut sqlx::MySqlConnection,
    alias: &str,
    metadata: &TableMetadata,
    refs_sql: &str,
    where_sql: Option<&str>,
) -> Result<Vec<Vec<String>>, String> {
    let projection = metadata
        .primary_key
        .iter()
        .enumerate()
        .map(|(index, key)| {
            format!(
                "CASE WHEN {alias}.{key} IS NULL THEN 'NULL' \
                 ELSE CONCAT('X''', HEX(CAST({alias}.{key} AS BINARY)), '''') END AS `k{index}`",
                alias = quote_identifier(alias),
                key = quote_identifier(key)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let where_clause = where_sql
        .map(|clause| format!(" WHERE {clause}"))
        .unwrap_or_default();
    let limit = rollback_capture_row_limit();
    let sql = format!(
        "SELECT {projection} FROM {refs_sql}{where_clause} LIMIT {} FOR UPDATE",
        limit + 1
    );
    let rows = conn
        .fetch_all(sqlx::raw_sql(&sql))
        .await
        .map_err(|error| format!("Could not materialize affected keys: {error}"))?;
    if rows.len() > limit {
        return Err(format!(
            "Rollback protection refused to capture more than {limit} affected rows \
             (set TABULARIS_ROLLBACK_CAPTURE_LIMIT to raise the cap or split the statement)"
        ));
    }
    let width = metadata.primary_key.len();
    let mut keys = BTreeSet::new();
    for row in rows {
        let mut key = Vec::with_capacity(width);
        for index in 0..width {
            key.push(mysql_text(&row, index)?);
        }
        keys.insert(key);
    }
    Ok(keys.into_iter().collect())
}

fn encoded_pk_filter(metadata: &TableMetadata, keys: &[Vec<String>]) -> String {
    if keys.is_empty() {
        return "FALSE".to_string();
    }
    keys.iter()
        .map(|key| {
            format!(
                "({})",
                metadata
                    .primary_key
                    .iter()
                    .zip(key)
                    .map(|(column, value)| {
                        format!(
                            "CAST({} AS BINARY) <=> {}",
                            quote_identifier(column),
                            value
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" AND ")
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

async fn load_multi_table_metadata(
    conn: &mut sqlx::MySqlConnection,
    plan: &MultiTablePlan,
    require_assignable: bool,
) -> Result<Vec<TableMetadata>, String> {
    let mut metas = Vec::with_capacity(plan.targets.len());
    for target in &plan.targets {
        let metadata = load_locked_dml_metadata(conn, &target.table, true).await?;
        if metadata.primary_key.is_empty() {
            return Err(format!(
                "{UNPROTECTABLE}rollback for {} requires a declared primary key",
                metadata.qualified_name()
            ));
        }
        if require_assignable {
            for assigned in &target.assigned_columns {
                let column = metadata.column(assigned).ok_or_else(|| {
                    format!("{UNPROTECTABLE}UPDATE references unknown column {assigned}")
                })?;
                if column.generated {
                    return Err(format!(
                        "{UNPROTECTABLE}UPDATE of generated column {} cannot be rollback-protected",
                        column.name
                    ));
                }
                if metadata.is_primary_key(&column.name) {
                    return Err(format!(
                        "{UNPROTECTABLE}UPDATE of primary-key column {} would change row identity",
                        column.name
                    ));
                }
            }
        }
        metas.push(metadata);
    }
    Ok(metas)
}

/// Multi-table / aliased UPDATE: materialize target PKs, capture before
/// images, run the original statement verbatim, diff, and journal per-row
/// inverse updates.
async fn execute_multi_update(
    conn: &mut sqlx::MySqlConnection,
    query: &str,
    plan: &MultiTablePlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
) -> Result<QueryResult, String> {
    let metas = load_multi_table_metadata(conn, plan, true).await?;

    let mut before_per_target = Vec::with_capacity(plan.targets.len());
    for (target, metadata) in plan.targets.iter().zip(&metas) {
        let keys = materialize_target_keys(
            conn,
            &target.alias,
            metadata,
            &plan.refs_sql,
            plan.where_sql.as_deref(),
        )
        .await?;
        let filter = encoded_pk_filter(metadata, &keys);
        let before = if keys.is_empty() {
            Vec::new()
        } else {
            capture_rows(conn, metadata, Some(&filter)).await?
        };
        before_per_target.push(before);
    }

    let result = super::exec_on_mysql_conn(conn, query, None, 1, text).await?;

    let mut total_changed = 0_u64;
    let mut total_matched = 0_u64;
    for ((target, metadata), before) in plan
        .targets
        .iter()
        .zip(&metas)
        .zip(before_per_target)
    {
        total_matched += before.len() as u64;
        let after = capture_rows_by_primary_keys(conn, metadata, &before).await?;
        let before_by_key = rows_by_primary_key(metadata, before)?;
        let after_by_key = rows_by_primary_key(metadata, after)?;
        if before_by_key.len() != after_by_key.len()
            || before_by_key.keys().ne(after_by_key.keys())
        {
            return Err(format!(
                "UPDATE changed row identity in {}; transaction was rolled back",
                metadata.qualified_name()
            ));
        }
        let mut rollback_steps = Vec::new();
        let mut changed_before = Vec::new();
        let mut changed_after = Vec::new();
        let mut actual_changed_columns = BTreeSet::new();
        let writable: Vec<String> = metadata
            .writable_columns()
            .map(|column| column.name.clone())
            .collect();
        for (key, before_row) in before_by_key {
            let after_row = after_by_key
                .get(&key)
                .expect("key sets were verified as equal");
            if before_row.values == after_row.values {
                continue;
            }
            for (index, column) in writable.iter().enumerate() {
                if before_row.values.get(index) != after_row.values.get(index) {
                    actual_changed_columns.insert(column.clone());
                }
            }
            total_changed += 1;
            rollback_steps.push(RollbackStep {
                statement_index,
                sql: build_update_rollback(metadata, &before_row, after_row)?,
                expected_affected_rows: Some(1),
            });
            changed_before.push(before_row);
            changed_after.push(after_row.clone());
        }
        if !rollback_steps.is_empty() {
            rollback_journal.add_steps(rollback_steps)?;
        }
        if !changed_before.is_empty() {
            recovery_journal.add_statement(recovery_dml_statement(
                query,
                statement_index,
                "update",
                metadata,
                actual_changed_columns.into_iter().collect(),
                plan.where_sql.clone(),
                changed_before,
                changed_after,
            ))?;
        }
        let _ = target;
    }

    if result.affected_rows != total_changed && result.affected_rows != total_matched {
        return Err(format!(
            "UPDATE reported {} affected rows but the row diff found {total_changed}; transaction was rolled back",
            result.affected_rows
        ));
    }
    Ok(result)
}

/// Multi-table / aliased DELETE: materialize target PKs, capture full before
/// images, run the original statement verbatim, verify the captured rows are
/// gone, and journal per-row restoring inserts.
async fn execute_multi_delete(
    conn: &mut sqlx::MySqlConnection,
    query: &str,
    plan: &MultiTablePlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
) -> Result<QueryResult, String> {
    let metas = load_multi_table_metadata(conn, plan, false).await?;

    let mut captured = Vec::with_capacity(plan.targets.len());
    for (target, metadata) in plan.targets.iter().zip(&metas) {
        let keys = materialize_target_keys(
            conn,
            &target.alias,
            metadata,
            &plan.refs_sql,
            plan.where_sql.as_deref(),
        )
        .await?;
        let filter = encoded_pk_filter(metadata, &keys);
        let before = if keys.is_empty() {
            Vec::new()
        } else {
            capture_rows(conn, metadata, Some(&filter)).await?
        };
        captured.push((filter, before));
    }

    let result = super::exec_on_mysql_conn(conn, query, None, 1, text).await?;

    let mut total_deleted = 0_u64;
    for (metadata, (filter, before)) in metas.iter().zip(captured) {
        if before.is_empty() {
            continue;
        }
        let remaining = capture_rows(conn, metadata, Some(&filter)).await?;
        if !remaining.is_empty() {
            return Err(format!(
                "DELETE left {} of its matched rows in {}; transaction was rolled back",
                remaining.len(),
                metadata.qualified_name()
            ));
        }
        total_deleted += before.len() as u64;
        let rollback_steps = before
            .iter()
            .map(|row| RollbackStep {
                statement_index,
                sql: build_delete_rollback_insert(metadata, row),
                expected_affected_rows: Some(1),
            })
            .collect();
        rollback_journal.add_steps(rollback_steps)?;
        recovery_journal.add_statement(recovery_dml_statement(
            query,
            statement_index,
            "delete",
            metadata,
            metadata
                .writable_columns()
                .map(|column| column.name.clone())
                .collect(),
            plan.where_sql.clone(),
            before,
            Vec::new(),
        ))?;
    }

    if result.affected_rows != total_deleted {
        return Err(format!(
            "DELETE reported {} affected rows but {total_deleted} before-images were locked; transaction was rolled back",
            result.affected_rows
        ));
    }
    Ok(result)
}

async fn execute_protected_ddl(
    conn: &mut sqlx::MySqlConnection,
    query: &str,
    plan: &DdlPlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    _text: super::TextProto,
) -> Result<QueryResult, String> {
    let inverse = prepare_ddl_inverse(conn, plan).await?;
    let (operation, objects, affected_columns) = recovery_ddl_details(conn, plan).await?;
    rollback_journal.add_step(RollbackStep {
        statement_index,
        sql: inverse.clone(),
        expected_affected_rows: None,
    })?;
    // CREATE VIEW is not supported by MySQL's prepared-statement protocol.
    // Run the whole reversible DDL allowlist through COM_QUERY so behavior is
    // consistent across MySQL and MariaDB.
    match super::exec_on_mysql_conn(conn, query, None, 1, super::TextProto::protocol_only(true))
        .await
    {
        Ok(result) => {
            recovery_journal.add_statement(RecoveryStatement {
                id: String::new(),
                index: statement_index,
                executed_at: String::new(),
                sql: query.trim().to_string(),
                category: "ddl".to_string(),
                operation,
                objects,
                affected_columns,
                condition: None,
                columns: Vec::new(),
                primary_key: Vec::new(),
                before_rows: Vec::new(),
                after_rows: Vec::new(),
                inverse_sql: Some(inverse),
                exact: true,
            })?;
            Ok(result)
        }
        Err(error) => Err(format!(
            "{error}; DDL outcome may be partial or unknown, so its durable inverse was retained"
        )),
    }
}

fn recovery_dml_statement(
    query: &str,
    statement_index: usize,
    operation: &str,
    metadata: &TableMetadata,
    affected_columns: Vec<String>,
    condition: Option<String>,
    before_rows: Vec<CapturedRow>,
    after_rows: Vec<CapturedRow>,
) -> RecoveryStatement {
    RecoveryStatement {
        id: String::new(),
        index: statement_index,
        executed_at: String::new(),
        sql: query.trim().to_string(),
        category: "dml".to_string(),
        operation: operation.to_string(),
        objects: vec![RecoveryObject {
            kind: "table".to_string(),
            schema: metadata.schema.clone(),
            name: metadata.name.clone(),
        }],
        affected_columns,
        condition,
        columns: metadata
            .writable_columns()
            .map(|column| RecoveryColumn {
                name: column.name.clone(),
                data_type: column.data_type.clone(),
            })
            .collect(),
        primary_key: metadata.primary_key.clone(),
        before_rows: before_rows
            .into_iter()
            .map(|row| RecoveryRow { values: row.values })
            .collect(),
        after_rows: after_rows
            .into_iter()
            .map(|row| RecoveryRow { values: row.values })
            .collect(),
        inverse_sql: None,
        exact: true,
    }
}

async fn recovery_ddl_details(
    conn: &mut sqlx::MySqlConnection,
    plan: &DdlPlan,
) -> Result<(String, Vec<RecoveryObject>, Vec<String>), String> {
    let table_object = |object: ObjectName| RecoveryObject {
        kind: "table".to_string(),
        schema: object
            .schema
            .expect("resolved recovery object always has a schema"),
        name: object.name,
    };
    match plan {
        DdlPlan::CreateTable(table) => Ok((
            "create_table".to_string(),
            vec![table_object(resolve_object(conn, table).await?)],
            Vec::new(),
        )),
        DdlPlan::CreateDatabase(database) => Ok((
            "create_database".to_string(),
            vec![RecoveryObject {
                kind: "database".to_string(),
                schema: database.clone(),
                name: database.clone(),
            }],
            Vec::new(),
        )),
        DdlPlan::CreateView(view) => Ok((
            "create_view".to_string(),
            vec![table_object(resolve_object(conn, view).await?)],
            Vec::new(),
        )),
        DdlPlan::CreateIndex { table, index } | DdlPlan::AlterAddIndex { table, index } => Ok((
            "add_index".to_string(),
            vec![table_object(resolve_object(conn, table).await?)],
            vec![index.clone()],
        )),
        DdlPlan::RenameTable { from, to } | DdlPlan::AlterRenameTable { from, to } => Ok((
            "rename_table".to_string(),
            vec![
                table_object(resolve_object(conn, from).await?),
                table_object(resolve_object(conn, to).await?),
            ],
            Vec::new(),
        )),
        DdlPlan::AlterAddColumn { table, column } => Ok((
            "add_column".to_string(),
            vec![table_object(resolve_object(conn, table).await?)],
            vec![column.clone()],
        )),
        DdlPlan::AlterRenameColumn { table, from, to } => Ok((
            "rename_column".to_string(),
            vec![table_object(resolve_object(conn, table).await?)],
            vec![from.clone(), to.clone()],
        )),
    }
}

fn recovery_server_identity_label(server: &ServerIdentity) -> String {
    match server {
        ServerIdentity::Uuid(uuid) => format!("uuid:{uuid}"),
        ServerIdentity::Uid(uid) => format!("uid:{uid}"),
        ServerIdentity::HostPort { hostname, port } => format!("host:{hostname}:{port}"),
    }
}

async fn read_rollback_environment(
    conn: &mut sqlx::MySqlConnection,
    connection_id: String,
    connection_name: String,
) -> Result<RollbackEnvironment, String> {
    let row = conn
        .fetch_one(sqlx::raw_sql(
            "SELECT DATABASE(), CURRENT_USER(), @@hostname, @@port",
        ))
        .await
        .map_err(|error| format!("Could not read rollback environment: {error}"))?;
    let database = row
        .try_get::<Option<String>, _>(0)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Rollback protection requires a selected database".to_string())?;
    let current_user = row
        .try_get::<String, _>(1)
        .map_err(|error| error.to_string())?;
    let hostname = row
        .try_get::<String, _>(2)
        .map_err(|error| error.to_string())?;
    let port = row
        .try_get::<u32, _>(3)
        .map_err(|error| error.to_string())?;
    let identity_rows = conn
        .fetch_all(sqlx::raw_sql(
            "SHOW VARIABLES WHERE Variable_name IN ('server_uuid', 'server_uid')",
        ))
        .await
        .map_err(|error| format!("Could not read server identity: {error}"))?;
    let mut server_uuid = None;
    let mut server_uid = None;
    for identity_row in identity_rows {
        let name = identity_row
            .try_get::<String, _>(0)
            .map_err(|error| error.to_string())?;
        let value = identity_row
            .try_get::<String, _>(1)
            .map_err(|error| error.to_string())?;
        if value.trim().is_empty() {
            continue;
        }
        if name.eq_ignore_ascii_case("server_uuid") {
            server_uuid = Some(value);
        } else if name.eq_ignore_ascii_case("server_uid") {
            server_uid = Some(value);
        }
    }
    let server = if let Some(uuid) = server_uuid {
        ServerIdentity::Uuid(uuid)
    } else if let Some(uid) = server_uid {
        ServerIdentity::Uid(uid)
    } else {
        ServerIdentity::HostPort {
            hostname,
            port: u16::try_from(port)
                .map_err(|_| format!("Server reported invalid TCP port {port}"))?,
        }
    };
    Ok(RollbackEnvironment {
        connection_id,
        connection_name,
        database,
        current_user,
        server,
    })
}

#[derive(Debug, Clone)]
struct ColumnMetadata {
    name: String,
    data_type: String,
    generated: bool,
    auto_increment: bool,
}

/// Whether an `information_schema.COLUMNS` row describes a generated column.
///
/// Keyed on GENERATION_EXPRESSION rather than EXTRA. MySQL writes
/// `DEFAULT_GENERATED` into EXTRA for any column declared
/// `DEFAULT CURRENT_TIMESTAMP`, so testing EXTRA for the substring "GENERATED"
/// classified ordinary `created_at` / `updated_at` columns as generated and
/// refused to rollback-protect an UPDATE that assigned to one. A real
/// generated column always carries an expression; an ordinary one never does.
pub(super) fn is_generated_column(generation_expression: &str) -> bool {
    !generation_expression.trim().is_empty()
}

#[derive(Debug, Clone)]
struct TableMetadata {
    schema: String,
    name: String,
    engine: String,
    columns: Vec<ColumnMetadata>,
    primary_key: Vec<String>,
    auto_increment_next: Option<u64>,
}

impl TableMetadata {
    fn qualified_name(&self) -> String {
        format!(
            "{}.{}",
            quote_identifier(&self.schema),
            quote_identifier(&self.name)
        )
    }

    fn column(&self, name: &str) -> Option<&ColumnMetadata> {
        self.columns
            .iter()
            .find(|column| column.name.eq_ignore_ascii_case(name))
    }

    fn writable_columns(&self) -> impl Iterator<Item = &ColumnMetadata> {
        self.columns.iter().filter(|column| !column.generated)
    }

    fn is_primary_key(&self, name: &str) -> bool {
        self.primary_key
            .iter()
            .any(|column| column.eq_ignore_ascii_case(name))
    }

    fn auto_increment_primary_key(&self) -> Option<&ColumnMetadata> {
        if self.primary_key.len() != 1 {
            return None;
        }
        self.column(&self.primary_key[0])
            .filter(|column| column.auto_increment)
    }
}

#[derive(Debug, Clone)]
struct CapturedRow {
    values: Vec<String>,
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

async fn load_table_metadata(
    conn: &mut sqlx::MySqlConnection,
    object: &ObjectName,
) -> Result<TableMetadata, String> {
    let schema = match &object.schema {
        Some(schema) => schema.clone(),
        None => current_database(conn).await?,
    };
    let table_sql = format!(
        "SELECT ENGINE, TABLE_TYPE, AUTO_INCREMENT \
         FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {}",
        sql_hex(schema.as_bytes()),
        sql_hex(object.name.as_bytes())
    );
    let table_row = conn
        .fetch_optional(sqlx::raw_sql(&table_sql))
        .await
        .map_err(|error| format!("Could not inspect target table: {error}"))?
        .ok_or_else(|| format!("Target table {}.{} does not exist", schema, object.name))?;
    let engine = table_row
        .try_get::<Option<String>, _>(0)
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let table_type = mysql_text(&table_row, 1)?;
    if !table_type.eq_ignore_ascii_case("BASE TABLE") {
        return Err("DML through views is not rollback-protected".to_string());
    }
    let auto_increment_next = table_row
        .try_get::<Option<u64>, _>(2)
        .map_err(|error| error.to_string())?;

    // GENERATION_EXPRESSION, not EXTRA, decides whether a column is generated.
    // MySQL puts `DEFAULT_GENERATED` in EXTRA for any column with
    // `DEFAULT CURRENT_TIMESTAMP`, so matching on the substring "GENERATED"
    // classified ordinary `created_at` / `updated_at` columns as generated and
    // refused to protect an UPDATE that touched them. Only a real generated
    // column carries an expression here.
    let column_sql = format!(
        "SELECT COLUMN_NAME, DATA_TYPE, EXTRA, GENERATION_EXPRESSION \
         FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {} \
         ORDER BY ORDINAL_POSITION",
        sql_hex(schema.as_bytes()),
        sql_hex(object.name.as_bytes())
    );
    let column_rows = conn
        .fetch_all(sqlx::raw_sql(&column_sql))
        .await
        .map_err(|error| format!("Could not inspect target columns: {error}"))?;
    let mut columns = Vec::with_capacity(column_rows.len());
    for row in column_rows {
        let name = mysql_text(&row, 0)?;
        let data_type = mysql_text(&row, 1)?;
        let extra = mysql_text(&row, 2)?;
        // NULL for ordinary columns, the expression text for generated ones.
        let generation_expression = mysql_text(&row, 3).unwrap_or_default();
        columns.push(ColumnMetadata {
            name,
            data_type,
            generated: is_generated_column(&generation_expression),
            auto_increment: extra.to_ascii_uppercase().contains("AUTO_INCREMENT"),
        });
    }
    if columns.is_empty() {
        return Err("Target table has no inspectable columns".to_string());
    }

    let primary_sql = format!(
        "SELECT COLUMN_NAME FROM information_schema.STATISTICS \
         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {} AND INDEX_NAME = 'PRIMARY' \
         ORDER BY SEQ_IN_INDEX",
        sql_hex(schema.as_bytes()),
        sql_hex(object.name.as_bytes())
    );
    let primary_key = conn
        .fetch_all(sqlx::raw_sql(&primary_sql))
        .await
        .map_err(|error| format!("Could not inspect primary key: {error}"))?
        .into_iter()
        .map(|row| mysql_text(&row, 0))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TableMetadata {
        schema,
        name: object.name.clone(),
        engine,
        columns,
        primary_key,
        auto_increment_next,
    })
}

async fn load_locked_dml_metadata(
    conn: &mut sqlx::MySqlConnection,
    object: &ObjectName,
    check_cascades: bool,
) -> Result<TableMetadata, String> {
    let initial = load_table_metadata(conn, object).await.map_err(|error| {
        // A view target is a legitimate statement whose inverse we cannot
        // build — degrade instead of refusing the write outright.
        if error.contains("not rollback-protected") {
            format!("{UNPROTECTABLE}{error}")
        } else {
            error
        }
    })?;
    let lock_sql = format!(
        "SELECT 1 FROM {} WHERE FALSE FOR UPDATE",
        initial.qualified_name()
    );
    conn.fetch_optional(sqlx::raw_sql(&lock_sql))
        .await
        .map_err(|error| format!("Could not lock target table metadata: {error}"))?;
    let locked = load_table_metadata(conn, object).await?;
    // Table-shape refusals (engine, triggers, cascades, exotic column types)
    // mark the statement as unprotectable rather than failing it: under
    // default-on protection a runnable statement must stay runnable, with the
    // loss of exactness recorded in the recovery journal.
    ensure_dml_safe_table(conn, &locked, check_cascades)
        .await
        .map_err(|error| format!("{UNPROTECTABLE}{error}"))?;
    Ok(locked)
}

async fn current_database(conn: &mut sqlx::MySqlConnection) -> Result<String, String> {
    conn.fetch_one(sqlx::raw_sql("SELECT DATABASE()"))
        .await
        .map_err(|error| error.to_string())?
        .try_get::<Option<String>, _>(0)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "No database is selected".to_string())
}

async fn ensure_dml_safe_table(
    conn: &mut sqlx::MySqlConnection,
    metadata: &TableMetadata,
    check_cascades: bool,
) -> Result<(), String> {
    if !metadata.engine.eq_ignore_ascii_case("INNODB") {
        return Err(format!(
            "Table {} uses {}; exact transactional rollback requires InnoDB",
            metadata.qualified_name(),
            metadata.engine
        ));
    }
    const UNSUPPORTED_TYPES: &[&str] = &[
        "geometry",
        "point",
        "linestring",
        "polygon",
        "multipoint",
        "multilinestring",
        "multipolygon",
        "geometrycollection",
        "vector",
    ];
    if let Some(column) = metadata.columns.iter().find(|column| {
        UNSUPPORTED_TYPES
            .iter()
            .any(|kind| column.data_type.eq_ignore_ascii_case(kind))
    }) {
        return Err(format!(
            "Column {} uses unsupported rollback type {}",
            column.name, column.data_type
        ));
    }
    let trigger_sql = format!(
        "SELECT COUNT(*) FROM information_schema.TRIGGERS \
         WHERE TRIGGER_SCHEMA = {} AND EVENT_OBJECT_TABLE = {}",
        sql_hex(metadata.schema.as_bytes()),
        sql_hex(metadata.name.as_bytes())
    );
    if scalar_count(conn, &trigger_sql).await? > 0 {
        return Err(format!(
            "Table {} has triggers; hidden writes are fail-closed",
            metadata.qualified_name()
        ));
    }
    if check_cascades {
        let cascade_sql = format!(
            "SELECT COUNT(*) FROM information_schema.REFERENTIAL_CONSTRAINTS \
             WHERE ((CONSTRAINT_SCHEMA = {schema} AND TABLE_NAME = {table}) \
                OR (UNIQUE_CONSTRAINT_SCHEMA = {schema} AND REFERENCED_TABLE_NAME = {table})) \
               AND (DELETE_RULE NOT IN ('RESTRICT', 'NO ACTION') \
                 OR UPDATE_RULE NOT IN ('RESTRICT', 'NO ACTION'))",
            schema = sql_hex(metadata.schema.as_bytes()),
            table = sql_hex(metadata.name.as_bytes())
        );
        if scalar_count(conn, &cascade_sql).await? > 0 {
            return Err(format!(
                "Table {} participates in cascading foreign keys; transitive writes are fail-closed",
                metadata.qualified_name()
            ));
        }
    }
    Ok(())
}

async fn scalar_count(conn: &mut sqlx::MySqlConnection, sql: &str) -> Result<u64, String> {
    let value = conn
        .fetch_one(sqlx::raw_sql(sql))
        .await
        .map_err(|error| error.to_string())?
        .try_get::<i64, _>(0)
        .map_err(|error| error.to_string())?;
    u64::try_from(value).map_err(|_| "COUNT(*) returned a negative value".to_string())
}

fn validate_insert_columns(plan: &InsertPlan, metadata: &TableMetadata) -> Result<(), String> {
    let mut seen = HashSet::new();
    for column in &plan.columns {
        let normalized = column.to_ascii_lowercase();
        if !seen.insert(normalized) {
            return Err(format!("INSERT column {column} is repeated"));
        }
        let metadata_column = metadata
            .column(column)
            .ok_or_else(|| {
                format!("{UNPROTECTABLE}INSERT references unknown column {column}")
            })?;
        if metadata_column.generated {
            return Err(format!(
                "{UNPROTECTABLE}INSERT into generated column {} cannot be rollback-protected",
                metadata_column.name
            ));
        }
    }
    Ok(())
}

fn explicit_insert_key_condition(
    plan: &InsertPlan,
    metadata: &TableMetadata,
) -> Result<Option<String>, String> {
    if metadata.primary_key.is_empty() {
        return Ok(None);
    }
    let positions: Option<Vec<usize>> = metadata
        .primary_key
        .iter()
        .map(|key| {
            plan.columns
                .iter()
                .position(|column| column.eq_ignore_ascii_case(key))
        })
        .collect();
    let Some(positions) = positions else {
        return Ok(None);
    };
    let mut row_conditions = Vec::with_capacity(plan.rows.len());
    for row in &plan.rows {
        let mut key_conditions = Vec::with_capacity(positions.len());
        for (key, position) in metadata.primary_key.iter().zip(&positions) {
            let literal = row
                .get(*position)
                .ok_or_else(|| "INSERT key position is out of bounds".to_string())?;
            if !is_safe_key_literal(literal) {
                return Err(format!(
                    "INSERT primary-key expression {literal} is not a deterministic literal"
                ));
            }
            key_conditions.push(format!("{} <=> {}", quote_identifier(key), literal));
        }
        row_conditions.push(format!("({})", key_conditions.join(" AND ")));
    }
    Ok(Some(row_conditions.join(" OR ")))
}

fn is_safe_key_literal(value: &str) -> bool {
    let value = value.trim();
    if value.eq_ignore_ascii_case("NULL") || value.is_empty() {
        return false;
    }
    if (value.starts_with('\'') && value.ends_with('\''))
        || value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
            .is_some_and(|hex| !hex.is_empty() && hex.chars().all(|ch| ch.is_ascii_hexdigit()))
    {
        return true;
    }
    let upper = value.to_ascii_uppercase();
    if ((upper.starts_with("X'") || upper.starts_with("B'") || upper.starts_with("N'"))
        && value.ends_with('\''))
        || matches!(upper.as_str(), "TRUE" | "FALSE")
    {
        return true;
    }
    value.parse::<i128>().is_ok() || value.parse::<u128>().is_ok()
}

async fn auto_increment_insert_condition(
    conn: &mut sqlx::MySqlConnection,
    metadata: &TableMetadata,
    row_count: usize,
) -> Result<String, String> {
    let primary = metadata
        .auto_increment_primary_key()
        .ok_or_else(|| "AUTO_INCREMENT primary key is unavailable".to_string())?;
    let row = conn
        .fetch_one(sqlx::raw_sql(
            "SELECT LAST_INSERT_ID(), @@auto_increment_increment",
        ))
        .await
        .map_err(|error| format!("Could not resolve inserted AUTO_INCREMENT keys: {error}"))?;
    let first_id = row
        .try_get::<u64, _>(0)
        .map_err(|error| error.to_string())?;
    let increment = row
        .try_get::<u64, _>(1)
        .map_err(|error| error.to_string())?;
    if first_id == 0 || increment == 0 {
        return Err(
            "Server did not expose a usable LAST_INSERT_ID/auto_increment_increment".to_string(),
        );
    }
    let mut conditions = Vec::with_capacity(row_count);
    for offset in 0..row_count as u64 {
        let id = first_id
            .checked_add(offset.saturating_mul(increment))
            .ok_or_else(|| "AUTO_INCREMENT key range overflow".to_string())?;
        conditions.push(format!("{} = {}", quote_identifier(&primary.name), id));
    }
    Ok(conditions.join(" OR "))
}

/// Default before-image cap. An unbounded UPDATE/DELETE on a big table would
/// otherwise lock the whole table with `FOR UPDATE`, hex-encode every row into
/// memory (~2x data size) and rewrite the journal per statement. Fail closed
/// and tell the user to batch instead.
const ROLLBACK_CAPTURE_ROW_LIMIT: usize = 100_000;

fn rollback_capture_row_limit() -> usize {
    std::env::var("TABULARIS_ROLLBACK_CAPTURE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(ROLLBACK_CAPTURE_ROW_LIMIT)
}

async fn capture_rows(
    conn: &mut sqlx::MySqlConnection,
    metadata: &TableMetadata,
    condition: Option<&str>,
) -> Result<Vec<CapturedRow>, String> {
    let columns: Vec<_> = metadata.writable_columns().collect();
    let projection = columns
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
        .join(", ");
    let where_clause = condition
        .map(|condition| format!(" WHERE {condition}"))
        .unwrap_or_default();
    let limit = rollback_capture_row_limit();
    // LIMIT cap+1: seeing cap+1 rows proves the statement exceeds the budget
    // without counting the whole table first.
    let sql = format!(
        "SELECT {projection} FROM {}{where_clause} LIMIT {} FOR UPDATE",
        metadata.qualified_name(),
        limit + 1
    );
    let rows = conn
        .fetch_all(sqlx::raw_sql(&sql))
        .await
        .map_err(|error| format!("Could not capture row image: {error}"))?;
    if rows.len() > limit {
        return Err(format!(
            "Rollback protection refused to capture more than {limit} rows from {} \
             (set TABULARIS_ROLLBACK_CAPTURE_LIMIT to raise the cap, split the \
             statement into smaller batches, or run it with protection disabled)",
            metadata.qualified_name()
        ));
    }
    rows.into_iter()
        .map(|row| {
            let values = (0..columns.len())
                .map(|index| mysql_text(&row, index))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CapturedRow { values })
        })
        .collect()
}

async fn capture_rows_by_primary_keys(
    conn: &mut sqlx::MySqlConnection,
    metadata: &TableMetadata,
    rows: &[CapturedRow],
) -> Result<Vec<CapturedRow>, String> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let condition = captured_primary_key_filter(metadata, rows)?;
    capture_rows(conn, metadata, Some(&condition)).await
}

fn captured_primary_key_filter(
    metadata: &TableMetadata,
    rows: &[CapturedRow],
) -> Result<String, String> {
    if rows.is_empty() {
        return Ok("FALSE".to_string());
    }
    Ok(rows
        .iter()
        .map(|row| primary_key_condition(metadata, row))
        .collect::<Result<Vec<_>, _>>()?
        .join(" OR "))
}

pub(super) fn locked_write_sql(
    statement_prefix: &str,
    where_sql: Option<&str>,
    key_filter: &str,
) -> String {
    match where_sql {
        Some(where_sql) => {
            format!("{statement_prefix}\nWHERE ({where_sql}) AND ({key_filter})")
        }
        None => format!("{statement_prefix}\nWHERE {key_filter}"),
    }
}

fn rows_by_primary_key(
    metadata: &TableMetadata,
    rows: Vec<CapturedRow>,
) -> Result<BTreeMap<Vec<String>, CapturedRow>, String> {
    let mut mapped = BTreeMap::new();
    for row in rows {
        let key = primary_key_values(metadata, &row)?;
        if mapped.insert(key, row).is_some() {
            return Err("Captured duplicate primary-key row".to_string());
        }
    }
    Ok(mapped)
}

fn primary_key_values(metadata: &TableMetadata, row: &CapturedRow) -> Result<Vec<String>, String> {
    metadata
        .primary_key
        .iter()
        .map(|key| {
            let index = metadata
                .writable_columns()
                .position(|column| column.name.eq_ignore_ascii_case(key))
                .ok_or_else(|| format!("Primary key column {key} is not capturable"))?;
            row.values
                .get(index)
                .cloned()
                .ok_or_else(|| format!("Primary key column {key} is missing from row image"))
        })
        .collect()
}

fn primary_key_condition(metadata: &TableMetadata, row: &CapturedRow) -> Result<String, String> {
    let values = primary_key_values(metadata, row)?;
    Ok(format!(
        "({})",
        metadata
            .primary_key
            .iter()
            .zip(values)
            .map(|(column, value)| {
                format!("CAST({} AS BINARY) <=> {}", quote_identifier(column), value)
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    ))
}

fn full_row_guard(metadata: &TableMetadata, row: &CapturedRow) -> String {
    metadata
        .writable_columns()
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

fn build_insert_rollback_delete(
    metadata: &TableMetadata,
    row: &CapturedRow,
) -> Result<String, String> {
    Ok(format!(
        "DELETE FROM {} WHERE {} AND ({}) LIMIT 1",
        metadata.qualified_name(),
        primary_key_condition(metadata, row)?,
        full_row_guard(metadata, row)
    ))
}

fn restoration_literal(column: &ColumnMetadata, value: &str) -> String {
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

fn build_update_rollback(
    metadata: &TableMetadata,
    before: &CapturedRow,
    after: &CapturedRow,
) -> Result<String, String> {
    let assignments = metadata
        .writable_columns()
        .zip(&before.values)
        .filter(|(column, _)| !metadata.is_primary_key(&column.name))
        .map(|(column, value)| {
            format!(
                "{} = {}",
                quote_identifier(&column.name),
                restoration_literal(column, value)
            )
        })
        .collect::<Vec<_>>();
    if assignments.is_empty() {
        return Err("UPDATE target has no writable non-primary columns".to_string());
    }
    Ok(format!(
        "UPDATE {} SET {} WHERE {} AND ({}) LIMIT 1",
        metadata.qualified_name(),
        assignments.join(", "),
        primary_key_condition(metadata, after)?,
        full_row_guard(metadata, after)
    ))
}

fn build_delete_rollback_insert(metadata: &TableMetadata, row: &CapturedRow) -> String {
    let columns = metadata
        .writable_columns()
        .map(|column| quote_identifier(&column.name))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO {} ({columns}) VALUES ({})",
        metadata.qualified_name(),
        metadata
            .writable_columns()
            .zip(&row.values)
            .map(|(column, value)| restoration_literal(column, value))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

async fn prepare_ddl_inverse(
    conn: &mut sqlx::MySqlConnection,
    plan: &DdlPlan,
) -> Result<String, String> {
    match plan {
        DdlPlan::CreateTable(table) => {
            let table = resolve_object(conn, table).await?;
            ensure_table_absent(conn, &table).await?;
            Ok(format!("DROP TABLE IF EXISTS {}", table.quoted()))
        }
        DdlPlan::CreateDatabase(database) => {
            let sql = format!(
                "SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = {}",
                sql_hex(database.as_bytes())
            );
            if scalar_count(conn, &sql).await? > 0 {
                return Err(format!("Database {database} already exists"));
            }
            Ok(format!(
                "DROP DATABASE IF EXISTS {}",
                quote_identifier(database)
            ))
        }
        DdlPlan::CreateView(view) => {
            let view = resolve_object(conn, view).await?;
            ensure_table_absent(conn, &view).await?;
            Ok(format!("DROP VIEW IF EXISTS {}", view.quoted()))
        }
        DdlPlan::CreateIndex { table, index } | DdlPlan::AlterAddIndex { table, index } => {
            let table = resolve_object(conn, table).await?;
            ensure_table_exists(conn, &table).await?;
            ensure_index_absent(conn, &table, index).await?;
            Ok(format!(
                "ALTER TABLE {} DROP INDEX {}",
                table.quoted(),
                quote_identifier(index)
            ))
        }
        DdlPlan::RenameTable { from, to } | DdlPlan::AlterRenameTable { from, to } => {
            let from = resolve_object(conn, from).await?;
            let to = resolve_object(conn, to).await?;
            ensure_table_exists(conn, &from).await?;
            ensure_table_absent(conn, &to).await?;
            Ok(format!("RENAME TABLE {} TO {}", to.quoted(), from.quoted()))
        }
        DdlPlan::AlterAddColumn { table, column } => {
            let table = resolve_object(conn, table).await?;
            ensure_table_exists(conn, &table).await?;
            ensure_column_absent(conn, &table, column).await?;
            Ok(format!(
                "ALTER TABLE {} DROP COLUMN {}",
                table.quoted(),
                quote_identifier(column)
            ))
        }
        DdlPlan::AlterRenameColumn { table, from, to } => {
            let table = resolve_object(conn, table).await?;
            ensure_table_exists(conn, &table).await?;
            ensure_column_exists(conn, &table, from).await?;
            ensure_column_absent(conn, &table, to).await?;
            Ok(format!(
                "ALTER TABLE {} RENAME COLUMN {} TO {}",
                table.quoted(),
                quote_identifier(to),
                quote_identifier(from)
            ))
        }
    }
}

async fn resolve_object(
    conn: &mut sqlx::MySqlConnection,
    object: &ObjectName,
) -> Result<ObjectName, String> {
    Ok(ObjectName {
        schema: Some(match &object.schema {
            Some(schema) => schema.clone(),
            None => current_database(conn).await?,
        }),
        name: object.name.clone(),
    })
}

async fn ensure_table_exists(
    conn: &mut sqlx::MySqlConnection,
    table: &ObjectName,
) -> Result<(), String> {
    if table_exists(conn, table).await? {
        Ok(())
    } else {
        Err(format!("Object {} does not exist", table.quoted()))
    }
}

async fn ensure_table_absent(
    conn: &mut sqlx::MySqlConnection,
    table: &ObjectName,
) -> Result<(), String> {
    if table_exists(conn, table).await? {
        Err(format!(
            "Object {} already exists; inverse DROP would be ambiguous",
            table.quoted()
        ))
    } else {
        Ok(())
    }
}

async fn table_exists(
    conn: &mut sqlx::MySqlConnection,
    table: &ObjectName,
) -> Result<bool, String> {
    let schema = table
        .schema
        .as_ref()
        .ok_or_else(|| "Resolved object has no schema".to_string())?;
    let sql = format!(
        "SELECT COUNT(*) FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {}",
        sql_hex(schema.as_bytes()),
        sql_hex(table.name.as_bytes())
    );
    Ok(scalar_count(conn, &sql).await? > 0)
}

async fn ensure_index_absent(
    conn: &mut sqlx::MySqlConnection,
    table: &ObjectName,
    index: &str,
) -> Result<(), String> {
    let schema = table
        .schema
        .as_ref()
        .ok_or_else(|| "Resolved object has no schema".to_string())?;
    let sql = format!(
        "SELECT COUNT(*) FROM information_schema.STATISTICS \
         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {} AND INDEX_NAME = {}",
        sql_hex(schema.as_bytes()),
        sql_hex(table.name.as_bytes()),
        sql_hex(index.as_bytes())
    );
    if scalar_count(conn, &sql).await? == 0 {
        Ok(())
    } else {
        Err(format!(
            "Index {} already exists on {}",
            index,
            table.quoted()
        ))
    }
}

async fn ensure_column_exists(
    conn: &mut sqlx::MySqlConnection,
    table: &ObjectName,
    column: &str,
) -> Result<(), String> {
    if column_exists(conn, table, column).await? {
        Ok(())
    } else {
        Err(format!(
            "Column {} does not exist on {}",
            column,
            table.quoted()
        ))
    }
}

async fn ensure_column_absent(
    conn: &mut sqlx::MySqlConnection,
    table: &ObjectName,
    column: &str,
) -> Result<(), String> {
    if column_exists(conn, table, column).await? {
        Err(format!(
            "Column {} already exists on {}",
            column,
            table.quoted()
        ))
    } else {
        Ok(())
    }
}

async fn column_exists(
    conn: &mut sqlx::MySqlConnection,
    table: &ObjectName,
    column: &str,
) -> Result<bool, String> {
    let schema = table
        .schema
        .as_ref()
        .ok_or_else(|| "Resolved object has no schema".to_string())?;
    let sql = format!(
        "SELECT COUNT(*) FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {} AND COLUMN_NAME = {}",
        sql_hex(schema.as_bytes()),
        sql_hex(table.name.as_bytes()),
        sql_hex(column.as_bytes())
    );
    Ok(scalar_count(conn, &sql).await? > 0)
}

fn sql_hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(2 + bytes.len() * 2);
    value.push_str("0x");
    for byte in bytes {
        value.push_str(&format!("{byte:02X}"));
    }
    value
}

/// A read needs no inverse statement, so what can make a SELECT unsafe here is
/// narrow: an external write side effect (INTO OUTFILE/DUMPFILE) or a stored
/// function that writes tables behind the query.
///
/// Reads use [`ensure_no_user_defined_function_calls`] rather than the proven-
/// function allowlist. Both refuse stored functions; the allowlist additionally
/// refuses every built-in it has not heard of, and that is where it went wrong
/// — 193 names and still missing GROUP_CONCAT, ROW_NUMBER, LAG, MD5, UUID.
/// Write statements keep the allowlist: they carry a rollback file, so an
/// unrecognised function there is worth failing closed over.
///
/// `SELECT ... FOR UPDATE` / `FOR SHARE` / `LOCK IN SHARE MODE` stay read-only:
/// they take row locks inside the caller's transaction but change no data.
fn classify_select(tokens: &[Token]) -> Result<ProtectedStatement, BlockedStatement> {
    if contains_word(tokens, "OUTFILE") || contains_word(tokens, "DUMPFILE") {
        return Err(BlockedStatement::unsupported(
            "SELECT INTO OUTFILE/DUMPFILE has an external write side effect",
        ));
    }
    if tokens
        .windows(2)
        .any(|pair| pair[0].text == ":" && pair[1].text == "=")
    {
        return Err(BlockedStatement::unsupported(
            "user-variable assignment with := is forbidden because session mutations are not represented in the rollback file",
        ));
    }
    ensure_no_user_defined_function_calls(tokens)?;
    Ok(ProtectedStatement::ReadOnly)
}

fn classify_with(tokens: &[Token]) -> Result<ProtectedStatement, BlockedStatement> {
    let mut depth = 0_i32;
    for token in tokens.iter().skip(1) {
        match token.text.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ if depth == 0 => match token.upper() {
                "SELECT" => return classify_select(tokens),
                "INSERT" | "UPDATE" | "DELETE" | "REPLACE" => {
                    return Err(BlockedStatement::unsupported(
                        "CTE write statements are not supported by the exact row-diff planner",
                    ));
                }
                _ => {}
            },
            _ => {}
        }
    }
    Err(BlockedStatement::unsupported(
        "could not identify the top-level WITH statement",
    ))
}

fn classify_transaction_control(tokens: &[Token]) -> Result<ProtectedStatement, BlockedStatement> {
    let plan = if matches_word_sequence(tokens, &["START", "TRANSACTION"])
        || matches_word_sequence(tokens, &["BEGIN"])
        || matches_word_sequence(tokens, &["BEGIN", "WORK"])
    {
        Some(TransactionPlan::Start)
    } else if matches_word_sequence(tokens, &["COMMIT"])
        || matches_word_sequence(tokens, &["COMMIT", "WORK"])
    {
        Some(TransactionPlan::Commit)
    } else if matches_word_sequence(tokens, &["ROLLBACK"])
        || matches_word_sequence(tokens, &["ROLLBACK", "WORK"])
    {
        Some(TransactionPlan::Rollback)
    } else {
        None
    };

    plan.map(ProtectedStatement::Transaction)
        .ok_or_else(|| {
            BlockedStatement::unsupported(
                "only exact START TRANSACTION/BEGIN, COMMIT, and ROLLBACK boundaries are supported; modifiers, savepoints, and server controls fail closed",
            )
        })
}

fn matches_word_sequence(tokens: &[Token], expected: &[&str]) -> bool {
    tokens.len() == expected.len()
        && tokens
            .iter()
            .zip(expected)
            .all(|(token, expected)| token.kind == TokenKind::Word && token.upper() == *expected)
}

fn classify_set(sql: &str, tokens: &[Token]) -> Result<ProtectedStatement, BlockedStatement> {
    if let Some((variable, assignment_index)) = direct_user_variable(tokens) {
        if contains_word(tokens, "OUTFILE") || contains_word(tokens, "DUMPFILE") {
            return Err(BlockedStatement::unsupported(
                "SET expressions with SELECT INTO OUTFILE/DUMPFILE have an external write side effect",
            ));
        }
        let allowed_colon_assignment = variable
            .to_ascii_uppercase()
            .starts_with("TABULARIS_")
            .then_some(assignment_index)
            .filter(|index| {
                tokens.get(*index).is_some_and(|token| token.text == ":")
                    && tokens
                        .get(*index + 1)
                        .is_some_and(|token| token.text == "=")
                    && tokens[*index].end == tokens[*index + 1].start
            });
        ensure_only_proven_function_calls_with_assignment(tokens, allowed_colon_assignment)?;
        return Ok(ProtectedStatement::Session(SessionPlan::UserVariable));
    }

    // Any `SET` that only changes session behaviour is safe here: it touches no
    // table data, so there is nothing to invert. `session_vars` already owns
    // that judgement, so reuse it rather than growing a second copy that drifts
    // — this function used to allow `SET @var` alone, which refused
    // `SET NAMES utf8mb4`, `SET SESSION sql_mode = ...`, `SET time_zone = ...`
    // and friends even though the session-variable feature handled them fine.
    //
    // What stays refused: GLOBAL/PERSIST (reaches other connections),
    // PASSWORD/ROLE (identity), TRANSACTION characteristics, and `autocommit`
    // — that last one is the only SET that genuinely breaks rollback, since
    // autocommit=1 commits each statement the moment it runs.
    if crate::session_vars::is_rollback_safe(sql) {
        if contains_word(tokens, "OUTFILE") || contains_word(tokens, "DUMPFILE") {
            return Err(BlockedStatement::unsupported(
                "SET expressions with SELECT INTO OUTFILE/DUMPFILE have an external write side effect",
            ));
        }
        return Ok(ProtectedStatement::Session(SessionPlan::Setting));
    }

    Err(BlockedStatement::unsupported(
        "only SET @user_variable is allowed; session/global settings can invalidate rollback semantics",
    ))
}

fn direct_user_variable(tokens: &[Token]) -> Option<(&str, usize)> {
    if tokens.get(1).is_some_and(|token| token.text == "@") {
        let variable = tokens.get(2)?;
        if !matches!(variable.kind, TokenKind::Word | TokenKind::QuotedIdentifier) {
            return None;
        }
        return Some((variable.text.as_str(), 3));
    }

    let variable = tokens.get(1)?.text.strip_prefix('@')?;
    (!variable.is_empty()).then_some((variable, 2))
}

fn ensure_only_proven_function_calls(tokens: &[Token]) -> Result<(), BlockedStatement> {
    ensure_only_proven_function_calls_with_assignment(tokens, None)
}

/// Refuses only the function calls that are *identifiably* user-defined, which
/// is what reads actually need to worry about: a stored function can write to
/// tables, and those writes land in no rollback file.
///
/// Two markers identify them without consulting any list:
///   * schema qualification — `app.mutate_users(...)`
///   * a quoted identifier  — `` `mutate_users`(...) ``
///
/// Built-in functions can be neither. That matters because the allowlist used
/// by [`ensure_only_proven_function_calls`] holds 193 names and still missed
/// GROUP_CONCAT, ROW_NUMBER, LAG, MD5 and UUID — every miss rejected a read
/// that was never a risk, and adding names never converges.
///
/// A bare `SELECT mutate_users(1)` on a stored function in the *current* schema
/// still slips through: nothing in the statement distinguishes it from a
/// built-in. Writes it performs are outside rollback protection. That gap is
/// the deliberate cost of not maintaining a list; write statements keep the
/// stricter check, since they are the ones with a rollback file to falsify.
fn ensure_no_user_defined_function_calls(tokens: &[Token]) -> Result<(), BlockedStatement> {
    for (idx, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::Word | TokenKind::QuotedIdentifier)
            || tokens.get(idx + 1).map(|next| next.text.as_str()) != Some("(")
        {
            continue;
        }
        let schema_qualified = idx > 0 && tokens[idx - 1].text == ".";
        if token.kind == TokenKind::QuotedIdentifier || schema_qualified {
            return Err(BlockedStatement::unsupported(format!(
                "function call {}(...) is user-defined; stored functions can hide writes that no rollback file records",
                token.text
            )));
        }
    }
    Ok(())
}

fn ensure_only_proven_function_calls_with_assignment(
    tokens: &[Token],
    allowed_colon_assignment: Option<usize>,
) -> Result<(), BlockedStatement> {
    if tokens.windows(2).enumerate().any(|(index, pair)| {
        pair[0].text == ":" && pair[1].text == "=" && Some(index) != allowed_colon_assignment
    }) {
        return Err(BlockedStatement::unsupported(
            "user-variable assignment with := is forbidden because session mutations are not represented in the rollback file",
        ));
    }
    for (idx, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::Word | TokenKind::QuotedIdentifier)
            || tokens.get(idx + 1).map(|next| next.text.as_str()) != Some("(")
        {
            continue;
        }

        let schema_qualified = idx > 0 && tokens[idx - 1].text == ".";
        if token.kind == TokenKind::QuotedIdentifier
            || schema_qualified
            || !is_proven_side_effect_free_function(token.upper())
        {
            return Err(BlockedStatement::unsupported(format!(
                "function call {}(...) is not proven side-effect-free; stored/UDF calls are forbidden because they can hide writes",
                token.text
            )));
        }
    }
    Ok(())
}

fn is_proven_side_effect_free_function(name: &str) -> bool {
    matches!(
        name,
        // Parenthesized SQL constructs.
        "ALL"
            | "AND"
            | "ANY"
            | "AS"
            | "EXISTS"
            | "IN"
            | "NOT"
            | "OR"
            | "OVER"
            | "PARTITION"
            | "ROW"
            | "SOME"
            | "VALUES"
            | "XOR"
            // Numeric and aggregate functions.
            | "ABS"
            | "ACOS"
            | "ASIN"
            | "ATAN"
            | "ATAN2"
            | "AVG"
            | "CEIL"
            | "CEILING"
            | "COS"
            | "COT"
            | "COUNT"
            | "CRC32"
            | "DEGREES"
            | "EXP"
            | "FLOOR"
            | "GREATEST"
            | "LEAST"
            | "LN"
            | "LOG"
            | "LOG10"
            | "LOG2"
            | "MAX"
            | "MIN"
            | "MOD"
            | "PI"
            | "POW"
            | "POWER"
            | "RADIANS"
            | "ROUND"
            | "SIGN"
            | "SIN"
            | "SQRT"
            | "STD"
            | "STDDEV"
            | "STDDEV_POP"
            | "STDDEV_SAMP"
            | "SUM"
            | "TAN"
            | "TRUNCATE"
            | "VARIANCE"
            | "VAR_POP"
            | "VAR_SAMP"
            // String, binary, and encoding functions.
            | "ASCII"
            | "BIN"
            | "BIT_COUNT"
            | "BIT_LENGTH"
            | "CHAR"
            | "CHARACTER_LENGTH"
            | "CHAR_LENGTH"
            | "CONCAT"
            | "CONCAT_WS"
            | "ELT"
            | "EXPORT_SET"
            | "FIELD"
            | "FIND_IN_SET"
            | "FORMAT"
            | "FROM_BASE64"
            | "HEX"
            | "INSTR"
            | "LCASE"
            | "LEFT"
            | "LENGTH"
            | "LOCATE"
            | "LOWER"
            | "LPAD"
            | "LTRIM"
            | "MAKE_SET"
            | "MID"
            | "OCT"
            | "OCTET_LENGTH"
            | "ORD"
            | "POSITION"
            | "QUOTE"
            | "REGEXP_INSTR"
            | "REGEXP_LIKE"
            | "REGEXP_REPLACE"
            | "REGEXP_SUBSTR"
            | "REPEAT"
            | "REPLACE"
            | "REVERSE"
            | "RIGHT"
            | "RPAD"
            | "RTRIM"
            | "SOUNDEX"
            | "SPACE"
            | "STRCMP"
            | "SUBSTR"
            | "SUBSTRING"
            | "SUBSTRING_INDEX"
            | "TO_BASE64"
            | "TRIM"
            | "UCASE"
            | "UNHEX"
            | "UPPER"
            // Date and time functions.
            | "ADDDATE"
            | "ADDTIME"
            | "CONVERT_TZ"
            | "CURDATE"
            | "CURRENT_DATE"
            | "CURRENT_TIME"
            | "CURRENT_TIMESTAMP"
            | "CURTIME"
            | "DATE"
            | "DATEDIFF"
            | "DATE_ADD"
            | "DATE_FORMAT"
            | "DATE_SUB"
            | "DAY"
            | "DAYNAME"
            | "DAYOFMONTH"
            | "DAYOFWEEK"
            | "DAYOFYEAR"
            | "EXTRACT"
            | "FROM_DAYS"
            | "FROM_UNIXTIME"
            | "GET_FORMAT"
            | "HOUR"
            | "LAST_DAY"
            | "MAKEDATE"
            | "MAKETIME"
            | "MICROSECOND"
            | "MINUTE"
            | "MONTH"
            | "MONTHNAME"
            | "NOW"
            | "PERIOD_ADD"
            | "PERIOD_DIFF"
            | "QUARTER"
            | "SEC_TO_TIME"
            | "SECOND"
            | "STR_TO_DATE"
            | "SUBDATE"
            | "SUBTIME"
            | "SYSDATE"
            | "TIME"
            | "TIMEDIFF"
            | "TIMESTAMP"
            | "TIMESTAMPADD"
            | "TIMESTAMPDIFF"
            | "TIME_FORMAT"
            | "TIME_TO_SEC"
            | "TO_DAYS"
            | "TO_SECONDS"
            | "UNIX_TIMESTAMP"
            | "UTC_DATE"
            | "UTC_TIME"
            | "UTC_TIMESTAMP"
            | "WEEK"
            | "WEEKDAY"
            | "WEEKOFYEAR"
            | "YEAR"
            | "YEARWEEK"
            // Conditional, conversion, JSON, and read-only metadata functions.
            | "CAST"
            | "COALESCE"
            | "CONNECTION_ID"
            | "CONVERT"
            | "CURRENT_ROLE"
            | "CURRENT_USER"
            | "DATABASE"
            | "DEFAULT"
            | "IF"
            | "IFNULL"
            | "INET6_ATON"
            | "INET6_NTOA"
            | "INET_ATON"
            | "INET_NTOA"
            | "ISNULL"
            | "JSON_ARRAY"
            | "JSON_ARRAYAGG"
            | "JSON_ARRAY_APPEND"
            | "JSON_ARRAY_INSERT"
            | "JSON_CONTAINS"
            | "JSON_CONTAINS_PATH"
            | "JSON_DEPTH"
            | "JSON_EXTRACT"
            | "JSON_INSERT"
            | "JSON_KEYS"
            | "JSON_LENGTH"
            | "JSON_MERGE_PATCH"
            | "JSON_OBJECT"
            | "JSON_OBJECTAGG"
            | "JSON_PRETTY"
            | "JSON_QUOTE"
            | "JSON_REMOVE"
            | "JSON_REPLACE"
            | "JSON_SEARCH"
            | "JSON_SET"
            | "JSON_TYPE"
            | "JSON_UNQUOTE"
            | "JSON_VALID"
            | "NULLIF"
            | "ROW_COUNT"
            | "SCHEMA"
            | "SESSION_USER"
            | "SYSTEM_USER"
            | "USER"
            | "VERSION"
            // Window functions.
            | "CUME_DIST"
            | "DENSE_RANK"
            | "FIRST_VALUE"
            | "LAG"
            | "LAST_VALUE"
            | "LEAD"
            | "NTH_VALUE"
            | "NTILE"
            | "PERCENT_RANK"
            | "RANK"
            | "ROW_NUMBER"
    )
}

fn parse_insert(sql: &str, tokens: &[Token]) -> Result<InsertPlan, BlockedStatement> {
    expect_word(
        tokens,
        1,
        "INTO",
        "INSERT modifiers such as IGNORE are not supported",
    )?;
    let (table, mut idx) = parse_object_name(tokens, 2)?;
    if tokens.get(idx).map(|t| t.text.as_str()) != Some("(") {
        return Err(BlockedStatement::unsupported(
            "INSERT must use an explicit column list",
        ));
    }
    let columns_end = matching_paren(tokens, idx)?;
    let columns = parse_identifier_list(&tokens[idx + 1..columns_end])?;
    idx = columns_end + 1;
    if !matches!(
        tokens.get(idx).map(Token::upper),
        Some("VALUES") | Some("VALUE")
    ) {
        return Err(BlockedStatement::unsupported(
            "only INSERT ... VALUES is supported; INSERT SELECT/SET is fail-closed",
        ));
    }
    idx += 1;

    let mut rows = Vec::new();
    while idx < tokens.len() {
        if tokens[idx].text != "(" {
            return Err(BlockedStatement::unsupported(
                "INSERT ... VALUES may not contain ON DUPLICATE KEY, RETURNING, or trailing clauses",
            ));
        }
        let row_end = matching_paren(tokens, idx)?;
        ensure_only_proven_function_calls(&tokens[idx + 1..row_end])?;
        let expressions = split_expressions(sql, &tokens[idx + 1..row_end])?;
        if expressions.len() != columns.len() {
            return Err(BlockedStatement::unsupported(
                "INSERT row value count does not match its explicit column list",
            ));
        }
        rows.push(expressions);
        idx = row_end + 1;
        if idx == tokens.len() {
            break;
        }
        if tokens[idx].text != "," {
            return Err(BlockedStatement::unsupported(
                "INSERT ... VALUES trailing clauses are not supported",
            ));
        }
        idx += 1;
    }

    if rows.is_empty() {
        return Err(BlockedStatement::unsupported(
            "INSERT ... VALUES must contain at least one row",
        ));
    }
    Ok(InsertPlan {
        table,
        columns,
        rows,
    })
}

fn parse_update(sql: &str, tokens: &[Token]) -> Result<UpdatePlan, BlockedStatement> {
    let (table, set_idx) = parse_object_name(tokens, 1)?;
    expect_word(
        tokens,
        set_idx,
        "SET",
        "multi-table UPDATE, aliases, and UPDATE modifiers are not supported",
    )?;

    let where_idx = find_top_level_word(tokens, set_idx + 1, &["WHERE"]);
    if find_top_level_word(tokens, set_idx + 1, &["ORDER", "LIMIT", "RETURNING"]).is_some() {
        return Err(BlockedStatement::unsupported(
            "UPDATE ORDER BY/LIMIT/RETURNING is not supported by the exact row-diff planner",
        ));
    }
    let assignments_end = where_idx.unwrap_or(tokens.len());
    let assigned_columns = parse_assignments(&tokens[set_idx + 1..assignments_end])?;
    ensure_only_proven_function_calls(&tokens[set_idx + 1..])?;
    let statement_prefix_end = where_idx
        .map(|idx| tokens[idx].start)
        .unwrap_or_else(|| statement_end(sql, tokens));
    let statement_prefix = sql[..statement_prefix_end].trim().to_string();
    let where_sql = where_idx.map(|idx| {
        sql[tokens[idx].end..statement_end(sql, tokens)]
            .trim()
            .to_string()
    });
    if where_sql.as_deref() == Some("") {
        return Err(BlockedStatement::unsupported(
            "UPDATE WHERE clause is empty",
        ));
    }
    Ok(UpdatePlan {
        table,
        assigned_columns,
        where_sql,
        statement_prefix,
    })
}

fn parse_delete(sql: &str, tokens: &[Token]) -> Result<DeletePlan, BlockedStatement> {
    expect_word(
        tokens,
        1,
        "FROM",
        "multi-table DELETE and DELETE modifiers are not supported",
    )?;
    let (table, idx) = parse_object_name(tokens, 2)?;
    if find_top_level_word(tokens, idx, &["ORDER", "LIMIT", "RETURNING", "USING"]).is_some() {
        return Err(BlockedStatement::unsupported(
            "multi-table DELETE and DELETE ORDER BY/LIMIT/RETURNING are not supported",
        ));
    }
    ensure_only_proven_function_calls(&tokens[idx..])?;
    let where_idx = find_top_level_word(tokens, idx, &["WHERE"]);
    if where_idx.is_none() && idx != tokens.len() {
        return Err(BlockedStatement::unsupported(
            "DELETE aliases and trailing clauses are not supported",
        ));
    }
    if let Some(where_idx) = where_idx {
        if where_idx != idx {
            return Err(BlockedStatement::unsupported(
                "DELETE aliases and trailing clauses are not supported",
            ));
        }
    }
    let statement_prefix_end = where_idx
        .map(|idx| tokens[idx].start)
        .unwrap_or_else(|| statement_end(sql, tokens));
    let statement_prefix = sql[..statement_prefix_end].trim().to_string();
    let where_sql = where_idx.map(|where_idx| {
        sql[tokens[where_idx].end..statement_end(sql, tokens)]
            .trim()
            .to_string()
    });
    if where_sql.as_deref() == Some("") {
        return Err(BlockedStatement::unsupported(
            "DELETE WHERE clause is empty",
        ));
    }
    Ok(DeletePlan {
        table,
        where_sql,
        statement_prefix,
    })
}

/// Finds the top-level `ON DUPLICATE KEY UPDATE` sequence, distinguishing it
/// from any `JOIN … ON` inside a SELECT source.
fn find_on_duplicate_key_update(tokens: &[Token], start: usize) -> Option<usize> {
    let mut depth = 0_i32;
    for idx in start..tokens.len() {
        match tokens[idx].text.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ if depth == 0
                && tokens[idx].kind == TokenKind::Word
                && tokens[idx].upper() == "ON"
                && tokens.len() >= idx + 4
                && tokens[idx + 1].upper() == "DUPLICATE"
                && tokens[idx + 2].upper() == "KEY"
                && tokens[idx + 3].upper() == "UPDATE" =>
            {
                return Some(idx);
            }
            _ => {}
        }
    }
    None
}

/// Parses `VALUES (…), (…) [AS alias [(cols)]]` into raw row expressions.
fn parse_values_rows(
    sql: &str,
    tokens: &[Token],
) -> Result<Vec<Vec<String>>, BlockedStatement> {
    // tokens[0] is VALUES/VALUE.
    let mut idx = 1;
    let mut rows = Vec::new();
    while idx < tokens.len() {
        if tokens[idx].text != "(" {
            break;
        }
        let row_end = matching_paren(tokens, idx)?;
        ensure_only_proven_function_calls(&tokens[idx + 1..row_end])?;
        rows.push(split_expressions(sql, &tokens[idx + 1..row_end])?);
        idx = row_end + 1;
        if tokens.get(idx).map(|t| t.text.as_str()) == Some(",") {
            idx += 1;
            continue;
        }
        break;
    }
    if rows.is_empty() {
        return Err(BlockedStatement::unsupported(
            "INSERT ... VALUES must contain at least one row",
        ));
    }
    // Optional MySQL 8.0.19 row alias: AS name [(col, …)].
    if tokens.get(idx).is_some_and(|t| t.upper() == "AS") {
        idx += 1;
        let (_, next) = parse_single_identifier(tokens, idx)?;
        idx = next;
        if tokens.get(idx).map(|t| t.text.as_str()) == Some("(") {
            idx = matching_paren(tokens, idx)? + 1;
        }
    }
    if idx != tokens.len() {
        return Err(BlockedStatement::unsupported(
            "unexpected trailing tokens after INSERT ... VALUES",
        ));
    }
    Ok(rows)
}

/// Parses the extended INSERT family:
/// `INSERT [IGNORE] INTO t [(cols)] (VALUES … | SELECT …) [ON DUPLICATE KEY
/// UPDATE …]`. Everything else fails closed.
fn parse_insert_family(
    sql: &str,
    tokens: &[Token],
) -> Result<InsertFamilyPlan, BlockedStatement> {
    let mut idx = 1;
    let mut ignore = false;
    if tokens.get(idx).is_some_and(|t| t.upper() == "IGNORE") {
        ignore = true;
        idx += 1;
    }
    expect_word(
        tokens,
        idx,
        "INTO",
        "INSERT modifiers other than IGNORE are not supported",
    )?;
    let (table, mut idx) = parse_object_name(tokens, idx + 1)?;

    // A "(" here is either an explicit column list or a parenthesized SELECT.
    let mut columns = None;
    if tokens.get(idx).map(|t| t.text.as_str()) == Some("(") {
        let inner = tokens.get(idx + 1).map(Token::upper);
        if !matches!(inner, Some("SELECT") | Some("WITH")) {
            let columns_end = matching_paren(tokens, idx)?;
            columns = Some(parse_identifier_list(&tokens[idx + 1..columns_end])?);
            idx = columns_end + 1;
        }
    }

    let source_end = find_on_duplicate_key_update(tokens, idx).unwrap_or(tokens.len());
    let source_tokens = &tokens[idx..source_end];
    let source = match source_tokens.first().map(Token::upper) {
        Some("VALUES") | Some("VALUE") => {
            InsertSource::Values(parse_values_rows(sql, source_tokens)?)
        }
        Some("SELECT") | Some("WITH") | Some("(") => {
            InsertSource::Select(raw_token_range(sql, source_tokens)?)
        }
        _ => {
            return Err(BlockedStatement::unsupported(
                "INSERT source must be VALUES or SELECT",
            ));
        }
    };

    let upsert = if source_end < tokens.len() {
        let assignment_tokens = &tokens[source_end + 4..];
        let assigned_columns = parse_assignments(assignment_tokens)?;
        ensure_only_proven_function_calls(assignment_tokens)?;
        if matches!(source, InsertSource::Select(_))
            && assignment_tokens.iter().any(|t| t.text == ".")
        {
            // A qualified reference into the SELECT would not resolve once
            // the source is re-executed as literal VALUES.
            return Err(BlockedStatement::unsupported(
                "ON DUPLICATE KEY UPDATE with qualified references to the SELECT source is not supported",
            ));
        }
        Some(UpsertTail {
            tail_sql: sql[tokens[source_end].start..statement_end(sql, tokens)]
                .trim()
                .to_string(),
            assigned_columns,
        })
    } else {
        None
    };

    Ok(InsertFamilyPlan {
        table,
        columns,
        source,
        ignore,
        upsert,
    })
}

const JOIN_KEYWORDS: &[&str] = &[
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "OUTER",
    "CROSS",
    "NATURAL",
    "STRAIGHT_JOIN",
];

/// Extracts `(table, alias)` pairs from raw table references (`a JOIN b ON …`,
/// `t1, t2`, `db.t AS x`). Derived tables, index hints, and anything else that
/// would make the alias map unreliable fail closed.
fn parse_table_references(
    tokens: &[Token],
) -> Result<Vec<(ObjectName, String)>, BlockedStatement> {
    let mut references = Vec::new();
    let mut idx = 0;
    loop {
        if tokens.get(idx).map(|t| t.text.as_str()) == Some("(") {
            return Err(BlockedStatement::unsupported(
                "derived tables and subqueries in table references are not supported",
            ));
        }
        let (object, next) = parse_object_name(tokens, idx)?;
        idx = next;
        let mut alias = object.name.clone();
        if tokens.get(idx).is_some_and(|t| t.upper() == "AS") {
            let (name, next) = parse_single_identifier(tokens, idx + 1)?;
            alias = name;
            idx = next;
        } else if tokens.get(idx).is_some_and(|t| {
            is_identifier(t)
                && !JOIN_KEYWORDS.contains(&t.upper())
                && !matches!(t.upper(), "ON" | "USING" | "USE" | "FORCE" | "IGNORE")
        }) {
            alias = tokens[idx].text.clone();
            idx += 1;
        }
        if tokens
            .get(idx)
            .is_some_and(|t| matches!(t.upper(), "USE" | "FORCE" | "IGNORE"))
        {
            return Err(BlockedStatement::unsupported(
                "index hints in table references are not supported",
            ));
        }
        references.push((object, alias));

        // Skip the join condition (or nothing) until the next table
        // reference: a top-level "," or a JOIN-family keyword run.
        let mut depth = 0_i32;
        let mut next_table = None;
        while idx < tokens.len() {
            match tokens[idx].text.as_str() {
                "(" => depth += 1,
                ")" => depth -= 1,
                "," if depth == 0 => {
                    next_table = Some(idx + 1);
                    break;
                }
                _ if depth == 0
                    && tokens[idx].kind == TokenKind::Word
                    && JOIN_KEYWORDS.contains(&tokens[idx].upper()) =>
                {
                    let mut join_end = idx;
                    while tokens
                        .get(join_end)
                        .is_some_and(|t| JOIN_KEYWORDS.contains(&t.upper()))
                    {
                        join_end += 1;
                    }
                    next_table = Some(join_end);
                    break;
                }
                _ => {}
            }
            idx += 1;
        }
        match next_table {
            Some(next) => idx = next,
            None => break,
        }
    }
    if references.is_empty() {
        return Err(BlockedStatement::unsupported(
            "could not identify any table reference",
        ));
    }
    Ok(references)
}

/// Like [`parse_assignments`] but keeps the left-hand qualifier so
/// multi-table targets can be resolved.
fn parse_qualified_assignments(
    tokens: &[Token],
) -> Result<Vec<(Option<String>, String)>, BlockedStatement> {
    if tokens.is_empty() {
        return Err(BlockedStatement::unsupported("UPDATE SET clause is empty"));
    }
    let mut result = Vec::new();
    let mut segment_start = 0;
    let mut depth = 0_i32;
    for idx in 0..=tokens.len() {
        let at_end = idx == tokens.len();
        if !at_end {
            match tokens[idx].text.as_str() {
                "(" => depth += 1,
                ")" => depth -= 1,
                _ => {}
            }
        }
        if at_end || (depth == 0 && tokens[idx].text == ",") {
            let segment = &tokens[segment_start..idx];
            let equals = segment
                .iter()
                .position(|token| token.text == "=")
                .ok_or_else(|| {
                    BlockedStatement::unsupported("UPDATE assignment must contain =")
                })?;
            let lhs = &segment[..equals];
            if segment[equals + 1..].is_empty() {
                return Err(BlockedStatement::unsupported(
                    "UPDATE assignment value is empty",
                ));
            }
            let (qualifier, column) = match lhs {
                [column] if is_identifier(column) => (None, column.text.clone()),
                [qualifier, dot, column]
                    if is_identifier(qualifier) && dot.text == "." && is_identifier(column) =>
                {
                    (Some(qualifier.text.clone()), column.text.clone())
                }
                _ => {
                    return Err(BlockedStatement::unsupported(
                        "UPDATE assignment target must be a simple column",
                    ));
                }
            };
            result.push((qualifier, column));
            segment_start = idx + 1;
        }
    }
    Ok(result)
}

fn resolve_reference<'a>(
    references: &'a [(ObjectName, String)],
    qualifier: &str,
) -> Option<&'a (ObjectName, String)> {
    references
        .iter()
        .find(|(_, alias)| alias.eq_ignore_ascii_case(qualifier))
        .or_else(|| {
            references
                .iter()
                .find(|(object, _)| object.name.eq_ignore_ascii_case(qualifier))
        })
}

/// Multi-table / aliased UPDATE: `UPDATE <refs> SET <assignments> [WHERE …]`.
fn parse_multi_update(sql: &str, tokens: &[Token]) -> Result<MultiTablePlan, BlockedStatement> {
    if tokens
        .get(1)
        .is_some_and(|t| matches!(t.upper(), "LOW_PRIORITY" | "IGNORE"))
    {
        return Err(BlockedStatement::unsupported(
            "UPDATE modifiers are not supported",
        ));
    }
    let set_idx = find_top_level_word(tokens, 1, &["SET"]).ok_or_else(|| {
        BlockedStatement::unsupported("UPDATE must contain a SET clause")
    })?;
    if set_idx <= 1 {
        return Err(BlockedStatement::unsupported(
            "UPDATE is missing its table references",
        ));
    }
    let references = parse_table_references(&tokens[1..set_idx])?;
    if find_top_level_word(tokens, set_idx + 1, &["ORDER", "LIMIT", "RETURNING"]).is_some() {
        return Err(BlockedStatement::unsupported(
            "UPDATE ORDER BY/LIMIT/RETURNING is not supported by the exact row-diff planner",
        ));
    }
    let where_idx = find_top_level_word(tokens, set_idx + 1, &["WHERE"]);
    let assignments_end = where_idx.unwrap_or(tokens.len());
    let assignments = parse_qualified_assignments(&tokens[set_idx + 1..assignments_end])?;
    ensure_only_proven_function_calls(&tokens[set_idx + 1..])?;

    let mut targets: Vec<MultiTableTarget> = Vec::new();
    for (qualifier, column) in assignments {
        let (object, alias) = match &qualifier {
            Some(q) => resolve_reference(&references, q).ok_or_else(|| {
                BlockedStatement::unsupported(
                    "UPDATE assignment qualifier does not match a table reference",
                )
            })?,
            None if references.len() == 1 => &references[0],
            None => {
                return Err(BlockedStatement::unsupported(
                    "unqualified assignments are ambiguous in a multi-table UPDATE",
                ));
            }
        };
        match targets
            .iter_mut()
            .find(|target| target.alias.eq_ignore_ascii_case(alias))
        {
            Some(target) => target.assigned_columns.push(column),
            None => targets.push(MultiTableTarget {
                table: object.clone(),
                alias: alias.clone(),
                assigned_columns: vec![column],
            }),
        }
    }

    let refs_sql = raw_token_range(sql, &tokens[1..set_idx])?;
    let where_sql = where_idx
        .map(|idx| sql[tokens[idx].end..statement_end(sql, tokens)].trim().to_string());
    if where_sql.as_deref() == Some("") {
        return Err(BlockedStatement::unsupported(
            "UPDATE WHERE clause is empty",
        ));
    }
    Ok(MultiTablePlan {
        targets,
        refs_sql,
        where_sql,
    })
}

/// Strips an optional `.*` suffix from a DELETE target alias list entry.
fn parse_delete_target(
    tokens: &[Token],
    idx: usize,
) -> Result<(String, usize), BlockedStatement> {
    let (name, mut next) = parse_single_identifier(tokens, idx)?;
    if tokens.get(next).map(|t| t.text.as_str()) == Some(".")
        && tokens.get(next + 1).map(|t| t.text.as_str()) == Some("*")
    {
        next += 2;
    }
    Ok((name, next))
}

/// Multi-table / aliased DELETE:
/// `DELETE a [, b] FROM <refs> [WHERE]`,
/// `DELETE FROM a [, b] USING <refs> [WHERE]`, or
/// `DELETE FROM t [AS] alias [WHERE]` (single table with alias).
fn parse_multi_delete(sql: &str, tokens: &[Token]) -> Result<MultiTablePlan, BlockedStatement> {
    if find_top_level_word(tokens, 1, &["ORDER", "LIMIT", "RETURNING"]).is_some() {
        return Err(BlockedStatement::unsupported(
            "DELETE ORDER BY/LIMIT/RETURNING is not supported",
        ));
    }
    let (target_aliases, refs_start, refs_end) = if tokens
        .get(1)
        .is_some_and(|t| t.upper() == "FROM")
    {
        let using_idx = find_top_level_word(tokens, 2, &["USING"]);
        match using_idx {
            Some(using_idx) => {
                // DELETE FROM <targets> USING <refs>.
                let mut aliases = Vec::new();
                let mut idx = 2;
                loop {
                    let (name, next) = parse_delete_target(tokens, idx)?;
                    aliases.push(name);
                    idx = next;
                    if tokens.get(idx).map(|t| t.text.as_str()) == Some(",") {
                        idx += 1;
                        continue;
                    }
                    break;
                }
                if idx != using_idx {
                    return Err(BlockedStatement::unsupported(
                        "unexpected tokens in DELETE target list",
                    ));
                }
                let where_idx = find_top_level_word(tokens, using_idx + 1, &["WHERE"]);
                (aliases, using_idx + 1, where_idx.unwrap_or(tokens.len()))
            }
            None => {
                // DELETE FROM t [AS] alias [WHERE]: single aliased table.
                let where_idx = find_top_level_word(tokens, 2, &["WHERE"]);
                let refs_end = where_idx.unwrap_or(tokens.len());
                let references = parse_table_references(&tokens[2..refs_end])?;
                if references.len() != 1 {
                    return Err(BlockedStatement::unsupported(
                        "multi-table DELETE must name its targets before FROM or via USING",
                    ));
                }
                (vec![references[0].1.clone()], 2, refs_end)
            }
        }
    } else {
        // DELETE <targets> FROM <refs>.
        let mut aliases = Vec::new();
        let mut idx = 1;
        loop {
            let (name, next) = parse_delete_target(tokens, idx)?;
            aliases.push(name);
            idx = next;
            if tokens.get(idx).map(|t| t.text.as_str()) == Some(",") {
                idx += 1;
                continue;
            }
            break;
        }
        expect_word(
            tokens,
            idx,
            "FROM",
            "DELETE targets must be followed by FROM",
        )?;
        let where_idx = find_top_level_word(tokens, idx + 1, &["WHERE"]);
        (aliases, idx + 1, where_idx.unwrap_or(tokens.len()))
    };

    if refs_start >= refs_end {
        return Err(BlockedStatement::unsupported(
            "DELETE is missing its table references",
        ));
    }
    let references = parse_table_references(&tokens[refs_start..refs_end])?;
    let mut targets = Vec::new();
    for alias in &target_aliases {
        let (object, resolved_alias) = resolve_reference(&references, alias).ok_or_else(|| {
            BlockedStatement::unsupported(
                "DELETE target does not match a table reference",
            )
        })?;
        if targets
            .iter()
            .any(|target: &MultiTableTarget| target.alias.eq_ignore_ascii_case(resolved_alias))
        {
            continue;
        }
        targets.push(MultiTableTarget {
            table: object.clone(),
            alias: resolved_alias.clone(),
            assigned_columns: Vec::new(),
        });
    }
    let where_idx = find_top_level_word(tokens, refs_end, &["WHERE"]);
    let where_sql = where_idx
        .map(|idx| sql[tokens[idx].end..statement_end(sql, tokens)].trim().to_string());
    if where_sql.as_deref() == Some("") {
        return Err(BlockedStatement::unsupported(
            "DELETE WHERE clause is empty",
        ));
    }
    if let Some(where_idx) = where_idx {
        ensure_only_proven_function_calls(&tokens[where_idx..])?;
    }
    Ok(MultiTablePlan {
        targets,
        refs_sql: raw_token_range(sql, &tokens[refs_start..refs_end])?,
        where_sql,
    })
}

fn plan_temporary_table_write(
    sql: &str,
    temporary_tables: &[ObjectName],
) -> Result<Option<ProtectedStatement>, BlockedStatement> {
    if temporary_tables.is_empty() {
        return Ok(None);
    }
    let tokens = tokenize(sql)?;
    let tokens = trim_trailing_semicolon(&tokens);
    if has_top_level_symbol(tokens, 0, ";") {
        return Err(BlockedStatement::unsupported(
            "multiple SQL statements must be split before rollback planning",
        ));
    }
    let Some(first) = tokens.first().map(Token::upper) else {
        return Ok(None);
    };

    let (table, after_table) = match first {
        "INSERT" | "REPLACE" if tokens.get(1).is_some_and(|token| token.upper() == "INTO") => {
            parse_object_name(tokens, 2)?
        }
        "UPDATE" => parse_object_name(tokens, 1)?,
        "DELETE" if tokens.get(1).is_some_and(|token| token.upper() == "FROM") => {
            parse_object_name(tokens, 2)?
        }
        "TRUNCATE" => {
            let table_index =
                usize::from(tokens.get(1).is_some_and(|token| token.upper() == "TABLE")) + 1;
            parse_object_name(tokens, table_index)?
        }
        _ => return Ok(None),
    };
    if !temporary_tables.contains(&table) {
        return Ok(None);
    }

    match first {
        "UPDATE"
            if !tokens
                .get(after_table)
                .is_some_and(|token| token.upper() == "SET") =>
        {
            return Err(BlockedStatement::unsupported(
                "multi-table UPDATE is not ignored even when its first target is temporary",
            ));
        }
        "DELETE" => {
            let first_clause = find_top_level_word(
                tokens,
                after_table,
                &["WHERE", "ORDER", "LIMIT", "RETURNING"],
            );
            if first_clause.is_some_and(|index| index != after_table)
                || (first_clause.is_none() && after_table != tokens.len())
                || contains_word(&tokens[after_table..], "USING")
            {
                return Err(BlockedStatement::unsupported(
                    "multi-table DELETE is not ignored even when its first target is temporary",
                ));
            }
        }
        "TRUNCATE" if after_table != tokens.len() => {
            return Err(BlockedStatement::unsupported(
                "TRUNCATE of a temporary table contains unsupported trailing syntax",
            ));
        }
        _ => {}
    }

    if contains_word(tokens, "OUTFILE") || contains_word(tokens, "DUMPFILE") {
        return Err(BlockedStatement::unsupported(
            "temporary-table statements with OUTFILE/DUMPFILE still have an external write side effect",
        ));
    }
    ensure_only_proven_function_calls(&tokens[after_table..])?;
    Ok(Some(ProtectedStatement::Temporary(
        TemporaryPlan::Statement,
    )))
}

fn parse_create_temporary(tokens: &[Token]) -> Result<TemporaryPlan, BlockedStatement> {
    expect_word(
        tokens,
        2,
        "TABLE",
        "CREATE TEMPORARY only supports session-local tables",
    )?;
    let mut table_index = 3;
    if tokens
        .get(table_index)
        .is_some_and(|token| token.upper() == "IF")
    {
        expect_word(
            tokens,
            table_index + 1,
            "NOT",
            "CREATE TEMPORARY TABLE IF must be followed by NOT EXISTS",
        )?;
        expect_word(
            tokens,
            table_index + 2,
            "EXISTS",
            "CREATE TEMPORARY TABLE IF must be followed by NOT EXISTS",
        )?;
        table_index += 3;
    }
    let (table, definition_index) = parse_object_name(tokens, table_index)?;
    if definition_index >= tokens.len() {
        return Err(BlockedStatement::unsupported(
            "CREATE TEMPORARY TABLE definition is missing",
        ));
    }

    if let Some(query_index) = find_top_level_word(tokens, definition_index, &["SELECT", "WITH"]) {
        match tokens[query_index].upper() {
            "SELECT" => {
                classify_select(&tokens[query_index..])?;
            }
            "WITH" => {
                classify_with(&tokens[query_index..])?;
            }
            _ => unreachable!("query keyword allowlist is exhaustive"),
        }
    }
    Ok(TemporaryPlan::Create(table))
}

fn parse_drop_temporary(tokens: &[Token]) -> Result<TemporaryPlan, BlockedStatement> {
    expect_word(
        tokens,
        2,
        "TABLE",
        "DROP TEMPORARY only supports session-local tables",
    )?;
    let mut index = 3;
    if tokens.get(index).is_some_and(|token| token.upper() == "IF") {
        expect_word(
            tokens,
            index + 1,
            "EXISTS",
            "DROP TEMPORARY TABLE IF must be followed by EXISTS",
        )?;
        index += 2;
    }

    let mut tables = Vec::new();
    loop {
        let (table, next) = parse_object_name(tokens, index)?;
        tables.push(table);
        index = next;
        if index == tokens.len() {
            break;
        }
        if tokens[index].text == "," {
            index += 1;
            continue;
        }
        if matches!(tokens[index].upper(), "RESTRICT" | "CASCADE") && index + 1 == tokens.len() {
            break;
        }
        return Err(BlockedStatement::unsupported(
            "DROP TEMPORARY TABLE contains unsupported trailing syntax",
        ));
    }
    Ok(TemporaryPlan::Drop(tables))
}

fn parse_create(tokens: &[Token]) -> Result<DdlPlan, BlockedStatement> {
    match tokens.get(1).map(Token::upper) {
        Some("TABLE") => {
            if tokens
                .get(2)
                .is_some_and(|token| token.kind == TokenKind::Word && token.upper() == "IF")
            {
                return Err(BlockedStatement::unsupported(
                    "CREATE TABLE IF NOT EXISTS is ambiguous because the inverse must not drop a pre-existing table",
                ));
            }
            let (table, idx) = parse_object_name(tokens, 2)?;
            if idx >= tokens.len() {
                return Err(BlockedStatement::unsupported(
                    "CREATE TABLE definition is missing",
                ));
            }
            if find_top_level_word(tokens, idx, &["AS", "SELECT"]).is_some() {
                return Err(BlockedStatement::unsupported(
                    "CREATE TABLE ... SELECT is fail-closed because its expressions can hide writes outside the new table",
                ));
            }
            Ok(DdlPlan::CreateTable(table))
        }
        Some("DATABASE") | Some("SCHEMA") => {
            if tokens
                .get(2)
                .is_some_and(|token| token.kind == TokenKind::Word && token.upper() == "IF")
            {
                return Err(BlockedStatement::unsupported(
                    "CREATE DATABASE IF NOT EXISTS is ambiguous because the inverse must not drop a pre-existing database",
                ));
            }
            let (database, idx) = parse_single_identifier(tokens, 2)?;
            if idx != tokens.len() {
                return Err(BlockedStatement::unsupported(
                    "CREATE DATABASE IF NOT EXISTS and trailing clauses are not supported",
                ));
            }
            Ok(DdlPlan::CreateDatabase(database))
        }
        Some("VIEW") => {
            let (view, idx) = parse_object_name(tokens, 2)?;
            expect_word(tokens, idx, "AS", "CREATE VIEW must contain AS")?;
            Ok(DdlPlan::CreateView(view))
        }
        Some("UNIQUE") | Some("FULLTEXT") | Some("SPATIAL") | Some("INDEX") => {
            parse_create_index(tokens)
        }
        Some("OR") => Err(BlockedStatement::unsupported(
            "CREATE OR REPLACE is ambiguous because the previous object would need a metadata snapshot",
        )),
        Some("TEMPORARY") => Err(BlockedStatement::unsupported(
            "temporary objects cannot be restored by a rollback file in a later session",
        )),
        _ => Err(BlockedStatement::unsupported(
            "this CREATE object family is not supported; routines, triggers, and events can hide writes",
        )),
    }
}

fn parse_create_index(tokens: &[Token]) -> Result<DdlPlan, BlockedStatement> {
    let mut idx = 1;
    if matches!(
        tokens.get(idx).map(Token::upper),
        Some("UNIQUE") | Some("FULLTEXT") | Some("SPATIAL")
    ) {
        idx += 1;
    }
    expect_word(tokens, idx, "INDEX", "CREATE INDEX syntax is not supported")?;
    let (index, after_index) = parse_single_identifier(tokens, idx + 1)?;
    let on_idx = find_top_level_word(tokens, after_index, &["ON"]).ok_or_else(|| {
        BlockedStatement::unsupported("CREATE INDEX must identify its target table with ON")
    })?;
    if on_idx != after_index {
        return Err(BlockedStatement::unsupported(
            "CREATE INDEX USING before ON is not supported",
        ));
    }
    let (table, after_table) = parse_object_name(tokens, on_idx + 1)?;
    if after_table >= tokens.len() {
        return Err(BlockedStatement::unsupported(
            "CREATE INDEX definition is missing",
        ));
    }
    Ok(DdlPlan::CreateIndex { table, index })
}

fn parse_drop(tokens: &[Token]) -> Result<DdlPlan, BlockedStatement> {
    match tokens.get(1).map(Token::upper) {
        Some("TABLE") | Some("TEMPORARY") => Err(BlockedStatement::destructive(
            "DROP TABLE is forbidden by connection rollback protection",
        )),
        Some("DATABASE") | Some("SCHEMA") => Err(BlockedStatement::destructive(
            "DROP DATABASE is forbidden by connection rollback protection",
        )),
        Some("VIEW") => Err(BlockedStatement::unsupported(
            "DROP VIEW is fail-closed because MySQL cannot PREPARE its CREATE VIEW inverse for environment-gated execution",
        )),
        Some("INDEX") => Err(BlockedStatement::unsupported(
            "DROP INDEX requires reconstructing vendor-specific index metadata and is fail-closed",
        )),
        _ => Err(BlockedStatement::unsupported(
            "this DROP object family is not supported",
        )),
    }
}

fn parse_rename(tokens: &[Token]) -> Result<DdlPlan, BlockedStatement> {
    expect_word(tokens, 1, "TABLE", "only RENAME TABLE is supported")?;
    let (from, idx) = parse_object_name(tokens, 2)?;
    expect_word(tokens, idx, "TO", "RENAME TABLE must contain TO")?;
    let (to, end) = parse_object_name(tokens, idx + 1)?;
    if end != tokens.len() {
        return Err(BlockedStatement::unsupported(
            "multi-table RENAME is not supported by the rollback planner",
        ));
    }
    Ok(DdlPlan::RenameTable { from, to })
}

fn parse_alter(tokens: &[Token]) -> Result<DdlPlan, BlockedStatement> {
    expect_word(
        tokens,
        1,
        "TABLE",
        "only a reversible ALTER TABLE subset is supported",
    )?;
    let (table, idx) = parse_object_name(tokens, 2)?;
    if has_unsupported_alter_clause_separator(tokens, idx) {
        return Err(BlockedStatement::unsupported(
            "multi-clause ALTER TABLE is fail-closed; split it into individual statements",
        ));
    }
    match tokens.get(idx).map(Token::upper) {
        Some("ADD") => parse_alter_add(tokens, table, idx + 1),
        Some("RENAME") => parse_alter_rename(tokens, table, idx + 1),
        Some("DROP") | Some("MODIFY") | Some("CHANGE") | Some("CONVERT") => {
            Err(BlockedStatement::unsupported(
                "lossy ALTER TABLE operations are forbidden without a full affected-data reconstruction plan",
            ))
        }
        _ => Err(BlockedStatement::unsupported(
            "this ALTER TABLE operation is not in the reversible allowlist",
        )),
    }
}

fn has_unsupported_alter_clause_separator(tokens: &[Token], start: usize) -> bool {
    let mut depth = 0_i32;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.text.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            "," if depth == 0 => {
                let option_index = index + 1;
                let allowed = match tokens.get(option_index).map(Token::upper) {
                    Some("ALGORITHM") | Some("LOCK") => true,
                    Some("WITH") | Some("WITHOUT") => tokens
                        .get(option_index + 1)
                        .is_some_and(|next| next.upper() == "VALIDATION"),
                    _ => false,
                };
                if !allowed {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn parse_alter_add(
    tokens: &[Token],
    table: ObjectName,
    mut idx: usize,
) -> Result<DdlPlan, BlockedStatement> {
    let explicit_column = tokens
        .get(idx)
        .is_some_and(|token| token.upper() == "COLUMN");
    if explicit_column {
        idx += 1;
    }
    if tokens
        .get(idx)
        .is_some_and(|token| token.kind == TokenKind::Word && token.upper() == "IF")
    {
        return Err(BlockedStatement::unsupported(
            "ALTER TABLE ADD IF NOT EXISTS is ambiguous for rollback",
        ));
    }
    if matches!(
        tokens.get(idx).map(Token::upper),
        Some("UNIQUE") | Some("FULLTEXT") | Some("SPATIAL")
    ) {
        idx += 1;
    }
    if matches!(
        tokens.get(idx).map(Token::upper),
        Some("INDEX") | Some("KEY")
    ) {
        let (index, after_index) = parse_single_identifier(tokens, idx + 1)?;
        if after_index >= tokens.len() {
            return Err(BlockedStatement::unsupported(
                "ALTER TABLE ADD INDEX definition is missing",
            ));
        }
        return Ok(DdlPlan::AlterAddIndex { table, index });
    }
    if matches!(
        tokens.get(idx).map(Token::upper),
        Some("PRIMARY") | Some("FOREIGN") | Some("CONSTRAINT") | Some("CHECK")
    ) {
        return Err(BlockedStatement::unsupported(
            "ALTER TABLE ADD constraints are not in the reversible allowlist",
        ));
    }
    if !explicit_column {
        return Err(BlockedStatement::unsupported(
            "reversible column additions must use explicit ADD COLUMN syntax; other ADD operations are fail-closed",
        ));
    }
    let (column, after_column) = parse_single_identifier(tokens, idx)?;
    if after_column >= tokens.len() {
        return Err(BlockedStatement::unsupported(
            "ALTER TABLE ADD COLUMN definition is missing",
        ));
    }
    Ok(DdlPlan::AlterAddColumn { table, column })
}

fn parse_alter_rename(
    tokens: &[Token],
    table: ObjectName,
    idx: usize,
) -> Result<DdlPlan, BlockedStatement> {
    if tokens
        .get(idx)
        .is_some_and(|token| token.upper() == "COLUMN")
    {
        let (from, to_keyword) = parse_single_identifier(tokens, idx + 1)?;
        expect_word(
            tokens,
            to_keyword,
            "TO",
            "ALTER TABLE RENAME COLUMN must contain TO",
        )?;
        let (to, end) = parse_single_identifier(tokens, to_keyword + 1)?;
        if end != tokens.len() {
            return Err(BlockedStatement::unsupported(
                "ALTER TABLE RENAME COLUMN contains unsupported trailing syntax",
            ));
        }
        return Ok(DdlPlan::AlterRenameColumn { table, from, to });
    }
    let mut target_idx = idx;
    if matches!(
        tokens.get(target_idx).map(Token::upper),
        Some("TO") | Some("AS")
    ) {
        target_idx += 1;
    }
    let (to, end) = parse_object_name(tokens, target_idx)?;
    if end != tokens.len() {
        return Err(BlockedStatement::unsupported(
            "ALTER TABLE RENAME contains unsupported trailing syntax",
        ));
    }
    Ok(DdlPlan::AlterRenameTable { from: table, to })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Word,
    Number,
    QuotedIdentifier,
    String,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    text: String,
    upper: String,
    start: usize,
    end: usize,
    kind: TokenKind,
}

impl Token {
    fn upper(&self) -> &str {
        &self.upper
    }
}

fn tokenize(sql: &str) -> Result<Vec<Token>, BlockedStatement> {
    let bytes = sql.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] == b'#' {
            i = skip_line(bytes, i + 1);
            continue;
        }
        if bytes[i] == b'-'
            && bytes.get(i + 1) == Some(&b'-')
            && bytes
                .get(i + 2)
                .is_none_or(|following| following.is_ascii_whitespace())
        {
            i = skip_line(bytes, i + 2);
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let mysql_executable = bytes.get(i + 2) == Some(&b'!');
            let mariadb_executable = bytes
                .get(i + 2)
                .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'M'))
                && bytes.get(i + 3) == Some(&b'!');
            if mysql_executable || mariadb_executable {
                return Err(BlockedStatement::unsupported(
                    "MySQL/MariaDB executable comments are forbidden because they can hide writes",
                ));
            }
            let Some(relative_end) = sql[i + 2..].find("*/") else {
                return Err(BlockedStatement::unsupported("unterminated block comment"));
            };
            i += relative_end + 4;
            continue;
        }

        let start = i;
        match bytes[i] {
            b'"' => {
                return Err(BlockedStatement::unsupported(
                    "double-quoted SQL is fail-closed because ANSI_QUOTES changes whether it is a string or identifier",
                ));
            }
            b'\'' => {
                let quote = bytes[i];
                i += 1;
                let mut closed = false;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        return Err(BlockedStatement::unsupported(
                            "backslash-escaped SQL strings are fail-closed because NO_BACKSLASH_ESCAPES changes their token boundaries",
                        ));
                    }
                    if bytes[i] == quote {
                        if bytes.get(i + 1) == Some(&quote) {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        closed = true;
                        break;
                    }
                    i += 1;
                }
                if !closed {
                    return Err(BlockedStatement::unsupported("unterminated SQL string"));
                }
                let text = sql[start..i].to_string();
                tokens.push(Token {
                    upper: text.clone(),
                    text,
                    start,
                    end: i,
                    kind: TokenKind::String,
                });
            }
            b'`' => {
                i += 1;
                let mut value = String::new();
                let mut closed = false;
                while i < bytes.len() {
                    if bytes[i] == b'`' {
                        if bytes.get(i + 1) == Some(&b'`') {
                            value.push('`');
                            i += 2;
                            continue;
                        }
                        i += 1;
                        closed = true;
                        break;
                    }
                    let ch = sql[i..].chars().next().expect("valid UTF-8 boundary");
                    value.push(ch);
                    i += ch.len_utf8();
                }
                if !closed {
                    return Err(BlockedStatement::unsupported(
                        "unterminated quoted identifier",
                    ));
                }
                tokens.push(Token {
                    upper: value.clone(),
                    text: value,
                    start,
                    end: i,
                    kind: TokenKind::QuotedIdentifier,
                });
            }
            c if is_word_start(c) => {
                i += 1;
                while i < bytes.len() && is_word_continue(bytes[i]) {
                    i += 1;
                }
                let text = sql[start..i].to_string();
                tokens.push(Token {
                    upper: text.to_ascii_uppercase(),
                    text,
                    start,
                    end: i,
                    kind: TokenKind::Word,
                });
            }
            c if c.is_ascii_digit() => {
                i += 1;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric()
                        || matches!(bytes[i], b'.' | b'_' | b'+' | b'-'))
                {
                    i += 1;
                }
                let text = sql[start..i].to_string();
                tokens.push(Token {
                    upper: text.clone(),
                    text,
                    start,
                    end: i,
                    kind: TokenKind::Number,
                });
            }
            _ => {
                let ch = sql[i..].chars().next().expect("valid UTF-8 boundary");
                i += ch.len_utf8();
                let text = ch.to_string();
                tokens.push(Token {
                    upper: text.clone(),
                    text,
                    start,
                    end: i,
                    kind: TokenKind::Symbol,
                });
            }
        }
    }
    Ok(tokens)
}

fn skip_line(bytes: &[u8], mut idx: usize) -> usize {
    while idx < bytes.len() && !matches!(bytes[idx], b'\r' | b'\n') {
        idx += 1;
    }
    idx
}

fn is_word_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

fn is_word_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn trim_trailing_semicolon(tokens: &[Token]) -> &[Token] {
    if tokens.last().is_some_and(|token| token.text == ";") {
        &tokens[..tokens.len() - 1]
    } else {
        tokens
    }
}

fn parse_object_name(
    tokens: &[Token],
    idx: usize,
) -> Result<(ObjectName, usize), BlockedStatement> {
    let (first, mut next) = parse_single_identifier(tokens, idx)?;
    if tokens.get(next).is_some_and(|token| token.text == ".") {
        let (second, end) = parse_single_identifier(tokens, next + 1)?;
        next = end;
        Ok((
            ObjectName {
                schema: Some(first),
                name: second,
            },
            next,
        ))
    } else {
        Ok((
            ObjectName {
                schema: None,
                name: first,
            },
            next,
        ))
    }
}

fn parse_single_identifier(
    tokens: &[Token],
    idx: usize,
) -> Result<(String, usize), BlockedStatement> {
    let token = tokens
        .get(idx)
        .ok_or_else(|| BlockedStatement::unsupported("expected an identifier"))?;
    match token.kind {
        TokenKind::Word | TokenKind::QuotedIdentifier => Ok((token.text.clone(), idx + 1)),
        _ => Err(BlockedStatement::unsupported("expected an identifier")),
    }
}

fn parse_identifier_list(tokens: &[Token]) -> Result<Vec<String>, BlockedStatement> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut idx = 0;
    loop {
        let (identifier, next) = parse_single_identifier(tokens, idx)?;
        result.push(identifier);
        idx = next;
        if idx == tokens.len() {
            break;
        }
        if tokens[idx].text != "," {
            return Err(BlockedStatement::unsupported(
                "INSERT column list must contain only identifiers",
            ));
        }
        idx += 1;
    }
    Ok(result)
}

fn matching_paren(tokens: &[Token], open: usize) -> Result<usize, BlockedStatement> {
    if tokens.get(open).map(|token| token.text.as_str()) != Some("(") {
        return Err(BlockedStatement::unsupported(
            "expected opening parenthesis",
        ));
    }
    let mut depth = 0_i32;
    for (idx, token) in tokens.iter().enumerate().skip(open) {
        match token.text.as_str() {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return Ok(idx);
                }
            }
            _ => {}
        }
    }
    Err(BlockedStatement::unsupported("unbalanced parentheses"))
}

fn split_expressions(sql: &str, tokens: &[Token]) -> Result<Vec<String>, BlockedStatement> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    let mut expressions = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    for (idx, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            "," if depth == 0 => {
                expressions.push(raw_token_range(sql, &tokens[start..idx])?);
                start = idx + 1;
            }
            _ => {}
        }
    }
    expressions.push(raw_token_range(sql, &tokens[start..])?);
    Ok(expressions)
}

fn raw_token_range(sql: &str, tokens: &[Token]) -> Result<String, BlockedStatement> {
    let first = tokens
        .first()
        .ok_or_else(|| BlockedStatement::unsupported("empty SQL expression"))?;
    let last = tokens.last().expect("checked non-empty");
    let value = sql[first.start..last.end].trim();
    if value.is_empty() {
        Err(BlockedStatement::unsupported("empty SQL expression"))
    } else {
        Ok(value.to_string())
    }
}

fn parse_assignments(tokens: &[Token]) -> Result<Vec<String>, BlockedStatement> {
    if tokens.is_empty() {
        return Err(BlockedStatement::unsupported("UPDATE SET clause is empty"));
    }
    let mut result = Vec::new();
    let mut segment_start = 0;
    let mut depth = 0_i32;
    for idx in 0..=tokens.len() {
        let at_end = idx == tokens.len();
        if !at_end {
            match tokens[idx].text.as_str() {
                "(" => depth += 1,
                ")" => depth -= 1,
                _ => {}
            }
        }
        if at_end || (depth == 0 && tokens[idx].text == ",") {
            let segment = &tokens[segment_start..idx];
            let equals = segment
                .iter()
                .position(|token| token.text == "=")
                .ok_or_else(|| BlockedStatement::unsupported("UPDATE assignment must contain ="))?;
            let lhs = &segment[..equals];
            let rhs = &segment[equals + 1..];
            if rhs.is_empty() {
                return Err(BlockedStatement::unsupported(
                    "UPDATE assignment value is empty",
                ));
            }
            let column = match lhs {
                [column] if is_identifier(column) => column.text.clone(),
                [qualifier, dot, column]
                    if is_identifier(qualifier) && dot.text == "." && is_identifier(column) =>
                {
                    column.text.clone()
                }
                _ => {
                    return Err(BlockedStatement::unsupported(
                        "UPDATE assignment target must be a simple column",
                    ));
                }
            };
            result.push(column);
            segment_start = idx + 1;
        }
    }
    Ok(result)
}

fn is_identifier(token: &Token) -> bool {
    matches!(token.kind, TokenKind::Word | TokenKind::QuotedIdentifier)
}

fn find_top_level_word(tokens: &[Token], start: usize, words: &[&str]) -> Option<usize> {
    let mut depth = 0_i32;
    for (idx, token) in tokens.iter().enumerate().skip(start) {
        match token.text.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ if depth == 0 && token.kind == TokenKind::Word && words.contains(&token.upper()) => {
                return Some(idx);
            }
            _ => {}
        }
    }
    None
}

fn has_top_level_symbol(tokens: &[Token], start: usize, symbol: &str) -> bool {
    let mut depth = 0_i32;
    for token in tokens.iter().skip(start) {
        match token.text.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ if depth == 0 && token.text == symbol => return true,
            _ => {}
        }
    }
    false
}

fn contains_word(tokens: &[Token], word: &str) -> bool {
    tokens
        .iter()
        .any(|token| token.kind == TokenKind::Word && token.upper() == word)
}

fn expect_word(
    tokens: &[Token],
    idx: usize,
    expected: &str,
    reason: &str,
) -> Result<(), BlockedStatement> {
    if tokens
        .get(idx)
        .is_some_and(|token| token.kind == TokenKind::Word && token.upper() == expected)
    {
        Ok(())
    } else {
        Err(BlockedStatement::unsupported(reason))
    }
}

fn statement_end(sql: &str, tokens: &[Token]) -> usize {
    tokens.last().map_or(sql.len(), |token| token.end)
}

fn quote_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', "``"))
}
use crate::models::{
    BatchStatementResult, ConnectionParams, QueryResult, RollbackUnsupportedPolicy,
};
use crate::recovery_history::{
    RecoveryColumn, RecoveryJournal, RecoveryObject, RecoveryRow, RecoveryStatement,
};
use crate::rollback_sql::{RollbackEnvironment, RollbackJournal, RollbackStep, ServerIdentity};
use sqlx::{Column, Connection, Executor, Row};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Whether MySQL commits the open transaction as a side effect of this
/// statement. Covers the DDL and administrative families from the server's
/// implicit-commit list; anything unrecognised answers `false`, which only
/// keeps the existing checkpoint behaviour.
fn causes_implicit_commit(sql: &str) -> bool {
    let body = crate::drivers::common::strip_leading_sql_comments(sql);
    let keyword: String = body
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .flat_map(|c| c.to_uppercase())
        .collect();
    matches!(
        keyword.as_str(),
        "CREATE"
            | "ALTER"
            | "DROP"
            | "RENAME"
            | "TRUNCATE"
            | "GRANT"
            | "REVOKE"
            | "FLUSH"
            | "LOCK"
            | "UNLOCK"
            | "ANALYZE"
            | "OPTIMIZE"
            | "REPAIR"
            | "CACHE"
            | "INSTALL"
            | "UNINSTALL"
            | "LOAD"
    )
}
