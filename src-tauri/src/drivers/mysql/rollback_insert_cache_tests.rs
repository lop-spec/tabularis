use super::super::super::TextProto;
use super::super::{execute_insert, plan_for_rollback, InsertPlan};
use super::*;
use crate::{
    recovery_history::RecoveryJournal,
    rollback_sql::{RollbackEnvironment, RollbackJournal, ServerIdentity},
};
use std::{fs, time::Instant};

#[path = "rollback_insert_fixture.rs"]
mod fixture;
use fixture::Fixture;

fn insert(id: u64) -> (String, ProtectedStatement) {
    let sql = format!("INSERT INTO items (id) VALUES ({id})");
    let plan = plan_for_rollback(&sql).unwrap();
    (sql, plan)
}

fn insert_plan(statement: &ProtectedStatement) -> &InsertPlan {
    match statement {
        ProtectedStatement::Dml(DmlPlan::Insert(plan)) => plan,
        _ => panic!("expected a plain insert"),
    }
}

fn journals(root: &std::path::Path) -> (RollbackJournal, RecoveryJournal) {
    (
        RollbackJournal::create_for_test(
            root,
            RollbackEnvironment {
                connection_id: "fixture".into(),
                connection_name: "fixture".into(),
                database: "fixture".into(),
                current_user: "fixture".into(),
                server: ServerIdentity::Uuid("fixture".into()),
            },
        ),
        RecoveryJournal::create_for_test(root),
    )
}

fn recovery_statements(path: &std::path::Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            let mut statement = value.get("statement")?.clone();
            let object = statement.as_object_mut().unwrap();
            object.remove("id");
            object.remove("executedAt");
            Some(statement)
        })
        .collect()
}

async fn run_insert_batch(count: usize, reuse: bool) -> (usize, String, Vec<serde_json::Value>) {
    let mut fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let (mut rollback, mut recovery) = journals(root.path());
    let mut cache = InsertMetadataCache::default();
    let start = Instant::now();
    for index in 0..count {
        let (sql, statement) = insert(index as u64 + 1);
        cache.before_statement(&statement, true);
        let result = execute_insert(
            &mut fixture.conn,
            &sql,
            insert_plan(&statement),
            index,
            &mut rollback,
            &mut recovery,
            TextProto::protocol_only(true),
            reuse.then_some(&mut cache),
        )
        .await
        .unwrap();
        assert_eq!(result.affected_rows, 1);
        // Every INSERT has durable inverse and recovery entries before the
        // next one, regardless of whether table inspection is reused.
        assert_eq!(rollback.checkpoint(), index + 1);
        assert_eq!(recovery.checkpoint(), index + 1);
    }
    let requests = fixture.count();
    eprintln!(
        "INSERT protocol fixture: count={count}, reuse={reuse}, requests={requests}, elapsed={:?}",
        start.elapsed()
    );
    if reuse {
        assert_eq!(cache.inspections, 1);
        assert_eq!(cache.hits, count - 1);
    }
    let rollback_sql = fs::read_to_string(rollback.finalize().unwrap()).unwrap();
    let recovered = recovery_statements(&recovery.finalize().unwrap());
    assert_eq!(fixture.state.lock().unwrap().rows.len(), count);
    assert_eq!(recovered.len(), count);
    (requests, rollback_sql, recovered)
}

#[tokio::test]
async fn protected_3429_insert_batch_removes_repeated_roundtrips_without_changing_journals() {
    // Both paths retain the existing COM_QUERY selection used by bastions.
    // This measures real sqlx protocol requests, not production DB latency;
    // SQL text, execution order and the selected protocol are unchanged.
    let count = 3429;
    let (before, baseline_sql, baseline_recovery) = run_insert_batch(count, false).await;
    let (after, optimized_sql, optimized_recovery) = run_insert_batch(count, true).await;
    assert_eq!(before, count * 13);
    assert_eq!(after, count * 3 + 10);
    assert!(after * 100 < before * 30, "at least 70% fewer requests");
    assert_eq!(optimized_sql, baseline_sql);
    assert_eq!(optimized_recovery, baseline_recovery);
}

#[tokio::test]
async fn every_non_insert_and_transaction_boundary_invalidates_reuse() {
    let mut fixture = Fixture::new().await;
    let (_, statement) = insert(1);
    let object = &insert_plan(&statement).table;
    for boundary in [
        "SELECT 1",
        "USE other_schema",
        "SET @a = 1",
        "SET SESSION sql_mode = ''",
        "COMMIT",
        "ROLLBACK",
        "START TRANSACTION",
        "UPDATE items SET id = 2 WHERE id = 1",
        "DELETE FROM items WHERE id = 1",
        "CREATE TEMPORARY TABLE items (id INT)",
        "DROP TEMPORARY TABLE items",
        "ALTER TABLE items ADD COLUMN label INT",
        "INSERT IGNORE INTO items (id) VALUES (1)",
    ] {
        let mut cache = InsertMetadataCache::default();
        cache.before_statement(&statement, true);
        cache.load(&mut fixture.conn, object).await.unwrap();
        let boundary = plan_for_rollback(boundary).unwrap();
        cache.before_statement(&boundary, true);
        assert!(cache.entry.is_none(), "boundary: {boundary:?}");
        cache.before_statement(&statement, true);
        cache.load(&mut fixture.conn, object).await.unwrap();
        assert_eq!(cache.inspections, 2);
        assert_eq!(cache.hits, 0);
    }
}

#[tokio::test]
async fn reuse_never_crosses_autocommit_batches_tables_or_database_scope() {
    let mut fixture = Fixture::new().await;
    let (_, statement) = insert(1);
    let object = &insert_plan(&statement).table;
    let mut cache = InsertMetadataCache::default();
    for _ in 0..2 {
        cache.before_statement(&statement, false);
        cache.load(&mut fixture.conn, object).await.unwrap();
        assert!(cache.entry.is_none());
    }
    assert_eq!(fixture.count(), 20);
    cache.before_statement(&statement, true);
    cache.load(&mut fixture.conn, object).await.unwrap();
    let another = plan_for_rollback("INSERT INTO other_items (id) VALUES (1)").unwrap();
    cache.before_statement(&another, true);
    assert!(cache.entry.is_none());
    cache
        .load(&mut fixture.conn, &insert_plan(&another).table)
        .await
        .unwrap();
    assert_eq!(cache.inspections, 4);

    cache.before_statement(&plan_for_rollback("USE next_schema").unwrap(), true);
    fixture.state.lock().unwrap().schema = "next_schema".into();
    cache.before_statement(&statement, true);
    assert_eq!(
        cache.load(&mut fixture.conn, object).await.unwrap().schema,
        "next_schema"
    );
    drop(cache); // Returning or aborting a batch drops its cache, not its MDL.
    let mut next_batch = InsertMetadataCache::default();
    next_batch.before_statement(&statement, true);
    next_batch.load(&mut fixture.conn, object).await.unwrap();
    assert_eq!(next_batch.inspections, 1);
    assert_eq!(next_batch.hits, 0);
}

#[tokio::test]
async fn invalidated_metadata_rechecks_hidden_writes_and_never_caches_a_refusal() {
    let mut fixture = Fixture::new().await;
    let (_, statement) = insert(1);
    let object = &insert_plan(&statement).table;
    let mut cache = InsertMetadataCache::default();
    cache.before_statement(&statement, true);
    cache.load(&mut fixture.conn, object).await.unwrap();
    cache.before_statement(&ProtectedStatement::ReadOnly, true);
    fixture.state.lock().unwrap().triggers = true;
    cache.before_statement(&statement, true);
    assert!(cache
        .load(&mut fixture.conn, object)
        .await
        .unwrap_err()
        .contains("has triggers"));
    assert!(cache.entry.is_none());
    fixture.state.lock().unwrap().triggers = false;
    fixture.state.lock().unwrap().engine = "MyISAM".into();
    assert!(cache
        .load(&mut fixture.conn, object)
        .await
        .unwrap_err()
        .contains("requires InnoDB"));
    assert!(cache.entry.is_none());
}

#[tokio::test]
async fn auto_increment_is_refreshed_and_counter_read_failure_never_executes_an_insert() {
    let mut fixture = Fixture::new().await;
    fixture.state.lock().unwrap().auto_increment = true;
    let root = tempfile::tempdir().unwrap();
    let (mut rollback, mut recovery) = journals(root.path());
    let mut cache = InsertMetadataCache::default();
    for id in [10, 20] {
        let (sql, statement) = insert(id);
        cache.before_statement(&statement, true);
        execute_insert(
            &mut fixture.conn,
            &sql,
            insert_plan(&statement),
            id as usize,
            &mut rollback,
            &mut recovery,
            TextProto::protocol_only(true),
            Some(&mut cache),
        )
        .await
        .unwrap();
    }
    assert_eq!(cache.inspections, 1);
    assert_eq!(cache.counter_refreshes, 1);
    fixture.state.lock().unwrap().fail_refresh = true;
    let (sql, statement) = insert(30);
    cache.before_statement(&statement, true);
    let error = execute_insert(
        &mut fixture.conn,
        &sql,
        insert_plan(&statement),
        30,
        &mut rollback,
        &mut recovery,
        TextProto::protocol_only(true),
        Some(&mut cache),
    )
    .await
    .unwrap_err();
    assert!(error.contains("Could not refresh"));
    assert!(!fixture.state.lock().unwrap().rows.contains(&30));
    assert_eq!(recovery.checkpoint(), 2);
    let sql = fs::read_to_string(rollback.finalize().unwrap()).unwrap();
    assert!(sql.contains("AUTO_INCREMENT = 1;"));
    assert!(sql.contains("AUTO_INCREMENT = 11;"));
    assert!(!sql.contains("AUTO_INCREMENT = 21;"));
}

#[tokio::test]
async fn reused_table_shape_does_not_skip_duplicate_key_checks_or_row_images() {
    let mut fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let (mut rollback, mut recovery) = journals(root.path());
    let mut cache = InsertMetadataCache::default();
    let (sql, statement) = insert(1);
    cache.before_statement(&statement, true);
    execute_insert(
        &mut fixture.conn,
        &sql,
        insert_plan(&statement),
        0,
        &mut rollback,
        &mut recovery,
        TextProto::protocol_only(true),
        Some(&mut cache),
    )
    .await
    .unwrap();
    cache.before_statement(&statement, true);
    let error = execute_insert(
        &mut fixture.conn,
        &sql,
        insert_plan(&statement),
        1,
        &mut rollback,
        &mut recovery,
        TextProto::protocol_only(true),
        Some(&mut cache),
    )
    .await
    .unwrap_err();
    assert!(error.contains("primary key already exists"));
    assert_eq!(fixture.state.lock().unwrap().rows.len(), 1);
    assert_eq!(recovery.checkpoint(), 1);
    assert_eq!(rollback.checkpoint(), 1);
    assert_eq!(cache.hits, 1);
}
