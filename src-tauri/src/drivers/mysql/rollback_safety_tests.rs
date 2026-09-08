use super::*;

fn table() -> TableMetadata {
    TableMetadata {
        schema: "audit_fixture".into(),
        name: "items".into(),
        engine: "InnoDB".into(),
        columns: vec![
            ColumnMetadata {
                name: "id".into(),
                data_type: "int".into(),
                generated: false,
                auto_increment: false,
            },
            ColumnMetadata {
                name: "code".into(),
                data_type: "varchar".into(),
                generated: false,
                auto_increment: false,
            },
        ],
        primary_key: vec!["id".into()],
        auto_increment_next: None,
    }
}

#[test]
fn strict_preflight_refuses_default_and_legacy_unprotected_policy() {
    let plans = vec![ProtectedStatement::Unsupported(
        BlockedStatement::unsupported("no key"),
    )];
    for policy in [None, Some(RollbackUnsupportedPolicy::ExecuteUnprotected)] {
        let error = safety::preflight(&plans, policy).unwrap_err();
        assert!(error.contains("No unprotected retry"));
        assert!(
            !error.contains(ROLLBACK_RISK_REVIEW_PREFIX),
            "must not offer an unsafe retry modal"
        );
    }
    assert!(safety::preflight(&plans, Some(RollbackUnsupportedPolicy::Skip)).is_ok());
    assert!(safety::preflight(&[ProtectedStatement::ReadOnly], None).is_ok());
}

#[test]
fn all_insert_select_conversion_is_refused_including_bit_and_repeated_keys() {
    for sql in [
        "INSERT INTO audit_fixture.items (id, flag) SELECT 1, 1",
        "INSERT INTO audit_fixture.items SELECT * FROM audit_fixture.source",
        "INSERT IGNORE INTO audit_fixture.items (id) SELECT id FROM audit_fixture.source",
        "INSERT INTO audit_fixture.items (id,v) SELECT id,v FROM audit_fixture.source ON DUPLICATE KEY UPDATE v=VALUES(v)",
        "INSERT INTO audit_fixture.items (id) SELECT 1 AS id UNION ALL SELECT 1 AS id",
        "INSERT INTO audit_fixture.items (id,v) SELECT NOW(3),NOW(3)",
    ] {
        assert!(plan_for_rollback(sql).is_err(), "{sql}");
    }
    let internal_plan = InsertFamilyPlan {
        table: ObjectName {
            schema: Some("audit_fixture".into()),
            name: "items".into(),
        },
        columns: None,
        source: InsertSource::Select("SELECT 1".into()),
        ignore: false,
        upsert: None,
    };
    assert!(
        safety::validate_insert_source(&internal_plan).is_err(),
        "runtime also refuses parser bypass"
    );
}

#[test]
fn single_row_literal_upsert_and_ignore_remain_supported() {
    for sql in [
        "INSERT INTO audit_fixture.items (id,code) VALUES (1,'a') ON DUPLICATE KEY UPDATE code='b'",
        "INSERT IGNORE INTO audit_fixture.items (id,code) VALUES (1,'a'),(2,'b')",
        "INSERT INTO audit_fixture.items VALUES (1,'a')",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::SupportedDml,
            "{sql}"
        );
    }
    assert!(plan_for_rollback("INSERT INTO audit_fixture.items (id,code) VALUES (1,'a'),(1,'b') ON DUPLICATE KEY UPDATE code=VALUES(code)").is_err());
}

#[test]
fn hidden_functions_cannot_enter_any_normalized_dml_expression() {
    for sql in [
        "INSERT INTO audit_fixture.items (id,v) SELECT 1,audit_fixture.bump_other_table()",
        "UPDATE audit_fixture.items d JOIN audit_fixture.source s ON d.id=s.id AND audit_fixture.bump_other_table(s.id)=1 SET d.code='x'",
        "DELETE d FROM audit_fixture.items d JOIN audit_fixture.source s ON d.id=s.id AND audit_fixture.bump_other_table(s.id)=1",
        "UPDATE audit_fixture.items d JOIN audit_fixture.source s ON d.id=s.id AND hidden_write(s.id)=1 SET d.code='x'",
        "DELETE d FROM audit_fixture.items d JOIN audit_fixture.source s ON d.id=s.id AND hidden_write(s.id)=1",
        "UPDATE audit_fixture.items d JOIN audit_fixture.source s ON d.id=s.id SET d.code=hidden_write(s.id)",
    ] { assert!(plan_for_rollback(sql).is_err(), "{sql}"); }
}

#[test]
fn unstable_selection_and_multiple_write_targets_require_manual_rewrite() {
    for sql in [
        "UPDATE audit_fixture.items d SET d.code='x' WHERE d.id=IF(NOW(6)<'2030-01-01',1,2)",
        "UPDATE audit_fixture.items SET code='x' WHERE RAND()>0.5",
        "DELETE FROM audit_fixture.items WHERE SYSDATE()>'2030-01-01'",
        "DELETE d FROM audit_fixture.items d WHERE d.id=IF(SYSDATE()<'2030-01-01',1,2)",
        "UPDATE audit_fixture.items d JOIN audit_fixture.source s ON d.id=s.id SET d.code='x',s.code='y'",
        "DELETE d,s FROM audit_fixture.items d JOIN audit_fixture.source s ON d.id=s.id",
        "DELETE d FROM audit_fixture.items d WHERE EXISTS (SELECT 1 FROM audit_fixture.source s WHERE s.id=d.id)",
    ] { assert!(plan_for_rollback(sql).is_err(), "{sql}"); }
}

#[test]
fn join_update_actual_write_is_intersected_with_captured_keys() {
    let sql = "UPDATE audit_fixture.items d JOIN audit_fixture.source s ON d.id=s.id SET d.code=s.code WHERE s.active=1 OR d.id=9;";
    let ProtectedStatement::Dml(DmlPlan::MultiUpdate(plan)) = plan_for_rollback(sql).unwrap()
    else {
        panic!("multi plan")
    };
    let rows = vec![CapturedRow {
        values: vec!["X'31'".into(), "X'61'".into()],
    }];
    let actual = safety::restricted_multi_sql(sql, &plan, &table(), &rows).unwrap();
    assert!(actual.contains("WHERE (s.active=1 OR d.id=9) AND ("));
    assert!(actual.contains("`d`.`id` <=> CAST(CONVERT(X'31'"));
    assert!(actual.contains("SET d.code=s.code"));
    assert!(!actual.ends_with(';'));
    let none = safety::restricted_multi_sql(sql, &plan, &table(), &[]).unwrap();
    assert!(none.ends_with("AND (FALSE)"));
}

#[test]
fn join_delete_without_where_is_also_key_restricted() {
    let sql = "DELETE d FROM audit_fixture.items d LEFT JOIN audit_fixture.source s ON d.id=s.id";
    let ProtectedStatement::Dml(DmlPlan::MultiDelete(plan)) = plan_for_rollback(sql).unwrap()
    else {
        panic!("multi plan")
    };
    let actual = safety::restricted_multi_sql(sql, &plan, &table(), &[]).unwrap();
    assert_eq!(actual, format!("{sql}\nWHERE (TRUE) AND (FALSE)"));
}

#[test]
fn comments_cannot_swallow_the_added_key_restriction() {
    let sql = "UPDATE audit_fixture.items d SET d.code='x' -- keep this comment\nWHERE d.id=1";
    let ProtectedStatement::Dml(DmlPlan::MultiUpdate(plan)) = plan_for_rollback(sql).unwrap()
    else {
        panic!("multi plan")
    };
    let actual = safety::restricted_multi_sql(sql, &plan, &table(), &[]).unwrap();
    assert!(actual.contains("-- keep this comment\nWHERE"));
    let tokens = tokenize(&actual).unwrap();
    assert!(find_top_level_word(&tokens, 0, &["WHERE"]).is_some());
    assert!(tokens.iter().any(|token| token.upper() == "FALSE"));
}

#[test]
fn after_lookup_tracks_old_stable_identity_not_mutated_secondary_values() {
    let row = CapturedRow {
        values: vec!["X'31'".into(), "X'6f6c64'".into()],
    };
    let filter = captured_primary_key_filter(&table(), &[row]).unwrap();
    assert!(filter.contains("`id`"));
    assert!(!filter.contains("`code`"));
    assert!(!filter.contains("X'6f6c64'"));
}

#[test]
fn upsert_tripwire_rejects_the_audit_counterexample_and_missing_images() {
    assert!(safety::validate_upsert_counts(2, 2, 1, 0, 0).is_err());
    assert!(safety::validate_upsert_counts(2, 1, 0, 0, 0).is_err());
    assert!(
        safety::validate_upsert_counts(1, 1, 1, 0, 1).is_err(),
        "changed update always counts 2"
    );
    assert!(safety::validate_upsert_counts(2, 1, 1, 0, 1).is_ok());
    for no_op_count in [0, 1] {
        assert!(safety::validate_upsert_counts(no_op_count, 1, 1, 0, 0).is_ok());
    }
    assert!(safety::validate_upsert_counts(1, 0, 1, 1, 0).is_ok());
    assert!(safety::validate_upsert_counts(2, 0, 2, 2, 0).is_err());
}

#[test]
fn ignore_refuses_truncation_and_truncated_warning_lists() {
    assert!(safety::complete_duplicate_warnings(0, &[]).is_ok());
    assert!(safety::complete_duplicate_warnings(2, &[1062, 1062]).is_ok());
    for (count, codes) in [
        (1, vec![1265]),
        (2, vec![1062]),
        (1, vec![]),
        (2, vec![1062, 1264]),
    ] {
        assert!(safety::complete_duplicate_warnings(count, &codes).is_err());
    }
}

#[test]
fn conflict_handling_requires_gap_locking_isolation() {
    for value in ["REPEATABLE-READ", "SERIALIZABLE", "repeatable read"] {
        assert!(safety::safe_conflict_isolation(value));
    }
    for value in ["READ-COMMITTED", "READ-UNCOMMITTED", "", "unknown"] {
        assert!(!safety::safe_conflict_isolation(value));
    }
}

#[test]
fn read_and_explain_routes_do_not_hide_function_writes() {
    for sql in [
        "SELECT hidden_write()",
        "EXPLAIN ANALYZE SELECT hidden_write()",
        "SHOW TABLES WHERE hidden_write()=1",
        "SELECT audit_fixture.hidden_write()",
    ] {
        assert!(plan_for_rollback(sql).is_err(), "{sql}");
    }
    for sql in [
        "SELECT GROUP_CONCAT(code), MD5(code) FROM audit_fixture.items",
        "SELECT UUID()",
        "SELECT ROW_NUMBER() OVER (ORDER BY id) FROM audit_fixture.items",
    ] {
        assert_eq!(
            plan_for_rollback(sql),
            Ok(ProtectedStatement::ReadOnly),
            "{sql}"
        );
    }
}

#[test]
fn session_cannot_disguise_active_transaction_isolation() {
    for sql in [
        "SET SESSION transaction_isolation='REPEATABLE-READ'",
        "SET @@session.tx_isolation='REPEATABLE-READ'",
        "SET transaction_read_only=0",
    ] {
        assert!(plan_for_rollback(sql).is_err(), "{sql}");
    }
}

#[test]
fn fallback_identity_uses_only_an_entire_verified_unique_index() {
    let columns = vec![
        ("a_nullable".into(), "nullable".into(), false),
        ("b_partial".into(), "ok".into(), true),
        ("b_partial".into(), "prefix".into(), false),
        ("c_stable".into(), "tenant".into(), true),
        ("c_stable".into(), "external_id".into(), true),
    ];
    assert_eq!(
        safety::stable_unique_key(columns),
        Some(vec!["tenant".into(), "external_id".into()])
    );
    assert_eq!(
        safety::stable_unique_key(vec![("expression".into(), "".into(), false)]),
        None
    );
    assert_eq!(safety::stable_unique_key(vec![]), None);
}
