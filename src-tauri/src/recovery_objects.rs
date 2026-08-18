//! Names the objects a change statement touches, so recovery can restore them
//! from the backup connection.
//!
//! Deliberately permissive, unlike the rollback guard's lexer
//! (`drivers/mysql/rollback_guard.rs`), which rejects double quotes, escapes
//! and executable comments to stay fail-closed. The two answer opposite
//! questions. The guard decides *may this run*, so refusing is safe. This
//! module decides *what has to be restored afterwards*, and a statement whose
//! objects cannot be named is reported as an unrecoverable conflict
//! (`recovery_history.rs`, `compare_selected_statements`). Missing an object
//! silently loses the only path back; naming one object too many costs an
//! extra table restore. So every ambiguity here resolves toward naming more.

use crate::recovery_history::RecoveryObject;

pub const KIND_TABLE: &str = "table";
/// A session-scoped table. Named so the restore can say it deliberately
/// skipped it, rather than hunting the backup for something that never
/// outlived the connection.
pub const KIND_TEMPORARY: &str = "temporary table";
pub const KIND_DATABASE: &str = "database";
pub const KIND_VIEW: &str = "view";
pub const KIND_PROCEDURE: &str = "procedure";
pub const KIND_FUNCTION: &str = "function";
pub const KIND_TRIGGER: &str = "trigger";
pub const KIND_EVENT: &str = "event";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    /// An identifier chain: `db`.`tbl` arrives as `["db", "tbl"]`, already
    /// unquoted. `quoted` marks a chain that was written with backticks or
    /// double quotes, which means it can never be a keyword.
    Ident {
        parts: Vec<String>,
        upper: String,
        quoted: bool,
    },
    Punct(char),
    /// A string literal with its escapes resolved. The text matters: it is
    /// where `CALL p('orders', …)` and `PREPARE s FROM 'DROP TABLE t'` keep
    /// the object being changed.
    Str(String),
    Num,
}

impl Tok {
    /// The uppercased text when this token could be a keyword. Quoted
    /// identifiers return `""` so a table actually named `table` is not read
    /// as the TABLE keyword.
    fn keyword(&self) -> &str {
        match self {
            Tok::Ident {
                upper,
                quoted: false,
                parts,
            } if parts.len() == 1 => upper,
            _ => "",
        }
    }

    fn is_punct(&self, c: char) -> bool {
        matches!(self, Tok::Punct(p) if *p == c)
    }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80
}

/// Reads a quoted identifier body, honouring the doubled-quote escape
/// (`` `a``b` `` is one name containing a backtick).
fn read_quoted(bytes: &[u8], sql: &str, start: usize, quote: u8) -> (String, usize) {
    let mut i = start + 1;
    let mut value = String::new();
    while i < bytes.len() {
        if bytes[i] == quote {
            if bytes.get(i + 1) == Some(&quote) {
                value.push(quote as char);
                i += 2;
                continue;
            }
            i += 1;
            return (value, i);
        }
        let ch_start = i;
        // Step by character, not byte, so multi-byte names survive intact.
        let mut ch_end = i + 1;
        while ch_end < bytes.len() && (bytes[ch_end] & 0xC0) == 0x80 {
            ch_end += 1;
        }
        value.push_str(&sql[ch_start..ch_end]);
        i = ch_end;
    }
    // Unterminated: take what we have rather than dropping the object.
    (value, i)
}

fn tokenize(sql: &str) -> Vec<Tok> {
    let bytes = sql.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            // `/*! … */` and `/*M! … */` are executed by the server, so the
            // statements inside them are real changes; step over the marker
            // and keep lexing the body instead of skipping to `*/`.
            if bytes.get(i + 2) == Some(&b'!') {
                i += 3;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                continue;
            }
            if bytes
                .get(i + 2)
                .is_some_and(|c| c.eq_ignore_ascii_case(&b'M'))
                && bytes.get(i + 3) == Some(&b'!')
            {
                i += 4;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                continue;
            }
            match sql[i + 2..].find("*/") {
                Some(rel) => i += rel + 4,
                None => break,
            }
            continue;
        }
        if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
            // Closing marker of an executable comment whose body we lexed.
            i += 2;
            continue;
        }
        if b == b'\'' {
            i += 1;
            let mut value = String::new();
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    // Enough of MySQL's escapes to keep an object name intact.
                    let escaped = match bytes[i + 1] {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'0' => '\0',
                        other => other as char,
                    };
                    value.push(escaped);
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        value.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                let ch_start = i;
                let mut ch_end = i + 1;
                while ch_end < bytes.len() && (bytes[ch_end] & 0xC0) == 0x80 {
                    ch_end += 1;
                }
                value.push_str(&sql[ch_start..ch_end]);
                i = ch_end;
            }
            tokens.push(Tok::Str(value));
            continue;
        }
        if b.is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.') {
                i += 1;
            }
            tokens.push(Tok::Num);
            continue;
        }

        if b == b'`' || b == b'"' || is_ident_byte(b) {
            let mut parts: Vec<String> = Vec::new();
            let mut quoted = false;
            loop {
                if i < bytes.len() && (bytes[i] == b'`' || bytes[i] == b'"') {
                    let (value, next) = read_quoted(bytes, sql, i, bytes[i]);
                    parts.push(value);
                    quoted = true;
                    i = next;
                } else if i < bytes.len() && is_ident_byte(bytes[i]) {
                    let start = i;
                    while i < bytes.len() && is_ident_byte(bytes[i]) {
                        i += 1;
                    }
                    parts.push(sql[start..i].to_string());
                } else {
                    // A trailing dot (`db.`) still yields the parts we have.
                    break;
                }
                // Only a dot continues the chain, and only when a name follows.
                if bytes.get(i) == Some(&b'.')
                    && bytes
                        .get(i + 1)
                        .is_some_and(|n| *n == b'`' || *n == b'"' || is_ident_byte(*n))
                {
                    i += 1;
                    continue;
                }
                break;
            }
            if parts.is_empty() {
                i += 1;
                continue;
            }
            let upper = if parts.len() == 1 {
                parts[0].to_ascii_uppercase()
            } else {
                String::new()
            };
            tokens.push(Tok::Ident {
                parts,
                upper,
                quoted,
            });
            continue;
        }

        tokens.push(Tok::Punct(b as char));
        i += 1;
    }

    tokens
}

/// Cursor over the token stream with the small vocabulary the parsers need.
struct Cursor<'a> {
    toks: &'a [Tok],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(toks: &'a [Tok]) -> Self {
        Cursor { toks, i: 0 }
    }

    fn keyword(&self) -> &str {
        self.toks.get(self.i).map(Tok::keyword).unwrap_or("")
    }

    fn keyword_at(&self, offset: usize) -> &str {
        self.toks
            .get(self.i + offset)
            .map(Tok::keyword)
            .unwrap_or("")
    }

    fn done(&self) -> bool {
        self.i >= self.toks.len()
    }

    fn bump(&mut self) {
        self.i += 1;
    }

    fn eat(&mut self, word: &str) -> bool {
        if self.keyword() == word {
            self.bump();
            return true;
        }
        false
    }

    /// Steps over any of `words`, repeatedly. Used for the modifier soup that
    /// sits between a verb and its object (`IF NOT EXISTS`, `TEMPORARY`, …).
    fn skip_any(&mut self, words: &[&str]) {
        while !self.done() && words.contains(&self.keyword()) {
            self.bump();
        }
    }

    /// Steps over `DEFINER = user@host`, `ALGORITHM = UNDEFINED`,
    /// `SQL SECURITY DEFINER` and friends, which may appear in any order
    /// before the object keyword of a CREATE/ALTER.
    fn skip_definer_clauses(&mut self) {
        loop {
            match self.keyword() {
                "DEFINER" | "ALGORITHM" => {
                    self.bump();
                    if self.toks.get(self.i).is_some_and(|t| t.is_punct('=')) {
                        self.bump();
                    }
                    // user@host lexes as several tokens; consume until the
                    // next word that can start an object clause.
                    while !self.done() {
                        let kw = self.keyword();
                        if matches!(
                            kw,
                            "TABLE"
                                | "VIEW"
                                | "TRIGGER"
                                | "PROCEDURE"
                                | "FUNCTION"
                                | "EVENT"
                                | "INDEX"
                                | "DATABASE"
                                | "SCHEMA"
                                | "SQL"
                                | "ALGORITHM"
                                | "DEFINER"
                        ) {
                            break;
                        }
                        self.bump();
                    }
                }
                "SQL" if self.keyword_at(1) == "SECURITY" => {
                    self.bump();
                    self.bump();
                    self.bump();
                }
                _ => return,
            }
        }
    }

    /// Reads the identifier at the cursor as a `(schema, name)` pair.
    /// A bare name yields an empty schema, which the caller fills in with the
    /// run's database.
    fn qualified(&mut self) -> Option<(String, String)> {
        let tok = self.toks.get(self.i)?;
        let Tok::Ident { parts, .. } = tok else {
            return None;
        };
        let pair = match parts.len() {
            0 => return None,
            1 => (String::new(), parts[0].clone()),
            // `db`.`tbl`.`col` cannot appear as an object name; take the last
            // two so a stray prefix does not shift the table name.
            n => (parts[n - 2].clone(), parts[n - 1].clone()),
        };
        self.bump();
        Some(pair)
    }

    /// Skips a parenthesised group, including nested ones.
    fn skip_parens(&mut self) {
        if !self.toks.get(self.i).is_some_and(|t| t.is_punct('(')) {
            return;
        }
        let mut depth = 0usize;
        while !self.done() {
            if let Some(Tok::Punct(c)) = self.toks.get(self.i) {
                if *c == '(' {
                    depth += 1;
                } else if *c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        self.bump();
                        return;
                    }
                }
            }
            self.bump();
        }
    }
}

fn retag_tables_as_temporary(objects: &mut [RecoveryObject]) {
    for object in objects.iter_mut() {
        if object.kind == KIND_TABLE {
            object.kind = KIND_TEMPORARY.to_string();
        }
    }
}

fn object(kind: &str, schema: String, name: String) -> RecoveryObject {
    RecoveryObject {
        kind: kind.to_string(),
        schema,
        name,
    }
}

fn push_pair(out: &mut Vec<RecoveryObject>, kind: &str, pair: Option<(String, String)>) {
    if let Some((schema, name)) = pair {
        if !name.is_empty() {
            out.push(object(kind, schema, name));
        }
    }
}

/// Comma-separated object names, as in `DROP TABLE a, b, c`.
fn qualified_list(cursor: &mut Cursor, kind: &str, out: &mut Vec<RecoveryObject>) {
    loop {
        push_pair(out, kind, cursor.qualified());
        // `DROP TABLE t RESTRICT` and friends end the list.
        if cursor.toks.get(cursor.i).is_some_and(|t| t.is_punct(',')) {
            cursor.bump();
            continue;
        }
        return;
    }
}

/// Words that end a table-reference list in a DML statement.
///
/// `AS` is deliberately absent: it introduces an alias, and stopping there
/// would drop every table after the first aliased one — `UPDATE t1 AS a JOIN
/// t2 AS b SET …` would silently restore only `t1`. The alias itself is
/// consumed by the `expect_name` state below.
const TABLE_REF_STOP: &[&str] = &[
    "SET", "WHERE", "ON", "USING", "ORDER", "LIMIT", "GROUP", "HAVING", "VALUES", "VALUE",
    "SELECT", "UNION", "RETURNING",
];

/// Collects every table named in a FROM/JOIN chain.
///
/// Reads more than strictly changes: `UPDATE a JOIN b SET a.x = 1` only writes
/// `a`, but naming `b` too just adds a redundant restore, while missing the
/// written table loses it for good. Sub-queries are stepped over, since a
/// table only read inside one is not modified by this statement.
fn table_refs(cursor: &mut Cursor, out: &mut Vec<RecoveryObject>) {
    let mut expect_name = true;
    while !cursor.done() {
        if cursor.toks.get(cursor.i).is_some_and(|t| t.is_punct('(')) {
            cursor.skip_parens();
            expect_name = false;
            continue;
        }
        let kw = cursor.keyword();
        if TABLE_REF_STOP.contains(&kw) {
            return;
        }
        if matches!(
            kw,
            "JOIN" | "INNER" | "LEFT" | "RIGHT" | "FULL" | "CROSS" | "STRAIGHT_JOIN" | "NATURAL"
        ) {
            cursor.bump();
            expect_name = true;
            continue;
        }
        if cursor.toks.get(cursor.i).is_some_and(|t| t.is_punct(',')) {
            cursor.bump();
            expect_name = true;
            continue;
        }
        if cursor.toks.get(cursor.i).is_some_and(|t| t.is_punct(';')) {
            return;
        }
        match cursor.toks.get(cursor.i) {
            Some(Tok::Ident { .. }) => {
                if expect_name {
                    push_pair(out, KIND_TABLE, cursor.qualified());
                    expect_name = false;
                } else {
                    // An alias, or `PARTITION (p0)` — not a new table.
                    cursor.bump();
                }
            }
            _ => cursor.bump(),
        }
    }
}

/// The verb and every object a change statement touches.
///
/// Returns `("", vec![])` for input with no recognisable leading keyword.
pub fn parse_change_objects(sql: &str) -> (String, Vec<RecoveryObject>) {
    let toks = tokenize(sql);
    let mut cursor = Cursor::new(&toks);
    let mut out: Vec<RecoveryObject> = Vec::new();

    let verb = cursor.keyword().to_string();
    if verb.is_empty() {
        return (String::new(), out);
    }
    cursor.bump();

    let operation = match verb.as_str() {
        "CREATE" => {
            cursor.skip_any(&["OR", "REPLACE"]);
            let temporary = cursor.eat("TEMPORARY");
            cursor.skip_any(&["UNIQUE", "FULLTEXT", "SPATIAL"]);
            cursor.skip_definer_clauses();
            let op = parse_create(&mut cursor, &mut out);
            if temporary {
                retag_tables_as_temporary(&mut out);
                return (format!("create temporary {}", op.trim_start_matches("create ")), out);
            }
            op
        }
        "DROP" => {
            let temporary = cursor.eat("TEMPORARY");
            let op = parse_drop(&mut cursor, &mut out);
            if temporary {
                retag_tables_as_temporary(&mut out);
                return (format!("drop temporary {}", op.trim_start_matches("drop ")), out);
            }
            op
        }
        "ALTER" => {
            cursor.skip_any(&["ONLINE", "OFFLINE", "IGNORE"]);
            cursor.skip_definer_clauses();
            parse_alter(&mut cursor, &mut out)
        }
        "RENAME" => {
            cursor.skip_any(&["TABLE", "TABLES"]);
            // `a TO b, c TO d`: both sides matter — the old name disappears
            // and the new one appears.
            loop {
                push_pair(&mut out, KIND_TABLE, cursor.qualified());
                if cursor.eat("TO") {
                    push_pair(&mut out, KIND_TABLE, cursor.qualified());
                }
                if cursor.toks.get(cursor.i).is_some_and(|t| t.is_punct(',')) {
                    cursor.bump();
                    continue;
                }
                break;
            }
            "rename table".to_string()
        }
        "TRUNCATE" => {
            cursor.skip_any(&["TABLE"]);
            push_pair(&mut out, KIND_TABLE, cursor.qualified());
            "truncate table".to_string()
        }
        "INSERT" | "REPLACE" => {
            cursor.skip_any(&["IGNORE", "LOW_PRIORITY", "DELAYED", "HIGH_PRIORITY", "INTO"]);
            push_pair(&mut out, KIND_TABLE, cursor.qualified());
            verb.to_lowercase()
        }
        "UPDATE" => {
            cursor.skip_any(&["IGNORE", "LOW_PRIORITY"]);
            table_refs(&mut cursor, &mut out);
            "update".to_string()
        }
        "DELETE" => {
            cursor.skip_any(&["LOW_PRIORITY", "QUICK", "IGNORE"]);
            // `DELETE a, b FROM …` lists aliases before FROM; the real tables
            // follow FROM (or USING), so skip ahead to them.
            while !cursor.done() && cursor.keyword() != "FROM" {
                cursor.bump();
            }
            if cursor.eat("FROM") {
                table_refs(&mut cursor, &mut out);
            }
            if cursor.eat("USING") {
                table_refs(&mut cursor, &mut out);
            }
            "delete".to_string()
        }
        "LOAD" => {
            // LOAD DATA [LOCAL] INFILE '…' [REPLACE|IGNORE] INTO TABLE tbl
            while !cursor.done() && !(cursor.keyword() == "INTO") {
                cursor.bump();
            }
            cursor.skip_any(&["INTO", "TABLE"]);
            push_pair(&mut out, KIND_TABLE, cursor.qualified());
            "load data".to_string()
        }
        "CALL" => {
            // The body is opaque here, but naming the routine lets recovery
            // report exactly which procedure has to be reviewed.
            push_pair(&mut out, KIND_PROCEDURE, cursor.qualified());
            "call".to_string()
        }
        // `OPTIMIZE`/`REPAIR`/`ANALYZE TABLE` rebuild or rewrite the table —
        // REPAIR can drop rows outright — so the target has to be named even
        // though none of them is a plain DML write.
        verb @ ("OPTIMIZE" | "REPAIR" | "ANALYZE" | "CHECKSUM") => {
            cursor.skip_any(&["NO_WRITE_TO_BINLOG", "LOCAL", "TABLE", "TABLES"]);
            qualified_list(&mut cursor, KIND_TABLE, &mut out);
            format!("{} table", verb.to_lowercase())
        }
        // `WITH cte AS (…) UPDATE …`: the leading keyword says nothing about
        // what is written, so step over every CTE definition and re-dispatch
        // on the statement that follows.
        "WITH" => {
            cursor.skip_any(&["RECURSIVE"]);
            loop {
                if cursor.qualified().is_none() {
                    break;
                }
                // An optional column list, then the required AS ( … ).
                cursor.skip_parens();
                cursor.skip_any(&["AS"]);
                cursor.skip_parens();
                if cursor.toks.get(cursor.i).is_some_and(|t| t.is_punct(',')) {
                    cursor.bump();
                    continue;
                }
                break;
            }
            let rest = &toks[cursor.i.min(toks.len())..];
            let (inner_op, inner_objects) = parse_from_tokens(rest);
            out.extend(inner_objects);
            if inner_op.is_empty() {
                "with".to_string()
            } else {
                inner_op
            }
        }
        "GRANT" => "grant".to_string(),
        "REVOKE" => "revoke".to_string(),
        other => other.to_lowercase(),
    };

    // `DELETE FROM a USING a, b` names `a` from both clauses; a repeat would
    // schedule the same table for restore twice. Order is kept so the first
    // mention still leads.
    let mut seen = std::collections::HashSet::new();
    out.retain(|object| {
        seen.insert((
            object.kind.clone(),
            object.schema.clone(),
            object.name.clone(),
        ))
    });

    (operation, out)
}

/// Re-entry point for a statement that starts partway through a token stream,
/// used for the body of a `WITH … ` prefix.
fn parse_from_tokens(toks: &[Tok]) -> (String, Vec<RecoveryObject>) {
    // Rendering back to text would lose quoting; recursing on the token slice
    // keeps `db`.`tbl` intact. Guarded against a `WITH` inside a `WITH`.
    if toks.is_empty() || toks.first().map(Tok::keyword) == Some("WITH") {
        return (String::new(), Vec::new());
    }
    let mut rendered = String::new();
    for tok in toks {
        match tok {
            Tok::Ident { parts, quoted, .. } => {
                let joined = parts
                    .iter()
                    .map(|p| {
                        if *quoted {
                            format!("`{}`", p.replace('`', "``"))
                        } else {
                            p.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(".");
                rendered.push_str(&joined);
            }
            Tok::Punct(c) => rendered.push(*c),
            Tok::Str(_) => rendered.push_str("''"),
            Tok::Num => rendered.push('0'),
        }
        rendered.push(' ');
    }
    parse_change_objects(&rendered)
}

/// A statement whose real target is only knowable against the live session.
///
/// Change scripts here lean on `PREPARE`/`EXECUTE` for idempotent guarded DDL
/// and on `CALL p('table', 'column', 'DDL fragment')` helpers, so the table
/// name never appears as an identifier in the statement text — it sits in a
/// user variable or a string argument. Static parsing cannot reach it, which
/// is why these came back with no object at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicSource {
    /// `PREPARE s FROM @ddl` — the SQL text is the value of that variable.
    UserVariable { statement: String, variable: String },
    /// `PREPARE s FROM 'DROP TABLE t'` — already readable.
    Literal { statement: String, sql: String },
    /// `EXECUTE s` — resolved from the matching PREPARE.
    Execute { statement: String },
    /// `CALL p('orders', …)` — the routine, plus every string argument, any
    /// of which may name the object the routine changes.
    Call {
        schema: String,
        routine: String,
        string_args: Vec<String>,
    },
}

/// Classifies a statement that static parsing could not pin an object on.
/// Returns `None` when the statement is not one of the dynamic forms.
pub fn dynamic_source(sql: &str) -> Option<DynamicSource> {
    let toks = tokenize(sql);
    let mut cursor = Cursor::new(&toks);

    match cursor.keyword() {
        "PREPARE" => {
            cursor.bump();
            let (_, statement) = cursor.qualified()?;
            if !cursor.eat("FROM") {
                return None;
            }
            match cursor.toks.get(cursor.i) {
                Some(Tok::Str(text)) => Some(DynamicSource::Literal {
                    statement,
                    sql: text.clone(),
                }),
                Some(Tok::Punct('@')) => {
                    cursor.bump();
                    let (_, variable) = cursor.qualified()?;
                    Some(DynamicSource::UserVariable {
                        statement,
                        variable,
                    })
                }
                _ => None,
            }
        }
        "EXECUTE" => {
            cursor.bump();
            let (_, statement) = cursor.qualified()?;
            Some(DynamicSource::Execute { statement })
        }
        "CALL" => {
            cursor.bump();
            let (schema, routine) = cursor.qualified()?;
            let string_args = cursor
                .toks
                .iter()
                .skip(cursor.i)
                .filter_map(|t| match t {
                    Tok::Str(text) => Some(text.clone()),
                    _ => None,
                })
                .collect();
            Some(DynamicSource::Call {
                schema,
                routine,
                string_args,
            })
        }
        _ => None,
    }
}

fn parse_create(cursor: &mut Cursor, out: &mut Vec<RecoveryObject>) -> String {
    match cursor.keyword() {
        "TABLE" => {
            cursor.bump();
            cursor.skip_any(&["IF", "NOT", "EXISTS"]);
            push_pair(out, KIND_TABLE, cursor.qualified());
            // `CREATE TABLE a LIKE b` / `… AS SELECT … FROM b`: only `a` is
            // created; `b` is read, so it must not be restored over.
            "create table".to_string()
        }
        "INDEX" => {
            cursor.bump();
            cursor.skip_any(&["IF", "NOT", "EXISTS"]);
            // The index name is not the object to restore — the table is.
            let _ = cursor.qualified();
            while !cursor.done() && cursor.keyword() != "ON" {
                cursor.bump();
            }
            if cursor.eat("ON") {
                push_pair(out, KIND_TABLE, cursor.qualified());
            }
            "create index".to_string()
        }
        "VIEW" => {
            cursor.bump();
            cursor.skip_any(&["IF", "NOT", "EXISTS"]);
            push_pair(out, KIND_VIEW, cursor.qualified());
            "create view".to_string()
        }
        "TRIGGER" => {
            cursor.bump();
            cursor.skip_any(&["IF", "NOT", "EXISTS"]);
            push_pair(out, KIND_TRIGGER, cursor.qualified());
            // `… BEFORE INSERT ON tbl`: the table carries the trigger, so it
            // has to come back too.
            while !cursor.done() && cursor.keyword() != "ON" {
                cursor.bump();
            }
            if cursor.eat("ON") {
                push_pair(out, KIND_TABLE, cursor.qualified());
            }
            "create trigger".to_string()
        }
        "PROCEDURE" => {
            cursor.bump();
            cursor.skip_any(&["IF", "NOT", "EXISTS"]);
            push_pair(out, KIND_PROCEDURE, cursor.qualified());
            "create procedure".to_string()
        }
        "FUNCTION" => {
            cursor.bump();
            cursor.skip_any(&["IF", "NOT", "EXISTS"]);
            push_pair(out, KIND_FUNCTION, cursor.qualified());
            "create function".to_string()
        }
        "EVENT" => {
            cursor.bump();
            cursor.skip_any(&["IF", "NOT", "EXISTS"]);
            push_pair(out, KIND_EVENT, cursor.qualified());
            "create event".to_string()
        }
        kind @ ("DATABASE" | "SCHEMA") => {
            let op = format!("create {}", kind.to_lowercase());
            cursor.bump();
            cursor.skip_any(&["IF", "NOT", "EXISTS"]);
            if let Some((_, name)) = cursor.qualified() {
                out.push(object(KIND_DATABASE, name, String::new()));
            }
            op
        }
        other if other.is_empty() => "create".to_string(),
        other => format!("create {}", other.to_lowercase()),
    }
}

fn parse_drop(cursor: &mut Cursor, out: &mut Vec<RecoveryObject>) -> String {
    match cursor.keyword() {
        "TABLE" | "TABLES" => {
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            qualified_list(cursor, KIND_TABLE, out);
            "drop table".to_string()
        }
        "INDEX" => {
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            let _ = cursor.qualified();
            while !cursor.done() && cursor.keyword() != "ON" {
                cursor.bump();
            }
            if cursor.eat("ON") {
                push_pair(out, KIND_TABLE, cursor.qualified());
            }
            "drop index".to_string()
        }
        "VIEW" => {
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            qualified_list(cursor, KIND_VIEW, out);
            "drop view".to_string()
        }
        "TRIGGER" => {
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            push_pair(out, KIND_TRIGGER, cursor.qualified());
            "drop trigger".to_string()
        }
        "PROCEDURE" => {
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            push_pair(out, KIND_PROCEDURE, cursor.qualified());
            "drop procedure".to_string()
        }
        "FUNCTION" => {
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            push_pair(out, KIND_FUNCTION, cursor.qualified());
            "drop function".to_string()
        }
        "EVENT" => {
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            push_pair(out, KIND_EVENT, cursor.qualified());
            "drop event".to_string()
        }
        kind @ ("DATABASE" | "SCHEMA") => {
            let op = format!("drop {}", kind.to_lowercase());
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            if let Some((_, name)) = cursor.qualified() {
                out.push(object(KIND_DATABASE, name, String::new()));
            }
            op
        }
        other if other.is_empty() => "drop".to_string(),
        other => format!("drop {}", other.to_lowercase()),
    }
}

fn parse_alter(cursor: &mut Cursor, out: &mut Vec<RecoveryObject>) -> String {
    match cursor.keyword() {
        "TABLE" => {
            cursor.bump();
            cursor.skip_any(&["IF", "EXISTS"]);
            push_pair(out, KIND_TABLE, cursor.qualified());
            // `ALTER TABLE a RENAME TO b` moves the table; both names have to
            // be restorable. `RENAME COLUMN`/`RENAME INDEX` do not.
            while !cursor.done() {
                if cursor.keyword() == "RENAME" {
                    cursor.bump();
                    if matches!(cursor.keyword(), "COLUMN" | "INDEX" | "KEY") {
                        continue;
                    }
                    cursor.skip_any(&["TO", "AS"]);
                    push_pair(out, KIND_TABLE, cursor.qualified());
                    continue;
                }
                // `EXCHANGE PARTITION p WITH TABLE other` swaps the contents
                // of both tables, so `other` changes just as much.
                if cursor.keyword() == "WITH" && cursor.keyword_at(1) == "TABLE" {
                    cursor.bump();
                    cursor.bump();
                    push_pair(out, KIND_TABLE, cursor.qualified());
                    continue;
                }
                cursor.bump();
            }
            "alter table".to_string()
        }
        "VIEW" => {
            cursor.bump();
            push_pair(out, KIND_VIEW, cursor.qualified());
            "alter view".to_string()
        }
        "PROCEDURE" => {
            cursor.bump();
            push_pair(out, KIND_PROCEDURE, cursor.qualified());
            "alter procedure".to_string()
        }
        "FUNCTION" => {
            cursor.bump();
            push_pair(out, KIND_FUNCTION, cursor.qualified());
            "alter function".to_string()
        }
        "EVENT" => {
            cursor.bump();
            push_pair(out, KIND_EVENT, cursor.qualified());
            "alter event".to_string()
        }
        kind @ ("DATABASE" | "SCHEMA") => {
            let op = format!("alter {}", kind.to_lowercase());
            cursor.bump();
            if let Some((_, name)) = cursor.qualified() {
                out.push(object(KIND_DATABASE, name, String::new()));
            }
            op
        }
        other if other.is_empty() => "alter".to_string(),
        other => format!("alter {}", other.to_lowercase()),
    }
}
