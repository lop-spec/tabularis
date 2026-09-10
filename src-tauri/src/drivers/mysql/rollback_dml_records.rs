//! One exact row-image validator/renderer for immediate and windowed execution.
use super::*;

pub(super) fn record(
    query: &str,
    plan: &DmlPlan,
    statement_index: usize,
    metadata: &TableMetadata,
    before: Vec<CapturedRow>,
    after: Vec<CapturedRow>,
    condition: Option<String>,
    result: QueryResult,
    rollback: &mut RollbackJournal,
    recovery: &mut RecoveryJournal,
) -> Result<QueryResult, String> {
    match plan {
        DmlPlan::Insert(plan) => {
            if result.affected_rows != plan.rows.len() as u64 || after.len() != plan.rows.len() {
                return Err("INSERT affected-row count or after-image does not match its VALUES plan; transaction must be rolled back".into());
            }
            let mut steps = Vec::new();
            if metadata.auto_increment_primary_key().is_some() {
                if let Some(next) = metadata.auto_increment_next {
                    steps.push(RollbackStep {
                        statement_index,
                        sql: format!(
                            "ALTER TABLE {} AUTO_INCREMENT = {next}",
                            metadata.qualified_name()
                        ),
                        expected_affected_rows: None,
                    });
                }
            }
            for row in &after {
                steps.push(RollbackStep {
                    statement_index,
                    sql: build_insert_rollback_delete(metadata, row)?,
                    expected_affected_rows: Some(1),
                });
            }
            rollback.add_steps(steps)?;
            recovery.add_statement(recovery_dml_statement(
                query,
                statement_index,
                "insert",
                metadata,
                metadata
                    .writable_columns()
                    .map(|c| c.name.clone())
                    .collect(),
                condition,
                Vec::new(),
                after,
            ))?;
        }
        DmlPlan::Update(plan) => {
            let before_by_key = rows_by_primary_key(metadata, before)?;
            let after_by_key = rows_by_primary_key(metadata, after)?;
            if before_by_key.len() != after_by_key.len()
                || before_by_key.keys().ne(after_by_key.keys())
            {
                return Err("UPDATE changed row identity or caused rows to disappear; transaction was rolled back".into());
            }
            let columns: Vec<_> = metadata
                .writable_columns()
                .map(|c| c.name.clone())
                .collect();
            let mut changed_columns = BTreeSet::new();
            let mut changed_before = Vec::new();
            let mut changed_after = Vec::new();
            let mut steps = Vec::new();
            for (key, old) in before_by_key {
                let new = &after_by_key[&key];
                if old.values == new.values {
                    continue;
                }
                if old.values.len() != columns.len() || new.values.len() != columns.len() {
                    return Err(
                        "UPDATE row image width does not match writable column metadata".into(),
                    );
                }
                for (index, name) in columns.iter().enumerate() {
                    if old.values[index] != new.values[index] {
                        changed_columns.insert(name.clone());
                    }
                }
                steps.push(RollbackStep {
                    statement_index,
                    sql: build_update_rollback(metadata, &old, new)?,
                    expected_affected_rows: Some(1),
                });
                changed_before.push(old);
                changed_after.push(new.clone());
            }
            if result.affected_rows != steps.len() as u64
                && result.affected_rows != after_by_key.len() as u64
            {
                return Err(format!("UPDATE reported {} affected rows but row diff found {}; transaction was rolled back", result.affected_rows, steps.len()));
            }
            if !steps.is_empty() {
                rollback.add_steps(steps)?;
            }
            recovery.add_statement(recovery_dml_statement(
                query,
                statement_index,
                "update",
                metadata,
                changed_columns.into_iter().collect(),
                plan.where_sql.clone(),
                changed_before,
                changed_after,
            ))?;
        }
        DmlPlan::Delete(plan) => {
            if result.affected_rows != before.len() as u64 {
                return Err(format!("DELETE reported {} affected rows but {} before-images were locked; transaction was rolled back", result.affected_rows, before.len()));
            }
            if !after.is_empty() {
                return Err(
                    "DELETE left captured rows behind; transaction must be rolled back".into(),
                );
            }
            if !before.is_empty() {
                rollback.add_steps(
                    before
                        .iter()
                        .map(|row| RollbackStep {
                            statement_index,
                            sql: build_delete_rollback_insert(metadata, row),
                            expected_affected_rows: Some(1),
                        })
                        .collect(),
                )?;
            }
            recovery.add_statement(recovery_dml_statement(
                query,
                statement_index,
                "delete",
                metadata,
                metadata
                    .writable_columns()
                    .map(|c| c.name.clone())
                    .collect(),
                plan.where_sql.clone(),
                before,
                Vec::new(),
            ))?;
        }
        _ => return Err("Internal row-image recorder received an unsupported DML family".into()),
    }
    Ok(result)
}
