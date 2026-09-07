use super::encoded_key_condition;

#[test]
fn integer_lookup_keeps_the_indexed_column_bare() {
    let sql = encoded_key_condition("`id`", "bigint", "X'3432'");
    assert_eq!(sql, "(`id` <=> CAST(CONVERT(X'3432' USING ascii) AS DECIMAL(65,0)) AND CAST(`id` AS BINARY) <=> X'3432')");
}

#[test]
fn all_integer_types_use_exact_decimal_constants() {
    for data_type in [
        "tinyint",
        "smallint",
        "mediumint",
        "int",
        "integer",
        "BIGINT",
        "year",
    ] {
        let sql = encoded_key_condition("`key`", data_type, "X'31'");
        assert!(
            sql.starts_with("(`key` <=> CAST(CONVERT("),
            "{data_type}: {sql}"
        );
        assert!(sql.contains("DECIMAL(65,0)"));
        assert!(sql.contains("AND CAST(`key` AS BINARY) <=> X'31'"));
    }
}

#[test]
fn bigint_boundaries_never_pass_through_float_or_signed_casts() {
    for value in [
        "18446744073709551615",
        "-9223372036854775808",
        "9007199254740993",
        "0",
    ] {
        let hex = value
            .bytes()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        let literal = format!("X'{hex}'");
        let sql = encoded_key_condition("`id`", "bigint", &literal);
        assert_eq!(sql.matches(&literal).count(), 2);
        assert!(sql.contains("DECIMAL(65,0)"));
        assert!(!sql.contains("SIGNED"));
        assert!(!sql.contains("DOUBLE"));
    }
}

#[test]
fn nullable_unique_keys_keep_null_safe_comparison() {
    let sql = encoded_key_condition("`key`", "int", "NULL");
    assert!(sql.contains("`key` <=> CAST(CONVERT(NULL"));
    assert!(sql.contains("CAST(`key` AS BINARY) <=> NULL"));
}

#[test]
fn non_integer_semantics_are_not_changed() {
    for data_type in [
        "varchar",
        "binary",
        "varbinary",
        "decimal",
        "bit",
        "datetime",
        "double",
        "",
    ] {
        assert_eq!(
            encoded_key_condition("`key`", data_type, "X'0041FF'"),
            "CAST(`key` AS BINARY) <=> X'0041FF'"
        );
    }
}

#[test]
fn exact_guard_rejects_noncanonical_integer_images() {
    // A candidate numeric match must not make a recorded "01" equal to "1".
    let sql = encoded_key_condition("`id`", "bigint", "X'3031'");
    assert!(sql.ends_with("AND CAST(`id` AS BINARY) <=> X'3031')"));
}
