//! Window-scoped session statements.
//!
//! `execute_query` runs each statement on whichever connection the pool hands
//! out, so a `SET` executed on its own is gone the moment that connection is
//! released: the next statement may land on a different connection and sees no
//! user variables at all. Editors expect the opposite — a `SET @cutoff = …`
//! typed in a tab stays in force for every later statement of that tab until it
//! is overwritten or the tab is closed.
//!
//! This module remembers the replayable `SET` statements per (connection, tab)
//! and hands them back as a preamble the drivers replay on the connection they
//! just acquired. Statements are replayed in the order they were executed, so a
//! later `SET` of the same target overwrites the earlier one exactly as it would
//! inside a single session.
//!
//! Recording fails closed: only statements whose effect is session-scoped *and*
//! idempotent are remembered. Anything whose replay would change meaning
//! (`SET GLOBAL`, `SET PERSIST`, `SET TRANSACTION`, `SET LOCAL`, `SET PASSWORD`,
//! `autocommit`) is ignored, because silently re-running those on every later
//! statement is worse than not remembering them.

#[cfg(test)]
mod tests;

use crate::drivers::common::strip_leading_sql_comments;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Upper bound of remembered statements per window. A tab that keeps assigning
/// fresh variable names would otherwise grow the preamble without limit, and
/// the preamble is replayed before every statement. Oldest entries are dropped
/// first; hitting this is a pathological case, not normal editor use.
const MAX_ENTRIES_PER_WINDOW: usize = 200;

/// One remembered statement plus the normalised names it assigns. `targets`
/// drives two things: dropping entries a later statement fully supersedes, and
/// telling the driver what to undo once the connection goes back to the pool.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SessionEntry {
    targets: Vec<String>,
    sql: String,
}

/// What a driver needs to give one query the window's session state and then
/// hand the connection back the way it found it.
///
/// The undo half is not optional. A pooled connection is shared with every
/// other window and with background metadata queries, so a replayed
/// `SET @cutoff` that is left behind would silently feed another tab's query a
/// value it never set. `user_variables` and `session_settings` are undone
/// differently per dialect, so the driver — not this module — builds the
/// statements.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionPreamble {
    /// The remembered statements, in the order they were executed.
    pub setup: Vec<String>,
    /// User variable names including the sigil (`@cutoff`).
    pub user_variables: Vec<String>,
    /// Session setting names without any scope qualifier (`sort_buffer_size`).
    pub session_settings: Vec<String>,
}

impl SessionPreamble {
    pub fn is_empty(&self) -> bool {
        self.setup.is_empty()
    }
}

type SessionMap = HashMap<(String, String), Vec<SessionEntry>>;

fn sessions() -> &'static Mutex<SessionMap> {
    static SESSIONS: OnceLock<Mutex<SessionMap>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A poisoned mutex here means some other thread panicked while holding it; the
/// map itself is a plain `HashMap` that cannot be left half-updated, so
/// recovering the guard is safe and keeps session state usable.
fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Remembers `sql` for `(connection_id, context_id)` when it is a replayable
/// session statement. Call it only after the statement actually succeeded, so a
/// rejected `SET` is never replayed.
pub fn record_statement(connection_id: &str, context_id: &str, sql: &str) {
    let Some(targets) = replayable_targets(sql) else {
        return;
    };
    let trimmed = trim_statement(sql);
    if trimmed.is_empty() {
        return;
    }

    let mut map = lock_unpoisoned(sessions());
    let entries = map
        .entry((connection_id.to_string(), context_id.to_string()))
        .or_default();

    // Drop entries whose targets are all reassigned by this statement: they can
    // no longer influence the final state, so replaying them is pure overhead.
    entries.retain(|entry| !entry.targets.iter().all(|t| targets.contains(t)));
    entries.push(SessionEntry {
        targets,
        sql: trimmed.to_string(),
    });

    if entries.len() > MAX_ENTRIES_PER_WINDOW {
        let overflow = entries.len() - MAX_ENTRIES_PER_WINDOW;
        log::warn!(
            "Session state for window {context_id} exceeded {MAX_ENTRIES_PER_WINDOW} statements; dropping the {overflow} oldest"
        );
        entries.drain(0..overflow);
    }
}

/// Statements to replay before a query issued by this window, in execution
/// order, plus the names the driver must undo afterwards. `None` when the
/// window has no session state, which keeps the zero-`SET` case on exactly the
/// code path it uses today.
pub fn preamble(connection_id: &str, context_id: &str) -> Option<SessionPreamble> {
    let map = lock_unpoisoned(sessions());
    let entries = map.get(&(connection_id.to_string(), context_id.to_string()))?;
    if entries.is_empty() {
        return None;
    }

    let mut preamble = SessionPreamble::default();
    for entry in entries {
        preamble.setup.push(entry.sql.clone());
        for target in &entry.targets {
            let bucket = if target.starts_with('@') {
                &mut preamble.user_variables
            } else {
                &mut preamble.session_settings
            };
            if !bucket.contains(target) {
                bucket.push(target.clone());
            }
        }
    }
    Some(preamble)
}

/// Forgets one window's session state (tab closed, or the user reset it).
pub fn clear_window(connection_id: &str, context_id: &str) {
    let mut map = lock_unpoisoned(sessions());
    map.remove(&(connection_id.to_string(), context_id.to_string()));
}

/// Forgets every window of a connection (disconnected, or connection deleted).
/// Returns how many windows were dropped so callers can log it.
pub fn clear_connection(connection_id: &str) -> usize {
    let mut map = lock_unpoisoned(sessions());
    let before = map.len();
    map.retain(|(conn, _), _| conn != connection_id);
    before - map.len()
}

/// Statement text remembered for a window, for display in the UI.
pub fn list_window(connection_id: &str, context_id: &str) -> Vec<String> {
    preamble(connection_id, context_id)
        .map(|preamble| preamble.setup)
        .unwrap_or_default()
}

/// Strips leading comments plus trailing whitespace and statement terminators.
fn trim_statement(sql: &str) -> &str {
    strip_leading_sql_comments(sql)
        .trim()
        .trim_end_matches(';')
        .trim_end()
}

/// Prefixes that are syntactically `SET` but must never be replayed.
///
/// * `GLOBAL` / `PERSIST` / `@@GLOBAL.` — server-wide, already durable, and
///   re-applying them on every statement would keep rewriting server state.
/// * `TRANSACTION` / `SESSION CHARACTERISTICS` / `CONSTRAINTS` — scoped to the
///   next transaction; replaying changes when it takes effect.
/// * `LOCAL` — transaction-scoped in PostgreSQL (a MySQL synonym for SESSION);
///   ambiguous across dialects, so treated as not replayable.
/// * `PASSWORD` / `ROLE` / `SESSION AUTHORIZATION` — identity changes, not
///   editor state.
/// * `NAMES` / `CHARACTER SET` — connection-level encoding. There is no way to
///   restore the previous value when the connection goes back to the pool, so a
///   replay would leave every other window decoding results with this window's
///   charset.
const NON_REPLAYABLE_PREFIXES: &[&str] = &[
    "GLOBAL ",
    "GLOBAL.",
    "@@GLOBAL.",
    "@@GLOBAL ",
    "PERSIST ",
    "PERSIST_ONLY ",
    "PERSIST.",
    "TRANSACTION ",
    "SESSION TRANSACTION ",
    "SESSION CHARACTERISTICS ",
    "LOCAL ",
    "CONSTRAINTS ",
    "PASSWORD",
    "ROLE ",
    "DEFAULT ROLE ",
    "SESSION AUTHORIZATION",
    "NAMES ",
    "CHARACTER SET ",
];

/// The normalised names a `SET` assigns, or `None` when the statement must not
/// be replayed. Fails closed: anything not understood returns `None`.
pub(crate) fn replayable_targets(sql: &str) -> Option<Vec<String>> {
    let statement = trim_statement(sql);
    if statement.is_empty() || contains_top_level_semicolon(statement) {
        return None;
    }

    let rest = strip_keyword(statement, "SET")?;
    let upper = rest.to_uppercase();
    if NON_REPLAYABLE_PREFIXES
        .iter()
        .any(|prefix| upper.starts_with(prefix))
    {
        return None;
    }

    let mut targets = Vec::new();
    for (index, assignment) in split_top_level(rest, ',').into_iter().enumerate() {
        // Only the first comma-separated part must be an assignment. MySQL's
        // `SET @a = 1, @b = 2` gives a target per part, while PostgreSQL's
        // `SET search_path TO analytics, public` has one target followed by
        // more values — those trailing parts are not assignments and are not
        // a reason to refuse the statement.
        let target = match (assignment_target(assignment), index) {
            (Some(target), _) => target,
            (None, 0) => return None,
            (None, _) => continue,
        };
        if target.contains("autocommit") {
            // Replaying `autocommit = 0` would open a transaction on every
            // statement that the pool then rolls back on release, which looks
            // like silently discarded writes. Refuse to remember it.
            return None;
        }
        if !targets.contains(&target) {
            targets.push(target);
        }
    }

    (!targets.is_empty()).then_some(targets)
}

/// Prefixes that must not be replayed, yet cannot make a rollback wrong.
///
/// `SET NAMES` / `SET CHARACTER SET` change how the connection encodes results.
/// Replaying them is unsafe because a pooled connection carries the charset to
/// whichever window gets it next, and there is no way to restore the previous
/// value — but the charset has no bearing on whether a rollback statement
/// reproduces the original rows.
const REPLAY_UNSAFE_BUT_ROLLBACK_SAFE: &[&str] = &["NAMES ", "CHARACTER SET "];

/// Whether this `SET` only changes how the current session behaves, leaving
/// rollback correctness intact.
///
/// This asks a different question from [`replayable_targets`], which is why the
/// two disagree on `SET NAMES`:
///
/// * `replayable_targets` — "can this be re-applied on whatever connection the
///   pool hands out next?" `SET NAMES` fails that, so it is never remembered.
/// * `is_rollback_safe` — "can a rollback file still reproduce the original
///   rows after this ran?" `SET NAMES` passes that; encoding does not change
///   which rows a rollback statement restores.
///
/// So `SET NAMES utf8mb4` should execute but not be replayed. Both answers are
/// correct; they answer different questions. Refusing it in the rollback guard
/// was the bug — lop's execution history has it blocked repeatedly, and because
/// one refused statement fails the whole batch it took the rest of the batch
/// down with it.
pub(crate) fn is_rollback_safe(sql: &str) -> bool {
    if replayable_targets(sql).is_some() {
        return true;
    }
    let statement = trim_statement(sql);
    if statement.is_empty() || contains_top_level_semicolon(statement) {
        return false;
    }
    let Some(rest) = strip_keyword(statement, "SET") else {
        return false;
    };
    let upper = rest.to_uppercase();
    REPLAY_UNSAFE_BUT_ROLLBACK_SAFE
        .iter()
        .any(|prefix| upper.starts_with(prefix))
}

/// Returns what follows `keyword` when the statement starts with it as a whole
/// word, else `None`.
fn strip_keyword<'a>(statement: &'a str, keyword: &str) -> Option<&'a str> {
    let head = statement.get(..keyword.len())?;
    if !head.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let rest = &statement[keyword.len()..];
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    Some(rest.trim_start())
}

/// Normalised left-hand side of one assignment, or `None` when it is not a
/// plain `name = value` / `name TO value` pair.
fn assignment_target(assignment: &str) -> Option<String> {
    let assignment = assignment.trim();
    let name = match find_top_level_assignment_operator(assignment) {
        Some(position) => &assignment[..position],
        None => return None,
    };
    let name = name.trim().trim_end_matches(":");

    // Strip the scope qualifier so `SET SESSION x`, `SET @@session.x` and
    // `SET @@x` all overwrite the same target.
    let mut normalised = name.trim().to_lowercase();
    for prefix in ["session ", "@@session.", "@@local.", "@@"] {
        if let Some(stripped) = normalised.strip_prefix(prefix) {
            normalised = stripped.trim().to_string();
            break;
        }
    }

    let normalised = normalised.trim().to_string();
    if normalised.is_empty() || normalised.chars().any(char::is_whitespace) {
        return None;
    }
    Some(normalised)
}

/// Byte offset of the `=` or ` TO ` that separates name from value, ignoring
/// quoted text and anything nested in parentheses.
fn find_top_level_assignment_operator(assignment: &str) -> Option<usize> {
    let bytes = assignment.as_bytes();
    let mut scanner = Scanner::default();
    for (index, character) in assignment.char_indices() {
        if scanner.consume(character) {
            continue;
        }
        if character == '=' {
            return Some(index);
        }
        // PostgreSQL's `SET search_path TO public`.
        let preceded_by_space = index > 0 && bytes[index - 1].is_ascii_whitespace();
        let rest = &bytes[index..];
        if preceded_by_space
            && rest.len() > 2
            && rest[0].eq_ignore_ascii_case(&b'T')
            && rest[1].eq_ignore_ascii_case(&b'O')
            && rest[2].is_ascii_whitespace()
        {
            return Some(index);
        }
    }
    None
}

/// Splits on `separator` at nesting depth zero, ignoring quoted text.
fn split_top_level(input: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut scanner = Scanner::default();
    for (index, character) in input.char_indices() {
        if scanner.consume(character) {
            continue;
        }
        if character == separator {
            parts.push(&input[start..index]);
            start = index + character.len_utf8();
        }
    }
    parts.push(&input[start..]);
    parts
}

fn contains_top_level_semicolon(input: &str) -> bool {
    let mut scanner = Scanner::default();
    for character in input.chars() {
        if scanner.consume(character) {
            continue;
        }
        if character == ';' {
            return true;
        }
    }
    false
}

/// Tracks quoting and parenthesis depth while walking a statement.
#[derive(Default)]
struct Scanner {
    quote: Option<char>,
    depth: usize,
    escaped: bool,
}

impl Scanner {
    /// Feeds one character. Returns `true` when the caller must not interpret
    /// it — because it is quoted, nested, or part of the quoting syntax itself.
    ///
    /// A doubled quote (`'it''s'`) needs no special case: the second quote
    /// reopens the string the first one closed, and no character sits between
    /// them for the caller to misread.
    fn consume(&mut self, character: char) -> bool {
        if self.escaped {
            self.escaped = false;
            return true;
        }
        if let Some(quote) = self.quote {
            if character == '\\' && quote != '`' {
                self.escaped = true;
                return true;
            }
            if character == quote {
                self.quote = None;
            }
            return true;
        }

        match character {
            '\'' | '"' | '`' => {
                self.quote = Some(character);
                true
            }
            '(' => {
                self.depth += 1;
                true
            }
            ')' => {
                self.depth = self.depth.saturating_sub(1);
                true
            }
            _ => self.depth > 0,
        }
    }
}
