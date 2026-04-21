//! Pure helpers to import an EXPLAIN plan from a file on disk.
//!
//! This module contains no I/O side effects (other than `read_explain_file`) so the
//! parser can be unit-tested directly against raw strings.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;
use tauri::{Manager, Runtime, State};

use crate::models::{ExplainNode, ExplainPlan};

/// Holds the path passed via `--explain <FILE>` on the CLI, so the frontend
/// can claim it once the visual-explain window has mounted.
///
/// The slot is cleared on read to avoid re-opening the same plan if the
/// window is navigated.
#[derive(Default)]
pub struct PendingExplainFile(pub Mutex<Option<String>>);

impl PendingExplainFile {
    pub fn set(&self, path: String) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(path);
        }
    }

    pub fn take(&self) -> Option<String> {
        self.0.lock().ok().and_then(|mut guard| guard.take())
    }
}

/// Tauri command: parse an EXPLAIN file from disk.
#[tauri::command]
pub async fn load_explain_from_file(path: String) -> Result<ExplainPlan, String> {
    load_from_file(&path).await
}

/// Tauri command: pop the CLI-provided file path (if any).
///
/// Returns `None` after the first successful read, allowing the window to
/// differentiate "cold start from CLI" from "opened manually".
#[tauri::command]
pub fn get_pending_explain_file(state: State<'_, PendingExplainFile>) -> Option<String> {
    state.take()
}

/// Creates (or focuses) the standalone Visual Explain window.
///
/// Runs fully synchronously so it can be invoked from the Tauri `setup` hook
/// without taking a detour through the async runtime.
pub fn spawn_visual_explain_window<R: Runtime, M: Manager<R>>(
    app: &M,
    file: Option<String>,
) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    if let Some(path) = file.as_ref() {
        if let Some(state) = app.try_state::<PendingExplainFile>() {
            state.set(path.clone());
        }
    }

    if let Some(existing) = app.get_webview_window("visual-explain") {
        existing
            .set_focus()
            .map_err(|e| format!("Failed to focus Visual Explain window: {e}"))?;
        return Ok(());
    }

    WebviewWindowBuilder::new(
        app,
        "visual-explain",
        WebviewUrl::App("/visual-explain".into()),
    )
    .title("tabularis - Visual Explain")
    .inner_size(1280.0, 820.0)
    .min_inner_size(900.0, 600.0)
    .center()
    .build()
    .map_err(|e| format!("Failed to create Visual Explain window: {e}"))?;

    Ok(())
}

/// Tauri command wrapper around [`spawn_visual_explain_window`].
#[tauri::command]
pub async fn open_visual_explain_window<R: Runtime>(
    app: tauri::AppHandle<R>,
    file: Option<String>,
) -> Result<(), String> {
    spawn_visual_explain_window(&app, file)
}

/// Supported source formats that a user may drop into the `--explain` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplainSourceFormat {
    /// Postgres `EXPLAIN (FORMAT JSON [, ANALYZE, BUFFERS])` output.
    PostgresJson,
    /// Postgres default `EXPLAIN` output — indentation-based tree with
    /// `cost=X..Y rows=N width=W` headers and optional `actual time` blocks.
    PostgresText,
}

/// Detect which format the raw file content uses.
///
/// JSON is recognised by the leading `[` or `{`; the text form is recognised by
/// the presence of a Postgres cost header (`cost=X..Y rows=N width=W`).
pub fn detect_format(raw: &str) -> Result<ExplainSourceFormat, String> {
    let trimmed = raw.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        return Ok(ExplainSourceFormat::PostgresJson);
    }
    if looks_like_postgres_text(raw) {
        return Ok(ExplainSourceFormat::PostgresText);
    }
    Err("Unsupported EXPLAIN file format: expected Postgres JSON or text output".to_string())
}

/// A cost header is the most reliable marker of a Postgres text plan.
fn looks_like_postgres_text(raw: &str) -> bool {
    raw.lines()
        .any(|line| line.contains("(cost=") && line.contains("width="))
}

/// Read a file from disk and parse it into an [`ExplainPlan`].
///
/// The heavy file read happens on a blocking thread via `tokio::task::spawn_blocking`
/// so this async wrapper never stalls the runtime.
pub async fn load_from_file(path: &str) -> Result<ExplainPlan, String> {
    let buf = PathBuf::from(path);
    let content = tokio::task::spawn_blocking(move || std::fs::read_to_string(&buf))
        .await
        .map_err(|e| format!("Failed to read explain file: {e}"))?
        .map_err(|e| format!("Failed to read explain file: {e}"))?;

    let plan = parse_explain(&content)?;
    Ok(with_source_path(plan, path))
}

/// Parse a raw EXPLAIN file content into an [`ExplainPlan`].
pub fn parse_explain(raw: &str) -> Result<ExplainPlan, String> {
    match detect_format(raw)? {
        ExplainSourceFormat::PostgresJson => parse_postgres_json(raw),
        ExplainSourceFormat::PostgresText => parse_postgres_text(raw),
    }
}

/// Parse a Postgres `EXPLAIN (FORMAT JSON)` document into [`ExplainPlan`].
///
/// Postgres emits a top-level JSON array with one element per explained statement.
/// We honour this by picking the first element; each object carries a `Plan`
/// node plus optional `Planning Time` / `Execution Time` timings.
pub fn parse_postgres_json(raw: &str) -> Result<ExplainPlan, String> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|e| format!("Failed to parse EXPLAIN JSON: {e}"))?;

    let top = first_statement(&value)?;
    let plan_obj = top
        .get("Plan")
        .ok_or_else(|| "EXPLAIN JSON missing 'Plan' key".to_string())?;

    let mut counter: u32 = 0;
    let root = parse_pg_plan_node(plan_obj, &mut counter);

    let planning_time_ms = top.get("Planning Time").and_then(Value::as_f64);
    let execution_time_ms = top.get("Execution Time").and_then(Value::as_f64);
    let has_analyze_data = root.actual_rows.is_some() || root.actual_time_ms.is_some();

    Ok(ExplainPlan {
        root,
        planning_time_ms,
        execution_time_ms,
        original_query: String::new(),
        driver: "postgres".to_string(),
        has_analyze_data,
        raw_output: Some(raw.to_string()),
    })
}

/// Attach the source path as the `original_query` placeholder so the UI can
/// display "From file: …" without breaking the existing schema.
fn with_source_path(mut plan: ExplainPlan, path: &str) -> ExplainPlan {
    if plan.original_query.is_empty() {
        let filename = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path);
        plan.original_query = format!("-- loaded from {filename}");
    }
    plan
}

fn first_statement(value: &Value) -> Result<&Value, String> {
    match value {
        Value::Array(items) => items
            .first()
            .ok_or_else(|| "EXPLAIN JSON array is empty".to_string()),
        Value::Object(_) => Ok(value),
        _ => Err("EXPLAIN JSON must be an array or object".to_string()),
    }
}

const PG_KNOWN_KEYS: &[&str] = &[
    "Node Type",
    "Relation Name",
    "Startup Cost",
    "Total Cost",
    "Plan Rows",
    "Actual Rows",
    "Actual Total Time",
    "Actual Loops",
    "Shared Hit Blocks",
    "Shared Read Blocks",
    "Filter",
    "Index Cond",
    "Join Type",
    "Hash Cond",
    "Plans",
];

fn parse_pg_plan_node(node: &Value, counter: &mut u32) -> ExplainNode {
    let id = format!("node_{counter}");
    *counter += 1;

    let obj = node.as_object();

    let node_type = node
        .get("Node Type")
        .and_then(Value::as_str)
        .unwrap_or("Unknown")
        .to_string();

    let relation = node
        .get("Relation Name")
        .and_then(Value::as_str)
        .map(String::from);
    let startup_cost = node.get("Startup Cost").and_then(Value::as_f64);
    let total_cost = node.get("Total Cost").and_then(Value::as_f64);
    let plan_rows = node.get("Plan Rows").and_then(Value::as_f64);
    let actual_rows = node.get("Actual Rows").and_then(Value::as_f64);
    let actual_time_ms = node.get("Actual Total Time").and_then(Value::as_f64);
    let actual_loops = node.get("Actual Loops").and_then(Value::as_u64);
    let buffers_hit = node.get("Shared Hit Blocks").and_then(Value::as_u64);
    let buffers_read = node.get("Shared Read Blocks").and_then(Value::as_u64);
    let filter = node.get("Filter").and_then(Value::as_str).map(String::from);
    let index_condition = node
        .get("Index Cond")
        .and_then(Value::as_str)
        .map(String::from);
    let join_type = node
        .get("Join Type")
        .and_then(Value::as_str)
        .map(String::from);
    let hash_condition = node
        .get("Hash Cond")
        .and_then(Value::as_str)
        .map(String::from);

    let mut extra = std::collections::HashMap::new();
    if let Some(map) = obj {
        for (k, v) in map {
            if !PG_KNOWN_KEYS.contains(&k.as_str()) {
                extra.insert(k.clone(), v.clone());
            }
        }
    }

    let children = node
        .get("Plans")
        .and_then(Value::as_array)
        .map(|plans| {
            plans
                .iter()
                .map(|child| parse_pg_plan_node(child, counter))
                .collect()
        })
        .unwrap_or_default();

    ExplainNode {
        id,
        node_type,
        relation,
        startup_cost,
        total_cost,
        plan_rows,
        actual_rows,
        actual_time_ms,
        actual_loops,
        buffers_hit,
        buffers_read,
        filter,
        index_condition,
        join_type,
        hash_condition,
        extra,
        children,
    }
}

// ---------------------------------------------------------------------------
// Postgres text EXPLAIN parser
// ---------------------------------------------------------------------------
//
// Accepts output such as:
//
//     QUERY PLAN
//     --------------------------------------------------------
//      Hash Join  (cost=1.00..10.00 rows=5 width=40) (actual time=0.10..0.20 rows=5 loops=1)
//        Hash Cond: (a.id = b.id)
//        ->  Seq Scan on a  (cost=0.00..5.00 rows=100 width=4)
//        ->  Hash  (cost=0.50..0.50 rows=1 width=36)
//              ->  Seq Scan on b  (cost=0.00..0.50 rows=1 width=36)
//      Planning Time: 0.123 ms
//      Execution Time: 0.456 ms
//     (6 rows)

/// Parse a Postgres text EXPLAIN dump into an [`ExplainPlan`].
pub fn parse_postgres_text(raw: &str) -> Result<ExplainPlan, String> {
    let mut planning_time_ms: Option<f64> = None;
    let mut execution_time_ms: Option<f64> = None;
    let mut has_analyze_data = false;
    let mut counter: u32 = 0;

    let mut stack: Vec<(usize, ExplainNode)> = Vec::new();
    let mut root: Option<ExplainNode> = None;

    for line in raw.lines() {
        let raw_line = line.trim_end();
        if raw_line.trim().is_empty() {
            continue;
        }
        if is_text_metadata_line(raw_line) {
            continue;
        }

        let indent = leading_width(raw_line);
        let trimmed = raw_line.trim_start();

        if let Some(value) = strip_ms_suffix(trimmed, "Planning Time:") {
            planning_time_ms = Some(value);
            continue;
        }
        if let Some(value) = strip_ms_suffix(trimmed, "Execution Time:") {
            execution_time_ms = Some(value);
            continue;
        }

        if let Some(node) = parse_text_node_header(trimmed, &mut counter) {
            if node.actual_rows.is_some() || node.actual_time_ms.is_some() {
                has_analyze_data = true;
            }
            pop_until_shallower(&mut stack, indent, &mut root);
            stack.push((indent, node));
        } else if let Some((_, top)) = stack.last_mut() {
            apply_text_attribute(top, trimmed);
        }
    }

    // Drain remaining frames into their parents (or promote to root).
    pop_until_shallower(&mut stack, 0, &mut root);
    while let Some((_, node)) = stack.pop() {
        promote_or_attach(&mut stack, &mut root, node);
    }

    let root = root.ok_or_else(|| "No plan nodes found in EXPLAIN text output".to_string())?;

    Ok(ExplainPlan {
        root,
        planning_time_ms,
        execution_time_ms,
        original_query: String::new(),
        driver: "postgres".to_string(),
        has_analyze_data,
        raw_output: Some(raw.to_string()),
    })
}

/// Pop stack frames with indent greater than or equal to `cutoff`, attaching
/// them as children of the frame beneath (or promoting them to `root`).
fn pop_until_shallower(
    stack: &mut Vec<(usize, ExplainNode)>,
    cutoff: usize,
    root: &mut Option<ExplainNode>,
) {
    while let Some((indent, _)) = stack.last() {
        if *indent < cutoff {
            break;
        }
        let (_, node) = stack.pop().expect("checked by last()");
        promote_or_attach(stack, root, node);
    }
}

fn promote_or_attach(
    stack: &mut [(usize, ExplainNode)],
    root: &mut Option<ExplainNode>,
    node: ExplainNode,
) {
    if let Some((_, parent)) = stack.last_mut() {
        parent.children.push(node);
    } else if root.is_none() {
        *root = Some(node);
    } else {
        // Two independent roots in the same document (unusual): nest the
        // extras under the first so we don't silently lose data.
        if let Some(existing) = root.as_mut() {
            existing.children.push(node);
        }
    }
}

fn is_text_metadata_line(line: &str) -> bool {
    let t = line.trim();
    if t.eq_ignore_ascii_case("QUERY PLAN") {
        return true;
    }
    if !t.is_empty() && t.chars().all(|c| c == '-') {
        return true;
    }
    // Footer like "(6 rows)"
    if t.starts_with('(') && t.ends_with("rows)") {
        return true;
    }
    false
}

fn leading_width(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

fn strip_ms_suffix(line: &str, prefix: &str) -> Option<f64> {
    let rest = line.strip_prefix(prefix)?.trim();
    let value = rest.strip_suffix("ms").unwrap_or(rest).trim();
    value.parse::<f64>().ok()
}

/// Parse a single node header line (with the optional leading `->` stripped).
///
/// Returns `None` when the line lacks the Postgres cost signature, signalling
/// that the caller should treat it as an attribute of the enclosing node.
fn parse_text_node_header(content: &str, counter: &mut u32) -> Option<ExplainNode> {
    // Strip the optional "->" arrow marking a child node.
    let body = content.strip_prefix("->").map(str::trim_start).unwrap_or(content);

    let cost_pos = body.find("(cost=")?;
    let header = body[..cost_pos].trim();
    let tail = &body[cost_pos..];

    let (cost_inner, after_cost) = extract_parens(tail)?;
    let (startup_cost, total_cost, plan_rows) = parse_cost_fields(cost_inner);

    // The actual-time block (present only with ANALYZE) immediately follows.
    let actual_section = after_cost.trim_start();
    let (actual_time_ms, actual_rows, actual_loops) =
        if let Some((inner, _)) = extract_parens(actual_section) {
            parse_actual_fields(inner)
        } else {
            (None, None, None)
        };

    let (node_type, relation) = split_node_type_and_relation(header);

    let id = format!("node_{counter}");
    *counter += 1;

    Some(ExplainNode {
        id,
        node_type,
        relation,
        startup_cost,
        total_cost,
        plan_rows,
        actual_rows,
        actual_time_ms,
        actual_loops,
        buffers_hit: None,
        buffers_read: None,
        filter: None,
        index_condition: None,
        join_type: None,
        hash_condition: None,
        extra: std::collections::HashMap::new(),
        children: Vec::new(),
    })
}

/// Split on the last " on " so we keep modifiers like
/// "Index Scan using users_pkey" in the node type.
fn split_node_type_and_relation(header: &str) -> (String, Option<String>) {
    if let Some(idx) = header.rfind(" on ") {
        let node_type = header[..idx].trim().to_string();
        let relation = header[idx + 4..].trim().to_string();
        if relation.is_empty() {
            (node_type, None)
        } else {
            (node_type, Some(relation))
        }
    } else {
        (header.trim().to_string(), None)
    }
}

/// Extract the content of the first parenthesised group from `input`, plus the
/// remainder after the closing `)`. Input must start with `(`.
fn extract_parens(input: &str) -> Option<(&str, &str)> {
    if !input.starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    for (i, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let inner = &input[1..i];
                    let rest = &input[i + 1..];
                    return Some((inner, rest));
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_cost_fields(inner: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    let mut startup = None;
    let mut total = None;
    let mut rows = None;

    if let Some(rest) = inner.strip_prefix("cost=") {
        let mut parts = rest.split_whitespace();
        if let Some(cost_expr) = parts.next() {
            if let Some((s, t)) = cost_expr.split_once("..") {
                startup = s.parse().ok();
                total = t.parse().ok();
            }
        }
        for part in parts {
            if let Some(v) = part.strip_prefix("rows=") {
                rows = v.parse().ok();
            }
        }
    }
    (startup, total, rows)
}

fn parse_actual_fields(inner: &str) -> (Option<f64>, Option<f64>, Option<u64>) {
    // Two shapes:
    //   "actual time=0.10..0.20 rows=5 loops=1"
    //   "actual rows=5 loops=1"  (for cheap nodes under BUFFERS-only)
    //   "never executed"
    let inner = inner.trim();
    if inner.eq_ignore_ascii_case("never executed") {
        return (None, None, None);
    }
    let rest = inner.strip_prefix("actual ").unwrap_or(inner);

    let mut total_time = None;
    let mut rows = None;
    let mut loops = None;

    for part in rest.split_whitespace() {
        if let Some(expr) = part.strip_prefix("time=") {
            if let Some((_, t)) = expr.split_once("..") {
                total_time = t.parse().ok();
            }
        } else if let Some(v) = part.strip_prefix("rows=") {
            rows = v.parse().ok();
        } else if let Some(v) = part.strip_prefix("loops=") {
            loops = v.parse().ok();
        }
    }
    (total_time, rows, loops)
}

fn apply_text_attribute(node: &mut ExplainNode, content: &str) {
    if let Some(rest) = content.strip_prefix("Filter:") {
        node.filter = Some(rest.trim().to_string());
    } else if let Some(rest) = content.strip_prefix("Index Cond:") {
        node.index_condition = Some(rest.trim().to_string());
    } else if let Some(rest) = content.strip_prefix("Hash Cond:") {
        node.hash_condition = Some(rest.trim().to_string());
    } else if let Some(rest) = content.strip_prefix("Join Type:") {
        node.join_type = Some(rest.trim().to_string());
    } else if let Some((key, value)) = content.split_once(':') {
        node.extra.insert(
            key.trim().to_string(),
            Value::String(value.trim().to_string()),
        );
    }
}
