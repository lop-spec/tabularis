use super::rollback_guard::{
    classify_for_rollback, complete_statement_without_database, locked_write_sql,
    plan_batch_for_rollback, plan_batch_for_rollback_with_policy, plan_for_rollback,
    review_batch_for_rollback, validate_pinned_transaction_structure,
    validate_transaction_structure, DmlPlan, ProtectedStatement, ProtectionClass, TemporaryPlan,
    TransactionPlan,
};
use super::should_use_rollback_guard;
use crate::models::RollbackUnsupportedPolicy;

#[test]
fn classifies_supported_dml_families() {
    for sql in [
        "INSERT INTO users (id, name) VALUES (1, 'Ada')",
        "UPDATE users SET name = 'Grace' WHERE id = 1",
        "DELETE FROM users WHERE id = 1",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::SupportedDml,
            "{sql}"
        );
    }
}

#[test]
fn classifies_supported_ddl_families() {
    for sql in [
        "CREATE TABLE audit_log (id BIGINT PRIMARY KEY)",
        "CREATE DATABASE archive_2026",
        "CREATE VIEW active_users AS SELECT id FROM users WHERE active = 1",
        "CREATE UNIQUE INDEX users_email_uq ON users (email)",
        "RENAME TABLE users TO users_archive",
        "ALTER TABLE users ADD COLUMN note VARCHAR(255)",
        "ALTER TABLE users ADD INDEX users_name_idx (name)",
        "ALTER TABLE users RENAME COLUMN note TO memo",
        "ALTER TABLE users RENAME TO app_users",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::SupportedDdl,
            "{sql}"
        );
    }
}

#[test]
fn blocks_required_destructive_ddl() {
    for sql in [
        "DROP TABLE users",
        "TRUNCATE TABLE users",
        "DROP DATABASE production",
        "DROP SCHEMA production",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::BlockedDestructive,
            "{sql}"
        );
    }
}

#[test]
fn treats_explicit_temporary_table_lifecycle_as_session_only() {
    for sql in [
        "DROP TEMPORARY TABLE IF EXISTS temp_users",
        "CREATE TEMPORARY TABLE temp_users (id BIGINT)",
        "CREATE TEMPORARY TABLE IF NOT EXISTS temp_users LIKE users",
        "CREATE TEMPORARY TABLE temp_users AS SELECT id FROM users",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::SessionOnly,
            "{sql}"
        );
    }
}

#[test]
fn ignores_only_proven_temporary_table_writes_in_a_batch() {
    let queries = [
        "DROP TEMPORARY TABLE IF EXISTS temp_users",
        "CREATE TEMPORARY TABLE temp_users (id BIGINT, name VARCHAR(100))",
        "INSERT INTO temp_users VALUES (1, 'Ada')",
        "UPDATE temp_users SET name = 'Grace' WHERE id = 1 ORDER BY id LIMIT 1",
        "DELETE FROM temp_users WHERE id = 1 ORDER BY id LIMIT 1",
        "TRUNCATE TABLE temp_users",
        "UPDATE users SET name = 'Grace' WHERE id = 1",
    ]
    .map(str::to_string);

    let plans = plan_batch_for_rollback(&queries).unwrap();
    assert!(matches!(
        &plans[0],
        ProtectedStatement::Temporary(TemporaryPlan::Drop(_))
    ));
    assert!(matches!(
        &plans[1],
        ProtectedStatement::Temporary(TemporaryPlan::Create(_))
    ));
    for plan in &plans[2..6] {
        assert_eq!(
            plan,
            &ProtectedStatement::Temporary(TemporaryPlan::Statement)
        );
    }
    assert!(matches!(
        &plans[6],
        ProtectedStatement::Dml(DmlPlan::Update(_))
    ));
}

#[test]
fn stops_ignoring_a_name_after_the_temporary_table_is_dropped() {
    let queries = [
        "CREATE TEMPORARY TABLE temp_users (id BIGINT)",
        "DROP TEMPORARY TABLE temp_users",
        "UPDATE temp_users SET id = 2 WHERE id = 1",
    ]
    .map(str::to_string);

    let plans = plan_batch_for_rollback(&queries).unwrap();
    assert!(matches!(
        &plans[2],
        ProtectedStatement::Dml(DmlPlan::Update(_))
    ));
}

#[test]
fn temp_exception_does_not_hide_real_or_unprovable_writes() {
    for queries in [
        [
            "CREATE TEMPORARY TABLE temp_users (id BIGINT)",
            "UPDATE temp_users JOIN users ON users.id = temp_users.id SET users.active = 0",
        ],
        [
            "CREATE TEMPORARY TABLE temp_users (id BIGINT)",
            "INSERT INTO temp_users SELECT app.mutate_users()",
        ],
        [
            "CREATE TEMPORARY TABLE temp_users (id BIGINT)",
            "PREPARE stmt FROM @sql",
        ],
    ] {
        let queries = queries.map(str::to_string);
        assert!(plan_batch_for_rollback(&queries).is_err(), "{queries:?}");
    }
}

#[test]
fn fail_closes_every_unsupported_write_family() {
    for sql in [
        "REPLACE INTO users (id) VALUES (1)",
        "INSERT INTO users (id) SELECT id FROM staging",
        "INSERT IGNORE INTO users (id) VALUES (1)",
        "INSERT INTO users (id) VALUES (1) ON DUPLICATE KEY UPDATE id = VALUES(id)",
        "UPDATE users u JOIN teams t ON t.id = u.team_id SET u.active = 0",
        "DELETE u FROM users u JOIN teams t ON t.id = u.team_id",
        "LOAD DATA LOCAL INFILE 'users.csv' INTO TABLE users",
        "LOAD XML LOCAL INFILE 'users.xml' INTO TABLE users",
        "CALL mutate_users()",
        "DO mutate_users()",
        "PREPARE stmt FROM @sql",
        "EXECUTE stmt",
        "DEALLOCATE PREPARE stmt",
        "CREATE TRIGGER users_bu BEFORE UPDATE ON users FOR EACH ROW SET NEW.name = UPPER(NEW.name)",
        "CREATE PROCEDURE mutate_users() UPDATE users SET active = 0",
        "CREATE FUNCTION mutate_users() RETURNS INT RETURN 1",
        "CREATE EVENT purge_users ON SCHEDULE EVERY 1 DAY DO DELETE FROM users",
        "CREATE TABLE IF NOT EXISTS users (id BIGINT)",
        "CREATE TABLE copied_users AS SELECT * FROM users",
        "CREATE DATABASE IF NOT EXISTS archive",
        "CREATE OR REPLACE VIEW active_users AS SELECT id FROM users",
        "ALTER TABLE users DROP COLUMN email",
        "ALTER TABLE users MODIFY COLUMN email VARCHAR(500)",
        "ALTER TABLE users CHANGE COLUMN email email VARCHAR(500)",
        "ALTER TABLE users ADD CONSTRAINT users_team_fk FOREIGN KEY (team_id) REFERENCES teams(id)",
        "ALTER TABLE users ADD COLUMN IF NOT EXISTS note VARCHAR(255)",
        "ALTER TABLE users ADD note VARCHAR(255)",
        "ALTER TABLE users ADD PARTITION (PARTITION p2027 VALUES LESS THAN (2028))",
        "ALTER TABLE users ADD SYSTEM VERSIONING",
        "DROP INDEX users_email_uq ON users",
        "DROP VIEW active_users",
        "DROP PROCEDURE mutate_users",
        "DROP FUNCTION mutate_users",
        "DROP TRIGGER users_bu",
        "DROP EVENT purge_users",
        "GRANT SELECT ON app.* TO 'reader'@'%'",
        "REVOKE SELECT ON app.* FROM 'reader'@'%'",
        "SET GLOBAL read_only = 1",
        "LOCK TABLES users WRITE",
        "WITH changed AS (SELECT id FROM users) UPDATE users SET active = 0",
        "CREATE TABLE safe_table (id BIGINT); DROP TABLE users",
        "SELECT 1; UPDATE users SET active = 0",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::BlockedUnsupported,
            "{sql}"
        );
    }
}

#[test]
fn reviews_every_unsupported_statement_before_execution() {
    let queries = [
        "UPDATE users SET active = 1 WHERE id = 1",
        "INSERT INTO users (id) SELECT id FROM staging",
        "CALL mutate_users()",
    ]
    .map(str::to_string);

    let review = review_batch_for_rollback(&queries).expect("risk review");
    assert_eq!(review.statements.len(), 2);
    assert_eq!(review.statements[0].index, 2);
    assert_eq!(
        review.statements[0].sql,
        "INSERT INTO users (id) SELECT id FROM staging"
    );
    assert!(review.statements[0].reason.contains("INSERT SELECT"));
    assert!(!review.statements[0].destructive);
    assert_eq!(review.statements[1].index, 3);
    assert!(review.statements[1].reason.contains("statically provable"));
}

#[test]
fn explicit_user_policy_preserves_unsupported_statement_slots() {
    let queries = [
        "INSERT INTO users (id, name) VALUES (1, 'Ada')",
        "INSERT INTO users (id) SELECT id FROM staging",
    ]
    .map(str::to_string);

    for policy in [
        RollbackUnsupportedPolicy::Skip,
        RollbackUnsupportedPolicy::ExecuteUnprotected,
    ] {
        let plans = plan_batch_for_rollback_with_policy(&queries, policy).unwrap();
        assert!(matches!(
            &plans[0],
            ProtectedStatement::Dml(DmlPlan::Insert(_))
        ));
        assert!(matches!(&plans[1], ProtectedStatement::Unsupported(_)));
    }

    let blocked = plan_batch_for_rollback(&queries).unwrap_err();
    assert!(blocked.contains("statement 2"));
}

#[test]
fn protected_skips_complete_callbacks_without_database_or_audit() {
    let queries = ["INSERT INTO users (id) SELECT id FROM staging"].map(str::to_string);
    let plans =
        plan_batch_for_rollback_with_policy(&queries, RollbackUnsupportedPolicy::Skip).unwrap();
    let callback_indexes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let callback_observed = callback_indexes.clone();
    let callback = move |index: usize, statement: &crate::models::BatchStatementResult| {
        callback_observed.lock().unwrap().push(index);
        assert!(!crate::audit_outbox::statement_was_executed(statement));
        Ok::<(), String>(())
    };

    let explicit_skip = complete_statement_without_database(
        4,
        &plans[0],
        false,
        Some(RollbackUnsupportedPolicy::Skip),
        None,
        Some(&callback),
    )
    .unwrap()
    .expect("unsupported skip must complete before database execution");
    assert_eq!(explicit_skip.skipped, Some(true));

    let subsequent_skip = complete_statement_without_database(
        5,
        &ProtectedStatement::ReadOnly,
        true,
        Some(RollbackUnsupportedPolicy::Skip),
        None,
        Some(&callback),
    )
    .unwrap()
    .expect("stopped batch must complete before database execution");
    assert_eq!(subsequent_skip.skipped, None);
    assert!(subsequent_skip
        .error
        .as_deref()
        .unwrap()
        .starts_with("Skipped because "));
    assert_eq!(*callback_indexes.lock().unwrap(), vec![4, 5]);
}

#[test]
fn protected_skip_callback_failure_stops_before_database_execution() {
    let queries = ["INSERT INTO users (id) SELECT id FROM staging"].map(str::to_string);
    let plans =
        plan_batch_for_rollback_with_policy(&queries, RollbackUnsupportedPolicy::Skip).unwrap();
    let callback_indexes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let callback_observed = callback_indexes.clone();
    let callback = move |index: usize, statement: &crate::models::BatchStatementResult| {
        callback_observed.lock().unwrap().push(index);
        assert!(!crate::audit_outbox::statement_was_executed(statement));
        Err("audit append failed".to_string())
    };

    let error = complete_statement_without_database(
        6,
        &plans[0],
        false,
        Some(RollbackUnsupportedPolicy::Skip),
        None,
        Some(&callback),
    )
    .unwrap_err();
    assert_eq!(error, "audit append failed");
    assert_eq!(*callback_indexes.lock().unwrap(), vec![6]);
}

#[test]
fn explicit_risk_execution_stays_on_the_pinned_transaction_connection() {
    let mut params = crate::models::ConnectionParams {
        rollback_protection_enabled: Some(true),
        rollback_unsupported_policy: Some(RollbackUnsupportedPolicy::ExecuteUnprotected),
        ..Default::default()
    };
    assert!(!should_use_rollback_guard(&params));

    params.transaction_context_id = Some("editor-tab-1".to_string());
    assert!(should_use_rollback_guard(&params));
}

#[test]
fn fail_closes_other_mysql_and_mariadb_write_families() {
    for sql in [
        "INSERT INTO users (id) VALUES (1) RETURNING id",
        "UPDATE users SET active = 0 ORDER BY id LIMIT 1",
        "DELETE FROM users ORDER BY id LIMIT 1",
        "WITH changed AS (SELECT 1) INSERT INTO users (id) VALUES (1)",
        "WITH changed AS (SELECT 1) DELETE FROM users WHERE id = 1",
        "MERGE INTO users USING staging ON users.id = staging.id WHEN MATCHED THEN UPDATE SET users.active = 0",
        "CREATE USER 'writer'@'%' IDENTIFIED BY 'secret'",
        "ALTER USER 'writer'@'%' ACCOUNT LOCK",
        "DROP USER 'writer'@'%'",
        "RENAME USER 'writer'@'%' TO 'archived'@'%'",
        "CREATE ROLE app_writer",
        "DROP ROLE app_writer",
        "SET ROLE app_writer",
        "CREATE SEQUENCE order_seq",
        "ALTER SEQUENCE order_seq RESTART WITH 100",
        "DROP SEQUENCE order_seq",
        "CREATE TABLESPACE app_space ADD DATAFILE 'app.ibd' ENGINE=InnoDB",
        "ALTER TABLESPACE app_space RENAME TO archived_space",
        "DROP TABLESPACE app_space",
        "CREATE SERVER remote_db FOREIGN DATA WRAPPER mysql OPTIONS (HOST 'db')",
        "ALTER SERVER remote_db OPTIONS (HOST 'db2')",
        "DROP SERVER remote_db",
        "ALTER DATABASE app CHARACTER SET utf8mb4",
        "ALTER EVENT purge_users DISABLE",
        "ALTER INSTANCE ROTATE INNODB MASTER KEY",
        "CREATE RESOURCE GROUP batch TYPE = USER",
        "ALTER RESOURCE GROUP batch VCPU = 0",
        "DROP RESOURCE GROUP batch",
        "CREATE SPATIAL REFERENCE SYSTEM 900000 NAME 'custom'",
        "DROP SPATIAL REFERENCE SYSTEM 900000",
        "IMPORT TABLE FROM 'users.sdi'",
        "CACHE INDEX users IN cache_one",
        "CHECK TABLE users FOR UPGRADE",
        "CHECKSUM TABLE users",
        "PURGE BINARY LOGS BEFORE NOW()",
        "RESET MASTER",
        "RESTART",
        "SHUTDOWN",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::BlockedUnsupported,
            "{sql}"
        );
    }
}

#[test]
fn classifies_supported_transaction_boundaries() {
    for (sql, expected) in [
        ("START TRANSACTION", TransactionPlan::Start),
        ("BEGIN", TransactionPlan::Start),
        ("BEGIN WORK", TransactionPlan::Start),
        ("COMMIT", TransactionPlan::Commit),
        ("COMMIT WORK", TransactionPlan::Commit),
        ("ROLLBACK", TransactionPlan::Rollback),
        ("ROLLBACK WORK", TransactionPlan::Rollback),
    ] {
        assert_eq!(
            plan_for_rollback(sql),
            Ok(ProtectedStatement::Transaction(expected)),
            "{sql}"
        );
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::SessionOnly,
            "{sql}"
        );
    }
}

#[test]
fn only_allows_exact_supported_transaction_boundaries() {
    for sql in [
        "START TRANSACTION READ ONLY",
        "START REPLICA",
        "BEGIN NOT ATOMIC",
        "COMMIT AND CHAIN",
        "ROLLBACK TO SAVEPOINT before_update",
        "SAVEPOINT before_update",
        "RELEASE SAVEPOINT before_update",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::BlockedUnsupported,
            "{sql}"
        );
    }
}

#[test]
fn validates_balanced_explicit_transaction_groups() {
    for sql in [
        vec![
            "START TRANSACTION",
            "UPDATE users SET name = 'Grace' WHERE id = 1",
            "COMMIT",
        ],
        vec![
            "BEGIN",
            "UPDATE users SET name = 'Grace' WHERE id = 1",
            "ROLLBACK",
        ],
        vec![
            "CREATE TABLE audit_log (id BIGINT PRIMARY KEY)",
            "START TRANSACTION",
            "UPDATE users SET name = 'Grace' WHERE id = 1",
            "COMMIT",
            "ALTER TABLE audit_log ADD COLUMN note VARCHAR(255)",
        ],
    ] {
        let plans = sql
            .iter()
            .map(|statement| plan_for_rollback(statement).unwrap())
            .collect::<Vec<_>>();
        assert!(validate_transaction_structure(&plans).is_ok(), "{sql:?}");
    }
}

#[test]
fn rejects_ambiguous_explicit_transaction_groups() {
    for sql in [
        vec!["COMMIT"],
        vec!["START TRANSACTION", "START TRANSACTION", "ROLLBACK"],
        vec![
            "START TRANSACTION",
            "UPDATE users SET name = 'Grace' WHERE id = 1",
        ],
        vec![
            "START TRANSACTION",
            "CREATE TABLE audit_log (id BIGINT PRIMARY KEY)",
            "COMMIT",
        ],
    ] {
        let plans = sql
            .iter()
            .map(|statement| plan_for_rollback(statement).unwrap())
            .collect::<Vec<_>>();
        assert!(validate_transaction_structure(&plans).is_err(), "{sql:?}");
    }
}

#[test]
fn pinned_transactions_can_span_multiple_run_all_batches() {
    let start_queries = ["START TRANSACTION"].map(str::to_string);
    let start_plans = start_queries
        .iter()
        .map(|statement| plan_for_rollback(statement).unwrap())
        .collect::<Vec<_>>();
    let opened =
        validate_pinned_transaction_structure(&start_plans, false, false, &start_queries).unwrap();
    assert!(opened.ends_active);

    let locked_read_queries =
        ["SELECT id, user_id FROM biz_customer_management WHERE id = 6083 FOR UPDATE"]
            .map(str::to_string);
    let locked_read_plans = locked_read_queries
        .iter()
        .map(|statement| plan_for_rollback(statement).unwrap())
        .collect::<Vec<_>>();
    let still_open = validate_pinned_transaction_structure(
        &locked_read_plans,
        opened.ends_active,
        false,
        &locked_read_queries,
    )
    .unwrap();
    assert!(still_open.ends_active);

    let commit_queries = ["COMMIT"].map(str::to_string);
    let commit_plans = commit_queries
        .iter()
        .map(|statement| plan_for_rollback(statement).unwrap())
        .collect::<Vec<_>>();
    let committed = validate_pinned_transaction_structure(
        &commit_plans,
        still_open.ends_active,
        false,
        &commit_queries,
    )
    .unwrap();
    assert!(!committed.ends_active);
}

#[test]
fn pinned_ddl_requires_acknowledgement_and_closes_the_boundary() {
    let queries = ["ALTER TABLE users ADD COLUMN note VARCHAR(255)"].map(str::to_string);
    let plans = queries
        .iter()
        .map(|statement| plan_for_rollback(statement).unwrap())
        .collect::<Vec<_>>();

    let review = validate_pinned_transaction_structure(&plans, true, false, &queries).unwrap_err();
    assert!(review.contains("TABULARIS_ROLLBACK_RISK_REVIEW:"));
    assert!(review.contains("\"kind\":\"implicit_commit\""));

    let accepted = validate_pinned_transaction_structure(&plans, true, true, &queries).unwrap();
    assert!(!accepted.ends_active);
    assert!(accepted.implicit_commit);
}

#[test]
fn allows_generated_tabularis_variables_without_rollback_steps() {
    for sql in [
        "SET @tabularis_environment_ok := ((DATABASE() <=> 'csr') AND (CURRENT_USER() <=> 'root@%'))",
        "SET @tabularis_environment_ok := ((CAST(DATABASE() AS BINARY) <=> 0x637372) AND (CAST(CURRENT_USER() AS BINARY) <=> 0x726F6F744025) AND (CAST(@@server_uuid AS BINARY) <=> 0x3435653137643961) AND (@@SESSION.foreign_key_checks = 1) AND (@@SESSION.unique_checks = 1))",
        "SET @tabularis_rollback_failed := FALSE",
        "SET @tabularis_statement_ok := IF(ROW_COUNT() = 1, @tabularis_rollback_failed, TRUE)",
        "SET @tabularis_rollback_failed := (@tabularis_rollback_failed OR (@tabularis_step_will_execute AND NOT (ROW_COUNT() <=> 1)))",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::SessionOnly,
            "{sql}"
        );
    }

    for sql in [
        "SET @user_id := 42",
        "SET @unsafe_result := mutate_users()",
        "SET @unsafe_result := (SELECT id FROM users INTO OUTFILE '/tmp/users.csv')",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::BlockedUnsupported,
            "{sql}"
        );
    }
}

#[test]
fn allows_read_only_and_user_variable_statements() {
    for sql in [
        "SELECT id FROM users",
        "WITH ids AS (SELECT id FROM users) SELECT * FROM ids",
        "SHOW CREATE TABLE users",
        "DESCRIBE users",
        "EXPLAIN SELECT * FROM users",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::ReadOnly,
            "{sql}"
        );
    }

    assert_eq!(
        classify_for_rollback("SET @user_id = 42").class,
        ProtectionClass::SessionOnly
    );
}

#[test]
fn ignores_comments_when_classifying() {
    assert_eq!(
        classify_for_rollback(
            "/* rollout */ -- update one row\n UPDATE `app`.`users` SET name = 'DML in text' WHERE id = 1"
        )
        .class,
        ProtectionClass::SupportedDml
    );
}

#[test]
fn preserves_identifier_case_and_insert_key_expressions() {
    let planned = plan_for_rollback(
        "INSERT INTO `AppDb`.`UserRecord` (`UserId`, `DisplayName`) VALUES (42, CONCAT('Ada', ' Lovelace'))",
    )
    .unwrap();
    let ProtectedStatement::Dml(DmlPlan::Insert(insert)) = planned else {
        panic!("expected an insert plan");
    };
    assert_eq!(insert.table.schema.as_deref(), Some("AppDb"));
    assert_eq!(insert.table.name, "UserRecord");
    assert_eq!(insert.columns, ["UserId", "DisplayName"]);
    assert_eq!(insert.rows[0][0], "42");
    assert_eq!(insert.rows[0][1], "CONCAT('Ada', ' Lovelace')");
}

#[test]
fn preserves_write_prefixes_for_locked_primary_key_intersection() {
    let update =
        plan_for_rollback("UPDATE `App`.`Users` SET `Score` = `Score` + 1 WHERE `Active` = 1")
            .unwrap();
    let ProtectedStatement::Dml(DmlPlan::Update(update)) = update else {
        panic!("expected an update plan");
    };
    assert_eq!(
        update.statement_prefix,
        "UPDATE `App`.`Users` SET `Score` = `Score` + 1"
    );
    assert_eq!(update.where_sql.as_deref(), Some("`Active` = 1"));

    let delete = plan_for_rollback("DELETE FROM `App`.`Users` WHERE `Expired` = 1").unwrap();
    let ProtectedStatement::Dml(DmlPlan::Delete(delete)) = delete else {
        panic!("expected a delete plan");
    };
    assert_eq!(delete.statement_prefix, "DELETE FROM `App`.`Users`");
    assert_eq!(delete.where_sql.as_deref(), Some("`Expired` = 1"));

    assert_eq!(
        locked_write_sql(
            "UPDATE users SET active = 0 -- keep parser boundary",
            Some("id = 1"),
            "`id` = 1",
        ),
        "UPDATE users SET active = 0 -- keep parser boundary\nWHERE (id = 1) AND (`id` = 1)"
    );
}

#[test]
fn blocks_mysql_executable_comments() {
    for sql in [
        "/*!50700 UPDATE users SET active = 0 WHERE id = 1 */",
        "/*M!100100 UPDATE users SET active = 0 WHERE id = 1 */",
        "/*m!100100 UPDATE users SET active = 0 WHERE id = 1 */",
    ] {
        let classified = classify_for_rollback(sql);
        assert_eq!(
            classified.class,
            ProtectionClass::BlockedUnsupported,
            "{sql}"
        );
        assert!(classified
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("executable comments")));
    }
}

#[test]
fn does_not_treat_double_minus_without_whitespace_as_a_comment() {
    let classified =
        classify_for_rollback("UPDATE users SET score = score--mutate_users() WHERE id = 1");
    assert_eq!(classified.class, ProtectionClass::BlockedUnsupported);
    assert!(classified
        .reason
        .as_deref()
        .is_some_and(|reason| reason.contains("hide writes")));
}

#[test]
fn blocks_sql_mode_dependent_quote_boundaries() {
    for sql in [
        r#"UPDATE users SET name = "Ada" WHERE id = 1"#,
        r#"UPDATE users SET name = 'Ada\'s' WHERE id = 1"#,
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::BlockedUnsupported,
            "{sql}"
        );
    }
}

#[test]
fn blocks_session_mutations_hidden_inside_expressions() {
    for sql in [
        "SELECT @seen := 1",
        "UPDATE users SET name = (@seen := 'changed') WHERE id = 1",
        "DELETE FROM users WHERE (@seen := id) = 1",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::BlockedUnsupported,
            "{sql}"
        );
    }
}

#[test]
fn blocks_external_select_writes_but_allows_normal_selects() {
    assert_eq!(
        classify_for_rollback("SELECT * FROM users INTO OUTFILE '/tmp/users.csv'").class,
        ProtectionClass::BlockedUnsupported
    );
    assert_eq!(
        classify_for_rollback("SELECT 'OUTFILE' AS harmless_text").class,
        ProtectionClass::ReadOnly
    );
    assert_eq!(
        classify_for_rollback("SELECT COUNT(*), COALESCE(name, '') FROM users").class,
        ProtectionClass::ReadOnly
    );
}

#[test]
fn blocks_unproven_stored_or_udf_calls_that_can_hide_writes() {
    for sql in [
        "SELECT mutate_users()",
        "SELECT app.mutate_users()",
        "SET @result = mutate_users()",
        "INSERT INTO users (id, name) VALUES (1, mutate_users())",
        "UPDATE users SET name = mutate_users() WHERE id = 1",
        "DELETE FROM users WHERE mutate_users(id) = 1",
    ] {
        let classified = classify_for_rollback(sql);
        assert_eq!(
            classified.class,
            ProtectionClass::BlockedUnsupported,
            "{sql}"
        );
        assert!(
            classified
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("hide writes")),
            "{sql}"
        );
    }
}
