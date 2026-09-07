#[test]
fn recovery_integer_keys_use_indexed_candidates_with_exact_guards() {
    let stmt = statement(0);
    let work = RowWork {
        schema: "app".into(),
        table: "users".into(),
        columns: stmt.columns.clone(),
        primary_key: stmt.primary_key.clone(),
        affected_columns: BTreeSet::new(),
        compare_all_columns: false,
        keys: BTreeSet::new(),
        order: 0,
        source_ids: BTreeSet::new(),
    };
    let indexed = "`id` <=> CAST(CONVERT(X'31' USING ascii) AS DECIMAL(65,0))";
    for sql in [
        key_condition(&work, &["X'31'".into()]),
        primary_key_condition(&work, &stmt.after_rows[0]).unwrap(),
        offline_pk_condition(&stmt, &[0], &stmt.after_rows[0]),
        build_update_sql(&work, &stmt.after_rows[0], &stmt.before_rows[0], &[1]).unwrap(),
        build_insert_sql(&work, &stmt.before_rows[0]).unwrap(),
        build_delete_sql(&work, &stmt.after_rows[0]).unwrap(),
    ] {
        assert!(sql.contains(indexed), "{sql}");
        assert!(sql.contains("CAST(`id` AS BINARY) <=> X'31'"), "{sql}");
    }
    let sql = build_update_sql(&work, &stmt.after_rows[0], &stmt.before_rows[0], &[1]).unwrap();
    assert!(sql.contains("CAST(`name` AS BINARY) <=> X'62'"));
}
