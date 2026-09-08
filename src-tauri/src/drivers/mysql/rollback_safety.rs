//! Strict admission and row-set checks shared by the rollback execution paths.
//! Unsupported shapes are refused, never retried as unprotected writes.
use super::*;

pub(super) fn refusal(reason: &str) -> String {
    log::warn!("Strict rollback protection refused execution: {reason}");
    format!(
        "Strict rollback protection refused execution: {reason}. Rewrite the statement into supported, explicitly keyed DML and retry with protection enabled. No unprotected retry was performed; earlier committed statements, if any, remain committed."
    )
}

pub(super) fn preflight(
    plans: &[ProtectedStatement],
    policy: Option<RollbackUnsupportedPolicy>,
) -> Result<(), String> {
    if policy == Some(RollbackUnsupportedPolicy::ExecuteUnprotected) {
        return Err(refusal(
            "execute_unprotected cannot bypass an enabled protection flag",
        ));
    }
    let reasons: Vec<String> = plans
        .iter()
        .enumerate()
        .filter_map(|(index, plan)| match plan {
            ProtectedStatement::Unsupported(blocked) => {
                if policy == Some(RollbackUnsupportedPolicy::Skip) {
                    log::warn!(
                        "Strict rollback protection will skip statement {}: {}",
                        index + 1,
                        blocked.reason
                    );
                    None
                } else {
                    Some(format!("statement {}: {}", index + 1, blocked.reason))
                }
            }
            _ => None,
        })
        .collect();
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(refusal(&reasons.join("; ")))
    }
}

pub(super) fn validate_insert_source(plan: &InsertFamilyPlan) -> Result<(), String> {
    match &plan.source {
        InsertSource::Select(_) => Err(refusal(
            "automatic INSERT SELECT materialization is disabled: source types, snapshot/locking semantics, default values and per-statement time cannot be preserved by hex VALUES chunking; explicitly materialize and review the intended typed rows first"
        )),
        InsertSource::Values(rows) if plan.upsert.is_some() && rows.len() != 1 => Err(refusal(
            "multi-row upsert can revisit row identities; split into explicitly reviewed single-row statements in one transaction (account for time/default-expression semantics)"
        )),
        _ => Ok(()),
    }
}

/// A write must never reevaluate a time-sensitive predicate or an unlocked
/// subquery after taking its before-image. The SET expressions still execute
/// once, in the original UPDATE; only row-selection expressions are restricted.
pub(super) fn repeatable_selection(tokens: &[Token]) -> Result<(), BlockedStatement> {
    ensure_only_proven_function_calls(tokens)?;
    const UNSTABLE: &[&str] = &[
        "SELECT",
        "WITH",
        "NOW",
        "SYSDATE",
        "CURDATE",
        "CURTIME",
        "CURRENT_DATE",
        "CURRENT_TIME",
        "CURRENT_TIMESTAMP",
        "LOCALTIME",
        "LOCALTIMESTAMP",
        "UTC_DATE",
        "UTC_TIME",
        "UTC_TIMESTAMP",
        "UNIX_TIMESTAMP",
        "RAND",
        "UUID",
        "UUID_SHORT",
        "LAST_INSERT_ID",
        "FOUND_ROWS",
        "ROW_COUNT",
    ];
    if tokens
        .iter()
        .any(|t| t.kind == TokenKind::Word && UNSTABLE.contains(&t.upper()))
    {
        return Err(BlockedStatement::unsupported(
            "row selection contains a subquery or a time/session-dependent expression; evaluate and review a stable key list before executing the write"
        ));
    }
    Ok(())
}

pub(super) fn single_target(plan: &MultiTablePlan) -> Result<(), BlockedStatement> {
    if plan.targets.len() != 1 {
        return Err(BlockedStatement::unsupported(
            "writes to multiple target aliases require manual transactional decomposition; automatic key restriction is not proven equivalent for outer/self joins"
        ));
    }
    Ok(())
}

/// Preserve JOIN/SET expressions but intersect actual writes with the locked
/// stable keys. A row-count equality alone is not a row-set proof.
pub(super) fn restricted_multi_sql(
    query: &str,
    plan: &MultiTablePlan,
    metadata: &TableMetadata,
    rows: &[CapturedRow],
) -> Result<String, String> {
    single_target(plan).map_err(|blocked| refusal(&blocked.reason))?;
    let alias = quote_identifier(&plan.targets[0].alias);
    let mut predicates = Vec::with_capacity(rows.len());
    for row in rows {
        let values = primary_key_values(metadata, row)?;
        let parts: Vec<String> = metadata
            .primary_key
            .iter()
            .zip(values)
            .map(|(column, value)| {
                crate::mysql_row_identity::encoded_key_condition(
                    &format!("{alias}.{}", quote_identifier(column)),
                    &metadata
                        .column(column)
                        .expect("validated identity metadata")
                        .data_type,
                    &value,
                )
            })
            .collect();
        predicates.push(format!("({})", parts.join(" AND ")));
    }
    let filter = if predicates.is_empty() {
        "FALSE".to_string()
    } else {
        predicates.join(" OR ")
    };
    let tokens = tokenize(query).map_err(|blocked| refusal(&blocked.reason))?;
    let tokens = trim_trailing_semicolon(&tokens);
    let end = statement_end(query, tokens);
    let prefix_end =
        find_top_level_word(tokens, 0, &["WHERE"]).map_or(end, |index| tokens[index].start);
    let condition = plan.where_sql.as_deref().unwrap_or("TRUE");
    // Reuse the single-table builder: its newline is essential when a line
    // comment ends the original SET clause immediately before WHERE.
    Ok(locked_write_sql(
        query[..prefix_end].trim_end(),
        Some(condition),
        &filter,
    ))
}

pub(super) fn validate_upsert_counts(
    affected: u64,
    before_count: usize,
    after_count: usize,
    inserted: usize,
    changed: u64,
) -> Result<(), String> {
    // Single input row, at most one conflicting existing row. CLIENT_FOUND_ROWS
    // changes ONLY the no-op outcome from 0 to 1, not changed updates from 2.
    let valid = after_count == 1
        && match (before_count, inserted, changed) {
            (0, 1, 0) => affected == 1,
            (1, 0, 1) => affected == 2,
            (1, 0, 0) => affected <= 1,
            _ => false,
        };
    if valid {
        Ok(())
    } else {
        Err(format!("Upsert row-set/count mismatch (affected={affected}, before={before_count}, after={after_count}, inserted={inserted}, changed={changed}); protected transaction must be rolled back"))
    }
}

pub(super) fn safe_conflict_isolation(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().replace(' ', "-").as_str(),
        "REPEATABLE-READ" | "SERIALIZABLE"
    )
}

pub(super) fn complete_duplicate_warnings(count: u64, codes: &[u64]) -> Result<(), String> {
    if count == codes.len() as u64 && codes.iter().all(|code| *code == 1062) {
        Ok(())
    } else {
        Err(format!("INSERT IGNORE produced non-duplicate or incompletely captured warnings ({count} reported, codes={codes:?}); protected transaction must be rolled back"))
    }
}

/// Pick an entire stable, full-column, NOT NULL unique index, never a subset.
/// Existing journal fields retain their wire name `primaryKey`, but represent
/// the chosen stable identity (PRIMARY first, otherwise this verified key).
pub(super) fn stable_unique_key(index_columns: Vec<(String, String, bool)>) -> Option<Vec<String>> {
    let mut indexes: BTreeMap<String, (Vec<String>, bool)> = BTreeMap::new();
    for (index, column, usable) in index_columns {
        let entry = indexes.entry(index).or_insert_with(|| (Vec::new(), true));
        entry.0.push(column);
        entry.1 &= usable;
    }
    indexes
        .into_values()
        .find_map(|(columns, usable)| (usable && !columns.is_empty()).then_some(columns))
}
