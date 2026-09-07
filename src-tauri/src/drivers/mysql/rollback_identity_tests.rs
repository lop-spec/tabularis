use super::*;

fn metadata() -> TableMetadata {
    TableMetadata {
        schema: "app".into(),
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
                name: "name".into(),
                data_type: "varchar".into(),
                generated: false,
                auto_increment: false,
            },
        ],
        primary_key: vec!["id".into()],
        auto_increment_next: None,
    }
}

fn assert_indexed(sql: &str) {
    assert!(
        sql.contains("`id` <=> CAST(CONVERT(X'31' USING ascii) AS DECIMAL(65,0))"),
        "{sql}"
    );
    assert!(sql.contains("CAST(`id` AS BINARY) <=> X'31'"), "{sql}");
}

#[test]
fn multi_table_before_and_after_images_use_indexed_keys() {
    assert_indexed(&encoded_pk_filter(&metadata(), &[vec!["X'31'".into()]]));
    assert_eq!(encoded_pk_filter(&metadata(), &[]), "FALSE");
}

#[test]
fn captured_keys_and_rollback_writes_keep_index_lookup_and_full_guards() {
    let before = CapturedRow {
        values: vec!["X'31'".into(), "X'61'".into()],
    };
    let after = CapturedRow {
        values: vec!["X'31'".into(), "X'62'".into()],
    };
    assert_indexed(&captured_primary_key_filter(&metadata(), &[before.clone()]).unwrap());
    let update = build_update_rollback(&metadata(), &before, &after).unwrap();
    assert_indexed(&update);
    assert!(update.contains("CAST(`name` AS BINARY) <=> X'62'"));
    let delete = build_insert_rollback_delete(&metadata(), &after).unwrap();
    assert_indexed(&delete);
    assert!(delete.contains("CAST(`name` AS BINARY) <=> X'62'"));
}

#[test]
fn insert_select_keys_use_the_same_typed_lookup() {
    let metadata = metadata();
    assert_indexed(
        &family_key_filter(
            &metadata,
            &["id".into()],
            &[vec!["X'31'".into()]],
            true,
            &[metadata.primary_key.clone()],
            false,
        )
        .unwrap(),
    );
    assert_eq!(
        family_key_filter(
            &metadata,
            &["id".into()],
            &[vec!["1".into()]],
            false,
            &[metadata.primary_key.clone()],
            false
        )
        .unwrap(),
        "(`id` <=> 1)"
    );
}

#[test]
fn composite_keys_preserve_each_type_and_or_grouping() {
    let mut metadata = metadata();
    metadata.primary_key.push("name".into());
    let sql = encoded_pk_filter(
        &metadata,
        &[
            vec!["X'31'".into(), "X'41'".into()],
            vec!["X'32'".into(), "X'42'".into()],
        ],
    );
    assert_indexed(&sql);
    assert!(
        sql.contains("AND CAST(`name` AS BINARY) <=> X'41') OR ("),
        "{sql}"
    );
    assert!(!sql.contains("`name` <=> CAST(CONVERT("));
}
