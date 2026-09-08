use super::{plan, preflight, runtime};
use serde::Serialize;
use sqlx::Row;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub connection_id: String,
    pub database: String,
    pub table: String,
    pub status: String,
    pub reason: String,
    pub progress: Option<f64>,
    pub max_replica_lag_ms: Option<u64>,
    pub finished: bool,
    pub sql: String,
    pub result_ddl: Option<String>,
    pub logs: VecDeque<String>,
}

pub struct Job {
    pub snapshot: Snapshot,
    pub directory: PathBuf,
    pub last_healthy: Option<Instant>,
    pub gate_open: bool,
    pub manual_pause: bool,
    pub cancel_requested: bool,
}

pub type SharedJob = Arc<Mutex<Job>>;

impl Job {
    pub fn note(&mut self, line: String) {
        if self.snapshot.logs.len() >= 200 {
            self.snapshot.logs.pop_front();
        }
        log::info!("Online DDL {}: {}", self.snapshot.id, line);
        self.snapshot.logs.push_back(line);
    }
    pub fn finish(&mut self, status: &str, reason: String) {
        self.gate_open = false;
        self.snapshot.status = status.into();
        self.snapshot.reason = reason.clone();
        self.snapshot.finished = true;
        self.note(reason);
    }
    pub fn gate_allows(&self) -> bool {
        !self.snapshot.finished
            && !self.manual_pause
            && !self.cancel_requested
            && self.gate_open
            && self
                .last_healthy
                .is_some_and(|at| at.elapsed() < Duration::from_secs(2))
    }
}

pub async fn serve_gate(listener: tokio::net::TcpListener, job: SharedJob, path: String) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            break;
        };
        let mut request = [0u8; 1_024];
        let read =
            tokio::time::timeout(Duration::from_millis(500), socket.read(&mut request)).await;
        let valid = read.ok().and_then(Result::ok).is_some_and(|n| {
            let line = String::from_utf8_lossy(&request[..n]);
            line.starts_with(&format!("HEAD {path} HTTP/1."))
                || line.starts_with(&format!("GET {path} HTTP/1."))
        });
        let healthy = valid && job.lock().unwrap().gate_allows();
        let status = if healthy {
            "200 OK"
        } else {
            "503 Service Unavailable"
        };
        let response =
            format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let _ = tokio::time::timeout(
            Duration::from_millis(500),
            socket.write_all(response.as_bytes()),
        )
        .await;
    }
}

async fn probe(prepared: &preflight::Prepared, heartbeat_query: &str) -> Result<u64, String> {
    let (uuid, read_only) = preflight::identity(&prepared.primary).await?;
    if uuid != prepared.uuid || read_only {
        return Err("Primary identity/role changed; migration remains paused".into());
    }
    let results = futures::future::join_all(prepared.replicas.iter().map(|replica| async {
        let row = sqlx::query(heartbeat_query)
            .fetch_one(&replica.pool)
            .await
            .map_err(|e| format!("Replica heartbeat unavailable: {e}"))?;
        let uuid: String = row.try_get("server_uuid").map_err(|e| e.to_string())?;
        let read_only: u64 = row.try_get("read_only").map_err(|e| e.to_string())?;
        if uuid != replica.uuid || read_only == 0 {
            return Err("Replica identity/role changed".to_string());
        }
        let value: String = row.try_get("value").map_err(|e| e.to_string())?;
        let at = chrono::DateTime::parse_from_rfc3339(&value)
            .map_err(|_| "Invalid replica heartbeat timestamp")?;
        let lag = chrono::Utc::now()
            .signed_duration_since(at)
            .num_milliseconds();
        if lag < 0 {
            return Err("Heartbeat is in the future; check the client clock".into());
        }
        Ok(lag as u64)
    }))
    .await;
    results
        .into_iter()
        .try_fold(0, |maximum, result| result.map(|lag| maximum.max(lag)))
}

async fn monitor(job: SharedJob, prepared: Arc<preflight::Prepared>, heartbeat_query: String) {
    let mut timer = tokio::time::interval(Duration::from_millis(500));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        if job.lock().unwrap().snapshot.finished {
            break;
        }
        let result = tokio::time::timeout(
            Duration::from_millis(1_500),
            probe(&prepared, &heartbeat_query),
        )
        .await
        .unwrap_or_else(|_| Err("Replica monitoring timed out; migration paused".into()));
        let mut task = job.lock().unwrap();
        let (healthy, reason, lag) = match result {
            Ok(lag) if lag <= plan::LAG_LIMIT_MS => (true, String::new(), Some(lag)),
            Ok(lag) => (
                false,
                format!("Replica lag {lag} ms exceeds {} ms", plan::LAG_LIMIT_MS),
                Some(lag),
            ),
            Err(error) => (false, error, None),
        };
        task.snapshot.max_replica_lag_ms = lag;
        task.gate_open = healthy;
        task.last_healthy = healthy.then(Instant::now);
        if task.cancel_requested {
            continue;
        }
        let reason = if task.manual_pause {
            "Paused by user".to_string()
        } else {
            reason
        };
        if task.snapshot.reason != reason {
            task.note(if reason.is_empty() {
                "Replica monitor healthy; migration may continue".into()
            } else {
                reason.clone()
            });
        }
        task.snapshot.status = if healthy && !task.manual_pause {
            "running"
        } else {
            "throttled"
        }
        .into();
        task.snapshot.reason = reason;
    }
}

async fn read_output<R: AsyncRead + Unpin>(stream: R, job: SharedJob, secret: String) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let safe = plan::redact(line.trim_end(), &secret);
                let mut task = job.lock().unwrap();
                if let Some(progress) = plan::progress(&safe) {
                    task.snapshot.progress = Some(progress);
                }
                task.note(safe);
            }
            Err(error) => {
                job.lock()
                    .unwrap()
                    .note(format!("gh-ost output read failed: {error}"));
                break;
            }
        }
    }
}

pub async fn execute(
    job: SharedJob,
    prepared: preflight::Prepared,
    request: plan::Request,
    binary: PathBuf,
) -> Result<(), String> {
    let directory = job.lock().unwrap().directory.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let path = format!("/{}", uuid::Uuid::new_v4());
    let gate = format!(
        "http://{}{path}",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    let replicas = prepared
        .replicas
        .iter()
        .map(|replica| replica.endpoint.clone())
        .collect::<Vec<_>>();
    let args = plan::arguments(&request, &prepared.params, &replicas, &directory, &gate)?;
    let secret = prepared.params.password.clone().unwrap_or_default();
    let config = plan::credential_file(
        prepared.params.username.as_deref().unwrap_or_default(),
        &secret,
    )?;
    std::fs::write(directory.join("credentials.cnf"), config).map_err(|e| e.to_string())?;
    let mut command = tokio::process::Command::new(binary);
    command
        .args(args)
        .current_dir(&directory)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .spawn()
        .map_err(|e| format!("Could not start gh-ost: {e}"))?;
    let stdout = child.stdout.take().ok_or("Missing gh-ost output pipe")?;
    let stderr = child.stderr.take().ok_or("Missing gh-ost error pipe")?;
    let prepared = Arc::new(prepared);
    let heartbeat_table = plan::quote(&format!("_{}_ghc", request.table))?;
    let heartbeat_query = format!("SELECT value, @@server_uuid AS server_uuid, CAST(@@read_only AS UNSIGNED) AS read_only FROM {}.{heartbeat_table} WHERE hint = 'heartbeat' AND id <= 255", plan::quote(&request.database)?);
    let gate_task = tokio::spawn(serve_gate(listener, job.clone(), path));
    let monitor_task = tokio::spawn(monitor(job.clone(), prepared.clone(), heartbeat_query));
    let stdout_task = tokio::spawn(read_output(stdout, job.clone(), secret.clone()));
    let stderr_task = tokio::spawn(read_output(stderr, job.clone(), secret));
    job.lock().unwrap().note(format!("gh-ost {} started; all {} replicas required, throttle at {} ms. Ordinary rollback journaling does not cover this external migration.", plan::VERSION, replicas.len(), plan::LAG_LIMIT_MS));
    let result = child.wait().await;
    monitor_task.abort();
    gate_task.abort();
    let _ = tokio::join!(stdout_task, stderr_task);
    runtime::remove_credentials(&directory);
    let status = result.map_err(|e| e.to_string())?;
    if status.success() {
        let (uuid, read_only) = preflight::identity(&prepared.primary).await?;
        if uuid != prepared.uuid || read_only {
            return Err("gh-ost exited successfully but primary identity changed; verify the actual schema before further actions".into());
        }
        let result_ddl = preflight::ddl(&prepared.primary, &prepared.target).await?;
        let mut task = job.lock().unwrap();
        task.snapshot.result_ddl = Some(result_ddl.clone());
        if result_ddl == prepared.original_ddl {
            task.finish("needs_review", "gh-ost exited successfully but the table definition is unchanged; inspect the retained logs".into());
        } else {
            task.snapshot.progress = Some(100.0);
            task.finish("succeeded", "Migration completed and table definition read back. Old table retained; it is not a lossless post-cutover rollback.".into());
        }
    } else {
        let mut task = job.lock().unwrap();
        if task.cancel_requested {
            task.finish("cancelled", "gh-ost stopped without automatic cleanup. Verify table state if cancellation overlapped cutover.".into());
        } else {
            task.finish("failed", format!("gh-ost exited with {status}; original/ghost tables are not automatically modified or removed by Tabularis"));
        }
    }
    Ok(())
}
