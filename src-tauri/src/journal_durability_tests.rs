use super::*;
use std::fs;

#[test]
fn default_appends_are_durable_and_linear() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut journal = JournalDurability::default();
    for _ in 0..3 {
        journal.append(file.as_file_mut(), b"record\n").unwrap();
    }
    assert_eq!(journal.syncs, 3);
    assert_eq!(journal.bytes_written(), 21);
    assert_eq!(fs::read(file.path()).unwrap(), b"record\nrecord\nrecord\n");
}

#[test]
fn thousands_of_uncommitted_records_share_one_commit_barrier() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut journal = JournalDurability::default();
    journal.append(file.as_file_mut(), b"header\n").unwrap();
    journal.defer();
    let mut expected = b"header\n".to_vec();
    for index in 0..3429 {
        let record = format!("{index}\n");
        journal
            .append(file.as_file_mut(), record.as_bytes())
            .unwrap();
        expected.extend_from_slice(record.as_bytes());
    }
    assert_eq!(fs::read(file.path()).unwrap(), b"header\n");
    assert_eq!(journal.syncs, 1);
    journal.finish(file.as_file_mut()).unwrap();
    assert_eq!(journal.syncs, 2);
    assert_eq!(fs::read(file.path()).unwrap(), expected);
    journal.finish(file.as_file_mut()).unwrap();
    assert_eq!(
        journal.syncs, 2,
        "empty barriers do not issue redundant fsyncs"
    );
    journal.append(file.as_file_mut(), b"finish\n").unwrap();
    assert_eq!(journal.syncs, 3, "default synchronous mode is restored");
}

#[test]
fn crash_before_barrier_leaves_only_the_pretransaction_durable_prefix() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut journal = JournalDurability::default();
    journal
        .append(file.as_file_mut(), b"earlier committed transaction\n")
        .unwrap();
    journal.defer();
    journal
        .append(file.as_file_mut(), b"uncommitted\n")
        .unwrap();
    drop(journal);
    assert_eq!(
        fs::read(file.path()).unwrap(),
        b"earlier committed transaction\n"
    );
}

#[test]
fn budget_flush_preserves_order_and_bounds_pending_memory() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut journal = JournalDurability::default();
    journal.defer();
    let full = vec![b'a'; MAX_PENDING_BYTES];
    journal.append(file.as_file_mut(), &full).unwrap();
    journal.append(file.as_file_mut(), b"b").unwrap();
    assert_eq!(journal.syncs, 1);
    assert_eq!(journal.pending, b"b");
    let oversized = vec![b'c'; MAX_PENDING_BYTES + 1];
    journal.append(file.as_file_mut(), &oversized).unwrap();
    assert!(journal.pending.is_empty());
    journal.finish(file.as_file_mut()).unwrap();
    let expected: Vec<u8> = full.into_iter().chain([b'b']).chain(oversized).collect();
    assert_eq!(fs::read(file.path()).unwrap(), expected);
}

#[test]
fn failed_sync_restores_prefix_and_permanently_blocks_reuse() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut journal = JournalDurability::default();
    journal.append(file.as_file_mut(), b"durable\n").unwrap();
    journal.defer();
    journal.append(file.as_file_mut(), b"torn\n").unwrap();
    journal.fail_next_sync = true;
    assert!(journal
        .finish(file.as_file_mut())
        .unwrap_err()
        .to_string()
        .contains("injected"));
    assert_eq!(fs::read(file.path()).unwrap(), b"durable\n");
    assert_eq!(journal.bytes_written(), 8);
    for _ in 0..2 {
        journal.defer();
        assert!(journal
            .append(file.as_file_mut(), b"must not commit")
            .is_err());
        assert!(journal.finish(file.as_file_mut()).is_err());
    }
    assert_eq!(fs::read(file.path()).unwrap(), b"durable\n");
}

#[test]
fn write_and_truncation_failures_also_poison_the_journal() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let mut readonly = File::open(file.path()).unwrap();
    let mut journal = JournalDurability::default();
    assert!(journal.append(&mut readonly, b"blocked").is_err());
    assert!(journal.poisoned);
    assert!(journal.finish(&mut readonly).is_err());
}

#[test]
fn nontransactional_steps_permanently_restore_immediate_durability() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut journal = JournalDurability::default();
    journal.defer();
    journal.append(file.as_file_mut(), b"before\n").unwrap();
    journal.require_immediate(file.as_file_mut()).unwrap();
    assert!(journal.requires_immediate());
    for _ in 0..2 {
        journal.defer();
        journal.append(file.as_file_mut(), b"counter\n").unwrap();
    }
    assert_eq!(journal.syncs, 3);
    assert_eq!(
        fs::read(file.path()).unwrap(),
        b"before\ncounter\ncounter\n"
    );
}

#[test]
fn repeated_defer_does_not_drop_records_between_statements_or_transactions() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut journal = JournalDurability::default();
    for _ in 0..2 {
        for _ in 0..3 {
            journal.defer();
            journal.append(file.as_file_mut(), b"x\n").unwrap();
        }
        journal.finish(file.as_file_mut()).unwrap();
    }
    assert_eq!(journal.syncs, 2);
    assert_eq!(fs::read(file.path()).unwrap(), b"x\nx\nx\nx\nx\nx\n");
}
