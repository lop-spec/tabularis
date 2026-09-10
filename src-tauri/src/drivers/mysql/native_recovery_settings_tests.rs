use super::*;

fn compatible() -> (Variables, Variables) {
    let mut global = Variables::new();
    let mut session = Variables::new();
    for &(scope, name, value) in REQUIREMENTS {
        match scope {
            VariableScope::Global => &mut global,
            VariableScope::Session => &mut session,
        }
        .insert(name.into(), value.into());
    }
    global.insert("version".into(), "8.0.36".into());
    global.insert("version_comment".into(), "Source distribution".into());
    global.insert(
        "server_uuid".into(),
        "00000000-0000-0000-0000-000000000001".into(),
    );
    (global, session)
}

#[test]
fn compatible_settings_never_certify_recovery_or_destructive_ddl() {
    let (global, session) = compatible();
    let report = assess(global, session);
    assert!(report.dml_settings_compatible());
    assert_eq!(
        report.outstanding_proofs,
        vec![
            OutstandingProof::BinlogReadAccess,
            OutstandingProof::DurableRetentionUntilMaterialized,
            OutstandingProof::ExactTransactionAttribution,
            OutstandingProof::LosslessRowDecodingAndSchemaHistory,
            OutstandingProof::CrashSafeIntentAndMaterialization,
            OutstandingProof::DdlBackupAndRestore,
        ]
    );
}

#[test]
fn every_missing_requirement_is_reported_without_defaulting_to_safe() {
    for &(scope, name, _) in REQUIREMENTS {
        let (mut global, mut session) = compatible();
        match scope {
            VariableScope::Global => &mut global,
            VariableScope::Session => &mut session,
        }
        .remove(name);
        let report = assess(global, session);
        assert!(!report.dml_settings_compatible(), "missing {name}");
        assert_eq!(report.blocking_settings.len(), 1);
        assert_eq!(report.blocking_settings[0].name, name);
        assert_eq!(report.blocking_settings[0].observed, None);
    }
}

#[test]
fn every_incompatible_requirement_is_a_blocker() {
    for &(scope, name, _) in REQUIREMENTS {
        let (mut global, mut session) = compatible();
        match scope {
            VariableScope::Global => &mut global,
            VariableScope::Session => &mut session,
        }
        .insert(name.into(), "unexpected".into());
        let report = assess(global, session);
        assert!(!report.dml_settings_compatible(), "invalid {name}");
        assert_eq!(report.blocking_settings.len(), 1);
        assert_eq!(report.blocking_settings[0].scope, scope);
        assert_eq!(report.blocking_settings[0].name, name);
    }
}

#[test]
fn delayed_binlog_and_redo_flushes_are_both_reported() {
    let (mut global, session) = compatible();
    global.insert("sync_binlog".into(), "1000".into());
    global.insert("innodb_flush_log_at_trx_commit".into(), "2".into());
    let report = assess(global, session);
    assert_eq!(
        report
            .blocking_settings
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["sync_binlog", "innodb_flush_log_at_trx_commit"]
    );
}

#[test]
fn global_row_images_cannot_mask_session_overrides() {
    for (name, value) in [
        ("binlog_format", "MIXED"),
        ("binlog_row_image", "MINIMAL"),
        ("sql_log_bin", "OFF"),
    ] {
        let (mut global, mut session) = compatible();
        global.insert(name.into(), session[name].clone());
        session.insert(name.into(), value.into());
        let report = assess(global, session);
        assert_eq!(report.blocking_settings.len(), 1);
        assert_eq!(report.blocking_settings[0].scope, VariableScope::Session);
        assert_eq!(report.blocking_settings[0].name, name);
    }
}

#[test]
fn retention_and_full_metadata_are_not_a_durable_archive_lease() {
    for interval in ["0", "1", "604800", "2592000"] {
        let (mut global, session) = compatible();
        global.insert("binlog_expire_logs_seconds".into(), interval.into());
        global.insert("binlog_row_metadata".into(), "FULL".into());
        let report = assess(global, session);
        assert!(report
            .outstanding_proofs
            .contains(&OutstandingProof::DurableRetentionUntilMaterialized));
        assert!(report
            .outstanding_proofs
            .contains(&OutstandingProof::LosslessRowDecodingAndSchemaHistory));
    }
}

#[test]
fn unknown_or_other_server_families_are_not_assumed_compatible() {
    for version in [
        None,
        Some("5.7.44"),
        Some("9.0.1"),
        Some("10.11.0-MariaDB"),
        Some("8.0.11-TiDB-v8.1.0"),
    ] {
        let (mut global, session) = compatible();
        global.remove("version");
        if let Some(version) = version {
            global.insert("version".into(), version.into());
        }
        assert!(assess(global, session)
            .blocking_settings
            .iter()
            .any(|item| item.name == "version"));
    }
    let (mut global, session) = compatible();
    global.insert("version_comment".into(), "TiDB Server".into());
    assert!(!assess(global, session).dml_settings_compatible());
}

#[test]
fn malformed_missing_or_nil_identity_is_blocked() {
    for identity in [
        None,
        Some(""),
        Some("unknown"),
        Some("00000000-0000-0000-0000-000000000000"),
    ] {
        let (mut global, session) = compatible();
        global.remove("server_uuid");
        if let Some(identity) = identity {
            global.insert("server_uuid".into(), identity.into());
        }
        let report = assess(global, session);
        assert_eq!(report.blocking_settings.len(), 1);
        assert_eq!(report.blocking_settings[0].name, "server_uuid");
    }
}

#[test]
fn server_boolean_and_case_representations_are_supported() {
    let (mut global, mut session) = compatible();
    global.insert("log_bin".into(), "1".into());
    global.insert("version".into(), "8.4.5".into());
    session.insert("sql_log_bin".into(), "on".into());
    session.insert("binlog_format".into(), "row".into());
    session.insert("binlog_row_image".into(), "full".into());
    assert!(assess(global, session).dml_settings_compatible());
}

#[test]
fn report_serialization_preserves_unknowns_and_unverified_evidence() {
    let (mut global, session) = compatible();
    global.remove("sync_binlog");
    let report = serde_json::to_value(assess(global, session)).unwrap();
    assert!(report["blockingSettings"][0]["observed"].is_null());
    assert_eq!(report["blockingSettings"][0]["scope"], "global");
    assert_eq!(report["outstandingProofs"].as_array().unwrap().len(), 6);
    assert!(report.get("ready").is_none());
}

#[test]
fn inspection_queries_are_read_only_fixed_allowlists_with_correct_scopes() {
    assert!(GLOBAL_QUERY.starts_with("SHOW GLOBAL VARIABLES WHERE Variable_name IN ("));
    assert!(SESSION_QUERY.starts_with("SHOW SESSION VARIABLES WHERE Variable_name IN ("));
    for &(scope, name, _) in REQUIREMENTS {
        let query = match scope {
            VariableScope::Global => GLOBAL_QUERY,
            VariableScope::Session => SESSION_QUERY,
        };
        assert!(query.contains(&format!("'{name}'")));
    }
    for query in [GLOBAL_QUERY, SESSION_QUERY] {
        assert!(!query.contains(';'));
        assert!(!query.contains("SET "));
        assert!(!query.to_ascii_lowercase().contains("password"));
        assert!(!query.to_ascii_lowercase().contains("authentication_string"));
    }
}
