#[cfg(test)]
mod tests {
    use crate::explain_import::{
        detect_format, parse_explain, parse_postgres_json, parse_postgres_text,
        ExplainSourceFormat, PendingExplainFile,
    };

    const POSTGRES_SIMPLE: &str = r#"[
      {
        "Plan": {
          "Node Type": "Seq Scan",
          "Relation Name": "users",
          "Startup Cost": 0.00,
          "Total Cost": 12.34,
          "Plan Rows": 100,
          "Plan Width": 80
        },
        "Planning Time": 0.123,
        "Execution Time": 4.56
      }
    ]"#;

    const POSTGRES_NESTED: &str = r#"[
      {
        "Plan": {
          "Node Type": "CTE Scan",
          "Startup Cost": 1.0,
          "Total Cost": 2.0,
          "Actual Rows": 11,
          "Actual Total Time": 2.089,
          "Actual Loops": 1,
          "Plans": [
            {
              "Node Type": "Seq Scan",
              "Relation Name": "orders",
              "Actual Rows": 11
            },
            {
              "Node Type": "Hash Join",
              "Join Type": "Inner",
              "Hash Cond": "(o.id = u.id)",
              "Filter": "(u.active)"
            }
          ]
        }
      }
    ]"#;

    #[test]
    fn detect_format_accepts_array() {
        assert_eq!(
            detect_format(" \n [\n  {} ]").unwrap(),
            ExplainSourceFormat::PostgresJson
        );
    }

    #[test]
    fn detect_format_accepts_object() {
        assert_eq!(
            detect_format("{ \"Plan\": {} }").unwrap(),
            ExplainSourceFormat::PostgresJson
        );
    }

    #[test]
    fn detect_format_rejects_plain_text() {
        assert!(detect_format("Seq Scan on users  (cost=0.00..12.34 rows=100)").is_err());
    }

    #[test]
    fn parse_postgres_json_flat_node() {
        let plan = parse_postgres_json(POSTGRES_SIMPLE).expect("should parse");
        assert_eq!(plan.driver, "postgres");
        assert_eq!(plan.root.node_type, "Seq Scan");
        assert_eq!(plan.root.relation.as_deref(), Some("users"));
        assert_eq!(plan.root.total_cost, Some(12.34));
        assert_eq!(plan.planning_time_ms, Some(0.123));
        assert_eq!(plan.execution_time_ms, Some(4.56));
        assert!(!plan.has_analyze_data);
        assert!(plan.raw_output.is_some());
        assert!(plan.root.children.is_empty());
        assert!(plan.root.extra.contains_key("Plan Width"));
    }

    #[test]
    fn parse_postgres_json_preserves_tree_and_flags() {
        let plan = parse_postgres_json(POSTGRES_NESTED).expect("should parse");
        assert_eq!(plan.root.children.len(), 2);
        assert!(
            plan.has_analyze_data,
            "analyze flag derived from Actual Rows"
        );
        assert_eq!(plan.root.actual_loops, Some(1));

        let first_child = &plan.root.children[0];
        assert_eq!(first_child.node_type, "Seq Scan");
        assert_eq!(first_child.relation.as_deref(), Some("orders"));

        let second_child = &plan.root.children[1];
        assert_eq!(second_child.node_type, "Hash Join");
        assert_eq!(second_child.join_type.as_deref(), Some("Inner"));
        assert_eq!(
            second_child.hash_condition.as_deref(),
            Some("(o.id = u.id)")
        );
        assert_eq!(second_child.filter.as_deref(), Some("(u.active)"));
    }

    #[test]
    fn parse_postgres_json_assigns_unique_ids() {
        let plan = parse_postgres_json(POSTGRES_NESTED).expect("should parse");
        let mut ids = vec![plan.root.id.clone()];
        for child in &plan.root.children {
            ids.push(child.id.clone());
        }
        let unique: std::collections::HashSet<_> = ids.iter().cloned().collect();
        assert_eq!(unique.len(), ids.len(), "node ids must be unique");
    }

    #[test]
    fn parse_explain_rejects_empty_array() {
        let err = parse_explain("[]").expect_err("empty array should fail");
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn parse_explain_rejects_missing_plan() {
        let err = parse_explain("[{\"NotAPlan\": 1}]").expect_err("missing Plan key");
        assert!(err.contains("Plan"), "got: {err}");
    }

    #[test]
    fn parse_explain_rejects_invalid_json() {
        let err = parse_explain("[not json]").expect_err("should fail on invalid json");
        assert!(err.contains("Failed to parse"), "got: {err}");
    }

    #[test]
    fn parse_explain_rejects_unsupported_format() {
        let err = parse_explain("-> Nested Loop  (cost=0.00..1.23)").expect_err("unsupported");
        assert!(err.contains("Unsupported"), "got: {err}");
    }

    #[test]
    fn parse_postgres_json_accepts_single_object() {
        let raw = r#"{ "Plan": { "Node Type": "Result" } }"#;
        let plan = parse_postgres_json(raw).expect("single object is allowed");
        assert_eq!(plan.root.node_type, "Result");
    }

    // ---- Postgres text format ----

    const POSTGRES_TEXT_FLAT: &str = "\
 Seq Scan on users  (cost=0.00..12.34 rows=100 width=80)
 Planning Time: 0.123 ms
";

    const POSTGRES_TEXT_ANALYZE: &str = "\
QUERY PLAN
-----------------------------------------------------------------------------
 Hash Join  (cost=1.00..10.00 rows=5 width=40) (actual time=0.10..0.20 rows=5 loops=1)
   Hash Cond: (a.id = b.id)
   ->  Seq Scan on a  (cost=0.00..5.00 rows=100 width=4) (actual time=0.01..0.05 rows=100 loops=1)
         Filter: (a.active)
   ->  Hash  (cost=0.50..0.50 rows=1 width=36) (actual time=0.02..0.02 rows=1 loops=1)
         ->  Seq Scan on b  (cost=0.00..0.50 rows=1 width=36) (actual time=0.01..0.01 rows=1 loops=1)
 Planning Time: 0.123 ms
 Execution Time: 0.456 ms
(7 rows)
";

    #[test]
    fn detect_format_recognises_text_output() {
        assert_eq!(
            detect_format(POSTGRES_TEXT_FLAT).unwrap(),
            ExplainSourceFormat::PostgresText
        );
    }

    #[test]
    fn parse_postgres_text_flat_node() {
        let plan = parse_postgres_text(POSTGRES_TEXT_FLAT).expect("should parse");
        assert_eq!(plan.driver, "postgres");
        assert_eq!(plan.root.node_type, "Seq Scan");
        assert_eq!(plan.root.relation.as_deref(), Some("users"));
        assert_eq!(plan.root.startup_cost, Some(0.0));
        assert_eq!(plan.root.total_cost, Some(12.34));
        assert_eq!(plan.root.plan_rows, Some(100.0));
        assert_eq!(plan.planning_time_ms, Some(0.123));
        assert_eq!(plan.execution_time_ms, None);
        assert!(!plan.has_analyze_data);
    }

    #[test]
    fn parse_postgres_text_analyze_tree() {
        let plan = parse_postgres_text(POSTGRES_TEXT_ANALYZE).expect("should parse");

        assert!(plan.has_analyze_data);
        assert_eq!(plan.planning_time_ms, Some(0.123));
        assert_eq!(plan.execution_time_ms, Some(0.456));
        assert_eq!(plan.root.node_type, "Hash Join");
        assert_eq!(plan.root.hash_condition.as_deref(), Some("(a.id = b.id)"));
        assert_eq!(plan.root.actual_rows, Some(5.0));
        assert_eq!(plan.root.actual_loops, Some(1));
        assert_eq!(plan.root.actual_time_ms, Some(0.20));

        assert_eq!(plan.root.children.len(), 2);

        let seq = &plan.root.children[0];
        assert_eq!(seq.node_type, "Seq Scan");
        assert_eq!(seq.relation.as_deref(), Some("a"));
        assert_eq!(seq.filter.as_deref(), Some("(a.active)"));

        let hash = &plan.root.children[1];
        assert_eq!(hash.node_type, "Hash");
        assert_eq!(hash.children.len(), 1);
        assert_eq!(hash.children[0].relation.as_deref(), Some("b"));
    }

    #[test]
    fn parse_postgres_text_skips_header_and_footer() {
        let raw = "\
QUERY PLAN
-----------
 Result  (cost=0.00..0.01 rows=1 width=4)
(1 row)
";
        let plan = parse_postgres_text(raw).expect("should parse");
        assert_eq!(plan.root.node_type, "Result");
        assert!(plan.root.children.is_empty());
    }

    #[test]
    fn parse_postgres_text_keeps_using_index_modifier() {
        let raw = "\
 Index Scan using users_pkey on users  (cost=0.00..8.00 rows=1 width=80)
";
        let plan = parse_postgres_text(raw).expect("should parse");
        assert_eq!(plan.root.node_type, "Index Scan using users_pkey");
        assert_eq!(plan.root.relation.as_deref(), Some("users"));
    }

    #[test]
    fn parse_postgres_text_rejects_no_plan() {
        let err = parse_postgres_text("QUERY PLAN\n---\n(0 rows)\n")
            .expect_err("no plan → error");
        assert!(err.contains("No plan nodes"), "got: {err}");
    }

    #[test]
    fn parse_explain_routes_to_text_parser() {
        let plan = parse_explain(POSTGRES_TEXT_FLAT).expect("should parse");
        assert_eq!(plan.root.node_type, "Seq Scan");
    }

    #[test]
    fn pending_explain_file_take_clears_slot() {
        let state = PendingExplainFile::default();
        state.set("/tmp/foo.json".to_string());
        assert_eq!(state.take(), Some("/tmp/foo.json".to_string()));
        assert_eq!(state.take(), None);
    }
}
