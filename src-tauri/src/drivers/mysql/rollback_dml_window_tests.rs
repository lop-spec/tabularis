use super::*;
#[path = "rollback_insert_fixture.rs"]
mod fixture;

fn metadata() -> TableMetadata {
    TableMetadata {
        schema: "fixture".into(),
        name: "items".into(),
        engine: "InnoDB".into(),
        columns: vec![
            ColumnMetadata {
                name: "id".into(),
                data_type: "bigint".into(),
                generated: false,
                auto_increment: false,
            },
            ColumnMetadata {
                name: "note".into(),
                data_type: "varchar".into(),
                generated: false,
                auto_increment: false,
            },
        ],
        primary_key: vec!["id".into()],
        auto_increment_next: None,
    }
}
fn plans(sql: &[String]) -> Vec<ProtectedStatement> {
    sql.iter().map(|s| plan_for_rollback(s).unwrap()).collect()
}
fn inserts(count: usize) -> Vec<String> {
    (1..=count)
        .map(|id| format!("INSERT INTO items (id) VALUES ({id})"))
        .collect()
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
#[test]
fn windows_are_bounded_by_rows_sql_size_and_boundaries() {
    let mut sql = inserts(300);
    let m = metadata();
    assert_eq!(candidates(&sql, &plans(&sql), 7, &m).len(), 128);
    sql.insert(2, "SELECT 1".into());
    let c = candidates(&sql, &plans(&sql), 7, &m);
    assert_eq!(c.len(), 2);
    assert_eq!(c[1].index, 8);
    let sql = vec![
        format!(
            "INSERT INTO items (id,note) VALUES (1,'{}')",
            "x".repeat(MAX_SQL_BYTES)
        ),
        "INSERT INTO items (id) VALUES (2)".into(),
    ];
    assert!(candidates(&sql, &plans(&sql), 0, &m).is_empty());
}
#[test]
fn equivalent_numeric_keys_cannot_overlap_a_window() {
    for duplicate in ["1", "01", "+1"] {
        let sql = vec![
            "INSERT INTO items (id) VALUES (1)".into(),
            format!("INSERT INTO items (id) VALUES ({duplicate})"),
        ];
        assert_eq!(candidates(&sql, &plans(&sql), 0, &metadata()).len(), 1);
    }
    let sql = vec!["INSERT INTO items (id) VALUES (1),(01)".into()];
    assert!(candidates(&sql, &plans(&sql), 0, &metadata()).is_empty());
}
#[test]
fn updates_and_deletes_require_disjoint_literal_identity_not_mutable_predicates() {
    let m = metadata();
    assert_eq!(equality_key("`id` = -42", &m), Some(vec![-42]));
    for condition in [
        "id > 1",
        "id = 1 OR id = 2",
        "id = 1 AND note = 'x'",
        "id = '1'",
        "id=1.0",
        "id=1e0",
        "id=NULL",
        "id=1 AND id=1",
        "id=(SELECT 1)",
    ] {
        assert_eq!(equality_key(condition, &m), None, "{condition}");
    }
    for verb in [
        "UPDATE items SET note='changed' WHERE",
        "DELETE FROM items WHERE",
    ] {
        let sql = vec![
            format!("{verb} id=1"),
            format!("{verb} id=2"),
            format!("{verb} id=1"),
        ];
        assert_eq!(candidates(&sql, &plans(&sql), 0, &m).len(), 2);
    }
    let sql = vec!["UPDATE items SET id=2 WHERE id=1".into()];
    assert!(candidates(&sql, &plans(&sql), 0, &m).is_empty());
}
#[test]
fn defaults_require_verified_native_rules_and_keys_cannot_clamp_in_permissive_modes() {
    for version in ["8.0.36", "8.4.6"] {
        assert!(native_default_rules(version));
    }
    for version in [
        "5.7.44",
        "9.0.0",
        "8.0.11-TiDB",
        "8.0.36-MariaDB",
        "unknown",
    ] {
        assert!(!native_default_rules(version));
    }
    let mut m = metadata();
    m.columns[0].data_type = "tinyint".into();
    for value in [0, 127] {
        assert!(key_cannot_be_coerced(&m, &vec![value]));
    }
    for value in [-1, 128, 255, 256] {
        assert!(!key_cannot_be_coerced(&m, &vec![value]));
    }
}
#[test]
fn expressions_cannot_hide_writes_in_functions_subqueries_or_executable_comments() {
    let m = metadata();
    for value in [
        "evil()",
        "(SELECT evil())",
        "'a' + evil() + 'b'",
        "(@v := 1)",
        "0 /*! + evil() */",
        "X'00' + evil() + X'00'",
    ] {
        for query in [
            format!("INSERT INTO items (id,note) VALUES (1,{value})"),
            format!("UPDATE items SET note={value} WHERE id=1"),
        ] {
            match plan_for_rollback(&query) {
                Err(blocked) => assert!(
                    !blocked.reason.is_empty(),
                    "strict preflight refusal must explain itself"
                ),
                Ok(plan) => assert!(candidates(&[query], &[plan], 0, &m).is_empty(), "{value}"),
            }
        }
    }
    for value in [
        "'中文;quoted'",
        "NULL",
        "X'00FF'",
        "-42.000001",
        "123456789012345678.123456",
        "ABS(-42)",
        "CONCAT('a','b')",
    ] {
        let sql = vec![format!("INSERT INTO items (id,note) VALUES (1,{value})")];
        assert_eq!(candidates(&sql, &plans(&sql), 0, &m).len(), 1, "{value}");
    }
    let sql = vec!["UPDATE items SET note=id+1 WHERE id=1".into()];
    assert_eq!(candidates(&sql, &plans(&sql), 0, &m).len(), 1);
}
#[test]
fn composite_integer_keys_are_complete_and_canonical() {
    let mut m = metadata();
    m.columns[1].data_type = "int".into();
    m.primary_key.push("note".into());
    assert!(integer_primary_key(&m));
    assert_eq!(equality_key("note=02 AND id=+1", &m), Some(vec![1, 2]));
    assert_eq!(equality_key("id=1", &m), None);
    m.columns[1].data_type = "varchar".into();
    assert!(!integer_primary_key(&m));
}
#[test]
fn row_selection_preserves_lossless_identity_and_rejects_corruption() {
    let m = metadata();
    let row = CapturedRow {
        values: vec!["X'2D3432'".into(), "NULL".into()],
    };
    let indexed = index_rows(&m, vec![row.clone()]).unwrap();
    assert_eq!(select_rows(&indexed, &[vec![-42]])[0].values, row.values);
    assert!(index_rows(&m, vec![row.clone(), row]).is_err());
    for invalid in ["NULL", "X'F'", "X'GG'", "X'FF'", "X'312E30'", "X'中'"] {
        assert!(index_rows(
            &m,
            vec![CapturedRow {
                values: vec![invalid.into(), "NULL".into()]
            }]
        )
        .is_err());
    }
}
#[test]
fn a_pending_window_cannot_cross_commit_scope_or_frame_boundaries() {
    let sql = inserts(2);
    let p = plans(&sql);
    let mut w = DmlWindow::default();
    w.active = Some(Active {
        candidates: candidates(&sql, &p, 10, &metadata()),
        before: BTreeMap::new(),
        results: vec![1],
        metadata: metadata(),
    });
    assert!(w.has_pending());
    assert!(w.before_statement(&p[1], 11, true, false).is_ok());
    for boundary in [
        "COMMIT",
        "ROLLBACK",
        "SELECT 1",
        "USE other",
        "ALTER TABLE items ADD COLUMN extra INT",
    ] {
        assert!(w
            .before_statement(&plan_for_rollback(boundary).unwrap(), 11, true, false)
            .is_err());
    }
    assert!(w.before_statement(&p[1], 12, true, false).is_err());
    assert!(w.before_statement(&p[1], 11, false, false).is_err());
    w.discard();
    assert!(!w.has_pending());
}
#[tokio::test]
async fn auto_increment_never_uses_a_window_or_suppresses_counter_refresh() {
    let mut f = fixture::Fixture::new().await;
    f.state.lock().unwrap().auto_increment = true;
    let sql = inserts(3);
    let p = plans(&sql);
    let mut cache = InsertMetadataCache::default();
    cache.before_statement(&p[0], true);
    let ProtectedStatement::Dml(DmlPlan::Insert(first)) = &p[0] else {
        panic!()
    };
    cache.load(&mut f.conn, &first.table).await.unwrap();
    let root = tempfile::tempdir().unwrap();
    let (mut r, mut h) = journals(root.path());
    let mut w = DmlWindow::default();
    let count = f.count();
    assert!(w
        .try_execute(
            &mut f.conn,
            &sql,
            &p,
            0,
            &cache,
            &mut r,
            &mut h,
            super::super::super::TextProto::protocol_only(true)
        )
        .await
        .is_none());
    assert_eq!(f.count(), count);
    assert_eq!(w.optimized, 0);
    cache.load(&mut f.conn, &first.table).await.unwrap();
    assert_eq!(f.count(), count + 1);
}
#[tokio::test]
async fn prior_nontransactional_steps_keep_later_plain_dml_immediate() {
    let mut f = fixture::Fixture::new().await;
    let sql = inserts(2);
    let p = plans(&sql);
    let mut cache = InsertMetadataCache::default();
    cache.before_statement(&p[0], true);
    let ProtectedStatement::Dml(DmlPlan::Insert(first)) = &p[0] else {
        panic!()
    };
    cache.load(&mut f.conn, &first.table).await.unwrap();
    let root = tempfile::tempdir().unwrap();
    let (mut r, mut h) = journals(root.path());
    r.add_step(RollbackStep {
        statement_index: 0,
        sql: "ALTER TABLE `fixture`.`prior` AUTO_INCREMENT = 42".into(),
        expected_affected_rows: None,
    })
    .unwrap();
    let mut window = DmlWindow::default();
    let requests = f.count();
    assert!(window
        .try_execute(
            &mut f.conn,
            &sql,
            &p,
            0,
            &cache,
            &mut r,
            &mut h,
            super::super::super::TextProto::protocol_only(true)
        )
        .await
        .is_none());
    assert_eq!(f.count(), requests);
    assert!(r.requires_immediate_durability());
    assert!(!window.has_pending());
    assert_eq!(window.optimized, 0);
}
#[tokio::test]
async fn window_faults_block_commit_and_still_send_physical_rollback() {
    for fault in [
        "capture",
        "images",
        "rollback_disk",
        "recovery_disk",
        "both_disks",
    ] {
        let mut f = fixture::Fixture::new().await;
        f.state.lock().unwrap().rows.insert(99);
        sqlx::raw_sql("START TRANSACTION")
            .execute(&mut f.conn)
            .await
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        let (mut r, mut h) = journals(root.path());
        let sql = inserts(2);
        let p = plans(&sql);
        let mut cache = InsertMetadataCache::default();
        cache.before_statement(&p[0], true);
        let ProtectedStatement::Dml(DmlPlan::Insert(first)) = &p[0] else {
            panic!()
        };
        cache.load(&mut f.conn, &first.table).await.unwrap();
        let mut w = DmlWindow::default();
        let text = super::super::super::TextProto::protocol_only(true);
        w.try_execute(&mut f.conn, &sql, &p, 0, &cache, &mut r, &mut h, text)
            .await
            .unwrap()
            .unwrap();
        assert!(w.has_pending());
        assert_eq!(
            r.checkpoint(),
            0,
            "Pending images have not been materialized"
        );
        match fault {
            "capture" => f.state.lock().unwrap().fail_capture = true,
            "images" => f.state.lock().unwrap().omit_capture = true,
            "rollback_disk" => r.fail_next_sync_for_test(),
            "recovery_disk" => h.fail_next_sync_for_test(),
            "both_disks" => {
                r.fail_next_sync_for_test();
                h.fail_next_sync_for_test();
            }
            _ => unreachable!(),
        }
        let outcome = w
            .try_execute(
                &mut f.conn,
                &sql[1..],
                &p[1..],
                1,
                &cache,
                &mut r,
                &mut h,
                text,
            )
            .await
            .unwrap();
        let commit = ProtectedStatement::Transaction(TransactionPlan::Commit);
        let image_fault = matches!(fault, "capture" | "images");
        if image_fault {
            let error = outcome.unwrap_err();
            assert!(
                if fault == "capture" {
                    error.contains("injected window capture failure")
                } else {
                    error.contains("after-image")
                },
                "{error}"
            );
            assert!(
                w.has_pending(),
                "Retain window ownership after failed materialization"
            );
            assert!(w.before_statement(&commit, 2, true, false).is_err());
        } else {
            outcome.unwrap();
            assert!(!w.has_pending());
            w.before_statement(&commit, 2, true, false).unwrap();
            let error =
                journal_barrier::before_statement(&commit, Some(&mut r), Some(&mut h)).unwrap_err();
            assert!(error.contains("not submitted"));
            assert!(
                journal_barrier::before_statement(&commit, Some(&mut r), Some(&mut h)).is_err(),
                "A poisoned barrier cannot be retried into success"
            );
        }
        journal_barrier::before_statement(
            &ProtectedStatement::Transaction(TransactionPlan::Rollback),
            Some(&mut r),
            Some(&mut h),
        )
        .unwrap();
        sqlx::raw_sql("ROLLBACK")
            .execute(&mut f.conn)
            .await
            .unwrap();
        w.discard();
        let rewind = super::super::rewind_journals(Some(&mut r), 0, Some(&mut h), 0);
        assert_eq!(rewind.is_err(), !image_fault);
        let state = f.state.lock().unwrap();
        assert_eq!(
            state.rows,
            [99].into_iter().collect(),
            "Physical ROLLBACK remains available after {fault}"
        );
        assert_eq!(state.queries.iter().filter(|q| *q == "ROLLBACK").count(), 1);
        assert!(!state.queries.iter().any(|q| q == "COMMIT"));
    }
}

async fn run(count: usize, windowed: bool) -> (String, Vec<serde_json::Value>, usize) {
    let mut f = fixture::Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let (mut r, mut h) = journals(root.path());
    let sql = inserts(count);
    let p = plans(&sql);
    let mut cache = InsertMetadataCache::default();
    let mut w = DmlWindow::default();
    let start = std::time::Instant::now();
    for i in 0..count {
        cache.before_statement(&p[i], true);
        w.before_statement(&p[i], i, true, false).unwrap();
        r.defer_transaction_writes();
        h.defer_transaction_writes();
        let fast = if windowed {
            w.try_execute(
                &mut f.conn,
                &sql[i..],
                &p[i..],
                i,
                &cache,
                &mut r,
                &mut h,
                super::super::super::TextProto::protocol_only(true),
            )
            .await
        } else {
            None
        };
        let result = if let Some(result) = fast {
            result.unwrap()
        } else {
            let ProtectedStatement::Dml(DmlPlan::Insert(plan)) = &p[i] else {
                panic!()
            };
            execute_insert(
                &mut f.conn,
                &sql[i],
                plan,
                i,
                &mut r,
                &mut h,
                super::super::super::TextProto::protocol_only(true),
                Some(&mut cache),
            )
            .await
            .unwrap()
        };
        assert_eq!(result.affected_rows, 1);
    }
    assert!(!w.has_pending());
    assert_eq!(r.checkpoint(), count);
    assert_eq!(h.checkpoint(), count);
    w.before_statement(
        &ProtectedStatement::Transaction(TransactionPlan::Commit),
        count,
        true,
        false,
    )
    .unwrap();
    journal_barrier::finish(Some(&mut r), Some(&mut h)).unwrap();
    let rollback = std::fs::read_to_string(r.finalize().unwrap()).unwrap();
    let recovery = std::fs::read_to_string(h.finalize().unwrap())
        .unwrap()
        .lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            let mut s = v.get("statement")?.clone();
            s.as_object_mut().unwrap().remove("id");
            s.as_object_mut().unwrap().remove("executedAt");
            Some(s)
        })
        .collect::<Vec<_>>();
    assert_eq!(f.state.lock().unwrap().rows.len(), count);
    eprintln!("INSERT row-image window fixture: count={count}, windowed={windowed}, requests={}, elapsed={:?}",f.count(),start.elapsed());
    (rollback, recovery, f.count())
}
#[tokio::test]
async fn windowed_3429_inserts_preserve_every_inverse_image_and_statement_order() {
    let count = 3429;
    let immediate = run(count, false).await;
    let windowed = run(count, true).await;
    let body = |sql: String| {
        sql.lines()
            .filter(|line| line.starts_with("DELETE "))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(body(immediate.0), body(windowed.0));
    assert_eq!(immediate.1, windowed.1);
    assert_eq!(immediate.2, 3 * count + 10);
    assert_eq!(
        windowed.2,
        count + 13 + 2 * (count - 1).div_ceil(MAX_STATEMENTS)
    );
}
