use super::*;

/// Every test uses its own window key so the shared process-wide map cannot
/// make tests observe each other's state when cargo runs them in parallel.
fn window(name: &str) -> (String, String) {
    (format!("conn-{name}"), format!("tab-{name}"))
}

/// The statements that would be replayed, ignoring the undo metadata.
fn setup(connection_id: &str, context_id: &str) -> Option<Vec<String>> {
    preamble(connection_id, context_id).map(|state| state.setup)
}

#[test]
fn records_user_variable_and_replays_it() {
    let (conn, tab) = window("user-var");
    record_statement(&conn, &tab, "SET @cutoff = '2026-08-01'");
    assert_eq!(
        setup(&conn, &tab),
        Some(vec!["SET @cutoff = '2026-08-01'".to_string()])
    );
}

#[test]
fn later_assignment_supersedes_the_earlier_one() {
    let (conn, tab) = window("override");
    record_statement(&conn, &tab, "SET @cutoff = '2026-08-01'");
    record_statement(&conn, &tab, "SET @cutoff = '2026-08-17';");
    assert_eq!(
        setup(&conn, &tab),
        Some(vec!["SET @cutoff = '2026-08-17'".to_string()])
    );
}

#[test]
fn keeps_replay_order_for_different_targets() {
    let (conn, tab) = window("order");
    record_statement(&conn, &tab, "SET @a = 1");
    record_statement(&conn, &tab, "SET @b = @a + 1");
    assert_eq!(
        setup(&conn, &tab),
        Some(vec!["SET @a = 1".to_string(), "SET @b = @a + 1".to_string()])
    );
}

#[test]
fn multi_assignment_statement_supersedes_single_ones() {
    let (conn, tab) = window("multi");
    record_statement(&conn, &tab, "SET @a = 1");
    record_statement(&conn, &tab, "SET @b = 2");
    record_statement(&conn, &tab, "SET @a = 10, @b = 20");
    assert_eq!(
        setup(&conn, &tab),
        Some(vec!["SET @a = 10, @b = 20".to_string()])
    );
}

#[test]
fn partially_overlapping_statement_keeps_the_earlier_one() {
    let (conn, tab) = window("partial");
    record_statement(&conn, &tab, "SET @a = 1, @b = 2");
    record_statement(&conn, &tab, "SET @a = 9");
    assert_eq!(
        setup(&conn, &tab),
        Some(vec![
            "SET @a = 1, @b = 2".to_string(),
            "SET @a = 9".to_string()
        ])
    );
}

#[test]
fn windows_and_connections_are_isolated() {
    let (conn, tab) = window("isolation");
    record_statement(&conn, &tab, "SET @a = 1");
    assert_eq!(setup(&conn, "other-tab"), None);
    assert_eq!(setup("other-conn", &tab), None);
}

#[test]
fn clear_window_forgets_only_that_window() {
    let (conn, tab) = window("clear-window");
    record_statement(&conn, &tab, "SET @a = 1");
    record_statement(&conn, "second-tab", "SET @b = 2");
    clear_window(&conn, &tab);
    assert_eq!(setup(&conn, &tab), None);
    assert!(setup(&conn, "second-tab").is_some());
    clear_connection(&conn);
}

#[test]
fn clear_connection_forgets_every_window_of_that_connection() {
    let (conn, tab) = window("clear-conn");
    record_statement(&conn, &tab, "SET @a = 1");
    record_statement(&conn, "another", "SET @b = 2");
    assert_eq!(clear_connection(&conn), 2);
    assert_eq!(setup(&conn, &tab), None);
    assert_eq!(setup(&conn, "another"), None);
}

#[test]
fn overflow_drops_the_oldest_statements() {
    let (conn, tab) = window("overflow");
    for index in 0..(MAX_ENTRIES_PER_WINDOW + 5) {
        record_statement(&conn, &tab, &format!("SET @v{index} = {index}"));
    }
    let replayed = setup(&conn, &tab).expect("window has state");
    assert_eq!(replayed.len(), MAX_ENTRIES_PER_WINDOW);
    assert_eq!(replayed[0], "SET @v5 = 5");
    clear_connection(&conn);
}

#[test]
fn failed_or_unparsed_statements_are_never_recorded() {
    let (conn, tab) = window("ignored");
    for sql in [
        "SELECT 1",
        "SET GLOBAL max_connections = 500",
        "SET @@GLOBAL.max_connections = 500",
        "SET PERSIST max_connections = 500",
        "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
        "SET SESSION TRANSACTION ISOLATION LEVEL READ COMMITTED",
        "SET LOCAL work_mem = '64MB'",
        "SET PASSWORD FOR 'a'@'%' = 'x'",
        "SET ROLE admin",
        "SET autocommit = 0",
        "SET SESSION autocommit = 0",
        "SET @@session.autocommit = 0",
        "SET @a = 1; DROP TABLE t",
        "SET @a",
        // Connection charset cannot be restored on release, so it is not
        // replayed either.
        "SET NAMES utf8mb4 COLLATE utf8mb4_general_ci",
        "SET CHARACTER SET utf8mb4",
        "",
    ] {
        record_statement(&conn, &tab, sql);
        assert_eq!(setup(&conn, &tab), None, "unexpectedly recorded: {sql}");
    }
}

#[test]
fn preamble_splits_targets_into_undo_buckets() {
    let (conn, tab) = window("buckets");
    record_statement(&conn, &tab, "SET @cutoff = '2026-08-01'");
    record_statement(&conn, &tab, "SET SESSION sort_buffer_size = 262144");
    record_statement(&conn, &tab, "SET @cutoff = '2026-08-17'");
    let state = preamble(&conn, &tab).expect("window has state");
    assert_eq!(state.user_variables, vec!["@cutoff".to_string()]);
    assert_eq!(state.session_settings, vec!["sort_buffer_size".to_string()]);
    clear_connection(&conn);
}

#[test]
fn session_scope_qualifiers_normalise_to_one_target() {
    assert_eq!(
        replayable_targets("SET SESSION sort_buffer_size = 262144"),
        Some(vec!["sort_buffer_size".to_string()])
    );
    assert_eq!(
        replayable_targets("SET @@session.sort_buffer_size = 262144"),
        Some(vec!["sort_buffer_size".to_string()])
    );
    assert_eq!(
        replayable_targets("SET @@sort_buffer_size = 262144"),
        Some(vec!["sort_buffer_size".to_string()])
    );
}

#[test]
fn recognises_dialect_specific_assignment_forms() {
    assert_eq!(
        replayable_targets("SET @a := 1"),
        Some(vec!["@a".to_string()])
    );
    assert_eq!(
        replayable_targets("SET search_path TO analytics, public"),
        Some(vec!["search_path".to_string()])
    );
    assert_eq!(
        replayable_targets("SET search_path = analytics, public"),
        Some(vec!["search_path".to_string()])
    );
    assert_eq!(
        replayable_targets("set @A = 1"),
        Some(vec!["@a".to_string()])
    );
}

#[test]
fn quoted_and_nested_separators_do_not_split_assignments() {
    assert_eq!(
        replayable_targets("SET @list = 'a,b;c'"),
        Some(vec!["@list".to_string()])
    );
    assert_eq!(
        replayable_targets("SET @joined = CONCAT('a', 'b'), @n = 2"),
        Some(vec!["@joined".to_string(), "@n".to_string()])
    );
    assert_eq!(
        replayable_targets("SET @quote = 'it''s = fine'"),
        Some(vec!["@quote".to_string()])
    );
    assert_eq!(
        replayable_targets(r"SET @esc = 'a\'b, c'"),
        Some(vec!["@esc".to_string()])
    );
}

#[test]
fn leading_comments_and_terminators_are_ignored() {
    assert_eq!(
        replayable_targets("-- pin the window\nSET @a = 1;"),
        Some(vec!["@a".to_string()])
    );
    assert_eq!(
        replayable_targets("/* header */ SET @a = 1 ;  "),
        Some(vec!["@a".to_string()])
    );
}

#[test]
fn recorded_statement_is_stored_without_its_terminator() {
    let (conn, tab) = window("terminator");
    record_statement(&conn, &tab, "-- note\nSET @a = 1;\n");
    assert_eq!(setup(&conn, &tab), Some(vec!["SET @a = 1".to_string()]));
    clear_connection(&conn);
}
