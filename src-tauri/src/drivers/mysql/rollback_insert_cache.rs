//! Reuse only transaction-locked table shape, never row images or mutable counters.
//! The owner is one execute-batch stack frame, not a pool or pinned-session cache.

use super::{
    load_locked_dml_metadata, sql_hex, DmlPlan, ObjectName, ProtectedStatement, TableMetadata,
};
use sqlx::{Executor, Row};

#[derive(Default)]
pub(super) struct InsertMetadataCache {
    eligible: bool,
    entry: Option<(ObjectName, TableMetadata)>,
    hits: usize,
    inspections: usize,
    counter_refreshes: usize,
    invalidations: usize,
}

impl InsertMetadataCache {
    pub(super) fn locked_metadata(&self, object: &ObjectName) -> Option<&TableMetadata> {
        self.entry
            .as_ref()
            .filter(|(key, _)| self.eligible && key == object)
            .map(|(_, metadata)| metadata)
    }

    /// Even reads, session settings, temporary-table changes and transaction
    /// boundaries end a reuse run. This avoids reasoning about hidden session
    /// effects and keeps USE/DDL/COMMIT/ROLLBACK invalidation fail-closed.
    pub(super) fn before_statement(&mut self, plan: &ProtectedStatement, in_transaction: bool) {
        self.eligible =
            in_transaction && matches!(plan, ProtectedStatement::Dml(DmlPlan::Insert(_)));
        let same_table = matches!(
            (plan, &self.entry),
            (ProtectedStatement::Dml(DmlPlan::Insert(plan)), Some((object, _)))
                if plan.table == *object
        );
        if !self.eligible || !same_table {
            if self.entry.take().is_some() {
                self.invalidations += 1;
            }
        }
    }

    pub(super) async fn load(
        &mut self,
        conn: &mut sqlx::MySqlConnection,
        object: &ObjectName,
    ) -> Result<TableMetadata, String> {
        if let Some((key, metadata)) = &self.entry {
            if self.eligible && key == object {
                let mut metadata = metadata.clone();
                // AUTO_INCREMENT is data-dependent even while the MDL is held.
                // Preserve the pre-statement value used by rollback generation;
                // caching it would corrupt the inverse of later inserts.
                if metadata.columns.iter().any(|column| column.auto_increment) {
                    let sql = format!(
                        "SELECT AUTO_INCREMENT FROM information_schema.TABLES \
                         WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {}",
                        sql_hex(metadata.schema.as_bytes()),
                        sql_hex(metadata.name.as_bytes())
                    );
                    let row = conn.fetch_one(sqlx::raw_sql(&sql)).await.map_err(|error| {
                        format!(
                            "Could not refresh protected INSERT auto-increment counter: {error}"
                        )
                    })?;
                    metadata.auto_increment_next = row
                        .try_get::<Option<u64>, _>(0)
                        .map_err(|error| error.to_string())?;
                    self.counter_refreshes += 1;
                }
                self.hits += 1;
                return Ok(metadata);
            }
        }
        self.inspections += 1;
        // A miss must perform the original lock + reinspection + admission
        // checks. Never retain a failed/partial load or bypass a refusal.
        self.entry = None;
        let metadata = load_locked_dml_metadata(conn, object, false).await?;
        if self.eligible {
            self.entry = Some((object.clone(), metadata.clone()));
        }
        Ok(metadata)
    }
}

impl Drop for InsertMetadataCache {
    fn drop(&mut self) {
        if self.inspections > 0 {
            // Also emitted on cancellation: no SQL or connection secrets here.
            log::info!(
                "Protected INSERT metadata reuse: hits={}, inspections={}, counter_refreshes={}, invalidations={}; reuse is limited to consecutive inserts in one explicit transaction and one batch",
                self.hits, self.inspections, self.counter_refreshes, self.invalidations
            );
        }
    }
}

#[cfg(test)]
#[path = "rollback_insert_cache_tests.rs"]
mod tests;
