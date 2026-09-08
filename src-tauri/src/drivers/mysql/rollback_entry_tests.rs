use super::*;
use crate::models::RollbackUnsupportedPolicy;

fn params() -> ConnectionParams {
    ConnectionParams {
        driver: "mysql".into(),
        host: Some("protected-tests.example.invalid".into()),
        port: Some(3306),
        username: Some("readonly_fixture".into()),
        rollback_protection_enabled: Some(true),
        ..Default::default()
    }
}

#[test]
fn legacy_policy_cannot_disable_strict_routing() {
    let mut p = params();
    p.rollback_unsupported_policy = Some(RollbackUnsupportedPolicy::ExecuteUnprotected);
    assert!(should_use_rollback_guard(&p));
    p.transaction_context_id = Some("fixture-context".into());
    assert!(should_use_rollback_guard(&p));
    p.rollback_protection_enabled = Some(false);
    assert!(
        !should_use_rollback_guard(&p),
        "only an explicit opt-out disables protection"
    );
}

#[tokio::test]
async fn unsupported_batch_and_direct_queries_fail_before_acquiring_a_connection() {
    let sql = "INSERT INTO audit_fixture.items (id) SELECT id FROM audit_fixture.source";
    for pinned in [false, true] {
        let mut p = params();
        if pinned {
            p.transaction_context_id = Some("fixture-context".into());
        }
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            execute_batch(&p, &[sql.to_string()], None, 1, None, None),
        )
        .await
        .expect("preflight must not attempt DNS or a database connection");
        assert!(result
            .unwrap_err()
            .contains("Strict rollback protection refused"));
    }
    let result = execute_query(&params(), sql, None, 1, None).await;
    assert!(result
        .unwrap_err()
        .contains("Strict rollback protection refused"));
    let hidden_read = execute_query(&params(), "SELECT hidden_write()", None, 1, None).await;
    assert!(hidden_read
        .unwrap_err()
        .contains("Strict rollback protection refused"));
}

#[tokio::test]
async fn grid_mutations_are_refused_before_connection_or_value_decoding() {
    let p = params();
    let key = HashMap::from([("id".to_string(), serde_json::json!(1))]);
    let insert = insert_record(&p, "items", HashMap::new(), 1024)
        .await
        .unwrap_err();
    let update = update_record(&p, "items", &key, "v", serde_json::json!(2), 1024)
        .await
        .unwrap_err();
    let delete = delete_record(&p, "items", &key).await.unwrap_err();
    for error in [insert, update, delete] {
        assert!(error.contains("Strict rollback protection refused grid mutation"));
    }
}
