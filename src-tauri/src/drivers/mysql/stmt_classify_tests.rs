use super::stmt_classify::{
    find_first_top_level_object_keyword, is_text_protocol_stmt, text_protocol_stmt_may_return_rows,
};

#[test]
fn find_first_top_level_object_keyword_skips_current_user_parentheses() {
    let stmt = "CURRENT_USER() FUNCTION sociedades_total() RETURNS INT RETURN 1";

    assert_eq!(
        find_first_top_level_object_keyword(stmt),
        Some("FUNCTION sociedades_total() RETURNS INT RETURN 1")
    );
}

#[test]
fn find_first_top_level_object_keyword_stops_at_view_before_body_keywords() {
    let stmt =
        "'root' @ 'localhost' VIEW v AS SELECT 'PROCEDURE' AS word UNION SELECT 'FUNCTION' AS word";

    assert_eq!(
        find_first_top_level_object_keyword(stmt),
        Some("VIEW v AS SELECT 'PROCEDURE' AS word UNION SELECT 'FUNCTION' AS word")
    );
}

#[test]
fn routes_repeated_whitespace_definer_routines_through_text_protocol() {
    for sql in [
        "CREATE  DEFINER=`root`@`localhost` PROCEDURE sociedades_close() SELECT 1;",
        "CREATE OR  REPLACE DEFINER=`root`@`localhost` FUNCTION sociedades_total() RETURNS INT RETURN 1;",
        "CREATE OR REPLACE  DEFINER=`root`@`localhost` FUNCTION sociedades_total() RETURNS INT RETURN 1;",
    ] {
        assert!(
            is_text_protocol_stmt(sql),
            "expected repeated-whitespace definer routine to route through text protocol: {sql}"
        );
    }
}

#[test]
fn keeps_repeated_whitespace_definer_views_out_of_text_protocol() {
    for sql in [
        "CREATE  DEFINER=`root`@`localhost` VIEW v AS SELECT 'PROCEDURE' AS word",
        "CREATE OR  REPLACE DEFINER=`root`@`localhost` VIEW v AS SELECT 'FUNCTION' AS word",
        "CREATE OR REPLACE  DEFINER=`root`@`localhost` VIEW v AS SELECT routine_name FROM routines",
    ] {
        assert!(
            !is_text_protocol_stmt(sql),
            "repeated-whitespace definer view must not route through text protocol: {sql}"
        );
    }
}

#[test]
fn routes_history_observed_mysql_1295_statements_through_text_protocol() {
    for sql in [
        "PREPARE workflow2_guard_stmt FROM @workflow2_guard_sql",
        "EXECUTE workflow2_guard_stmt",
        "DEALLOCATE PREPARE workflow2_guard_stmt",
        "USE `csr_sync_hub`",
        "SHOW WARNINGS",
        "CREATE EVENT IF NOT EXISTS mysql.ev_truncate_general_log_daily ON SCHEDULE EVERY 1 DAY DO TRUNCATE TABLE mysql.general_log",
        "DROP EVENT IF EXISTS mysql.ev_general_log_backup_7d",
        "RESIGNAL",
    ] {
        assert!(
            is_text_protocol_stmt(sql),
            "history-observed statement must use MySQL text protocol: {sql}"
        );
    }
}

#[test]
fn execute_statement_uses_result_stream_path() {
    assert!(text_protocol_stmt_may_return_rows(
        "-- dynamic SELECT\nEXECUTE workflow2_verify_stmt"
    ));
    assert!(!text_protocol_stmt_may_return_rows(
        "PREPARE stmt FROM @sql"
    ));
}
