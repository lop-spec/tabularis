use super::*;
use crate::models::{ConnectionParams, DatabaseSelection, SavedConnection};
use serde_json::Value;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Clone, Copy)]
enum AckMode {
    Complete,
    Partial,
    HttpFailure,
}

async fn spawn_ingest_server(
    expected_requests: usize,
    mode: AckMode,
) -> (String, tokio::task::JoinHandle<Vec<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind audit ingest test server");
    let address = listener.local_addr().expect("test server address");
    let handle = tokio::spawn(async move {
        let mut request_batches = Vec::with_capacity(expected_requests);
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().await.expect("accept audit request");
            let mut request = Vec::new();
            let header_end = loop {
                let mut chunk = [0_u8; 4096];
                let count = stream.read(&mut chunk).await.expect("read audit request");
                assert!(count > 0, "audit request ended before its headers");
                request.extend_from_slice(&chunk[..count]);
                if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = std::str::from_utf8(&request[..header_end])
                .expect("audit request headers are UTF-8");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().expect("content length"))
                })
                .expect("content-length header");
            while request.len() - header_end < content_length {
                let mut chunk = [0_u8; 4096];
                let count = stream
                    .read(&mut chunk)
                    .await
                    .expect("read audit request body");
                assert!(count > 0, "audit request body ended early");
                request.extend_from_slice(&chunk[..count]);
            }
            let payload: Value =
                serde_json::from_slice(&request[header_end..header_end + content_length])
                    .expect("decode audit request");
            let event_ids = payload["events"]
                .as_array()
                .expect("events array")
                .iter()
                .map(|event| event["event_id"].as_str().expect("event ID").to_string())
                .collect::<Vec<_>>();
            request_batches.push(event_ids.clone());

            let (status, body) = match mode {
                AckMode::Complete => (
                    "200 OK",
                    serde_json::json!({
                        "ok": true,
                        "data": {
                            "acknowledged": true,
                            "acknowledged_event_ids": event_ids,
                        }
                    })
                    .to_string(),
                ),
                AckMode::Partial => (
                    "200 OK",
                    serde_json::json!({
                        "ok": true,
                        "data": {
                            "acknowledged": true,
                            "acknowledged_event_ids": event_ids.into_iter().take(1).collect::<Vec<_>>(),
                        }
                    })
                    .to_string(),
                ),
                AckMode::HttpFailure => (
                    "503 Service Unavailable",
                    serde_json::json!({ "ok": false, "data": { "acknowledged": false } })
                        .to_string(),
                ),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write audit response");
        }
        request_batches
    });
    (format!("http://{address}/audit"), handle)
}

fn audit_state_with_events(count: usize) -> (TempDir, Arc<AuditState>) {
    let directory = TempDir::new().expect("temporary directory");
    let state = Arc::new(AuditState::new(directory.path().to_path_buf()).expect("create state"));
    for number in 0..count {
        state.append_blocking(&event(number)).expect("append event");
    }
    (directory, state)
}

fn audit_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("audit HTTP client")
}

fn connection_with_host(name: &str, id: &str, host: Option<&str>) -> SavedConnection {
    SavedConnection {
        id: id.to_string(),
        name: name.to_string(),
        params: ConnectionParams {
            driver: "mysql".to_string(),
            host: host.map(str::to_string),
            username: Some("audit_user".to_string()),
            database: DatabaseSelection::Single("orders".to_string()),
            ..ConnectionParams::default()
        },
        group_id: None,
        sort_order: None,
        detect_json_in_text_columns: None,
        appearance: None,
    }
}

fn connection(name: &str, id: &str) -> SavedConnection {
    connection_with_host(name, id, None)
}

fn target() -> AuditTarget {
    AuditTarget::from_connection(
        &connection("MySQL测试_阿里云", "dev-connection-id"),
        Some("runtime_db"),
    )
    .expect("test connection should be audited")
}

fn event(number: usize) -> AuditEvent {
    let mut value = AuditEvent::new(
        &target(),
        "UPDATE orders SET status = 'paid' WHERE id = 7",
        1_000_000,
        1_025_000,
        AuditExecutionStatus::Success,
        1,
        "",
        Some("batch-1"),
        Some(number),
        Some("editor-tab-1"),
    );
    value.event_id = format!("event-{number}");
    value
}

#[test]
fn target_classification_uses_local_connection_aliases_only() {
    let test = AuditTarget::from_connection(
        &connection_with_host("MySQL test", "test-id", Some("test-db.example.invalid")),
        None,
    )
    .expect("test alias should be audited");
    let prod = AuditTarget::from_connection(
        &connection_with_host(
            "MySQL production",
            "prod-id",
            Some("prod-db.example.invalid"),
        ),
        None,
    )
    .expect("production alias should be audited");
    let localized =
        AuditTarget::from_connection(&connection("MySQL测试_私有环境", "localized-id"), None)
            .expect("localized test alias should remain auditable");
    let unrelated = AuditTarget::from_connection(&connection("Local MySQL", "local-id"), None);
    let host_only = AuditTarget::from_connection(
        &connection_with_host(
            "Unclassified",
            "host-only-id",
            Some("database.example.invalid"),
        ),
        None,
    );
    let ambiguous =
        AuditTarget::from_connection(&connection("MySQL test production", "ambiguous-id"), None);

    assert_eq!(test.instance_id, "mysql-test");
    assert_eq!(prod.instance_id, "mysql-prod");
    assert_eq!(localized.instance_id, "mysql-test");
    assert!(unrelated.is_none());
    assert!(host_only.is_none());
    assert!(ambiguous.is_none());
}

#[test]
fn event_keeps_execution_identity_and_generates_unique_ids() {
    let first = AuditEvent::new(
        &target(),
        "/* audit */ UPDATE orders SET status = 'paid'",
        1_000_000,
        1_025_000,
        AuditExecutionStatus::Failed,
        0,
        "permission denied",
        Some("batch-9"),
        Some(4),
        Some("editor-tab-9"),
    );
    let second = AuditEvent::new(
        &target(),
        "/* audit */ UPDATE orders SET status = 'paid'",
        1_000_000,
        1_025_000,
        AuditExecutionStatus::Failed,
        0,
        "permission denied",
        Some("batch-9"),
        Some(4),
        Some("editor-tab-9"),
    );

    assert_ne!(first.event_id, second.event_id);
    assert_eq!(first.operation, "UPDATE");
    assert_eq!(first.execution_time_ms, 25);
    assert_eq!(first.database_name, "runtime_db");
    assert_eq!(first.statement_index, 4);
    assert_eq!(first.transaction_context_id, "editor-tab-9");
}

#[test]
fn transport_event_is_pseudonymized_and_sql_literals_are_redacted() {
    let mut original = event(7);
    original.error_message = "Duplicate entry 'private-value' for key 42".to_string();
    let transported = original
        .redacted_for_transport_with(|label, value| Ok(format!("{label}:{}", value.len())))
        .expect("redact transport event");

    assert_eq!(transported.event_id, original.event_id);
    assert_eq!(transported.instance_id, "mysql-test");
    assert_ne!(transported.connection_id, original.connection_id);
    assert_ne!(transported.connection_name, original.connection_name);
    assert_ne!(transported.database_account, original.database_account);
    assert_ne!(transported.database_name, original.database_name);
    assert!(!transported.sql_text.contains("paid"));
    assert!(!transported.sql_text.contains("7"));
    assert!(transported.sql_text.contains('?'));
    assert!(!transported.error_message.contains("private-value"));
    assert!(!transported.error_message.contains("42"));
    assert_eq!(
        original.sql_text,
        "UPDATE orders SET status = 'paid' WHERE id = 7"
    );
}

#[test]
fn outbox_retains_more_than_five_hundred_events_and_advances_only_after_ack() {
    let directory = TempDir::new().expect("temporary directory");
    let outbox = AuditOutbox::with_segment_limit(directory.path().to_path_buf(), 512)
        .expect("create outbox");
    for number in 0..550 {
        outbox.append(&event(number)).expect("append event");
    }

    let first = outbox.pending(500).expect("read first batch");
    assert_eq!(first.events.len(), 500);
    assert_eq!(first.events.first().unwrap().event_id, "event-0");
    assert_eq!(first.events.last().unwrap().event_id, "event-499");

    let retry = outbox.pending(500).expect("read retry batch");
    assert_eq!(retry.events, first.events);

    outbox
        .acknowledge(&first.next_cursor)
        .expect("persist acknowledgement cursor");
    let tail = outbox.pending(500).expect("read remaining batch");
    assert_eq!(tail.events.len(), 50);
    assert_eq!(tail.events.first().unwrap().event_id, "event-500");

    outbox
        .acknowledge(&tail.next_cursor)
        .expect("replace persisted acknowledgement cursor");
    assert!(outbox
        .pending(500)
        .expect("read after second ack")
        .events
        .is_empty());
}

#[test]
fn cursor_survives_reopen_and_append_never_rewrites_existing_events() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().to_path_buf();
    let outbox = AuditOutbox::with_segment_limit(root.clone(), 1024).expect("create outbox");
    for number in 0..3 {
        outbox.append(&event(number)).expect("append event");
    }
    let first = outbox.pending(2).expect("read first batch");
    outbox
        .acknowledge(&first.next_cursor)
        .expect("persist acknowledgement cursor");
    drop(outbox);

    let reopened = AuditOutbox::with_segment_limit(root, 1024).expect("reopen outbox");
    reopened.append(&event(3)).expect("append after reopen");
    let pending = reopened.pending(10).expect("read after reopen");
    assert_eq!(
        pending
            .events
            .iter()
            .map(|value| value.event_id.as_str())
            .collect::<Vec<_>>(),
        vec!["event-2", "event-3"]
    );
}

#[test]
fn append_after_fully_acknowledged_segment_is_still_visible() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().to_path_buf();
    let outbox = AuditOutbox::with_segment_limit(root.clone(), 1024).expect("create outbox");
    outbox.append(&event(0)).expect("append first event");
    let first = outbox.pending(10).expect("read first event");
    outbox
        .acknowledge(&first.next_cursor)
        .expect("acknowledge complete segment");
    drop(outbox);

    let reopened = AuditOutbox::with_segment_limit(root, 1024).expect("reopen outbox");
    reopened
        .append(&event(1))
        .expect("append after full acknowledgement");
    let pending = reopened.pending(10).expect("read appended event");
    assert_eq!(pending.events.len(), 1);
    assert_eq!(pending.events[0].event_id, "event-1");
}

#[test]
fn statement_events_cover_success_and_failure_with_real_indexes() {
    let success_result = crate::models::QueryResult {
        columns: vec![],
        rows: vec![],
        affected_rows: 1,
        truncated: false,
        pagination: None,
        additional_results: None,
    };
    let mut success = crate::models::BatchStatementResult::from_outcome(
        std::time::Instant::now(),
        Ok(success_result),
    );
    success.execution_time_ms = Some(10.25);
    let success_audit = AuditEvent::from_statement(
        "success-event-id",
        &target(),
        "UPDATE orders SET status = 'paid' WHERE id = 7",
        2_000_000,
        &success,
        Some("batch-2"),
        2,
        Some("editor-tab-2"),
    )
    .expect("successful statement should create an audit event");

    let mut failed = crate::models::BatchStatementResult::from_outcome(
        std::time::Instant::now(),
        Err("permission denied".to_string()),
    );
    failed.execution_time_ms = Some(25.75);
    let failed_audit = AuditEvent::from_statement(
        "failed-event-id",
        &target(),
        "UPDATE orders SET status = 'paid' WHERE id = 8",
        2_000_000,
        &failed,
        Some("batch-2"),
        3,
        Some("editor-tab-2"),
    )
    .expect("failed statement should create an audit event");

    assert_eq!(success_audit.event_id, "success-event-id");
    assert_eq!(
        success_audit.execution_status,
        AuditExecutionStatus::Success
    );
    assert_eq!(success_audit.affected_rows, 1);
    assert_eq!(success_audit.statement_index, 2);
    assert_eq!(failed_audit.event_id, "failed-event-id");
    assert_eq!(failed_audit.started_epoch_us, 1_974_250);
    assert_eq!(failed_audit.finished_epoch_us, 2_000_000);
    assert_eq!(failed_audit.execution_time_ms, 25);
    assert_eq!(failed_audit.execution_status, AuditExecutionStatus::Failed);
    assert_eq!(failed_audit.error_message, "permission denied");
    assert_eq!(failed_audit.statement_index, 3);
}

#[test]
fn protected_statements_not_sent_to_the_database_are_not_audited() {
    let explicit_skip = crate::models::BatchStatementResult::skipped("unsupported rollback");
    let stopped = crate::models::BatchStatementResult::from_outcome(
        std::time::Instant::now(),
        Err("Skipped because an earlier protected statement failed".to_string()),
    );
    let callback_indexes = std::sync::Mutex::new(Vec::new());
    let callback = |index: usize, statement: &crate::models::BatchStatementResult| {
        callback_indexes.lock().unwrap().push(index);
        assert!(statement.error.is_some());
        Ok::<(), String>(())
    };

    callback(4, &explicit_skip).unwrap();
    callback(5, &stopped).unwrap();
    assert_eq!(*callback_indexes.lock().unwrap(), vec![4, 5]);
    assert!(AuditEvent::from_statement(
        "skip-1",
        &target(),
        "DROP TABLE orders",
        1_000_000,
        &explicit_skip,
        Some("batch-3"),
        4,
        None,
    )
    .is_none());
    assert!(AuditEvent::from_statement(
        "skip-2",
        &target(),
        "UPDATE orders SET status = 'paid'",
        1_000_000,
        &stopped,
        Some("batch-3"),
        5,
        None,
    )
    .is_none());
}

#[test]
fn failed_and_cancelled_events_keep_distinct_outcomes() {
    let failed = AuditEvent::new(
        &target(),
        "DELETE FROM orders WHERE id = 7",
        2_000_000,
        2_100_000,
        AuditExecutionStatus::Failed,
        0,
        "permission denied",
        None,
        None,
        None,
    );
    let cancelled = AuditEvent::new(
        &target(),
        "SELECT SLEEP(30)",
        3_000_000,
        3_050_000,
        AuditExecutionStatus::Cancelled,
        0,
        "Query cancelled",
        None,
        None,
        None,
    );

    assert_eq!(failed.execution_status, AuditExecutionStatus::Failed);
    assert_eq!(failed.error_message, "permission denied");
    assert_eq!(cancelled.execution_status, AuditExecutionStatus::Cancelled);
    assert_eq!(cancelled.error_message, "Query cancelled");
}

#[tokio::test]
async fn sync_until_empty_drains_multiple_batches_after_complete_ack() {
    let (_directory, state) = audit_state_with_events(550);
    let (url, server) = spawn_ingest_server(2, AckMode::Complete).await;

    let acknowledged = state
        .sync_until_empty(&audit_client(), &url, "test-token")
        .await
        .expect("drain acknowledged outbox");
    let request_batches = server.await.expect("audit server completed");

    assert_eq!(acknowledged, 550);
    assert_eq!(request_batches.len(), 2);
    assert_eq!(request_batches[0].len(), 500);
    assert_eq!(request_batches[1].len(), 50);
    assert!(state
        .outbox
        .pending(DEFAULT_BATCH_SIZE)
        .expect("read drained outbox")
        .events
        .is_empty());
}

#[tokio::test]
async fn partial_ack_does_not_advance_the_cursor() {
    let (_directory, state) = audit_state_with_events(3);
    let expected = state
        .outbox
        .pending(DEFAULT_BATCH_SIZE)
        .expect("read pending before partial ack")
        .events;
    let (url, server) = spawn_ingest_server(1, AckMode::Partial).await;

    let error = state
        .sync_once(&audit_client(), &url, "test-token")
        .await
        .expect_err("partial acknowledgement must fail");
    server.await.expect("audit server completed");
    let retry = state
        .outbox
        .pending(DEFAULT_BATCH_SIZE)
        .expect("read retry batch")
        .events;

    assert!(error.contains("complete batch"));
    assert_eq!(retry, expected);
}

#[tokio::test]
async fn http_failure_does_not_advance_the_cursor() {
    let (_directory, state) = audit_state_with_events(3);
    let expected = state
        .outbox
        .pending(DEFAULT_BATCH_SIZE)
        .expect("read pending before HTTP failure")
        .events;
    let (url, server) = spawn_ingest_server(1, AckMode::HttpFailure).await;

    let error = state
        .sync_once(&audit_client(), &url, "test-token")
        .await
        .expect_err("HTTP failure must fail");
    server.await.expect("audit server completed");
    let retry = state
        .outbox
        .pending(DEFAULT_BATCH_SIZE)
        .expect("read retry batch")
        .events;

    assert!(error.contains("HTTP 503"));
    assert_eq!(retry, expected);
}
