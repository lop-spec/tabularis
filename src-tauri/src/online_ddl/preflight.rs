use super::plan::{self, Endpoint, Request};
use crate::commands::{
    expand_k8s_connection_params, expand_ssh_connection_params, find_connection_by_id,
    resolve_connection_params_with_identity,
};
use crate::models::{ConnectionParams, DatabaseSelection};
use sqlx::{MySqlPool, Row};
use std::collections::HashSet;
use std::time::Duration;
use tauri::{AppHandle, Runtime};

pub struct Replica {
    pub endpoint: Endpoint,
    pub pool: MySqlPool,
    pub uuid: String,
}

pub struct Prepared {
    pub params: ConnectionParams,
    pub primary: MySqlPool,
    pub replicas: Vec<Replica>,
    pub uuid: String,
    pub original_ddl: String,
    pub target: String,
}

pub async fn identity(pool: &MySqlPool) -> Result<(String, bool), String> {
    let row = sqlx::query("SELECT @@server_uuid AS server_uuid, CAST(@@read_only AS UNSIGNED) AS read_only, CAST(@@super_read_only AS UNSIGNED) AS super_read_only, CURRENT_USER() AS current_user, @@hostname AS hostname, @@version AS version")
        .fetch_one(pool).await.map_err(|e| e.to_string())?;
    let version: String = row.try_get("version").map_err(|e| e.to_string())?;
    if version.to_ascii_lowercase().contains("mariadb")
        || !(version.starts_with("8.") || version.starts_with("9."))
    {
        return Err("Online DDL currently requires Oracle-compatible MySQL 8.0 or later".into());
    }
    let uuid = row.try_get("server_uuid").map_err(|e| e.to_string())?;
    let read_only = row
        .try_get::<u64, _>("read_only")
        .map_err(|e| e.to_string())?
        != 0
        || row
            .try_get::<u64, _>("super_read_only")
            .map_err(|e| e.to_string())?
            != 0;
    Ok((uuid, read_only))
}

pub async fn ddl(pool: &MySqlPool, target: &str) -> Result<String, String> {
    let row = sqlx::query(&format!("SHOW CREATE TABLE {target}"))
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    row.try_get(1).map_err(|e| e.to_string())
}

async fn connect(params: &ConnectionParams) -> Result<MySqlPool, String> {
    let options = crate::pool_manager::build_mysql_options(params, None)?;
    sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(options)
        .await
        .map_err(|e| e.to_string())
}

pub async fn prepare<R: Runtime>(
    app: &AppHandle<R>,
    request: &Request,
) -> Result<Prepared, String> {
    plan::alter_clause(request)?;
    if request.replicas.is_empty() || request.replicas.len() > 16 {
        return Err("Select between one and sixteen direct read-replica endpoints".into());
    }
    let saved = find_connection_by_id(app, &request.connection_id)?;
    if saved.params.driver != "mysql" {
        return Err("Online DDL is only available for MySQL".into());
    }
    if saved.params.use_iam_auth.unwrap_or(false)
        || saved.params.enable_cleartext_plugin.unwrap_or(false)
        || saved
            .params
            .startup_script
            .as_ref()
            .is_some_and(|v| !v.trim().is_empty())
    {
        return Err("gh-ost does not inherit IAM tokens, cleartext auth or connection startup scripts; this connection is not supported".into());
    }
    let mut expanded = expand_ssh_connection_params(app, &saved.params).await?;
    expanded = expand_k8s_connection_params(app, &expanded).await?;
    expanded.database = DatabaseSelection::Single(request.database.clone());
    let params = resolve_connection_params_with_identity(&expanded, &saved.id, &saved.name)?;
    let primary = connect(&params).await?;
    let (uuid, read_only) = identity(&primary).await?;
    if read_only {
        return Err(
            "Selected connection is read-only; a direct primary endpoint is required".into(),
        );
    }
    let target = format!(
        "{}.{}",
        plan::quote(&request.database)?,
        plan::quote(&request.table)?
    );
    // gh-ost PR #1536 protects cutover from data loss when another session
    // accesses the ghost table. Never bypass this primary-side safety check.
    let instrument: Option<(String, String)> = sqlx::query_as(
        "SELECT ENABLED, TIMED FROM performance_schema.setup_instruments WHERE NAME = 'wait/lock/metadata/sql/mdl'",
    ).fetch_optional(&primary).await.map_err(|e| format!("Cannot inspect primary metadata-lock instrumentation: {e}"))?;
    if !instrument.is_some_and(|(enabled, timed)| enabled == "YES" && timed == "YES") {
        return Err("gh-ost requires primary Performance Schema metadata-lock instrumentation (ENABLED=YES, TIMED=YES). It is disabled; no DDL was started and no safety checks were skipped. Replica lag monitoring does not require replica Performance Schema.".into());
    }
    sqlx::query("SELECT m.OWNER_THREAD_ID FROM performance_schema.metadata_locks m JOIN performance_schema.threads t ON m.OWNER_THREAD_ID = t.THREAD_ID WHERE 1 = 0")
        .fetch_all(&primary).await.map_err(|e| format!("Primary metadata-lock visibility is required for safe cutover: {e}"))?;
    let original_ddl = ddl(&primary, &target).await?;
    let engine: Option<String> = sqlx::query_scalar("SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND TABLE_TYPE = 'BASE TABLE'")
        .bind(&request.database).bind(&request.table).fetch_optional(&primary).await.map_err(|e| e.to_string())?;
    if engine.as_deref() != Some("InnoDB") {
        return Err("Online DDL requires an InnoDB base table".into());
    }
    let mut replicas = Vec::new();
    let mut identities = HashSet::from([uuid.clone()]);
    for value in &request.replicas {
        let endpoint = plan::endpoint(value)?;
        let mut replica_params = expanded.clone();
        replica_params.host = Some(endpoint.host);
        replica_params.port = Some(endpoint.port);
        // Kubernetes port-forwards target one resource, not a topology.
        if replica_params.k8s_enabled.unwrap_or(false) {
            return Err("Online DDL cannot monitor a replica topology through a single Kubernetes port-forward".into());
        }
        let resolved = resolve_connection_params_with_identity(
            &replica_params,
            &format!("{}:osc:{}", saved.id, replicas.len()),
            &saved.name,
        )?;
        let pool = connect(&resolved).await?;
        let (replica_uuid, read_only) = identity(&pool).await?;
        if !read_only || !identities.insert(replica_uuid.clone()) {
            return Err("Replica endpoints must resolve to distinct read-only instances, not the primary or a load-balancing endpoint".into());
        }
        let rows = sqlx::query("SHOW REPLICA STATUS")
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("Cannot verify replica topology (SHOW REPLICA STATUS): {e}"))?;
        if rows.len() != 1 {
            return Err("Online DDL requires a single-source replica directly following the selected primary".into());
        }
        let source: String = rows[0]
            .try_get("Source_UUID")
            .or_else(|_| rows[0].try_get("Master_UUID"))
            .map_err(|e| e.to_string())?;
        if source != uuid {
            return Err("Replica source UUID does not match the selected primary".into());
        }
        let io: String = rows[0]
            .try_get("Replica_IO_Running")
            .or_else(|_| rows[0].try_get("Slave_IO_Running"))
            .map_err(|e| e.to_string())?;
        let sql: String = rows[0]
            .try_get("Replica_SQL_Running")
            .or_else(|_| rows[0].try_get("Slave_SQL_Running"))
            .map_err(|e| e.to_string())?;
        if io != "Yes" || sql != "Yes" {
            return Err(
                "Replica receiver/applier is not running; repair replication before migrating"
                    .into(),
            );
        }
        replicas.push(Replica {
            endpoint: Endpoint {
                host: resolved
                    .host
                    .clone()
                    .ok_or("Missing resolved replica host")?,
                port: resolved.port.unwrap_or(3306),
            },
            pool,
            uuid: replica_uuid,
        });
    }
    Ok(Prepared {
        params,
        primary,
        replicas,
        uuid,
        original_ddl,
        target,
    })
}
