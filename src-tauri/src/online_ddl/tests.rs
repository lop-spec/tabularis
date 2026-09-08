use super::{
    job::{Job, Snapshot},
    plan::*,
};
use crate::models::ConnectionParams;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

fn request(sql: &str) -> Request {
    Request {
        connection_id: "fixture".into(),
        database: "fixture_db".into(),
        table: "orders".into(),
        statements: vec![sql.into()],
        replicas: vec!["replica.example.invalid:3306".into()],
        aliyun_rds: true,
    }
}

#[test]
fn scopes_supported_editor_statements_to_exact_table() {
    assert_eq!(
        alter_clause(&request(
            "CREATE UNIQUE INDEX `idx_a` ON `orders` (`a`, `b`)"
        ))
        .unwrap(),
        "ADD UNIQUE INDEX `idx_a` (`a`, `b`)"
    );
    assert_eq!(
        alter_clause(&request(
            "ALTER TABLE `fixture_db`.`orders` ADD COLUMN `note` VARCHAR(32) NULL"
        ))
        .unwrap(),
        "ADD COLUMN `note` VARCHAR(32) NULL"
    );
    assert!(alter_clause(&request(
        "ALTER TABLE `orders` MODIFY COLUMN `note` VARCHAR(128) NULL"
    ))
    .is_ok());
    for sql in [
        "ALTER TABLE `other`.`orders` ADD COLUMN x INT",
        "ALTER TABLE `others` ADD COLUMN x INT",
        "ALTER TABLE `orders` CHANGE a b INT",
        "DROP TABLE `orders`",
        "ALTER TABLE `orders` ADD COLUMN a INT; DROP TABLE `orders`",
        "ALTER TABLE `orders` ADD COLUMN a INT /* hidden */",
    ] {
        assert!(alter_clause(&request(sql)).is_err(), "{sql}");
    }
    let mut req = request("ALTER TABLE `orders` ADD COLUMN a INT");
    req.statements.push("SELECT 1".into());
    assert!(alter_clause(&req).is_err());
}

#[test]
fn credentials_are_ini_literals_and_never_command_arguments() {
    let password = "p#;=\\word\" $!";
    let config = credential_file("fixture", password).unwrap();
    assert!(config.contains(&format!("\"\"\"{password}\"\"\"")));
    assert!(credential_file("fixture", "x\n[osc]").is_err());
    assert!(credential_file("fixture", "x\"\"\"y").is_err());
    let params = ConnectionParams {
        host: Some("primary.example.invalid".into()),
        password: Some(password.into()),
        ssl_mode: Some("required".into()),
        ..Default::default()
    };
    let args = arguments(
        &request("CREATE INDEX `idx_a` ON `orders` (`a`)"),
        &params,
        &[endpoint("replica.example.invalid").unwrap()],
        std::path::Path::new("fixture"),
        "http://127.0.0.1:1/token",
    )
    .unwrap();
    let joined = args.join("\n");
    assert!(!joined.contains(password));
    for flag in [
        "--max-lag-millis=3000",
        "--chunk-size=500",
        "--cut-over-lock-timeout-seconds=1",
        "--allow-on-master",
        "--aliyun-rds",
        "--panic-on-warnings",
        "--throttle-http=http://127.0.0.1:1/token",
    ] {
        assert!(args.contains(&flag.into()), "{flag}");
    }
    for forbidden in [
        "--ok-to-drop-table",
        "--initially-drop",
        "--skip-foreign-key",
        "--skip-renamed",
        "--switch-to-rbr",
        "--ignore-http-errors",
        "--serve-tcp-port",
    ] {
        assert!(!joined.contains(forbidden), "{forbidden}");
    }
    assert!(arguments(
        &request("ALTER TABLE `orders` ADD COLUMN a INT"),
        &params,
        &[],
        std::path::Path::new("fixture"),
        "http://127.0.0.1:1/token"
    )
    .is_err());
}

#[test]
fn endpoint_and_redaction_boundaries() {
    assert_eq!(
        endpoint("replica.example.invalid").unwrap().address(),
        "replica.example.invalid:3306"
    );
    assert_eq!(endpoint("[::1]:3307").unwrap().address(), "[::1]:3307");
    for invalid in [
        "",
        "mysql://replica.example.invalid",
        "user@replica.example.invalid",
        "a,b",
        "a b",
        "a:0",
    ] {
        assert!(endpoint(invalid).is_err(), "{invalid}");
    }
    assert_eq!(redact("secret secret", "secret"), "[REDACTED] [REDACTED]");
    assert_eq!(
        progress("Copy: 100/1000 10.0%; Applied: 40; Lag: 0.01s"),
        Some(10.0)
    );
    assert_eq!(progress("Applying..."), None);
    assert_eq!(progress("Copy: 0/0 NaN%;"), None);
}

fn job() -> Job {
    Job {
        snapshot: Snapshot {
            id: "test".into(),
            connection_id: "fixture".into(),
            database: "fixture_db".into(),
            table: "orders".into(),
            status: "running".into(),
            reason: String::new(),
            progress: None,
            max_replica_lag_ms: None,
            finished: false,
            sql: String::new(),
            result_ddl: None,
            logs: VecDeque::new(),
        },
        directory: std::path::PathBuf::new(),
        last_healthy: None,
        gate_open: false,
        manual_pause: false,
        cancel_requested: false,
    }
}

#[test]
fn gate_fails_closed_for_missing_stale_or_cancelled_monitor() {
    let mut job = job();
    assert!(!job.gate_allows());
    job.gate_open = true;
    assert!(!job.gate_allows());
    job.last_healthy = Some(Instant::now());
    assert!(job.gate_allows());
    job.manual_pause = true;
    assert!(!job.gate_allows());
    job.manual_pause = false;
    job.last_healthy = Some(Instant::now() - Duration::from_secs(3));
    assert!(!job.gate_allows());
    job.last_healthy = Some(Instant::now());
    job.cancel_requested = true;
    assert!(!job.gate_allows());
    job.cancel_requested = false;
    job.snapshot.finished = true;
    assert!(!job.gate_allows());
}

#[tokio::test]
async fn loopback_gate_is_an_actual_http_fail_closed_endpoint() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let job = std::sync::Arc::new(std::sync::Mutex::new(job()));
    let server = tokio::spawn(super::job::serve_gate(
        listener,
        job.clone(),
        "/token".into(),
    ));
    for (healthy, path, expected) in [
        (false, "/token", "503"),
        (true, "/token", "200"),
        (true, "/wrong", "503"),
    ] {
        {
            let mut state = job.lock().unwrap();
            state.gate_open = healthy;
            state.last_healthy = Some(Instant::now());
        }
        let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
        client
            .write_all(format!("HEAD {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with(&format!("HTTP/1.1 {expected}")));
    }
    server.abort();
}
