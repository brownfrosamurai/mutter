//! Local history store — SQLite via `rusqlite`, on-device only. STUB —
//! Phase 2 (cancel + history) work.
//!
//! Every transcription is written here regardless of what happens
//! downstream — this is what makes "copy and paste it at any time" (recovery
//! from a failed paste) actually possible, and it's what feeds the dashboard
//! (docs/mutter-project-plan.md Section 3, Section 8).
//!
//! Schema migrations use `rusqlite_migration` with a backup-then-migrate
//! failure path: before running any migration, copy the DB file to a
//! timestamped backup; if a migration fails, refuse to launch normally and
//! show a recovery screen naming the backup path — never launch against a
//! half-migrated schema (Section 11, Test Issue 6).
//!
//! Dashboard aggregates (time-saved, total count, WPM average) are
//! maintained as running totals updated on each insert, not recomputed by
//! scanning the whole table — with a manual "recompute from scratch" action
//! available in settings to correct drift (Section 8).

pub mod migrations;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("database error: {0}")]
    Database(String),
    #[error("migration failed, backup at: {0}")]
    MigrationFailed(String),
}

pub struct HistoryEntry {
    pub timestamp: i64,
    pub duration_secs: f64,
    pub text: String,
    pub language: String,
    pub engine: String,
}

pub struct HistoryStore {
    // rusqlite Connection goes here.
}

impl HistoryStore {
    /// Opens (or creates) the history DB at
    /// `~/Library/Application Support/Mutter/history.sqlite`, backing up
    /// before any migration and refusing to launch normally on migration
    /// failure — see module docs above.
    pub fn open() -> Result<Self, HistoryError> {
        unimplemented!("HistoryStore::open — Phase 2 work")
    }

    pub fn insert(&self, _entry: &HistoryEntry) -> Result<(), HistoryError> {
        unimplemented!("HistoryStore::insert — Phase 2 work")
    }

    /// Paginated, most-recent-first — never load the full table at once
    /// (Section 8, Performance Issue 9).
    pub fn list_page(&self, _page: u32, _page_size: u32) -> Result<Vec<HistoryEntry>, HistoryError> {
        unimplemented!("HistoryStore::list_page — Phase 5 (dashboard) work")
    }

    /// Manual drift-correction action — full-table scan, acceptable as an
    /// occasional operation even at large row counts (Section 8).
    pub fn recompute_aggregates(&self) -> Result<(), HistoryError> {
        unimplemented!("HistoryStore::recompute_aggregates — Phase 5 work")
    }
}
