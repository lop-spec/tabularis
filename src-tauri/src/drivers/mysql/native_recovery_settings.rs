//! Read-only prerequisites for database-backed recovery, not an execution permit.
//!
//! A successful settings assessment must never replace the existing journal by
//! itself. Log access, retention until durable materialization, transaction
//! attribution, decoding, and DDL recovery require separate verified evidence.
//! These observations are connection-local and point-in-time, not a cacheable
//! guarantee that an administrator cannot subsequently change global settings.

use serde::Serialize;
use sqlx::{MySqlConnection, Row};
use std::collections::BTreeMap;

const GLOBAL_QUERY: &str = "SHOW GLOBAL VARIABLES WHERE Variable_name IN \
    ('version','version_comment','server_uuid','log_bin','gtid_mode',\
    'enforce_gtid_consistency','sync_binlog','innodb_flush_log_at_trx_commit',\
    'binlog_expire_logs_seconds','binlog_row_metadata','binlog_transaction_compression')";
const SESSION_QUERY: &str = "SHOW SESSION VARIABLES WHERE Variable_name IN \
    ('sql_log_bin','binlog_format','binlog_row_image')";

type Variables = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableScope {
    Global,
    Session,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingMismatch {
    pub scope: VariableScope,
    pub name: String,
    pub observed: Option<String>,
    pub required: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutstandingProof {
    BinlogReadAccess,
    DurableRetentionUntilMaterialized,
    ExactTransactionAttribution,
    LosslessRowDecodingAndSchemaHistory,
    CrashSafeIntentAndMaterialization,
    DdlBackupAndRestore,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRecoverySettings {
    pub global: Variables,
    pub session: Variables,
    pub blocking_settings: Vec<SettingMismatch>,
    pub outstanding_proofs: Vec<OutstandingProof>,
}

impl NativeRecoverySettings {
    /// Necessary DML logging settings only; never authorization to execute SQL.
    pub fn dml_settings_compatible(&self) -> bool {
        self.blocking_settings.is_empty()
    }
}

const REQUIREMENTS: &[(VariableScope, &str, &str)] = &[
    (VariableScope::Global, "log_bin", "ON"),
    (VariableScope::Global, "gtid_mode", "ON"),
    (VariableScope::Global, "enforce_gtid_consistency", "ON"),
    (VariableScope::Global, "sync_binlog", "1"),
    (VariableScope::Global, "innodb_flush_log_at_trx_commit", "1"),
    (VariableScope::Session, "sql_log_bin", "ON"),
    (VariableScope::Session, "binlog_format", "ROW"),
    (VariableScope::Session, "binlog_row_image", "FULL"),
];

fn assess(global: Variables, session: Variables) -> NativeRecoverySettings {
    let mut blocking_settings = Vec::new();
    for &(scope, name, required) in REQUIREMENTS {
        let variables = match scope {
            VariableScope::Global => &global,
            VariableScope::Session => &session,
        };
        let observed = variables.get(name);
        let matches = observed.is_some_and(|value| {
            value.eq_ignore_ascii_case(required) || (required == "ON" && value == "1")
        });
        if !matches {
            blocking_settings.push(SettingMismatch {
                scope,
                name: name.into(),
                observed: observed.cloned(),
                required: required.into(),
            });
        }
    }
    let version = global.get("version");
    let supported_version = version.is_some_and(|value| {
        let family = format!(
            "{} {}",
            value,
            global.get("version_comment").map_or("", String::as_str)
        )
        .to_ascii_lowercase();
        (value.starts_with("8.0.") || value.starts_with("8.4."))
            && !family.contains("mariadb")
            && !family.contains("tidb")
    });
    if !supported_version {
        blocking_settings.push(SettingMismatch {
            scope: VariableScope::Global,
            name: "version".into(),
            observed: version.cloned(),
            required: "MySQL 8.0 or 8.4; other server families are not verified".into(),
        });
    }
    let server_uuid = global.get("server_uuid");
    if !server_uuid
        .is_some_and(|value| uuid::Uuid::parse_str(value).is_ok_and(|uuid| !uuid.is_nil()))
    {
        blocking_settings.push(SettingMismatch {
            scope: VariableScope::Global,
            name: "server_uuid".into(),
            observed: server_uuid.cloned(),
            required: "A valid non-nil server UUID for recovery-source binding".into(),
        });
    }
    NativeRecoverySettings {
        global,
        session,
        blocking_settings,
        // FULL row images and a positive expiry interval do not prove that a
        // log will survive client shutdown or that destructive DDL is reversible.
        outstanding_proofs: vec![
            OutstandingProof::BinlogReadAccess,
            OutstandingProof::DurableRetentionUntilMaterialized,
            OutstandingProof::ExactTransactionAttribution,
            OutstandingProof::LosslessRowDecodingAndSchemaHistory,
            OutstandingProof::CrashSafeIntentAndMaterialization,
            OutstandingProof::DdlBackupAndRestore,
        ],
    }
}

async fn read_variables(conn: &mut MySqlConnection, query: &str) -> Result<Variables, String> {
    let rows = sqlx::raw_sql(query)
        .fetch_all(conn)
        .await
        .map_err(|error| {
            let reason = format!("Native recovery settings inspection failed: {error}");
            log::warn!("{reason}; protection must remain unchanged");
            reason
        })?;
    let mut variables = Variables::new();
    for row in rows {
        let decoded = row.try_get::<String, _>(0).and_then(|name| {
            row.try_get::<String, _>(1)
                .map(|value| (name.to_ascii_lowercase(), value))
        });
        let (name, value) = decoded.map_err(|error| {
            let reason = format!("Native recovery settings could not be decoded: {error}");
            log::warn!("{reason}; protection must remain unchanged");
            reason
        })?;
        if variables.insert(name.clone(), value).is_some() {
            let reason = format!("Native recovery settings returned duplicate variable {name}");
            log::warn!("{reason}; protection must remain unchanged");
            return Err(reason);
        }
    }
    Ok(variables)
}

/// Inspects the physical connection that would execute the user's SQL. Never
/// uses a different pooled connection for session settings, grants privileges,
/// changes parameters, or enables a fallback execution path.
pub async fn inspect(conn: &mut MySqlConnection) -> Result<NativeRecoverySettings, String> {
    let global = read_variables(conn, GLOBAL_QUERY).await?;
    let session = read_variables(conn, SESSION_QUERY).await?;
    let report = assess(global, session);
    log::warn!(
        "Native recovery is not enabled by settings inspection: blocking_settings={:?}, outstanding_proofs={:?}",
        report.blocking_settings, report.outstanding_proofs
    );
    Ok(report)
}

#[cfg(test)]
#[path = "native_recovery_settings_tests.rs"]
mod tests;
