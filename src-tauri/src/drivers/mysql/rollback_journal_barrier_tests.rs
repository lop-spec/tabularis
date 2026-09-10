use super::*;
use crate::recovery_history::{RecoveryObject, RecoveryStatement};
use crate::rollback_sql::{RollbackEnvironment, RollbackStep, ServerIdentity};
use std::fs;

fn journals(root: &std::path::Path) -> (RollbackJournal, RecoveryJournal) {
    let rollback = RollbackJournal::create_for_test(
        root,
        RollbackEnvironment {
            connection_id: "barrier-fixture".into(),
            connection_name: "fixture".into(),
            database: "fixture".into(),
            current_user: "fixture".into(),
            server: ServerIdentity::Uuid("fixture".into()),
        },
    );
    (rollback, RecoveryJournal::create_for_test(root))
}

fn append(rollback: &mut RollbackJournal, recovery: &mut RecoveryJournal, index: usize) {
    rollback.defer_transaction_writes();
    recovery.defer_transaction_writes();
    rollback
        .add_step(RollbackStep {
            statement_index: index,
            sql: format!("DELETE FROM `fixture`.`items` WHERE id = {}", index + 1),
            expected_affected_rows: Some(1),
        })
        .unwrap();
    recovery
        .add_statement(RecoveryStatement {
            id: String::new(),
            index,
            executed_at: String::new(),
            sql: format!("INSERT INTO items (id) VALUES ({})", index + 1),
            category: "dml".into(),
            operation: "insert".into(),
            objects: vec![RecoveryObject {
                kind: "table".into(),
                schema: "fixture".into(),
                name: "items".into(),
            }],
            affected_columns: vec![],
            condition: None,
            columns: vec![],
            primary_key: vec![],
            before_rows: vec![],
            after_rows: vec![],
            inverse_sql: None,
            exact: true,
        })
        .unwrap();
}

#[test]
fn commit_persists_both_journals_before_submission_and_preserves_order() {
    let root = tempfile::tempdir().unwrap();
    let (mut rollback, mut recovery) = journals(root.path());
    let path = rollback.current_recovery_path().to_path_buf();
    for index in 0..4 {
        append(&mut rollback, &mut recovery, index);
    }
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
    let commit = ProtectedStatement::Transaction(TransactionPlan::Commit);
    before_statement(&commit, Some(&mut rollback), Some(&mut recovery)).unwrap();
    let lines: Vec<serde_json::Value> = fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(lines.len(), 5);
    for index in 0..4 {
        assert_eq!(lines[index + 1]["step"]["statement_index"], index);
    }
    let recovery_path = recovery.interrupt("simulated unknown COMMIT").unwrap();
    let text = fs::read_to_string(recovery_path).unwrap();
    assert_eq!(
        text.lines()
            .filter(|line| line.contains("\"record\":\"statement\""))
            .count(),
        4
    );
    let inverse = fs::read_to_string(rollback.abandon()).unwrap();
    assert!(inverse.find("id = 4").unwrap() < inverse.find("id = 1").unwrap());
    assert!(text.contains("interrupted"));
}

#[test]
fn either_or_both_failed_journals_block_commit_without_masking_failure() {
    for (fail_rollback, fail_recovery) in [(true, false), (false, true), (true, true)] {
        let root = tempfile::tempdir().unwrap();
        let (mut rollback, mut recovery) = journals(root.path());
        append(&mut rollback, &mut recovery, 0);
        if fail_rollback {
            rollback.fail_next_sync_for_test();
        }
        if fail_recovery {
            recovery.fail_next_sync_for_test();
        }
        let commit = ProtectedStatement::Transaction(TransactionPlan::Commit);
        let error =
            before_statement(&commit, Some(&mut rollback), Some(&mut recovery)).unwrap_err();
        assert!(error.contains("not submitted"));
        assert_eq!(error.contains("persist rollback steps"), fail_rollback);
        assert_eq!(error.contains("persist recovery history"), fail_recovery);
        assert!(before_statement(&commit, Some(&mut rollback), Some(&mut recovery)).is_err());
        // A failed disk cannot stop the physical ROLLBACK request.
        let cancel = ProtectedStatement::Transaction(TransactionPlan::Rollback);
        before_statement(&cancel, Some(&mut rollback), Some(&mut recovery)).unwrap();
    }
}

#[test]
fn non_dml_session_read_and_ddl_statements_are_barriers() {
    for sql in [
        "SELECT 1",
        "USE fixture",
        "ALTER TABLE items ADD COLUMN extra INT",
        "START TRANSACTION",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (mut rollback, mut recovery) = journals(root.path());
        append(&mut rollback, &mut recovery, 0);
        let plan = super::super::plan_for_rollback(sql).unwrap();
        before_statement(&plan, Some(&mut rollback), Some(&mut recovery)).unwrap();
        assert_eq!(
            fs::read_to_string(rollback.current_recovery_path())
                .unwrap()
                .lines()
                .count(),
            2
        );
    }
}

#[test]
fn consecutive_dml_keeps_buffering_but_rollback_rewinds_both_durably() {
    let root = tempfile::tempdir().unwrap();
    let (mut rollback, mut recovery) = journals(root.path());
    append(&mut rollback, &mut recovery, 0);
    finish(Some(&mut rollback), Some(&mut recovery)).unwrap();
    append(&mut rollback, &mut recovery, 1);
    let dml = super::super::plan_for_rollback("DELETE FROM items WHERE id = 2").unwrap();
    before_statement(&dml, Some(&mut rollback), Some(&mut recovery)).unwrap();
    assert_eq!(
        fs::read_to_string(rollback.current_recovery_path())
            .unwrap()
            .lines()
            .count(),
        2
    );
    super::super::rewind_journals(Some(&mut rollback), 1, Some(&mut recovery), 1).unwrap();
    assert_eq!(rollback.checkpoint(), 1);
    assert_eq!(recovery.checkpoint(), 1);
    let inverse = fs::read_to_string(rollback.finalize().unwrap()).unwrap();
    assert!(inverse.contains("id = 1"));
    assert!(!inverse.contains("id = 2"));
    let history = fs::read_to_string(recovery.finalize().unwrap()).unwrap();
    assert!(history.contains("\"checkpoint\":1"));
}

#[test]
fn auto_increment_steps_cannot_be_lost_with_an_uncommitted_buffer() {
    let root = tempfile::tempdir().unwrap();
    let (mut rollback, mut recovery) = journals(root.path());
    append(&mut rollback, &mut recovery, 0);
    rollback
        .add_step(RollbackStep {
            statement_index: 1,
            sql: "ALTER TABLE `fixture`.`items` AUTO_INCREMENT = 42".into(),
            expected_affected_rows: None,
        })
        .unwrap();
    assert!(rollback.requires_immediate_durability());
    assert_eq!(
        fs::read_to_string(rollback.current_recovery_path())
            .unwrap()
            .lines()
            .count(),
        3
    );
    rollback.defer_transaction_writes();
    rollback
        .add_step(RollbackStep {
            statement_index: 1,
            sql: "DELETE FROM `fixture`.`items` WHERE id = 42".into(),
            expected_affected_rows: Some(1),
        })
        .unwrap();
    assert_eq!(
        fs::read_to_string(rollback.current_recovery_path())
            .unwrap()
            .lines()
            .count(),
        4
    );
}

#[test]
fn executor_barriers_cover_both_batch_paths_and_managed_transactions() {
    let source = include_str!("rollback_guard.rs");
    assert_eq!(
        source.matches("journal_barrier::before_statement(").count(),
        2
    );
    assert_eq!(source.matches("&& (durability_failed").count(), 2);
    let managed = source
        .split("async fn execute_protected_dml(")
        .nth(1)
        .unwrap()
        .split("async fn execute_protected_dml_body(")
        .next()
        .unwrap();
    assert!(
        managed.find("journal_barrier::finish(").unwrap()
            < managed.find("transaction.commit()").unwrap()
    );
    assert!(managed.contains("transaction.rollback().await"));
}
