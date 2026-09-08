use crate::models::ConnectionParams;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const LAG_LIMIT_MS: u64 = 3_000;
pub const VERSION: &str = "1.1.11";

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub connection_id: String,
    pub database: String,
    pub table: String,
    pub statements: Vec<String>,
    pub replicas: Vec<String>,
    pub aliyun_rds: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}

impl Endpoint {
    pub fn address(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

pub fn endpoint(value: &str) -> Result<Endpoint, String> {
    let value = value.trim();
    if value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '@' | '?' | '#' | ','))
    {
        return Err("Replica must be a host:port endpoint, not a URL or proxy list".into());
    }
    let parsed = url::Url::parse(&format!("mysql://{value}"))
        .map_err(|_| "Invalid replica endpoint".to_string())?;
    let host = parsed
        .host_str()
        .ok_or("Replica hostname is missing")?
        .trim_matches(['[', ']'])
        .to_string();
    if host.starts_with('-') || parsed.port() == Some(0) {
        return Err("Invalid replica hostname or port".into());
    }
    Ok(Endpoint {
        host,
        port: parsed.port().unwrap_or(3306),
    })
}

pub fn quote(name: &str) -> Result<String, String> {
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(
            "Database/table identifier is empty, too long or contains control characters".into(),
        );
    }
    Ok(format!("`{}`", name.replace('`', "``")))
}

/// Accept only the single statement emitted by the existing structure editors.
/// Reject unsupported syntax rather than attempting a general SQL rewrite.
pub fn alter_clause(request: &Request) -> Result<String, String> {
    let database = quote(&request.database)?;
    let table = quote(&request.table)?;
    if request.table.len() > 59 {
        return Err(
            "Online DDL currently requires a table name no longer than 59 UTF-8 bytes".into(),
        );
    }
    if request.statements.len() != 1 {
        return Err("Online DDL requires exactly one structure-edit statement".into());
    }
    let sql = request.statements[0].trim().trim_end_matches(';').trim();
    if sql.is_empty()
        || sql.len() > 65_536
        || sql.contains([';', '\0'])
        || sql.contains("/*")
        || sql.contains("--")
        || sql.contains('#')
    {
        return Err(
            "Online DDL does not accept comments, delimiters or multi-statement SQL".into(),
        );
    }
    let target = format!(
        "(?:{}\\s*\\.\\s*)?{}",
        regex::escape(&database),
        regex::escape(&table)
    );
    let alter = Regex::new(&format!(r"(?is)^ALTER\s+TABLE\s+{target}\s+((?:ADD\s+(?:COLUMN|INDEX|UNIQUE)|MODIFY\s+COLUMN)\s+.+)$")).unwrap();
    if let Some(capture) = alter.captures(sql) {
        return Ok(capture[1].trim().to_string());
    }
    let index = Regex::new(&format!(
        r"(?is)^CREATE\s+(UNIQUE\s+)?INDEX\s+(`(?:``|[^`])+`)\s+ON\s+{target}\s+(\(.+\))$"
    ))
    .unwrap();
    if let Some(capture) = index.captures(sql) {
        return Ok(format!(
            "ADD {}INDEX {} {}",
            if capture.get(1).is_some() {
                "UNIQUE "
            } else {
                ""
            },
            &capture[2],
            &capture[3]
        ));
    }
    Err("This statement is not a supported add/modify-column or create-index operation on the selected table. Column renames and primary-key changes are not enabled.".into())
}

pub fn credential_file(user: &str, password: &str) -> Result<String, String> {
    for value in [user, password] {
        if value.contains(['\r', '\n', '\0']) || value.contains("\"\"\"") {
            return Err("gh-ost credentials contain unsupported INI delimiters".into());
        }
    }
    Ok(format!(
        "[client]\nuser = \"\"\"{user}\"\"\"\npassword = \"\"\"{password}\"\"\"\n"
    ))
}

pub fn arguments(
    request: &Request,
    params: &ConnectionParams,
    replicas: &[Endpoint],
    dir: &Path,
    gate: &str,
) -> Result<Vec<String>, String> {
    if replicas.is_empty() {
        return Err("At least one verified read replica is required".into());
    }
    let mut args = vec![
        format!(
            "--host={}",
            params.host.as_deref().ok_or("Missing primary hostname")?
        ),
        format!("--port={}", params.port.unwrap_or(3306)),
        format!("--database={}", request.database),
        format!("--table={}", request.table),
        format!("--alter={}", alter_clause(request)?),
        format!("--conf={}", dir.join("credentials.cnf").display()),
        "--allow-on-master".into(),
        "--assume-rbr".into(),
        "--execute".into(),
        "--verbose".into(),
        "--panic-on-warnings".into(),
        "--timestamp-old-table".into(),
        "--chunk-size=500".into(),
        "--dml-batch-size=10".into(),
        format!("--max-lag-millis={LAG_LIMIT_MS}"),
        "--mysql-timeout=5".into(),
        "--cut-over-lock-timeout-seconds=1".into(),
        format!(
            "--throttle-control-replicas={}",
            replicas
                .iter()
                .map(Endpoint::address)
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!("--throttle-http={gate}"),
        "--throttle-http-timeout-millis=500".into(),
        format!("--throttle-flag-file={}", dir.join("pause").display()),
        format!(
            "--throttle-additional-flag-file={}",
            dir.join("pause-all").display()
        ),
        format!("--panic-flag-file={}", dir.join("cancel").display()),
        // Relative socket paths also work in Windows AF_UNIX and avoid MAX_PATH limits.
        "--serve-socket-file=control.sock".into(),
        format!(
            "--replica-server-id={}",
            1_000_000_000u32 + (uuid::Uuid::new_v4().as_u128() % 1_000_000_000) as u32
        ),
    ];
    if request.aliyun_rds {
        args.push("--aliyun-rds".into());
    }
    match params.ssl_mode.as_deref().unwrap_or("required") {
        "disabled" | "disable" => {}
        "required" | "require" | "preferred" | "prefer" => {
            args.extend(["--ssl".into(), "--ssl-allow-insecure".into()]);
        }
        "verify_ca" | "verify_identity" => {
            if params.ssh_enabled.unwrap_or(false) || params.k8s_enabled.unwrap_or(false) {
                return Err("Verified TLS through a tunnel is not supported by the gh-ost CLI; no TLS downgrade will be performed".into());
            }
            args.push("--ssl".into());
        }
        _ => return Err("Unsupported TLS mode; refusing to downgrade transport security".into()),
    }
    for (flag, path) in [
        ("ssl-ca", &params.ssl_ca),
        ("ssl-cert", &params.ssl_cert),
        ("ssl-key", &params.ssl_key),
    ] {
        if let Some(path) = path.as_ref().filter(|p| !p.is_empty()) {
            args.push(format!("--{flag}={path}"));
        }
    }
    Ok(args)
}

pub fn progress(line: &str) -> Option<f64> {
    let rest = line.split("Copy: ").nth(1)?;
    let percent = rest
        .split_whitespace()
        .nth(1)?
        .trim_end_matches(';')
        .strip_suffix('%')?;
    let value: f64 = percent.parse().ok()?;
    value.is_finite().then_some(value.clamp(0.0, 100.0))
}

pub fn redact(line: &str, secret: &str) -> String {
    let mut safe = line.to_string();
    if !secret.is_empty() {
        safe = safe
            .replace(secret, "[REDACTED]")
            .replace(&urlencoding::encode(secret).into_owned(), "[REDACTED]");
    }
    safe.chars().take(8_192).collect()
}
