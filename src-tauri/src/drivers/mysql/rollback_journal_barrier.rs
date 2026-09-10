use super::{ProtectedStatement, RecoveryJournal, RollbackJournal, TransactionPlan};

/// DML owns a still-uncommitted InnoDB transaction. Everything else is a
/// durability barrier, except ROLLBACK: disk failure must never prevent it.
pub(super) fn before_statement(
    plan: &ProtectedStatement,
    rollback: Option<&mut RollbackJournal>,
    recovery: Option<&mut RecoveryJournal>,
) -> Result<(), String> {
    if matches!(
        plan,
        ProtectedStatement::Dml(_) | ProtectedStatement::Transaction(TransactionPlan::Rollback)
    ) {
        return Ok(());
    }
    finish(rollback, recovery)
}

pub(super) fn finish(
    rollback: Option<&mut RollbackJournal>,
    recovery: Option<&mut RecoveryJournal>,
) -> Result<(), String> {
    // Attempt both flushes, but never let the second success hide the first failure.
    let rollback = rollback.map_or(Ok(()), RollbackJournal::finish_transaction_writes);
    let recovery = recovery.map_or(Ok(()), RecoveryJournal::finish_transaction_writes);
    let errors: Vec<_> = rollback.err().into_iter().chain(recovery.err()).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        let reason = format!(
            "Protected journal barrier failed; the next SQL statement was not submitted: {}",
            errors.join("; ")
        );
        log::error!("{reason}");
        Err(reason)
    }
}

#[cfg(test)]
#[path = "rollback_journal_barrier_tests.rs"]
mod tests;
