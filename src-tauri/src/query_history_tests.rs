#[cfg(test)]
mod tests {
    use crate::query_history::{backfill_missing_database, QueryHistoryEntry};

    fn make_entry(id: &str, database: Option<&str>) -> QueryHistoryEntry {
        QueryHistoryEntry {
            id: id.into(),
            sql: "SELECT 1".into(),
            executed_at: "2024-01-01T00:00:00Z".into(),
            execution_time_ms: None,
            status: "success".into(),
            rows_affected: None,
            error: None,
            database: database.map(|s| s.into()),
        }
    }

    #[test]
    fn backfills_only_entries_with_none_database() {
        let mut entries = vec![
            make_entry("1", None),
            make_entry("2", Some("existing")),
            make_entry("3", None),
        ];
        let updated = backfill_missing_database(&mut entries, "app");
        assert_eq!(updated, 2);
        assert_eq!(entries[0].database.as_deref(), Some("app"));
        assert_eq!(entries[1].database.as_deref(), Some("existing"));
        assert_eq!(entries[2].database.as_deref(), Some("app"));
    }

    #[test]
    fn returns_zero_when_nothing_to_backfill() {
        let mut entries = vec![make_entry("1", Some("app")), make_entry("2", Some("app"))];
        let updated = backfill_missing_database(&mut entries, "other");
        assert_eq!(updated, 0);
        assert_eq!(entries[0].database.as_deref(), Some("app"));
        assert_eq!(entries[1].database.as_deref(), Some("app"));
    }

    #[test]
    fn handles_empty_list() {
        let mut entries: Vec<QueryHistoryEntry> = Vec::new();
        let updated = backfill_missing_database(&mut entries, "app");
        assert_eq!(updated, 0);
        assert!(entries.is_empty());
    }

    #[test]
    fn does_not_overwrite_empty_string_database() {
        let mut entries = vec![make_entry("1", Some(""))];
        let updated = backfill_missing_database(&mut entries, "app");
        assert_eq!(updated, 0);
        assert_eq!(entries[0].database.as_deref(), Some(""));
    }
}
