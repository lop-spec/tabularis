use super::rollback_guard::{
    classify_for_rollback, complete_statement_without_database, is_generated_column,
    locked_write_sql, plan_batch_for_rollback, plan_batch_for_rollback_with_policy,
    plan_for_rollback, review_batch_for_rollback, validate_pinned_transaction_structure,
    validate_transaction_structure, DmlPlan, ProtectedStatement, ProtectionClass, SessionPlan,
    TemporaryPlan, TransactionPlan,
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
    // A bare function name is indistinguishable from a built-in without a
    // server metadata lookup, so SELECT keeps the permissive read path.
    assert_eq!(
        classify_for_rollback("SELECT mutate_users()").class,
        ProtectionClass::ReadOnly
    );
    for sql in [
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

#[test]
fn online_ddl_execution_options_do_not_make_an_alter_multi_clause() {
    // How every online DDL in a real change script is written. The trailing
    // comma before ALGORITHM/LOCK used to read as "multi-clause", pushing a
    // plain ADD COLUMN onto the whole-table-rebuild recovery path.
    for sql in [
        "ALTER TABLE `csr`.`biz_shop` ADD COLUMN `note` varchar(64) NULL, ALGORITHM=INSTANT",
        "ALTER TABLE users ADD COLUMN note VARCHAR(255), ALGORITHM = INPLACE, LOCK = NONE",
        "ALTER TABLE users ADD INDEX users_name_idx (name), ALGORITHM=INPLACE, LOCK=NONE",
        "ALTER TABLE users ADD COLUMN note VARCHAR(255), LOCK=NONE",
        "ALTER TABLE users ADD COLUMN note VARCHAR(255), ALGORITHM=COPY, WITH VALIDATION",
    ] {
        assert_eq!(
            classify_for_rollback(sql).class,
            ProtectionClass::SupportedDdl,
            "{sql}"
        );
    }
}

#[test]
fn a_genuine_multi_clause_alter_is_still_fail_closed() {
    // Two real column actions must stay blocked: only the execution hints
    // are allowed to sit behind a top-level comma.
    for sql in [
        "ALTER TABLE users ADD COLUMN a INT, ADD COLUMN b INT",
        "ALTER TABLE users ADD COLUMN a INT, DROP COLUMN b",
        "ALTER TABLE users MODIFY COLUMN a INT, MODIFY COLUMN b INT",
        "ALTER TABLE users ADD COLUMN a INT, ALGORITHM=INSTANT, ADD COLUMN b INT",
    ] {
        assert_ne!(
            classify_for_rollback(sql).class,
            ProtectionClass::SupportedDdl,
            "{sql}"
        );
    }
}

#[test]
fn a_column_list_comma_is_not_a_clause_separator() {
    // Commas inside parentheses were already handled; keep them that way.
    assert_eq!(
        classify_for_rollback("ALTER TABLE users ADD INDEX ix (a, b, c), ALGORITHM=INPLACE").class,
        ProtectionClass::SupportedDdl
    );
}

#[test]
fn default_current_timestamp_is_not_a_generated_column() {
    // MySQL puts EXTRA='DEFAULT_GENERATED' on any column declared
    // `DEFAULT CURRENT_TIMESTAMP`, and appends 'on update CURRENT_TIMESTAMP'
    // when that is set too. Testing EXTRA for the substring "GENERATED"
    // therefore classified ordinary created_at / updated_at columns as
    // generated, and an UPDATE assigning to one could not be protected at
    // all — lop hit this on biz_kb_qa_v2_migration.updated_at, whose real
    // GENERATION_EXPRESSION is empty.
    for expression in ["", "   "] {
        assert!(!is_generated_column(expression), "expression={expression:?}");
    }
    for expression in ["(`a` + `b`)", "(json_extract(`doc`,'$.id'))"] {
        assert!(is_generated_column(expression), "expression={expression:?}");
    }
}

/// lop 2026-08-19：「for update，for share 这类也要支持上」「很多 set 变量和
/// select 这种都不要提示」。两个诉求同源——回滚守卫把不需要逆向 SQL 的语句
/// 判成了 unsupported，前端据此弹出「不可回滚」确认框。
#[test]
fn reads_and_session_statements_do_not_need_confirmation() {
    // 锁定读：在调用方事务里取行锁，不改数据，没有可逆向的东西
    for sql in [
        "SELECT id FROM t WHERE id = 1 FOR UPDATE",
        "SELECT id FROM t WHERE id = 1 FOR SHARE",
        "SELECT id FROM t WHERE id = 1 LOCK IN SHARE MODE",
        "SELECT id FROM t WHERE id = 1 FOR UPDATE NOWAIT",
        "SELECT id FROM t WHERE id = 1 FOR UPDATE SKIP LOCKED",
    ] {
        assert_eq!(plan_for_rollback(sql), Ok(ProtectedStatement::ReadOnly), "{sql}");
    }

    // 读语句不再跑「已证明无副作用」函数白名单：那份名单有 193 项，仍然漏掉
    // GROUP_CONCAT / ROW_NUMBER / LAG / MD5 / UUID，普通报表查询因此被拒。
    // 读没有回滚文件，白名单在这里本来就无从谈起。
    for sql in [
        "SELECT GROUP_CONCAT(name) FROM t",
        "SELECT ROW_NUMBER() OVER (ORDER BY id) FROM t",
        "SELECT LAG(v) OVER (ORDER BY id) FROM t",
        "SELECT MD5(name) FROM t",
        "SELECT UUID()",
        // lop 历史里真实撞到的三个
        "SELECT REGEXP_LIKE(content, 'x') FROM t",
        "SELECT REGEXP_REPLACE(content, 'a', 'b') FROM t",
        "SELECT GROUP_CONCAT(DISTINCT name ORDER BY id SEPARATOR ',') FROM t",
    ] {
        assert_eq!(plan_for_rollback(sql), Ok(ProtectedStatement::ReadOnly), "{sql}");
    }

    // 会话语句：这些形态在 lop 的执行历史里被反复拦下（`SET NAMES utf8mb4`、
    // `SET SESSION lock_wait_timeout`、`SET SESSION sql_safe_updates` 各若干次），
    // 而一条被拦会让整批失败，连累后面所有语句。
    for sql in [
        "SET @a = 1",
        "SET NAMES utf8mb4",
        "SET CHARACTER SET utf8mb4",
        "SET SESSION lock_wait_timeout = 10",
        "SET SESSION sql_safe_updates = 1",
        "SET SESSION sql_mode = 'STRICT_TRANS_TABLES'",
        "SET @@session.time_zone = '+08:00'",
        "SET sql_mode = ''",
        "SET time_zone = '+00:00'",
        "SET foreign_key_checks = 0",
        "SET group_concat_max_len = 102400",
        "SET SESSION group_concat_max_len = 102400, sql_mode = ''",
    ] {
        assert!(
            matches!(plan_for_rollback(sql), Ok(ProtectedStatement::Session(_))),
            "{sql} -> {:?}",
            plan_for_rollback(sql)
        );
    }
}

/// 反向断言：放宽不能把真正危险的形态也放过去，否则上面那些「不提示」毫无意义。
#[test]
fn dangerous_statements_are_still_refused() {
    // autocommit=1 会让每条语句立即提交，回滚保护形同虚设——这是唯一真正
    // 破坏回滚语义的 SET
    for sql in [
        "SET autocommit = 1",
        "SET SESSION autocommit = 1",
        "SET @@session.autocommit = 0",
    ] {
        assert!(plan_for_rollback(sql).is_err(), "{sql} 必须被拒");
    }

    // 影响其他连接、身份，或事务特性本身
    for sql in [
        "SET GLOBAL max_connections = 1000",
        "SET @@global.sql_mode = ''",
        "SET PERSIST max_connections = 1000",
        "SET PERSIST_ONLY max_connections = 1000",
        "SET PASSWORD FOR 'u'@'%' = 'x'",
        "SET ROLE admin",
        "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
        "SET SESSION TRANSACTION READ ONLY",
    ] {
        assert!(plan_for_rollback(sql).is_err(), "{sql} 必须被拒");
    }

    // 外部写副作用：去掉函数白名单后这条是读路径上仅存的拦截，必须还在
    for sql in [
        "SELECT * FROM t INTO OUTFILE '/tmp/x.csv'",
        "SELECT * FROM t INTO DUMPFILE '/tmp/x.bin'",
    ] {
        assert!(plan_for_rollback(sql).is_err(), "{sql} 必须被拒");
    }

    // 读语句仍然拒绝**可识别的**存储函数：它们能写表，而那些写不进回滚文件。
    // 判据是 schema 限定或反引号，不是名单——内置函数不可能长成这两种样子。
    for sql in [
        "SELECT app.mutate_users(id) FROM t",
        "SELECT `mutate_users`(id) FROM t",
    ] {
        assert!(plan_for_rollback(sql).is_err(), "{sql} 必须被拒");
    }

    // 写语句仍然要过完整白名单：它们有回滚文件，藏在函数里的写会让逆向 SQL 失真
    assert!(
        plan_for_rollback("UPDATE t SET v = my_custom_udf(1) WHERE id = 1").is_err(),
        "写语句里的未证明函数必须被拒"
    );
}

/// lop 2026-08-20：「我很早之前有让兼容 use database runall，现在还不支持呢」。
///
/// `USE db` 与写操作同批曾被整批拒绝，理由是回滚文件里的非限定表名会在恢复时
/// 解析到别的库。实际不是这样：两条路径都在**语句执行那一刻**用连接当时的
/// database 补全库名——DML 经 `load_table_metadata` 填 `TableMetadata.schema`
/// （`String` 而非 `Option`）后用 `qualified_name()`，DDL 经 `resolve_object`
/// 补全后再 `quoted()`。所以中途切库反而正是 USE + Run All 需要的行为。
#[test]
fn use_database_can_share_a_batch_with_writes() {
    for queries in [
        // lop 执行历史里被拒的真实形态
        vec![
            "USE `csr`",
            "UPDATE sys_backend_menus SET title = 'x' WHERE id = 1",
        ],
        vec![
            "USE csr",
            "INSERT INTO t (id) VALUES (1)",
            "DELETE FROM t WHERE id = 1",
        ],
        // 跨库：切一次再切一次，每条语句按它当时的库补全
        vec![
            "USE db_a",
            "UPDATE t SET v = 1 WHERE id = 1",
            "USE db_b",
            "UPDATE t SET v = 2 WHERE id = 1",
        ],
        // USE 与 DDL 同批
        vec!["USE csr", "CREATE TABLE audit_log (id BIGINT PRIMARY KEY)"],
    ] {
        let owned = queries.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(
            plan_batch_for_rollback(&owned).is_ok(),
            "USE 与写同批必须放行: {queries:?}"
        );
    }
}

/// 反向断言：USE 本身仍必须被识别为改变作用域的会话语句，而不是被顺手降级成
/// 普通设置。它与 `SET SESSION ...` 的区别正在于此——前者改变非限定名解析到
/// 哪个库，后者不改。判据退化时这条会红。
#[test]
fn use_is_still_classified_as_a_scope_change() {
    for sql in ["USE csr", "USE `csr_sync_hub`"] {
        assert_eq!(
            plan_for_rollback(sql),
            Ok(ProtectedStatement::Session(SessionPlan::ScopeChange)),
            "{sql}"
        );
    }
    // SET 走 Setting，不是 ScopeChange：两者混用会让 USE 的判据失去意义
    for sql in ["SET NAMES utf8mb4", "SET SESSION sql_mode = ''"] {
        assert_eq!(
            plan_for_rollback(sql),
            Ok(ProtectedStatement::Session(SessionPlan::Setting)),
            "{sql}"
        );
    }
}
