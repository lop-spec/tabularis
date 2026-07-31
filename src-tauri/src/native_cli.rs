use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use url::Url;

use crate::drivers::driver_trait::{DriverCapabilities, PluginManifest};
use crate::models::ConnectionParams;

pub const MONGODB_DRIVER_ID: &str = "mongodb";
pub const REDIS_DRIVER_ID: &str = "redis";
pub const OUTPUT_EVENT: &str = "native-cli-output";
pub const EXIT_EVENT: &str = "native-cli-exit";

const OUTPUT_BUFFER_LIMIT: usize = 4 * 1024 * 1024;
const TEST_TIMEOUT: Duration = Duration::from_secs(20);
static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCliKind {
    Mongosh,
    RedisCli,
}

impl NativeCliKind {
    pub fn from_driver_id(driver_id: &str) -> Option<Self> {
        match driver_id {
            MONGODB_DRIVER_ID => Some(Self::Mongosh),
            REDIS_DRIVER_ID | "redis-rust" | "redis-go" => Some(Self::RedisCli),
            _ => None,
        }
    }

    fn executable_name(self) -> &'static str {
        match self {
            Self::Mongosh => "mongosh",
            Self::RedisCli => "redis-cli",
        }
    }

    fn environment_override(self) -> &'static str {
        match self {
            Self::Mongosh => "TABULARIS_MONGOSH_PATH",
            Self::RedisCli => "TABULARIS_REDIS_CLI_PATH",
        }
    }

    fn manifest_value(self) -> &'static str {
        match self {
            Self::Mongosh => "mongosh",
            Self::RedisCli => "redis-cli",
        }
    }
}

pub fn is_native_cli_driver(driver_id: &str) -> bool {
    NativeCliKind::from_driver_id(driver_id).is_some()
}

pub fn canonical_driver_id(driver_id: &str) -> Option<&'static str> {
    match NativeCliKind::from_driver_id(driver_id)? {
        NativeCliKind::Mongosh => Some(MONGODB_DRIVER_ID),
        NativeCliKind::RedisCli => Some(REDIS_DRIVER_ID),
    }
}

pub fn manifests() -> [PluginManifest; 2] {
    [
        native_manifest(
            MONGODB_DRIVER_ID,
            "MongoDB Shell",
            27017,
            NativeCliKind::Mongosh,
            "mongodb",
            vec!["document".to_string(), "nosql".to_string()],
            "#10b981",
        ),
        native_manifest(
            REDIS_DRIVER_ID,
            "Redis CLI",
            6379,
            NativeCliKind::RedisCli,
            "redis",
            vec!["key-value".to_string(), "nosql".to_string()],
            "#ef4444",
        ),
    ]
}

fn native_manifest(
    id: &str,
    name: &str,
    default_port: u16,
    kind: NativeCliKind,
    engine: &str,
    paradigms: Vec<String>,
    color: &str,
) -> PluginManifest {
    let (connection_string_example, connection_uri_schemes) = match kind {
        NativeCliKind::Mongosh => (
            "mongodb://localhost:27017/test",
            vec!["mongodb".to_string(), "mongodb+srv".to_string()],
        ),
        NativeCliKind::RedisCli => (
            "redis://localhost:6379/0",
            vec!["redis".to_string(), "rediss".to_string()],
        ),
    };

    PluginManifest {
        id: id.to_string(),
        name: name.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: format!(
            "Built-in native {} PTY console. Uses the real CLI runtime instead of a database plugin.",
            kind.manifest_value()
        ),
        default_port: Some(default_port),
        capabilities: DriverCapabilities {
            connection_string: true,
            connection_string_example: connection_string_example.to_string(),
            connection_uri: true,
            connection_uri_schemes,
            supports_ssl: true,
            manage_tables: false,
            console_only: true,
            native_cli: Some(kind.manifest_value().to_string()),
            ..DriverCapabilities::default()
        },
        is_builtin: true,
        engine: Some(engine.to_string()),
        paradigms,
        default_username: String::new(),
        color: color.to_string(),
        icon: "terminal".to_string(),
        settings: Vec::new(),
        ui_extensions: None,
    }
}

pub async fn register_manifests() {
    for manifest in manifests() {
        crate::drivers::registry::register_manifest(manifest).await;
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NativeCliStartResponse {
    pub session_id: String,
    pub instance_id: String,
    pub running: bool,
    pub reused: bool,
    pub process_id: Option<u32>,
    pub executable: String,
    pub output_base64: String,
    pub output_sequence: u64,
    pub output_truncated: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NativeCliOutputEvent {
    session_id: String,
    instance_id: String,
    sequence: u64,
    data_base64: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NativeCliExitEvent {
    session_id: String,
    instance_id: String,
    exit_code: Option<u32>,
    signal: Option<String>,
    error: Option<String>,
}

#[derive(Default)]
struct OutputBuffer {
    bytes: Vec<u8>,
    sequence: u64,
    truncated: bool,
}

impl OutputBuffer {
    fn push(&mut self, bytes: &[u8]) -> u64 {
        self.sequence = self.sequence.saturating_add(1);
        self.bytes.extend_from_slice(bytes);
        if self.bytes.len() > OUTPUT_BUFFER_LIMIT {
            let overflow = self.bytes.len() - OUTPUT_BUFFER_LIMIT;
            self.bytes.drain(..overflow);
            self.truncated = true;
        }
        self.sequence
    }

    fn snapshot(&self) -> (String, u64, bool) {
        (
            base64::engine::general_purpose::STANDARD.encode(&self.bytes),
            self.sequence,
            self.truncated,
        )
    }

    fn clear(&mut self) {
        self.bytes.clear();
        self.truncated = false;
    }
}

struct NativeCliSession {
    connection_id: String,
    instance_id: String,
    executable: PathBuf,
    process_id: Option<u32>,
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    output: Mutex<OutputBuffer>,
    temp_files: Vec<PathBuf>,
}

impl NativeCliSession {
    fn snapshot(&self, session_id: &str, reused: bool) -> NativeCliStartResponse {
        let (output_base64, output_sequence, output_truncated) =
            lock_unpoison(&self.output).snapshot();
        NativeCliStartResponse {
            session_id: session_id.to_string(),
            instance_id: self.instance_id.clone(),
            running: true,
            reused,
            process_id: self.process_id,
            executable: self.executable.to_string_lossy().into_owned(),
            output_base64,
            output_sequence,
            output_truncated,
        }
    }

    fn kill(&self) -> Result<(), String> {
        lock_unpoison(&self.killer)
            .kill()
            .map_err(|error| format!("Failed to stop native CLI process: {error}"))
    }

    fn cleanup_temp_files(&self) {
        for path in &self.temp_files {
            if let Err(error) = fs::remove_file(path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!(
                        "Failed to remove native CLI bootstrap file {}: {}",
                        path.display(),
                        error
                    );
                }
            }
        }
    }
}

#[derive(Default)]
pub struct NativeCliState {
    sessions: Mutex<HashMap<String, Arc<NativeCliSession>>>,
}

impl Drop for NativeCliState {
    fn drop(&mut self) {
        let sessions = self
            .sessions
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for session in sessions.values() {
            let _ = session.kill();
            session.cleanup_temp_files();
        }
        sessions.clear();
    }
}

fn lock_unpoison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug)]
struct PreparedCommand {
    executable: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    temp_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
enum CommandMode {
    Interactive,
    Test,
}

pub fn start_session<R: Runtime>(
    app: &AppHandle<R>,
    state: Arc<NativeCliState>,
    connection_id: String,
    session_id: String,
    params: &ConnectionParams,
    cols: u16,
    rows: u16,
) -> Result<NativeCliStartResponse, String> {
    validate_session_id(&session_id)?;
    let kind = NativeCliKind::from_driver_id(&params.driver)
        .ok_or_else(|| format!("Driver '{}' is not a native CLI driver", params.driver))?;

    if let Some(existing) = lock_unpoison(&state.sessions).get(&session_id).cloned() {
        resize_session(&state, &session_id, cols, rows)?;
        return Ok(existing.snapshot(&session_id, true));
    }

    let prepared = prepare_command(
        app,
        params,
        kind,
        CommandMode::Interactive,
        Some(&session_id),
    )?;
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: rows.max(2),
            cols: cols.max(2),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to open native CLI PTY: {error:#}"))?;

    let mut command = CommandBuilder::new(&prepared.executable);
    for argument in &prepared.args {
        command.arg(argument);
    }
    for (name, value) in &prepared.env {
        command.env(name, value);
    }

    let mut child = pair.slave.spawn_command(command).map_err(|error| {
        format!(
            "Failed to start {} at '{}': {error:#}",
            kind.executable_name(),
            prepared.executable.display()
        )
    })?;
    drop(pair.slave);

    let process_id = child.process_id();
    let killer = child.clone_killer();
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("Failed to open native CLI output stream: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("Failed to open native CLI input stream: {error}"))?;

    let session = Arc::new(NativeCliSession {
        connection_id,
        instance_id: format!(
            "{}:{}",
            process_id.unwrap_or_default(),
            NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed)
        ),
        executable: prepared.executable,
        process_id,
        writer: Mutex::new(writer),
        master: Mutex::new(pair.master),
        killer: Mutex::new(killer),
        output: Mutex::new(OutputBuffer::default()),
        temp_files: prepared.temp_files,
    });

    lock_unpoison(&state.sessions).insert(session_id.clone(), Arc::clone(&session));

    let app_for_reader = app.clone();
    let reader_session_id = session_id.clone();
    let reader_session = Arc::clone(&session);
    thread::Builder::new()
        .name(format!("native-cli-reader-{session_id}"))
        .spawn(move || read_output_loop(app_for_reader, reader_session_id, reader_session, reader))
        .map_err(|error| format!("Failed to start native CLI output reader: {error}"))?;

    let app_for_wait = app.clone();
    let wait_session_id = session_id.clone();
    let wait_state = Arc::clone(&state);
    let wait_session = Arc::clone(&session);
    thread::Builder::new()
        .name(format!("native-cli-wait-{session_id}"))
        .spawn(move || {
            let result = child.wait();
            let mut sessions = lock_unpoison(&wait_state.sessions);
            let owns_slot = sessions
                .get(&wait_session_id)
                .is_some_and(|current| Arc::ptr_eq(current, &wait_session));
            if owns_slot {
                sessions.remove(&wait_session_id);
            }
            drop(sessions);
            wait_session.cleanup_temp_files();

            let payload = match result {
                Ok(status) => NativeCliExitEvent {
                    session_id: wait_session_id,
                    instance_id: wait_session.instance_id.clone(),
                    exit_code: Some(status.exit_code()),
                    signal: status.signal().map(str::to_string),
                    error: None,
                },
                Err(error) => NativeCliExitEvent {
                    session_id: wait_session_id,
                    instance_id: wait_session.instance_id.clone(),
                    exit_code: None,
                    signal: None,
                    error: Some(error.to_string()),
                },
            };
            let _ = app_for_wait.emit(EXIT_EVENT, payload);
        })
        .map_err(|error| format!("Failed to start native CLI process monitor: {error}"))?;

    Ok(session.snapshot(&session_id, false))
}

fn read_output_loop<R: Runtime>(
    app: AppHandle<R>,
    session_id: String,
    session: Arc<NativeCliSession>,
    mut reader: Box<dyn Read + Send>,
) {
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(size) => {
                let bytes = &chunk[..size];
                let sequence = lock_unpoison(&session.output).push(bytes);
                let payload = NativeCliOutputEvent {
                    session_id: session_id.clone(),
                    instance_id: session.instance_id.clone(),
                    sequence,
                    data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                };
                if let Err(error) = app.emit(OUTPUT_EVENT, payload) {
                    log::debug!("Failed to emit native CLI output: {}", error);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                log::debug!("Native CLI output reader ended: {}", error);
                break;
            }
        }
    }
}

pub fn write_session(state: &NativeCliState, session_id: &str, data: &str) -> Result<(), String> {
    let session = session_for(state, session_id)?;
    let mut writer = lock_unpoison(&session.writer);
    writer
        .write_all(data.as_bytes())
        .and_then(|_| writer.flush())
        .map_err(|error| format!("Failed to write native CLI input: {error}"))
}

pub fn interrupt_session(state: &NativeCliState, session_id: &str) -> Result<(), String> {
    write_session(state, session_id, "\u{3}")
}

pub fn resize_session(
    state: &NativeCliState,
    session_id: &str,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let session = session_for(state, session_id)?;
    let result = lock_unpoison(&session.master)
        .resize(PtySize {
            rows: rows.max(2),
            cols: cols.max(2),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to resize native CLI terminal: {error:#}"));
    result
}

pub fn clear_session_output(state: &NativeCliState, session_id: &str) -> Result<(), String> {
    let session = session_for(state, session_id)?;
    lock_unpoison(&session.output).clear();
    Ok(())
}

pub fn close_session(state: &NativeCliState, session_id: &str) -> Result<bool, String> {
    let session = lock_unpoison(&state.sessions).remove(session_id);
    let Some(session) = session else {
        return Ok(false);
    };
    let result = session.kill();
    session.cleanup_temp_files();
    result.map(|_| true)
}

pub fn close_connection_sessions(state: &NativeCliState, connection_id: &str) {
    let ids: Vec<String> = lock_unpoison(&state.sessions)
        .iter()
        .filter_map(|(id, session)| (session.connection_id == connection_id).then(|| id.clone()))
        .collect();
    for id in ids {
        if let Err(error) = close_session(state, &id) {
            log::warn!("Failed to close native CLI session '{}': {}", id, error);
        }
    }
}

fn session_for(state: &NativeCliState, session_id: &str) -> Result<Arc<NativeCliSession>, String> {
    lock_unpoison(&state.sessions)
        .get(session_id)
        .cloned()
        .ok_or_else(|| format!("Native CLI session '{}' is not running", session_id))
}

fn validate_session_id(session_id: &str) -> Result<(), String> {
    if session_id.is_empty()
        || session_id.len() > 160
        || !session_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_:".contains(character))
    {
        return Err("Invalid native CLI session id".to_string());
    }
    Ok(())
}

pub async fn test_connection<R: Runtime>(
    app: AppHandle<R>,
    params: ConnectionParams,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || test_connection_blocking(&app, &params))
        .await
        .map_err(|error| format!("Native CLI connection test task failed: {error}"))?
}

fn test_connection_blocking<R: Runtime>(
    app: &AppHandle<R>,
    params: &ConnectionParams,
) -> Result<String, String> {
    let kind = NativeCliKind::from_driver_id(&params.driver)
        .ok_or_else(|| format!("Driver '{}' is not a native CLI driver", params.driver))?;
    let prepared = prepare_command(app, params, kind, CommandMode::Test, None)?;

    let mut command = Command::new(&prepared.executable);
    command
        .args(&prepared.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in &prepared.env {
        command.env(name, value);
    }
    hide_windows_process(&mut command);

    let mut child = command.spawn().map_err(|error| {
        format!(
            "Failed to start {} at '{}': {}",
            kind.executable_name(),
            prepared.executable.display(),
            error
        )
    })?;
    let deadline = Instant::now() + TEST_TIMEOUT;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("Failed while testing native CLI connection: {error}"))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "{} connection test timed out after {} seconds",
                kind.executable_name(),
                TEST_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(50));
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut stream) = child.stdout.take() {
        let _ = stream.read_to_string(&mut stdout);
    }
    if let Some(mut stream) = child.stderr.take() {
        let _ = stream.read_to_string(&mut stderr);
    }

    if !status.success() {
        let detail = redact_connection_secrets(
            first_nonempty(
                &stderr,
                &stdout,
                "native CLI exited without an error message",
            ),
            params,
        );
        return Err(format!(
            "{} connection test failed: {}",
            kind.executable_name(),
            detail.trim()
        ));
    }
    if kind == NativeCliKind::RedisCli && !stdout.to_ascii_uppercase().contains("PONG") {
        return Err(format!(
            "redis-cli exited successfully but did not return PONG: {}",
            redact_connection_secrets(stdout.trim(), params)
        ));
    }

    Ok(format!(
        "{} connection successful ({})",
        kind.executable_name(),
        prepared.executable.display()
    ))
}

fn prepare_command<R: Runtime>(
    app: &AppHandle<R>,
    params: &ConnectionParams,
    kind: NativeCliKind,
    mode: CommandMode,
    session_id: Option<&str>,
) -> Result<PreparedCommand, String> {
    let executable = resolve_executable(app, params.native_cli_path.as_deref(), kind)?;
    let mut args = Vec::<OsString>::new();
    let mut env = vec![
        (OsString::from("TERM"), OsString::from("xterm-256color")),
        (OsString::from("COLORTERM"), OsString::from("truecolor")),
    ];
    let mut temp_files = Vec::new();
    let extra_args = parse_cli_args(params.native_cli_args.as_deref().unwrap_or(""))?;

    match kind {
        NativeCliKind::Mongosh => {
            let uri = build_mongodb_uri(params)?;
            env.push((OsString::from("TABULARIS_MONGODB_URI"), OsString::from(uri)));
            append_mongodb_tls_args(params, &mut args);
            match mode {
                CommandMode::Interactive => {
                    let id = session_id.ok_or("Native CLI session id is required")?;
                    let cache_dir = app
                        .path()
                        .app_cache_dir()
                        .map_err(|error| error.to_string())?;
                    let bootstrap_dir = cache_dir.join("native-cli");
                    fs::create_dir_all(&bootstrap_dir).map_err(|error| {
                        format!(
                            "Failed to create native CLI bootstrap directory '{}': {}",
                            bootstrap_dir.display(),
                            error
                        )
                    })?;
                    let bootstrap = bootstrap_dir.join(format!("mongosh-{}.js", safe_id(id)));
                    fs::write(
                        &bootstrap,
                        "db = connect(process.env.TABULARIS_MONGODB_URI);\n",
                    )
                    .map_err(|error| {
                        format!(
                            "Failed to write mongosh bootstrap '{}': {}",
                            bootstrap.display(),
                            error
                        )
                    })?;
                    temp_files.push(bootstrap.clone());
                    args.extend([
                        OsString::from("--nodb"),
                        OsString::from("--shell"),
                        OsString::from("--file"),
                        bootstrap.into_os_string(),
                    ]);
                }
                CommandMode::Test => {
                    args.extend([
                        OsString::from("--nodb"),
                        OsString::from("--quiet"),
                        OsString::from("--eval"),
                        OsString::from(
                            "const c=connect(process.env.TABULARIS_MONGODB_URI); quit(c.runCommand({ping:1}).ok===1?0:2);",
                        ),
                    ]);
                }
            }
            args.extend(extra_args);
        }
        NativeCliKind::RedisCli => {
            append_redis_connection(params, &mut args, &mut env)?;
            args.extend(extra_args);
            if matches!(mode, CommandMode::Test) {
                args.push(OsString::from("PING"));
            }
        }
    }

    Ok(PreparedCommand {
        executable,
        args,
        env,
        temp_files,
    })
}

fn append_mongodb_tls_args(params: &ConnectionParams, args: &mut Vec<OsString>) {
    if ssl_enabled(params.ssl_mode.as_deref()) {
        args.push(OsString::from("--tls"));
    }
    if let Some(path) = nonempty(params.ssl_ca.as_deref()) {
        args.extend([OsString::from("--tlsCAFile"), OsString::from(path)]);
    }
    if let Some(path) = nonempty(params.ssl_cert.as_deref()) {
        args.extend([
            OsString::from("--tlsCertificateKeyFile"),
            OsString::from(path),
        ]);
    }
}

fn append_redis_connection(
    params: &ConnectionParams,
    args: &mut Vec<OsString>,
    env: &mut Vec<(OsString, OsString)>,
) -> Result<(), String> {
    let mut password = nonempty(params.password.as_deref()).map(str::to_string);
    if let Some(raw_uri) = nonempty(params.connection_uri.as_deref()) {
        let mut uri = Url::parse(raw_uri)
            .map_err(|error| format!("Invalid Redis connection URI: {error}"))?;
        if !matches!(uri.scheme(), "redis" | "rediss") {
            return Err("Redis connection URI must use redis:// or rediss://".to_string());
        }
        if uri.username().is_empty() {
            if let Some(username) = nonempty(params.username.as_deref()) {
                uri.set_username(username)
                    .map_err(|_| "Invalid username in Redis connection URI".to_string())?;
            }
        }
        if password.is_none() {
            password = uri
                .password()
                .map(|value| {
                    urlencoding::decode(value)
                        .map(|decoded| decoded.into_owned())
                        .map_err(|error| format!("Invalid password encoding in Redis URI: {error}"))
                })
                .transpose()?;
        }
        if uri.password().is_some() {
            uri.set_password(None)
                .map_err(|_| "Failed to remove password from Redis URI argv".to_string())?;
        }
        args.extend([OsString::from("-u"), OsString::from(uri.as_str())]);
    } else {
        args.extend([
            OsString::from("-h"),
            OsString::from(params.host.as_deref().unwrap_or("127.0.0.1")),
            OsString::from("-p"),
            OsString::from(params.port.unwrap_or(6379).to_string()),
        ]);
        if let Some(username) = nonempty(params.username.as_deref()) {
            args.extend([OsString::from("--user"), OsString::from(username)]);
        }
        let database = params.database.primary().trim();
        if !database.is_empty() {
            args.extend([OsString::from("-n"), OsString::from(database)]);
        }
    }

    if let Some(password) = password.filter(|value| !value.is_empty()) {
        env.push((OsString::from("REDISCLI_AUTH"), OsString::from(password)));
    }
    if ssl_enabled(params.ssl_mode.as_deref()) {
        args.push(OsString::from("--tls"));
    }
    if let Some(path) = nonempty(params.ssl_ca.as_deref()) {
        args.extend([OsString::from("--cacert"), OsString::from(path)]);
    }
    if let Some(path) = nonempty(params.ssl_cert.as_deref()) {
        args.extend([OsString::from("--cert"), OsString::from(path)]);
    }
    if let Some(path) = nonempty(params.ssl_key.as_deref()) {
        args.extend([OsString::from("--key"), OsString::from(path)]);
    }
    Ok(())
}

fn build_mongodb_uri(params: &ConnectionParams) -> Result<String, String> {
    if let Some(raw_uri) = nonempty(params.connection_uri.as_deref()) {
        let mut uri = Url::parse(raw_uri)
            .map_err(|error| format!("Invalid MongoDB connection URI: {error}"))?;
        if !matches!(uri.scheme(), "mongodb" | "mongodb+srv") {
            return Err("MongoDB connection URI must use mongodb:// or mongodb+srv://".to_string());
        }
        apply_url_credentials(
            &mut uri,
            params.username.as_deref(),
            params.password.as_deref(),
        )?;
        return Ok(uri.to_string());
    }

    let host = params.host.as_deref().unwrap_or("127.0.0.1");
    let mut uri = Url::parse("mongodb://127.0.0.1").map_err(|error| error.to_string())?;
    uri.set_host(Some(host))
        .map_err(|_| format!("Invalid MongoDB host: {host}"))?;
    uri.set_port(Some(params.port.unwrap_or(27017)))
        .map_err(|_| "Invalid MongoDB port".to_string())?;
    let database = params.database.primary().trim();
    if !database.is_empty() {
        uri.set_path(database);
    }
    apply_url_credentials(
        &mut uri,
        params.username.as_deref(),
        params.password.as_deref(),
    )?;
    Ok(uri.to_string())
}

fn apply_url_credentials(
    uri: &mut Url,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<(), String> {
    if uri.username().is_empty() {
        if let Some(username) = nonempty(username) {
            uri.set_username(username)
                .map_err(|_| "Invalid username in native CLI connection".to_string())?;
        }
    }
    if uri.password().is_none() {
        if let Some(password) = password.filter(|value| !value.is_empty()) {
            uri.set_password(Some(password))
                .map_err(|_| "Invalid password in native CLI connection".to_string())?;
        }
    }
    Ok(())
}

fn resolve_executable<R: Runtime>(
    app: &AppHandle<R>,
    explicit: Option<&str>,
    kind: NativeCliKind,
) -> Result<PathBuf, String> {
    let executable_name = executable_file_name(kind);
    if let Some(path) = nonempty(explicit) {
        return validate_executable_candidate(PathBuf::from(path), &executable_name).ok_or_else(
            || {
                format!(
                    "Configured {} executable was not found or is not a file: {}",
                    kind.executable_name(),
                    path
                )
            },
        );
    }

    if let Some(path) = std::env::var_os(kind.environment_override()) {
        if let Some(resolved) = validate_executable_candidate(PathBuf::from(path), &executable_name)
        {
            return Ok(resolved);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.extend([
            resource_dir
                .join("native-cli")
                .join(kind.executable_name())
                .join("bin"),
            resource_dir.join("native-cli").join(kind.executable_name()),
            resource_dir.join("native-cli"),
            resource_dir,
        ]);
    }
    if let Ok(app_data_dir) = app.path().app_data_dir() {
        candidates.extend([
            app_data_dir
                .join("native-cli")
                .join(kind.executable_name())
                .join("bin"),
            app_data_dir.join("native-cli").join(kind.executable_name()),
            app_data_dir.join("native-cli"),
        ]);
    }
    if let Ok(current_executable) = std::env::current_exe() {
        if let Some(parent) = current_executable.parent() {
            candidates.extend([
                parent
                    .join("native-cli")
                    .join(kind.executable_name())
                    .join("bin"),
                parent.join("native-cli").join(kind.executable_name()),
                parent.join("native-cli"),
                parent.to_path_buf(),
            ]);
        }
    }
    for candidate in candidates {
        if let Some(resolved) = validate_executable_candidate(candidate, &executable_name) {
            return Ok(resolved);
        }
    }
    if let Ok(path) = which::which(kind.executable_name()) {
        return Ok(path);
    }

    Err(format!(
        "{} runtime not found. Bundle it under native-cli/{}/, set {}, configure an executable path in the connection, or add it to PATH.",
        kind.executable_name(),
        kind.executable_name(),
        kind.environment_override()
    ))
}

fn validate_executable_candidate(candidate: PathBuf, executable_name: &OsStr) -> Option<PathBuf> {
    let path = if candidate.is_dir() {
        candidate.join(executable_name)
    } else {
        candidate
    };
    path.is_file().then_some(path)
}

fn executable_file_name(kind: NativeCliKind) -> OsString {
    #[cfg(windows)]
    {
        OsString::from(format!("{}.exe", kind.executable_name()))
    }
    #[cfg(not(windows))]
    {
        OsString::from(kind.executable_name())
    }
}

fn parse_cli_args(input: &str) -> Result<Vec<OsString>, String> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut characters = input.chars().peekable();

    while let Some(character) = characters.next() {
        match quote {
            Some(active) if character == active => quote = None,
            Some('\'') => current.push(character),
            Some('"') if character == '\\' => {
                if let Some(next) = characters.peek().copied() {
                    if matches!(next, '"' | '\\') {
                        current.push(characters.next().unwrap_or(next));
                    } else {
                        current.push(character);
                    }
                } else {
                    current.push(character);
                }
            }
            Some(_) => current.push(character),
            None if matches!(character, '\'' | '"') => quote = Some(character),
            None if character.is_whitespace() => {
                if !current.is_empty() {
                    arguments.push(OsString::from(std::mem::take(&mut current)));
                }
            }
            None if character == '\\' => {
                if let Some(next) = characters.peek().copied() {
                    if next.is_whitespace() || matches!(next, '\'' | '"' | '\\') {
                        current.push(characters.next().unwrap_or(next));
                    } else {
                        current.push(character);
                    }
                } else {
                    current.push(character);
                }
            }
            None => current.push(character),
        }
    }
    if let Some(active) = quote {
        return Err(format!(
            "Unterminated {active} quote in native CLI arguments"
        ));
    }
    if !current.is_empty() {
        arguments.push(OsString::from(current));
    }
    Ok(arguments)
}

fn ssl_enabled(mode: Option<&str>) -> bool {
    matches!(
        mode.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some(
            "require" | "required" | "verify-ca" | "verify-full" | "verify_ca" | "verify_identity"
        )
    )
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn safe_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn first_nonempty<'a>(first: &'a str, second: &'a str, fallback: &'a str) -> &'a str {
    if !first.trim().is_empty() {
        first
    } else if !second.trim().is_empty() {
        second
    } else {
        fallback
    }
}

fn redact_connection_secrets(value: &str, params: &ConnectionParams) -> String {
    let mut secrets = Vec::new();
    if let Some(password) = nonempty(params.password.as_deref()) {
        secrets.push(password.to_string());
    }
    if let Some(raw_uri) = nonempty(params.connection_uri.as_deref()) {
        if let Ok(uri) = Url::parse(raw_uri) {
            if let Some(password) = nonempty(uri.password()) {
                secrets.push(password.to_string());
                if let Ok(decoded) = urlencoding::decode(password) {
                    secrets.push(decoded.into_owned());
                }
            }
        }
    }
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    secrets.dedup();

    let mut redacted = value.to_string();
    for secret in secrets {
        redacted = redacted.replace(&secret, "******");
    }
    redacted
}

#[cfg(windows)]
fn hide_windows_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_windows_process(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DatabaseSelection;

    #[test]
    fn native_driver_ids_are_reserved_and_redis_plugins_migrate() {
        assert_eq!(canonical_driver_id("mongodb"), Some(MONGODB_DRIVER_ID));
        assert_eq!(canonical_driver_id("redis"), Some(REDIS_DRIVER_ID));
        assert_eq!(canonical_driver_id("redis-rust"), Some(REDIS_DRIVER_ID));
        assert_eq!(canonical_driver_id("redis-go"), Some(REDIS_DRIVER_ID));
        assert_eq!(canonical_driver_id("postgres"), None);
    }

    #[test]
    fn parses_direct_argv_without_invoking_a_shell() {
        let parsed = parse_cli_args(
            r#"--authenticationDatabase admin --tlsCAFile "C:\certs\root ca.pem" --flag='a b'"#,
        )
        .unwrap();
        assert_eq!(
            parsed,
            vec![
                "--authenticationDatabase",
                "admin",
                "--tlsCAFile",
                r"C:\certs\root ca.pem",
                "--flag=a b",
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
        );
        assert!(parse_cli_args("--flag \"unterminated").is_err());
    }

    #[test]
    fn mongodb_credentials_are_encoded_in_environment_uri() {
        let params = ConnectionParams {
            driver: MONGODB_DRIVER_ID.to_string(),
            host: Some("mongo.internal".to_string()),
            port: Some(27018),
            username: Some("user@example.com".to_string()),
            password: Some("p@ss/word".to_string()),
            database: DatabaseSelection::Single("analytics".to_string()),
            ..ConnectionParams::default()
        };
        let uri = build_mongodb_uri(&params).unwrap();
        assert!(uri.starts_with(
            "mongodb://user%40example.com:p%40ss%2Fword@mongo.internal:27018/analytics"
        ));
    }

    #[test]
    fn redis_password_is_never_added_to_argv() {
        let params = ConnectionParams {
            driver: REDIS_DRIVER_ID.to_string(),
            host: Some("redis.internal".to_string()),
            port: Some(6380),
            username: Some("reader".to_string()),
            password: Some("top-secret".to_string()),
            database: DatabaseSelection::Single("2".to_string()),
            ..ConnectionParams::default()
        };
        let mut args = Vec::new();
        let mut env = Vec::new();
        append_redis_connection(&params, &mut args, &mut env).unwrap();
        assert!(!args.iter().any(|value| value == "top-secret"));
        assert!(env
            .iter()
            .any(|(name, value)| { name == "REDISCLI_AUTH" && value == "top-secret" }));
    }

    #[test]
    fn redis_uri_password_is_decoded_and_removed_from_argv() {
        let params = ConnectionParams {
            driver: REDIS_DRIVER_ID.to_string(),
            username: Some("reader".to_string()),
            password: Some(String::new()),
            connection_uri: Some("redis://:p%40ss%2Fword@redis.internal:6380/2".to_string()),
            ..ConnectionParams::default()
        };
        let mut args = Vec::new();
        let mut env = Vec::new();
        append_redis_connection(&params, &mut args, &mut env).unwrap();

        assert!(!args
            .iter()
            .any(|value| value.to_string_lossy().contains("p%40ss%2Fword")));
        assert!(args
            .iter()
            .any(|value| value.to_string_lossy().contains("reader@redis.internal")));
        assert!(env
            .iter()
            .any(|(name, value)| { name == "REDISCLI_AUTH" && value == "p@ss/word" }));
    }

    #[test]
    fn native_uri_schemes_and_tls_modes_are_validated() {
        let mongodb = ConnectionParams {
            connection_uri: Some("https://mongo.internal/test".to_string()),
            ..ConnectionParams::default()
        };
        assert!(build_mongodb_uri(&mongodb).is_err());

        let redis = ConnectionParams {
            connection_uri: Some("https://redis.internal/0".to_string()),
            ..ConnectionParams::default()
        };
        assert!(append_redis_connection(&redis, &mut Vec::new(), &mut Vec::new()).is_err());

        assert!(ssl_enabled(Some("required")));
        assert!(ssl_enabled(Some("verify_ca")));
        assert!(ssl_enabled(Some("verify_identity")));
        assert!(!ssl_enabled(Some("preferred")));
        assert!(!ssl_enabled(Some("disabled")));
    }

    #[test]
    fn connection_errors_redact_plain_and_encoded_uri_passwords() {
        let params = ConnectionParams {
            connection_uri: Some("redis://reader:p%40ss%2Fword@redis.internal:6380/2".to_string()),
            ..ConnectionParams::default()
        };
        let redacted = redact_connection_secrets(
            "auth p@ss/word failed for redis://reader:p%40ss%2Fword@redis.internal",
            &params,
        );
        assert!(!redacted.contains("p@ss/word"));
        assert!(!redacted.contains("p%40ss%2Fword"));
        assert_eq!(redacted.matches("******").count(), 2);
    }

    #[test]
    fn builtins_advertise_console_only_native_runtimes() {
        let manifests = manifests();
        assert!(manifests.iter().all(|manifest| manifest.is_builtin));
        assert!(manifests
            .iter()
            .all(|manifest| manifest.capabilities.console_only));
        assert!(manifests
            .iter()
            .all(|manifest| manifest.capabilities.connection_string));
        assert!(manifests
            .iter()
            .all(|manifest| manifest.capabilities.connection_uri));
        assert_eq!(
            manifests[0].capabilities.native_cli.as_deref(),
            Some("mongosh")
        );
        assert_eq!(
            manifests[0].capabilities.connection_uri_schemes,
            ["mongodb", "mongodb+srv"]
        );
        assert_eq!(
            manifests[1].capabilities.native_cli.as_deref(),
            Some("redis-cli")
        );
        assert_eq!(
            manifests[1].capabilities.connection_uri_schemes,
            ["redis", "rediss"]
        );
    }
}
