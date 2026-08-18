#[cfg(test)]
mod tests {
    use crate::recovery_history::RecoveryObject;
    use crate::recovery_objects::parse_change_objects;

    /// `(kind, schema, name)` triples, which read far better in assertions
    /// than the struct literal does.
    fn objects(sql: &str) -> (String, Vec<(String, String, String)>) {
        let (op, objs) = parse_change_objects(sql);
        (
            op,
            objs.into_iter()
                .map(|RecoveryObject { kind, schema, name }| (kind, schema, name))
                .collect(),
        )
    }

    fn names(sql: &str) -> Vec<String> {
        parse_change_objects(sql)
            .1
            .into_iter()
            .map(|o| {
                if o.schema.is_empty() {
                    o.name
                } else if o.name.is_empty() {
                    o.schema
                } else {
                    format!("{}.{}", o.schema, o.name)
                }
            })
            .collect()
    }

    fn kinds(sql: &str) -> Vec<String> {
        parse_change_objects(sql).1.into_iter().map(|o| o.kind).collect()
    }

    // ---- baseline: shapes the previous parser already handled ------------

    #[test]
    fn simple_table_ddl_and_dml() {
        assert_eq!(objects("DROP TABLE app.users").0, "drop table");
        assert_eq!(names("DROP TABLE app.users"), ["app.users"]);
        assert_eq!(names("ALTER TABLE `app`.`users` ADD COLUMN x INT"), ["app.users"]);
        assert_eq!(names("CREATE TABLE IF NOT EXISTS users (id INT)"), ["users"]);
        assert_eq!(names("TRUNCATE TABLE app.audit"), ["app.audit"]);
        assert_eq!(names("INSERT INTO app.orders (id) VALUES (1)"), ["app.orders"]);
        assert_eq!(names("UPDATE app.orders SET status = 1 WHERE id = 2"), ["app.orders"]);
        assert_eq!(names("DELETE FROM app.orders WHERE id = 2"), ["app.orders"]);
        assert_eq!(names("REPLACE INTO app.orders (id) VALUES (1)"), ["app.orders"]);
        assert_eq!(names("INSERT IGNORE INTO orders (id) VALUES (1)"), ["orders"]);
    }

    #[test]
    fn database_objects_keep_their_kind() {
        assert_eq!(kinds("CREATE DATABASE analytics"), ["database"]);
        assert_eq!(names("DROP SCHEMA IF EXISTS analytics"), ["analytics"]);
    }

    // ---- gap 1: statements naming more than one object -------------------

    #[test]
    fn drop_table_lists_every_table() {
        assert_eq!(
            names("DROP TABLE IF EXISTS a, b, `app`.`c`"),
            ["a", "b", "app.c"]
        );
    }

    #[test]
    fn rename_table_covers_both_sides_of_every_pair() {
        // The old name has to be recreated and the new one removed, so a
        // restore that knows only one of them cannot undo the rename.
        assert_eq!(
            names("RENAME TABLE a TO b, app.c TO app.d"),
            ["a", "b", "app.c", "app.d"]
        );
    }

    #[test]
    fn alter_table_rename_to_covers_the_new_name() {
        assert_eq!(names("ALTER TABLE a RENAME TO b"), ["a", "b"]);
    }

    #[test]
    fn alter_table_rename_column_is_not_a_new_table() {
        assert_eq!(names("ALTER TABLE a RENAME COLUMN x TO y"), ["a"]);
        assert_eq!(names("ALTER TABLE a RENAME INDEX ix_a TO ix_b"), ["a"]);
    }

    #[test]
    fn multi_table_dml_names_every_table() {
        assert_eq!(
            names("UPDATE a JOIN b ON a.id = b.id SET a.x = 1"),
            ["a", "b"]
        );
        assert_eq!(
            names("DELETE a, b FROM a INNER JOIN b ON a.id = b.id WHERE a.x = 1"),
            ["a", "b"]
        );
        assert_eq!(names("DELETE FROM a USING a, b WHERE a.id = b.id"), ["a", "b"]);
        assert_eq!(names("UPDATE a, b SET a.x = b.y"), ["a", "b"]);
    }

    // ---- gap 2: object types that used to yield nothing ------------------

    #[test]
    fn routines_and_views_are_named() {
        assert_eq!(kinds("CREATE VIEW v AS SELECT 1"), ["view"]);
        assert_eq!(names("DROP VIEW IF EXISTS app.v1, app.v2"), ["app.v1", "app.v2"]);
        assert_eq!(kinds("DROP PROCEDURE IF EXISTS app.p_sync"), ["procedure"]);
        assert_eq!(names("DROP PROCEDURE IF EXISTS app.p_sync"), ["app.p_sync"]);
        assert_eq!(kinds("CREATE FUNCTION f() RETURNS INT RETURN 1"), ["function"]);
        assert_eq!(kinds("DROP EVENT IF EXISTS ev_nightly"), ["event"]);
    }

    #[test]
    fn create_procedure_with_definer_clause() {
        // Real dumps carry `DEFINER=`root`@`%`` between the verb and the kind.
        let sql = "CREATE DEFINER=`root`@`%` PROCEDURE `app`.`p_fix`(IN n INT) BEGIN SELECT n; END";
        assert_eq!(parse_change_objects(sql).0, "create procedure");
        assert_eq!(names(sql), ["app.p_fix"]);
    }

    #[test]
    fn create_view_with_algorithm_and_security_clauses() {
        let sql = "CREATE ALGORITHM=UNDEFINED DEFINER=`svc`@`10.0.0.1` SQL SECURITY DEFINER VIEW `app`.`v_orders` AS SELECT 1";
        assert_eq!(parse_change_objects(sql).0, "create view");
        assert_eq!(names(sql), ["app.v_orders"]);
    }

    #[test]
    fn trigger_names_both_the_trigger_and_its_table() {
        let sql = "CREATE TRIGGER trg_audit AFTER INSERT ON app.orders FOR EACH ROW INSERT INTO app.audit VALUES (1)";
        let (op, objs) = objects(sql);
        assert_eq!(op, "create trigger");
        assert_eq!(
            objs,
            [
                ("trigger".into(), String::new(), "trg_audit".into()),
                ("table".into(), "app".into(), "orders".into()),
            ]
        );
    }

    // ---- gap 3: the table is not in the token slot after the verb --------

    #[test]
    fn create_index_names_the_table_not_the_index() {
        assert_eq!(names("CREATE INDEX ix_created ON app.orders (created_at)"), ["app.orders"]);
        assert_eq!(
            names("CREATE UNIQUE INDEX ix_u ON `app`.`orders` (a, b)"),
            ["app.orders"]
        );
        assert_eq!(names("DROP INDEX ix_created ON app.orders"), ["app.orders"]);
    }

    #[test]
    fn load_data_names_the_target_table() {
        assert_eq!(
            names("LOAD DATA LOCAL INFILE '/tmp/x.csv' INTO TABLE app.orders"),
            ["app.orders"]
        );
        assert_eq!(
            names("LOAD DATA INFILE '/tmp/x.csv' REPLACE INTO TABLE orders FIELDS TERMINATED BY ','"),
            ["orders"]
        );
    }

    #[test]
    fn call_names_the_routine() {
        assert_eq!(kinds("CALL app.p_rebuild(1, 2)"), ["procedure"]);
        assert_eq!(names("CALL app.p_rebuild(1, 2)"), ["app.p_rebuild"]);
    }

    // ---- gap 4: lexical shapes that broke whitespace splitting -----------

    #[test]
    fn no_space_before_the_column_list() {
        assert_eq!(names("INSERT INTO app.orders(id,name) VALUES (1,'x')"), ["app.orders"]);
        assert_eq!(names("CREATE TABLE t(id INT)"), ["t"]);
    }

    #[test]
    fn newlines_tabs_and_comments_between_tokens() {
        let sql = "DROP\n\tTABLE /* staging */ IF EXISTS\n  app.tmp_orders;";
        assert_eq!(names(sql), ["app.tmp_orders"]);
        assert_eq!(names("-- header\nDELETE FROM app.orders WHERE id = 1"), ["app.orders"]);
        assert_eq!(names("# hash comment\nTRUNCATE app.audit"), ["app.audit"]);
    }

    #[test]
    fn executable_comments_are_lexed_not_skipped() {
        // `/*!40000 ALTER TABLE … */` really runs on MySQL, so the table has
        // to be reported rather than swallowed as a comment.
        assert_eq!(
            names("/*!40000 ALTER TABLE `app`.`orders` DISABLE KEYS */"),
            ["app.orders"]
        );
        assert_eq!(names("/*M!100301 DROP TABLE t */"), ["t"]);
    }

    #[test]
    fn quoted_names_survive_dots_and_spaces() {
        assert_eq!(names("DROP TABLE `my db`.`weird.name`"), ["my db.weird.name"]);
        assert_eq!(names("DROP TABLE `a``b`"), ["a`b"]);
        assert_eq!(names("DROP TABLE \"app\".\"orders\""), ["app.orders"]);
    }

    #[test]
    fn a_table_named_like_a_keyword_is_still_a_name() {
        assert_eq!(names("DROP TABLE `table`"), ["table"]);
        assert_eq!(names("TRUNCATE TABLE `select`"), ["select"]);
    }

    #[test]
    fn trailing_semicolon_and_clauses_do_not_leak_into_the_name() {
        assert_eq!(names("DROP TABLE app.orders;"), ["app.orders"]);
        assert_eq!(names("DROP TABLE app.orders RESTRICT;"), ["app.orders"]);
        assert_eq!(names("TRUNCATE TABLE orders ;"), ["orders"]);
    }

    // ---- reads must not be mistaken for writes ---------------------------

    #[test]
    fn a_source_table_read_by_the_statement_is_not_an_object() {
        // Restoring `src` over the top would undo unrelated data.
        assert_eq!(names("CREATE TABLE dst LIKE src"), ["dst"]);
        assert_eq!(names("CREATE TABLE dst AS SELECT * FROM src"), ["dst"]);
        assert_eq!(names("INSERT INTO dst SELECT * FROM src"), ["dst"]);
    }

    #[test]
    fn subquery_tables_are_stepped_over() {
        assert_eq!(
            names("UPDATE app.orders SET x = (SELECT MAX(y) FROM app.other) WHERE id = 1"),
            ["app.orders"]
        );
        assert_eq!(
            names("DELETE FROM app.orders WHERE id IN (SELECT id FROM app.stale)"),
            ["app.orders"]
        );
    }

    // ---- aliases must not truncate the table list ------------------------

    #[test]
    fn aliased_multi_table_dml_still_names_every_table() {
        // The alias keyword used to end the list, so only the first table
        // survived — and every earlier test happened to omit `AS`.
        assert_eq!(
            names("UPDATE t1 AS a JOIN t2 AS b ON a.id = b.id SET a.x = 1, b.y = 2"),
            ["t1", "t2"]
        );
        assert_eq!(
            names("DELETE t1, t2 FROM t1 AS a INNER JOIN t2 AS b ON a.id = b.id"),
            ["t1", "t2"]
        );
        assert_eq!(names("UPDATE app.orders AS o SET o.x = 1"), ["app.orders"]);
    }

    // ---- statements whose verb hides the write ---------------------------

    #[test]
    fn cte_prefixed_writes_name_their_target() {
        assert_eq!(
            names("WITH c AS (SELECT 1) UPDATE app.orders SET x = 1"),
            ["app.orders"]
        );
        assert_eq!(
            names("WITH c AS (SELECT id FROM src) DELETE FROM app.orders WHERE id IN (SELECT id FROM c)"),
            ["app.orders"]
        );
        assert_eq!(
            names("WITH RECURSIVE c (n) AS (SELECT 1) INSERT INTO app.audit SELECT * FROM c"),
            ["app.audit"]
        );
    }

    #[test]
    fn maintenance_ddl_names_the_table() {
        // REPAIR can drop rows, so it is exactly the case that needs a target.
        assert_eq!(names("OPTIMIZE TABLE app.orders"), ["app.orders"]);
        assert_eq!(names("REPAIR TABLE app.orders"), ["app.orders"]);
        assert_eq!(names("ANALYZE TABLE app.orders"), ["app.orders"]);
        assert_eq!(names("OPTIMIZE NO_WRITE_TO_BINLOG TABLE a, b"), ["a", "b"]);
    }

    #[test]
    fn exchange_partition_names_both_tables() {
        assert_eq!(
            names("ALTER TABLE app.a EXCHANGE PARTITION p0 WITH TABLE app.b"),
            ["app.a", "app.b"]
        );
    }

    // ---- dynamic SQL is recognised for runtime resolution ----------------

    #[test]
    fn prepare_from_a_user_variable_is_recognised() {
        use crate::recovery_objects::{dynamic_source, DynamicSource};
        assert_eq!(
            dynamic_source("PREPARE release_stmt FROM @ddl"),
            Some(DynamicSource::UserVariable {
                statement: "release_stmt".into(),
                variable: "ddl".into(),
            })
        );
        assert_eq!(
            dynamic_source("PREPARE `stmt_guard` FROM @instance_guard_sql"),
            Some(DynamicSource::UserVariable {
                statement: "stmt_guard".into(),
                variable: "instance_guard_sql".into(),
            })
        );
    }

    #[test]
    fn prepare_from_a_literal_exposes_its_sql() {
        use crate::recovery_objects::{dynamic_source, DynamicSource};
        assert_eq!(
            dynamic_source("PREPARE s FROM 'DROP TABLE app.tmp'"),
            Some(DynamicSource::Literal {
                statement: "s".into(),
                sql: "DROP TABLE app.tmp".into(),
            })
        );
        // The literal then parses like any other statement.
        assert_eq!(names("DROP TABLE app.tmp"), ["app.tmp"]);
    }

    #[test]
    fn execute_and_call_are_recognised() {
        use crate::recovery_objects::{dynamic_source, DynamicSource};
        assert_eq!(
            dynamic_source("EXECUTE release_stmt"),
            Some(DynamicSource::Execute {
                statement: "release_stmt".into()
            })
        );
        // The helper-procedure pattern: the target table is an argument.
        assert_eq!(
            dynamic_source("CALL add_column_if_missing('sys_backend_menus', 'title', 'VARCHAR(128)')"),
            Some(DynamicSource::Call {
                schema: String::new(),
                routine: "add_column_if_missing".into(),
                string_args: vec![
                    "sys_backend_menus".into(),
                    "title".into(),
                    "VARCHAR(128)".into()
                ],
            })
        );
        assert_eq!(dynamic_source("UPDATE t SET x = 1"), None);
    }

    #[test]
    fn doubled_quotes_inside_a_call_argument_are_decoded() {
        use crate::recovery_objects::{dynamic_source, DynamicSource};
        // Real scripts pass DDL fragments containing '' escapes.
        let Some(DynamicSource::Call { string_args, .. }) = dynamic_source(
            "CALL add_column_if_missing('sys_backend_menus', 'title', 'VARCHAR(128) NOT NULL DEFAULT ''''')",
        ) else {
            panic!("expected a CALL");
        };
        assert_eq!(string_args[2], "VARCHAR(128) NOT NULL DEFAULT ''");
    }

    // ---- temporary tables are session-scoped -----------------------------

    #[test]
    fn temporary_tables_are_tagged_so_restore_can_skip_them() {
        assert_eq!(kinds("CREATE TEMPORARY TABLE tmp_x (id INT)"), ["temporary table"]);
        assert_eq!(kinds("DROP TEMPORARY TABLE IF EXISTS tmp_x"), ["temporary table"]);
        assert_eq!(parse_change_objects("CREATE TEMPORARY TABLE tmp_x (id INT)").0, "create temporary table");
        // A plain table must not be tagged.
        assert_eq!(kinds("CREATE TABLE t (id INT)"), ["table"]);
    }

    // ---- degenerate input must not panic ---------------------------------

    #[test]
    fn malformed_input_returns_empty_instead_of_panicking() {
        for sql in [
            "",
            "   ",
            ";",
            "-- only a comment",
            "/* unterminated",
            "DROP TABLE",
            "DROP TABLE `unterminated",
            "SELECT 1",
            "SET @x = 1",
            "GRANT SELECT ON *.* TO 'u'@'%'",
        ] {
            let _ = parse_change_objects(sql);
        }
        assert_eq!(parse_change_objects("").0, "");
        assert_eq!(parse_change_objects("GRANT SELECT ON *.* TO 'u'@'%'").0, "grant");
    }
}
