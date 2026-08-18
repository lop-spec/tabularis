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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DmlPlan {
    Insert(InsertPlan),
    Update(UpdatePlan),
    Delete(DeletePlan),
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
    ScopeChange,
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
        "SET" => classify_set(tokens),
        "USE" => Ok(ProtectedStatement::Session(SessionPlan::ScopeChange)),
        "START" | "BEGIN" | "COMMIT" | "ROLLBACK" => classify_transaction_control(tokens),
        "INSERT" => {
            parse_insert(sql, tokens).map(|plan| ProtectedStatement::Dml(DmlPlan::Insert(plan)))
        }
        "UPDATE" => {
            parse_update(sql, tokens).map(|plan| ProtectedStatement::Dml(DmlPlan::Update(plan)))
        }
        "DELETE" => {
            parse_delete(sql, tokens).map(|plan| ProtectedStatement::Dml(DmlPlan::Delete(plan)))
        }
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
            recovery_journal.discard()?;
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
    let (plans, review) = plan_batch_collecting_risks(queries);
    match params.rollback_unsupported_policy {
        None if !review.statements.is_empty() => {
            return Err(rollback_risk_review_error(&review));
        }
        _ => {}
    }

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
    let lifecycle = validate_pinned_transaction_structure(
        &plans,
        starts_active,
        params.allow_implicit_transaction_commit,
        queries,
    )?;

    let has_writes = plans.iter().any(|plan| {
        matches!(
            plan,
            ProtectedStatement::Dml(_) | ProtectedStatement::Ddl(_)
        )
    });
    if has_writes
        && plans
            .iter()
            .any(|plan| matches!(plan, ProtectedStatement::Session(SessionPlan::ScopeChange)))
    {
        return Err(
            "Rollback protection blocks USE when the same Run All batch contains writes; select the database in the connection scope instead"
                .to_string(),
        );
    }

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
                    super::exec_on_mysql_conn(conn, query, limit, page, text).await
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
        let recovery_file = session.journal.as_ref().map(|journal| {
            journal
                .current_recovery_path()
                .to_string_lossy()
                .to_string()
        });
        let conn = session.conn.take();
        // The rollback revision survives on disk by construction. The recovery
        // history must not be dropped mid-"recording": mark it interrupted so
        // RecoveryPage can still show and compare it — the unknown-COMMIT case
        // is exactly when the operator needs it.
        session.journal.take();
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
    let (plans, review) = plan_batch_collecting_risks(queries);
    match params.rollback_unsupported_policy {
        None if !review.statements.is_empty() => {
            return Err(rollback_risk_review_error(&review));
        }
        Some(RollbackUnsupportedPolicy::ExecuteUnprotected) => {
            return Err(
                "Internal rollback protection error: unprotected execution must use the normal batch path"
                    .to_string(),
            );
        }
        _ => {}
    }
    validate_transaction_structure(&plans)?;

    let has_writes = plans.iter().any(|plan| {
        matches!(
            plan,
            ProtectedStatement::Dml(_) | ProtectedStatement::Ddl(_)
        )
    });
    if has_writes
        && plans
            .iter()
            .any(|plan| matches!(plan, ProtectedStatement::Session(SessionPlan::ScopeChange)))
    {
        return Err(
            "Rollback protection blocks USE when the same Run All batch contains writes; select the database in the connection scope instead"
                .to_string(),
        );
    }

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
                unreachable!("unsupported statements are handled before execution")
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
        result.rollback_file = rollback_path.clone();
        if let Some(callback) = on_progress {
            callback(index, &result)?;
        }
        results.push(result);
    }

    if let Some(journal) = journal {
        let recovery_path = journal.current_recovery_path().to_path_buf();
        let final_path = journal.finalize().map_err(|error| {
            format!(
                "{error}. The synced recovery SQL remains at {}",
                recovery_path.display()
            )
        })?;
        let final_path = final_path.to_string_lossy().to_string();
        for result in &mut results {
            result.rollback_file = Some(final_path.clone());
        }
    }
    if let Some(recovery_journal) = recovery_journal {
        let path = recovery_journal.finalize().map_err(|error| {
            format!(
                "Protected writes completed, but the independent recovery history could not be finalized: {error}"
            )
        })?;
        log::info!("Recovery history finalized at {}", path.display());
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

async fn execute_protected_dml_body(
    conn: &mut sqlx::MySqlConnection,
    query: &str,
    plan: &DmlPlan,
    statement_index: usize,
    rollback_journal: &mut RollbackJournal,
    recovery_journal: &mut RecoveryJournal,
    text: super::TextProto,
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
    };
    outcome
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
        return Err(
            "INSERT requires literal values for every primary-key column, or one omitted AUTO_INCREMENT primary key"
                .to_string(),
        );
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
        return Err("UPDATE rollback requires a declared primary key".to_string());
    }
    for assigned in &plan.assigned_columns {
        let column = metadata
            .column(assigned)
            .ok_or_else(|| format!("UPDATE references unknown column {assigned}"))?;
        if column.generated {
            return Err(format!(
                "UPDATE of generated column {} cannot be rollback-protected",
                column.name
            ));
        }
        if metadata.is_primary_key(&column.name) {
            return Err(format!(
                "UPDATE of primary-key column {} is fail-closed because row identity would change",
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
        return Err("DELETE rollback requires a declared primary key".to_string());
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

    let column_sql = format!(
        "SELECT COLUMN_NAME, DATA_TYPE, EXTRA \
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
        columns.push(ColumnMetadata {
            name,
            data_type,
            generated: extra.to_ascii_uppercase().contains("GENERATED"),
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
    let initial = load_table_metadata(conn, object).await?;
    let lock_sql = format!(
        "SELECT 1 FROM {} WHERE FALSE FOR UPDATE",
        initial.qualified_name()
    );
    conn.fetch_optional(sqlx::raw_sql(&lock_sql))
        .await
        .map_err(|error| format!("Could not lock target table metadata: {error}"))?;
    let locked = load_table_metadata(conn, object).await?;
    ensure_dml_safe_table(conn, &locked, check_cascades).await?;
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
            .ok_or_else(|| format!("INSERT references unknown column {column}"))?;
        if metadata_column.generated {
            return Err(format!(
                "INSERT into generated column {} cannot be rollback-protected",
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

fn classify_select(tokens: &[Token]) -> Result<ProtectedStatement, BlockedStatement> {
    if contains_word(tokens, "OUTFILE") || contains_word(tokens, "DUMPFILE") {
        return Err(BlockedStatement::unsupported(
            "SELECT INTO OUTFILE/DUMPFILE has an external write side effect",
        ));
    }
    ensure_only_proven_function_calls(tokens)?;
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

fn classify_set(tokens: &[Token]) -> Result<ProtectedStatement, BlockedStatement> {
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
    if has_top_level_symbol(tokens, idx, ",") {
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
use sqlx::{Connection, Executor, Row};
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
