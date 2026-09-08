//! Thin, local gh-ost integration. No migration or cutover algorithm is implemented here.
mod job;
mod plan;
mod preflight;
mod runtime;
#[cfg(test)]
mod tests;

use job::{Job, SharedJob, Snapshot};
use once_cell::sync::Lazy;
use plan::Request;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Runtime};

static JOBS: Lazy<Mutex<HashMap<String, SharedJob>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn has_active_jobs() -> bool {
    JOBS.lock()
        .unwrap()
        .values()
        .any(|job| !job.lock().unwrap().snapshot.finished)
}

#[tauri::command]
pub fn preview_online_ddl(request: Request) -> Result<String, String> {
    Ok(format!(
        "ALTER TABLE {}.{} {};",
        plan::quote(&request.database)?,
        plan::quote(&request.table)?,
        plan::alter_clause(&request)?
    ))
}

#[tauri::command]
pub fn online_ddl_available() -> bool {
    !runtime::ENGINE.is_empty()
}

#[tauri::command]
pub fn get_online_ddl_job(connection_id: String) -> Option<Snapshot> {
    JOBS.lock()
        .unwrap()
        .get(&connection_id)
        .map(|job| job.lock().unwrap().snapshot.clone())
}

#[tauri::command]
pub fn control_online_ddl(
    connection_id: String,
    job_id: String,
    action: String,
) -> Result<(), String> {
    let job = JOBS
        .lock()
        .unwrap()
        .get(&connection_id)
        .cloned()
        .ok_or("No migration on this connection")?;
    let mut task = job.lock().unwrap();
    if task.snapshot.id != job_id || task.snapshot.finished {
        return Err("Migration is no longer active".into());
    }
    if task.cancel_requested {
        return Err("Cancellation is already pending".into());
    }
    match action.as_str() {
        "pause" => {
            std::fs::write(task.directory.join("pause"), []).map_err(|e| e.to_string())?;
            task.manual_pause = true;
            task.snapshot.status = "throttled".into();
            task.snapshot.reason = "Paused by user".into();
        }
        "resume" => {
            match std::fs::remove_file(task.directory.join("pause")) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
            task.manual_pause = false;
        }
        "cancel" => {
            std::fs::write(task.directory.join("cancel"), []).map_err(|e| e.to_string())?;
            task.cancel_requested = true;
            task.gate_open = false;
            task.snapshot.status = "cancelling".into();
            task.snapshot.reason = "Cancellation requested; awaiting gh-ost exit".into();
        }
        _ => return Err("Unsupported migration action".into()),
    }
    task.note(format!("User requested {action}"));
    Ok(())
}

#[tauri::command]
pub async fn start_online_ddl<R: Runtime>(
    app: AppHandle<R>,
    request: Request,
) -> Result<Snapshot, String> {
    let alter = plan::alter_clause(&request)?;
    let root = crate::paths::get_app_config_dir().join("online-ddl");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let binary = runtime::engine_path(&root).map_err(|error| {
        log::error!("Online DDL unavailable: {error}");
        error
    })?;
    let id = uuid::Uuid::new_v4().to_string();
    let directory = root.join(&id);
    let snapshot = Snapshot {
        id,
        connection_id: request.connection_id.clone(),
        database: request.database.clone(),
        table: request.table.clone(),
        status: "preparing".into(),
        reason: "Checking primary identity and all replica connections".into(),
        progress: None,
        max_replica_lag_ms: None,
        finished: false,
        sql: format!(
            "ALTER TABLE {}.{} {alter};",
            plan::quote(&request.database)?,
            plan::quote(&request.table)?
        ),
        result_ddl: None,
        logs: VecDeque::new(),
    };
    let shared = Arc::new(Mutex::new(Job {
        snapshot: snapshot.clone(),
        directory: directory.clone(),
        last_healthy: None,
        gate_open: false,
        manual_pause: false,
        cancel_requested: false,
    }));
    {
        let mut jobs = JOBS.lock().unwrap();
        if jobs
            .get(&request.connection_id)
            .is_some_and(|job| !job.lock().unwrap().snapshot.finished)
        {
            return Err("This connection already has an active migration".into());
        }
        runtime::private_directory(&directory)?;
        jobs.insert(request.connection_id.clone(), shared.clone());
    }
    tokio::spawn(async move {
        let result = async {
            let prepared = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                preflight::prepare(&app, &request),
            )
            .await
            .map_err(|_| {
                "Online DDL preflight timed out; no migration was started".to_string()
            })??;
            if shared.lock().unwrap().cancel_requested {
                shared.lock().unwrap().finish(
                    "cancelled",
                    "Cancelled before gh-ost started; no DDL executed".into(),
                );
                return Ok(());
            }
            job::execute(shared.clone(), prepared, request, binary).await
        }
        .await;
        runtime::remove_credentials(&directory);
        if let Err(error) = result {
            shared.lock().unwrap().finish("failed", error);
        }
    });
    Ok(snapshot)
}
