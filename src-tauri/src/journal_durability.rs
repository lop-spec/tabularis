//! Bounded write coalescing for journals owned by an uncommitted database transaction.
//!
//! Headers and ordinary appends remain synchronous. Only an executor that owns
//! rollback-on-disconnect semantics may defer writes, and it must finish BOTH
//! journals before submitting any statement that can commit the transaction.
use std::fs::File;
use std::io::{self, Write};

const MAX_PENDING_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct JournalDurability {
    pending: Vec<u8>,
    deferred: bool,
    strict: bool,
    synced_len: u64,
    poisoned: bool,
    #[cfg(test)]
    syncs: usize,
    #[cfg(test)]
    pub(crate) fail_next_sync: bool,
}

impl JournalDurability {
    pub(crate) fn defer(&mut self) {
        self.deferred = !self.strict;
    }

    pub(crate) fn require_immediate(&mut self, file: &mut File) -> io::Result<()> {
        if !self.strict {
            log::info!("Journal coalescing disabled: manual DDL or AUTO_INCREMENT recovery must retain immediate durability");
            self.strict = true;
        }
        self.finish(file)
    }

    pub(crate) fn requires_immediate(&self) -> bool {
        self.strict
    }

    pub(crate) fn append(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        self.check_usable()?;
        if !self.deferred {
            return self.persist(file, bytes);
        }
        if bytes.len() > MAX_PENDING_BYTES.saturating_sub(self.pending.len()) {
            self.flush_pending(file)?;
        }
        if bytes.len() > MAX_PENDING_BYTES {
            // A single large record must not introduce another unbounded copy.
            return self.persist(file, bytes);
        }
        self.pending.extend_from_slice(bytes);
        Ok(())
    }

    pub(crate) fn finish(&mut self, file: &mut File) -> io::Result<()> {
        self.deferred = false;
        self.check_usable()?;
        self.flush_pending(file)
    }

    fn check_usable(&self) -> io::Result<()> {
        if self.poisoned {
            Err(io::Error::other(
                "journal is unusable after an earlier write failure",
            ))
        } else {
            Ok(())
        }
    }

    fn flush_pending(&mut self, file: &mut File) -> io::Result<()> {
        let pending = std::mem::take(&mut self.pending);
        self.persist(file, &pending)
    }

    fn persist(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let outcome = crate::rollback_sql::run_blocking(|| {
            file.write_all(bytes)?;
            file.flush()?;
            #[cfg(test)]
            if std::mem::take(&mut self.fail_next_sync) {
                return Err(io::Error::other("injected journal sync failure"));
            }
            file.sync_all()
        });
        match outcome {
            Ok(()) => {
                self.synced_len += bytes.len() as u64;
                #[cfg(test)]
                {
                    self.syncs += 1;
                }
                Ok(())
            }
            Err(error) => {
                // Never reuse an uncertain file offset, even if truncation works.
                self.poisoned = true;
                log::error!("Journal persistence failed; commit must remain blocked: {error}");
                if let Err(truncate_error) = file.set_len(self.synced_len) {
                    log::error!("Could not restore the durable journal prefix: {truncate_error}");
                }
                Err(error)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn bytes_written(&self) -> u64 {
        self.synced_len
    }
}

#[cfg(test)]
#[path = "journal_durability_tests.rs"]
mod tests;
