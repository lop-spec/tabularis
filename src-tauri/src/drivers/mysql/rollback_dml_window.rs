//! Bounded, disjoint-key row-image windows owned by one explicit transaction.
//! SQL remains sequential and uses the original protocol. No pending window may
//! cross a batch, table, session, transaction, or cancellation boundary.
use super::*;

const MAX_STATEMENTS: usize = 128;
const MAX_SQL_BYTES: usize = 256 * 1024;
type Key = Vec<i128>;
type RowImages = BTreeMap<Key, CapturedRow>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Insert,
    Update,
    Delete,
}
fn target(plan: &DmlPlan) -> Option<(Kind, &ObjectName)> {
    match plan {
        DmlPlan::Insert(p) => Some((Kind::Insert, &p.table)),
        DmlPlan::Update(p) => Some((Kind::Update, &p.table)),
        DmlPlan::Delete(p) => Some((Kind::Delete, &p.table)),
        _ => None,
    }
}
struct Candidate {
    index: usize,
    query: String,
    plan: DmlPlan,
    keys: Vec<Key>,
    condition: String,
}
struct Active {
    candidates: Vec<Candidate>,
    before: RowImages,
    results: Vec<u64>,
    metadata: TableMetadata,
}
#[derive(Default)]
pub(super) struct DmlWindow {
    active: Option<Active>,
    metadata: Option<(Kind, ObjectName, TableMetadata)>,
    declined_until: usize,
    native_defaults: Option<bool>,
    optimized: usize,
    windows: usize,
    reasons: BTreeMap<&'static str, usize>,
}
impl DmlWindow {
    pub(super) fn has_pending(&self) -> bool {
        self.active.as_ref().is_some_and(|a| !a.results.is_empty())
    }
    pub(super) fn discard(&mut self) {
        self.active = None;
        self.metadata = None;
    }
    fn decline(&mut self, reason: &'static str) {
        *self.reasons.entry(reason).or_default() += 1;
    }
    pub(super) fn before_statement(
        &mut self,
        plan: &ProtectedStatement,
        index: usize,
        active_transaction: bool,
        stopped: bool,
    ) -> Result<(), String> {
        if stopped {
            self.discard();
            return Ok(());
        }
        if let Some(active) = &self.active {
            let expected = active.candidates.get(active.results.len());
            if !active_transaction
                || !matches!((expected, plan), (Some(c), ProtectedStatement::Dml(p)) if c.index == index && c.plan == *p)
            {
                return Err("Unmaterialized row images cannot cross an execution boundary; transaction must be closed without committing".into());
            }
        }
        let same = match (plan, &self.metadata) {
            (ProtectedStatement::Dml(p), Some((kind, object, _))) => {
                target(p).is_some_and(|(k, o)| k == *kind && o == object)
            }
            _ => false,
        };
        if !active_transaction || !same {
            self.metadata = None;
        }
        Ok(())
    }
    pub(super) async fn try_execute(
        &mut self,
        conn: &mut sqlx::MySqlConnection,
        queries: &[String],
        plans: &[ProtectedStatement],
        index: usize,
        cache: &InsertMetadataCache,
        rollback: &mut RollbackJournal,
        recovery: &mut RecoveryJournal,
        text: super::super::TextProto,
    ) -> Option<Result<QueryResult, String>> {
        if self.active.is_none() {
            if rollback.requires_immediate_durability() {
                self.decline(
                    "prior nontransactional recovery requires immediate capture and durability",
                );
                return None;
            }
            if index < self.declined_until {
                return None;
            }
            let Some(ProtectedStatement::Dml(plan)) = plans.first() else {
                return None;
            };
            let Some((kind, object)) = target(plan) else {
                self.decline("unsupported DML family");
                return None;
            };
            let count = plans.iter().take(MAX_STATEMENTS).take_while(|p| matches!(p, ProtectedStatement::Dml(p) if target(p) == Some((kind, object)))).count();
            if count < 2 {
                self.decline("no consecutive same-table window");
                return None;
            }
            let metadata = if let Some((_, _, m)) = &self.metadata {
                m.clone()
            } else if kind == Kind::Insert {
                let Some(m) = cache.locked_metadata(object) else {
                    self.decline("first INSERT establishes the metadata lock");
                    return None;
                };
                m.clone()
            } else {
                match load_locked_dml_metadata(conn, object, true).await {
                    Ok(m) => m,
                    Err(e) => return Some(Err(e)),
                }
            };
            if metadata
                .columns
                .iter()
                .any(|c| c.auto_increment || c.generated)
                || !integer_primary_key(&metadata)
            {
                self.declined_until = index + count;
                self.decline(
                    "AUTO_INCREMENT, generated columns, or noninteger identity requires immediate capture",
                );
                return None;
            }
            // Native MySQL 8 forbids subqueries, stored functions, and loadable
            // functions in defaults. Other server families need a separate
            // proof before omitted columns can join a disjoint-key window.
            // https://dev.mysql.com/doc/refman/8.0/en/data-type-defaults.html
            if self.native_defaults.is_none() {
                let version = match conn.fetch_one(sqlx::raw_sql("SELECT VERSION()")).await {
                    Ok(row) => match mysql_text(&row, 0) {
                        Ok(v) => v,
                        Err(e) => return Some(Err(e)),
                    },
                    Err(e) => {
                        return Some(Err(format!("Window server-family inspection failed: {e}")))
                    }
                };
                self.native_defaults = Some(native_default_rules(&version));
            }
            if self.native_defaults != Some(true) {
                self.declined_until = index + count;
                self.decline("server default-expression semantics have not been verified");
                return None;
            }
            self.metadata = Some((kind, object.clone(), metadata.clone()));
            let candidates = candidates(queries, plans, index, &metadata);
            if candidates.len() < 2 {
                self.declined_until = index + 1;
                self.decline("overlapping, nonliteral, invalid, or over-budget keys");
                return None;
            }
            let condition = candidates
                .iter()
                .map(|c| format!("({})", c.condition))
                .collect::<Vec<_>>()
                .join(" OR ");
            let before = match capture_rows(conn, &metadata, Some(&condition))
                .await
                .and_then(|rows| index_rows(&metadata, rows))
            {
                Ok(rows) => rows,
                Err(e) => return Some(Err(e)),
            };
            if kind == Kind::Insert && !before.is_empty() {
                self.declined_until = index + candidates.len();
                self.decline("existing INSERT keys retain statement-local refusal ordering");
                return None;
            }
            self.active = Some(Active {
                candidates,
                before,
                results: Vec::new(),
                metadata,
            });
            self.windows += 1;
        }
        Some(
            self.execute_next(conn, index, rollback, recovery, text)
                .await,
        )
    }
    async fn execute_next(
        &mut self,
        conn: &mut sqlx::MySqlConnection,
        index: usize,
        rollback: &mut RollbackJournal,
        recovery: &mut RecoveryJournal,
        text: super::super::TextProto,
    ) -> Result<QueryResult, String> {
        let active = self.active.as_mut().ok_or("Missing row-image window")?;
        let candidate = active
            .candidates
            .get(active.results.len())
            .ok_or("Row-image window exhausted")?;
        if candidate.index != index {
            return Err("Row-image window statement ordering mismatch".into());
        }
        let before = select_rows(&active.before, &candidate.keys);
        let sql = match &candidate.plan {
            DmlPlan::Insert(_) => candidate.query.clone(),
            DmlPlan::Update(p) => locked_write_sql(
                &p.statement_prefix,
                p.where_sql.as_deref(),
                &captured_primary_key_filter(&active.metadata, &before)?,
            ),
            DmlPlan::Delete(p) => locked_write_sql(
                &p.statement_prefix,
                p.where_sql.as_deref(),
                &captured_primary_key_filter(&active.metadata, &before)?,
            ),
            _ => return Err("Unexpected DML family in row-image window".into()),
        };
        rollback.defer_transaction_writes();
        recovery.defer_transaction_writes();
        let result = super::super::exec_on_mysql_conn(conn, &sql, None, 1, text).await?;
        if let DmlPlan::Insert(p) = &candidate.plan {
            if result.affected_rows != p.rows.len() as u64 {
                return Err(
                    "Windowed INSERT affected-row count mismatch; transaction must be rolled back"
                        .into(),
                );
            }
        }
        active.results.push(result.affected_rows);
        self.optimized += 1;
        if active.results.len() == active.candidates.len() {
            self.finish(conn, rollback, recovery).await?;
        }
        Ok(result)
    }
    async fn finish(
        &mut self,
        conn: &mut sqlx::MySqlConnection,
        rollback: &mut RollbackJournal,
        recovery: &mut RecoveryJournal,
    ) -> Result<(), String> {
        // Keep ownership until all row images have been validated and recorded.
        let active = self.active.as_ref().ok_or("Missing row-image window")?;
        let condition = if matches!(active.candidates[0].plan, DmlPlan::Insert(_)) {
            active
                .candidates
                .iter()
                .map(|c| format!("({})", c.condition))
                .collect::<Vec<_>>()
                .join(" OR ")
        } else {
            // Missing keys are not owned under READ COMMITTED. Never mistake a
            // concurrent insertion at such a key for one of our after-images.
            captured_primary_key_filter(
                &active.metadata,
                &active.before.values().cloned().collect::<Vec<_>>(),
            )?
        };
        let after = index_rows(
            &active.metadata,
            capture_rows(conn, &active.metadata, Some(&condition)).await?,
        )?;
        for (candidate, result) in active.candidates.iter().zip(&active.results) {
            let before = select_rows(&active.before, &candidate.keys);
            let after = select_rows(&after, &candidate.keys);
            dml_records::record(
                &candidate.query,
                &candidate.plan,
                candidate.index,
                &active.metadata,
                before,
                after,
                Some(candidate.condition.clone()),
                empty_write_result(*result),
                rollback,
                recovery,
            )?;
        }
        self.active = None;
        Ok(())
    }
}
impl Drop for DmlWindow {
    fn drop(&mut self) {
        if self.optimized > 0 || !self.reasons.is_empty() {
            log::info!("Protected row-image windows: statements={}, windows={}, immediate_capture_reasons={:?}", self.optimized, self.windows, self.reasons);
        }
    }
}
fn native_default_rules(version: &str) -> bool {
    let version = version.to_ascii_lowercase();
    (version.starts_with("8.0.") || version.starts_with("8.4."))
        && !version.contains("mariadb")
        && !version.contains("tidb")
}
fn proven_expression(expression: &str) -> bool {
    // Reuse the strict planner's single function/assignment allowlist rather
    // than introducing a second interpretation of builtin or stored calls.
    tokenize(expression).is_ok_and(|tokens| ensure_only_proven_function_calls(&tokens).is_ok())
}
fn integer_primary_key(metadata: &TableMetadata) -> bool {
    !metadata.primary_key.is_empty()
        && metadata.primary_key.iter().all(|k| {
            metadata.column(k).is_some_and(|c| {
                matches!(
                    c.data_type.to_ascii_lowercase().as_str(),
                    "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint"
                )
            })
        })
}
fn key_cannot_be_coerced(metadata: &TableMetadata, key: &Key) -> bool {
    // DATA_TYPE intentionally does not certify signedness. Use the common
    // nonnegative range, so even permissive SQL modes cannot clamp two
    // different planned identities onto the same physical primary key.
    key.len() == metadata.primary_key.len()
        && key.iter().zip(&metadata.primary_key).all(|(value, name)| {
            let Some(column) = metadata.column(name) else {
                return false;
            };
            let max = match column.data_type.to_ascii_lowercase().as_str() {
                "tinyint" => 127,
                "smallint" => 32767,
                "mediumint" => 8388607,
                "int" | "integer" => i32::MAX as i128,
                "bigint" => i64::MAX as i128,
                _ => return false,
            };
            *value >= 0 && *value <= max
        })
}
fn literal(value: &str) -> Option<i128> {
    let value = value.trim();
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}
fn equality_key(condition: &str, metadata: &TableMetadata) -> Option<Key> {
    // Deliberately recognize only PK-column = integer [AND ...]. No predicates
    // over mutable values, functions, subqueries, casts, IN, OR, or aliases.
    let tokens = tokenize(condition).ok()?;
    let mut values = BTreeMap::new();
    let mut i = 0;
    loop {
        let col = tokens.get(i)?;
        if !matches!(col.kind, TokenKind::Word | TokenKind::QuotedIdentifier) {
            return None;
        }
        let name = col.text.trim_matches('`').to_ascii_lowercase();
        if !metadata.is_primary_key(&name) || tokens.get(i + 1)?.text != "=" {
            return None;
        }
        i += 2;
        let sign = if matches!(tokens.get(i)?.text.as_str(), "+" | "-") {
            let s = tokens[i].text.clone();
            i += 1;
            s
        } else {
            String::new()
        };
        let value = literal(&format!("{sign}{}", tokens.get(i)?.text))?;
        if values.insert(name, value).is_some() {
            return None;
        }
        i += 1;
        if i == tokens.len() {
            break;
        }
        if tokens.get(i)?.upper() != "AND" {
            return None;
        }
        i += 1;
    }
    metadata
        .primary_key
        .iter()
        .map(|k| values.get(&k.to_ascii_lowercase()).copied())
        .collect()
}
fn candidates(
    queries: &[String],
    plans: &[ProtectedStatement],
    index: usize,
    metadata: &TableMetadata,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut keys = HashSet::new();
    let mut bytes = 0;
    let Some(ProtectedStatement::Dml(first)) = plans.first() else {
        return out;
    };
    let Some(target_kind) = target(first) else {
        return out;
    };
    for (offset, (query, plan)) in queries.iter().zip(plans).take(MAX_STATEMENTS).enumerate() {
        let ProtectedStatement::Dml(plan) = plan else {
            break;
        };
        if target(plan) != Some(target_kind)
            || query.contains(['\\', '#'])
            || query.contains("/*")
            || query.contains("--")
        {
            // Do not reason past SQL-mode-sensitive escapes or executable
            // comments. The existing immediate path retains their semantics.
            break;
        }
        let parsed = match plan {
            DmlPlan::Insert(p) => {
                if validate_insert_columns(p, metadata).is_err()
                    || p.rows
                        .iter()
                        .flatten()
                        .any(|value| !proven_expression(value))
                {
                    break;
                }
                let positions: Option<Vec<_>> = metadata
                    .primary_key
                    .iter()
                    .map(|k| p.columns.iter().position(|c| c.eq_ignore_ascii_case(k)))
                    .collect();
                let Some(positions) = positions else {
                    break;
                };
                let row_keys: Option<Vec<Key>> = p
                    .rows
                    .iter()
                    .map(|row| {
                        positions
                            .iter()
                            .map(|pos| row.get(*pos).and_then(|s| literal(s)))
                            .collect()
                    })
                    .collect();
                row_keys.zip(explicit_insert_key_condition(p, metadata).ok().flatten())
            }
            DmlPlan::Update(p) => {
                if !proven_expression(&p.statement_prefix)
                    || p.assigned_columns.iter().any(|c| {
                        metadata.is_primary_key(c) || metadata.column(c).is_none_or(|m| m.generated)
                    })
                {
                    break;
                }
                p.where_sql
                    .as_deref()
                    .and_then(|s| equality_key(s, metadata).map(|k| (vec![k], s.to_string())))
            }
            DmlPlan::Delete(p) => p
                .where_sql
                .as_deref()
                .and_then(|s| equality_key(s, metadata).map(|k| (vec![k], s.to_string()))),
            _ => None,
        };
        let Some((row_keys, condition)) = parsed else {
            break;
        };
        if !row_keys
            .iter()
            .all(|key| key_cannot_be_coerced(metadata, key))
        {
            break;
        }
        let own: HashSet<_> = row_keys.iter().cloned().collect();
        let cost = query
            .len()
            .saturating_add(condition.len().saturating_mul(2));
        if row_keys.is_empty()
            || own.len() != row_keys.len()
            || own.iter().any(|k| keys.contains(k))
            || keys.len() + own.len() > rollback_capture_row_limit().min(MAX_STATEMENTS)
            || bytes + cost > MAX_SQL_BYTES
        {
            break;
        }
        bytes += cost;
        keys.extend(own);
        out.push(Candidate {
            index: index + offset,
            query: query.clone(),
            plan: plan.clone(),
            keys: row_keys,
            condition,
        });
    }
    out
}
fn select_rows(rows: &RowImages, keys: &[Key]) -> Vec<CapturedRow> {
    let mut selected: Vec<_> = keys
        .iter()
        .filter_map(|key| rows.get(key).map(|row| (key, row)))
        .collect();
    selected.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
    selected.into_iter().map(|(_, row)| row.clone()).collect()
}
fn index_rows(metadata: &TableMetadata, rows: Vec<CapturedRow>) -> Result<RowImages, String> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let key = primary_key_values(metadata, &row)?
            .into_iter()
            .map(|value| {
                let hex = value
                    .strip_prefix("X'")
                    .and_then(|s| s.strip_suffix('\''))
                    .ok_or("Integer row identity was not losslessly encoded")?;
                if hex.len() % 2 != 0 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("Malformed integer row identity".to_string());
                }
                let bytes = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                let value = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                literal(&value)
                    .ok_or_else(|| "Captured integer row identity was invalid".to_string())
            })
            .collect::<Result<Key, String>>()?;
        if indexed.insert(key, row).is_some() {
            return Err("Window captured duplicate row identity".into());
        }
    }
    Ok(indexed)
}

#[cfg(test)]
#[path = "rollback_dml_window_tests.rs"]
mod tests;
