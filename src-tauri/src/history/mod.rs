//! Local history store — SQLite via `rusqlite`, on-device only.
//!
//! Every transcription is written here regardless of what happens
//! downstream — this is what makes "copy and paste it at any time" (recovery
//! from a failed paste) actually possible, and it's what feeds the dashboard
//! (docs/mutter-project-plan.md Section 3, Section 8).
//!
//! Schema migrations use `rusqlite_migration` with a backup-then-migrate
//! failure path: before running a migration that will actually change the
//! schema, the existing DB file is copied to a timestamped backup; if the
//! migration then fails, `open()` returns `HistoryError::MigrationFailed`
//! naming that backup path rather than silently launching against a
//! half-migrated schema (Section 11, Test Issue 6). The backup — and the
//! migration call itself — is skipped whenever the DB is already at the
//! latest schema version, so a normal app launch on an up-to-date DB
//! doesn't leave behind a new backup file every time (it used to, and that
//! meant unbounded backup accumulation over weeks of daily use — a real
//! bug caught by watching backups pile up in
//! `~/Library/Application Support/Mutter/` during dev testing).
//!
//! Dashboard aggregates (time-saved, total count, WPM average) are
//! maintained as running totals updated on each insert, not recomputed by
//! scanning the whole table — with `recompute_aggregates()` available as a
//! manual "recompute from scratch" action to correct drift (Section 8).

pub mod migrations;

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("database error: {0}")]
    Database(String),
    #[error("migration failed, backup at: {0}")]
    MigrationFailed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub timestamp: i64,
    pub duration_secs: f64,
    pub text: String,
    pub language: String,
    pub engine: String,
}

/// Section 8's dashboard metrics, computed from the running `aggregates`
/// row. `assumed_typing_wpm` is a configurable constant (default
/// [`DEFAULT_TYPING_WPM`]) — shown to the user as an assumption behind
/// "time saved", never presented as precise fact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub total_transcriptions: i64,
    pub total_word_count: i64,
    pub time_saved_minutes: f64,
    pub average_wpm: f64,
}

pub const DEFAULT_TYPING_WPM: f64 = 40.0;

pub struct HistoryStore {
    conn: Mutex<Connection>,
}

impl HistoryStore {
    /// Opens (or creates) the history DB at
    /// `~/Library/Application Support/Mutter/history.sqlite`. See module
    /// docs for the backup-then-migrate contract.
    pub fn open() -> Result<Self, HistoryError> {
        let dir = crate::paths::app_support_dir().map_err(|e| {
            HistoryError::Database(format!("could not resolve app support dir: {e}"))
        })?;
        Self::open_at(&dir.join("history.sqlite"))
    }

    fn open_at(db_path: &Path) -> Result<Self, HistoryError> {
        let db_existed = db_path.exists();
        let mut conn =
            Connection::open(db_path).map_err(|e| HistoryError::Database(e.to_string()))?;

        // Only worth a pre-migration backup (and the migration call at all)
        // when a migration will actually run. Without this check, every
        // single normal app launch — not just the rare schema-upgrade one —
        // would copy the whole DB file, accumulating one backup per launch
        // forever. An unreadable/corrupt existing file (current_version
        // erroring) still falls through to the backup+migrate path below,
        // since that's exactly the case the backup exists to protect.
        let needs_migration = if db_existed {
            match migrations::migrations().current_version(&conn) {
                Ok(v) => usize::from(&v) < migrations::LATEST_VERSION,
                Err(_) => true,
            }
        } else {
            true
        };
        if !needs_migration {
            return Ok(Self {
                conn: Mutex::new(conn),
            });
        }

        let backup_path = if db_existed {
            let backup_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let backup = db_path.with_extension(format!("sqlite.backup-{backup_secs}"));
            std::fs::copy(db_path, &backup).map_err(|e| {
                HistoryError::Database(format!("could not create pre-migration backup: {e}"))
            })?;
            Some(backup)
        } else {
            None
        };

        if let Err(e) = migrations::migrations().to_latest(&mut conn) {
            tracing::error!(error = %e, "history db migration failed");
            let backup_note = backup_path
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "none (fresh database — this should not happen)".to_string());
            return Err(HistoryError::MigrationFailed(backup_note));
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert(&self, entry: &HistoryEntry) -> Result<(), HistoryError> {
        let word_count = entry.text.split_whitespace().count() as i64;
        let mut conn = self.conn.lock().expect("history db lock poisoned");
        let tx = conn
            .transaction()
            .map_err(|e| HistoryError::Database(e.to_string()))?;

        tx.execute(
            "INSERT INTO history (timestamp, duration_secs, text, language, engine)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                entry.timestamp,
                entry.duration_secs,
                entry.text,
                entry.language,
                entry.engine
            ],
        )
        .map_err(|e| HistoryError::Database(e.to_string()))?;

        tx.execute(
            "UPDATE aggregates SET
                total_count = total_count + 1,
                total_word_count = total_word_count + ?1,
                total_dictation_seconds = total_dictation_seconds + ?2
             WHERE id = 0",
            params![word_count, entry.duration_secs],
        )
        .map_err(|e| HistoryError::Database(e.to_string()))?;

        tx.commit()
            .map_err(|e| HistoryError::Database(e.to_string()))
    }

    /// Paginated, most-recent-first — never loads the full table at once
    /// (Section 8, Performance Issue 9).
    pub fn list_page(&self, page: u32, page_size: u32) -> Result<Vec<HistoryEntry>, HistoryError> {
        let conn = self.conn.lock().expect("history db lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT timestamp, duration_secs, text, language, engine
                 FROM history ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| HistoryError::Database(e.to_string()))?;

        let offset = i64::from(page) * i64::from(page_size);
        let rows = stmt
            .query_map(params![page_size, offset], |row| {
                Ok(HistoryEntry {
                    timestamp: row.get(0)?,
                    duration_secs: row.get(1)?,
                    text: row.get(2)?,
                    language: row.get(3)?,
                    engine: row.get(4)?,
                })
            })
            .map_err(|e| HistoryError::Database(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| HistoryError::Database(e.to_string()))
    }

    /// Section 8's Metrics, computed from the running-aggregate row (no
    /// full-table scan).
    pub fn metrics(&self, assumed_typing_wpm: f64) -> Result<Metrics, HistoryError> {
        let conn = self.conn.lock().expect("history db lock poisoned");
        let (total_count, total_word_count, total_dictation_seconds): (i64, i64, f64) = conn
            .query_row(
                "SELECT total_count, total_word_count, total_dictation_seconds
                 FROM aggregates WHERE id = 0",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| HistoryError::Database(e.to_string()))?;

        let dictation_minutes = total_dictation_seconds / 60.0;
        let time_saved_minutes = if assumed_typing_wpm > 0.0 {
            (total_word_count as f64 / assumed_typing_wpm) - dictation_minutes
        } else {
            0.0
        };
        let average_wpm = if dictation_minutes > 0.0 {
            total_word_count as f64 / dictation_minutes
        } else {
            0.0
        };

        Ok(Metrics {
            total_transcriptions: total_count,
            total_word_count,
            time_saved_minutes,
            average_wpm,
        })
    }

    /// Per-language transcription counts, most-used first. Not part of
    /// Section 8's metric table (which only specifies time-saved/total/WPM/
    /// activity feed) — added because the dashboard's Languages section
    /// (built to match a user-supplied reference mockup) needs real data
    /// rather than a permanent "no data yet" placeholder. A plain `GROUP
    /// BY` over `history`, not a running aggregate — language cardinality
    /// is tiny (six languages, per the plan's scope), so a full scan here
    /// is cheap even at large row counts, unlike `recompute_aggregates`'s
    /// per-word tokenization.
    pub fn language_breakdown(&self) -> Result<Vec<(String, i64)>, HistoryError> {
        let conn = self.conn.lock().expect("history db lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT language, COUNT(*) as cnt FROM history
                 GROUP BY language ORDER BY cnt DESC",
            )
            .map_err(|e| HistoryError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| HistoryError::Database(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| HistoryError::Database(e.to_string()))
    }

    /// Manual drift-correction action — full-table scan, acceptable as an
    /// occasional operation even at large row counts (Section 8).
    pub fn recompute_aggregates(&self) -> Result<(), HistoryError> {
        let conn = self.conn.lock().expect("history db lock poisoned");

        let (total_count, total_dictation_seconds): (i64, f64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(duration_secs), 0.0) FROM history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| HistoryError::Database(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT text FROM history")
            .map_err(|e| HistoryError::Database(e.to_string()))?;
        let texts = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| HistoryError::Database(e.to_string()))?;

        let mut total_word_count: i64 = 0;
        for text in texts {
            let text = text.map_err(|e| HistoryError::Database(e.to_string()))?;
            total_word_count += text.split_whitespace().count() as i64;
        }
        drop(stmt);

        conn.execute(
            "UPDATE aggregates SET
                total_count = ?1, total_word_count = ?2, total_dictation_seconds = ?3
             WHERE id = 0",
            params![total_count, total_word_count, total_dictation_seconds],
        )
        .map_err(|e| HistoryError::Database(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_db_path() -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mutter-history-test-{}-{}.sqlite",
            std::process::id(),
            n
        ))
    }

    fn sample_entry(text: &str, duration_secs: f64) -> HistoryEntry {
        sample_entry_in(text, duration_secs, "en")
    }

    fn sample_entry_in(text: &str, duration_secs: f64, language: &str) -> HistoryEntry {
        HistoryEntry {
            timestamp: 1_700_000_000,
            duration_secs,
            text: text.to_string(),
            language: language.to_string(),
            engine: "whisper-small".to_string(),
        }
    }

    #[test]
    fn language_breakdown_counts_and_orders_by_usage() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();

        store.insert(&sample_entry_in("hi", 1.0, "en")).unwrap();
        store.insert(&sample_entry_in("hi", 1.0, "en")).unwrap();
        store
            .insert(&sample_entry_in("bonjour", 1.0, "fr"))
            .unwrap();

        let breakdown = store.language_breakdown().unwrap();
        assert_eq!(
            breakdown,
            vec![("en".to_string(), 2), ("fr".to_string(), 1)]
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn open_creates_a_fresh_migrated_db() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();
        let metrics = store.metrics(DEFAULT_TYPING_WPM).unwrap();
        assert_eq!(metrics.total_transcriptions, 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn insert_updates_running_aggregates() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();

        // 10 words in 30 seconds (0.5 min) => 20 WPM for this entry.
        store
            .insert(&sample_entry(
                "one two three four five six seven eight nine ten",
                30.0,
            ))
            .unwrap();

        let metrics = store.metrics(DEFAULT_TYPING_WPM).unwrap();
        assert_eq!(metrics.total_transcriptions, 1);
        assert!((metrics.average_wpm - 20.0).abs() < 1e-9);
        // time saved = (10 words / 40 wpm) - 0.5 min = 0.25 - 0.5 = -0.25 min
        assert!((metrics.time_saved_minutes - (-0.25)).abs() < 1e-9);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn list_page_is_most_recent_first_and_paginated() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();

        for i in 0..5 {
            let mut entry = sample_entry("word", 1.0);
            entry.timestamp = i;
            store.insert(&entry).unwrap();
        }

        let page0 = store.list_page(0, 2).unwrap();
        assert_eq!(page0.len(), 2);
        assert_eq!(page0[0].timestamp, 4);
        assert_eq!(page0[1].timestamp, 3);

        let page1 = store.list_page(1, 2).unwrap();
        assert_eq!(page1[0].timestamp, 2);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn recompute_aggregates_matches_incremental_updates() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();

        store.insert(&sample_entry("one two three", 10.0)).unwrap();
        store.insert(&sample_entry("four five", 5.0)).unwrap();

        let before = store.metrics(DEFAULT_TYPING_WPM).unwrap();
        store.recompute_aggregates().unwrap();
        let after = store.metrics(DEFAULT_TYPING_WPM).unwrap();

        assert_eq!(before.total_transcriptions, after.total_transcriptions);
        assert!((before.average_wpm - after.average_wpm).abs() < 1e-9);

        std::fs::remove_file(&path).ok();
    }

    /// Section 11's failure contract: a migration failure must back up the
    /// existing DB file (to a real, named path) and return
    /// `HistoryError::MigrationFailed` rather than silently launching
    /// against a half-migrated schema. Simulated here with a file that
    /// isn't a valid SQLite database at all — `to_latest` fails the same
    /// way a genuinely corrupted or half-migrated DB would.
    #[test]
    fn migration_failure_backs_up_existing_db_and_names_the_backup_path() {
        let path = temp_db_path();
        std::fs::write(&path, b"not a sqlite database").unwrap();

        let backup_path = match HistoryStore::open_at(&path) {
            Err(HistoryError::MigrationFailed(backup)) => backup,
            Err(other) => panic!("expected MigrationFailed, got {other:?}"),
            Ok(_) => panic!("expected MigrationFailed, opened successfully instead"),
        };
        assert!(
            std::path::Path::new(&backup_path).exists(),
            "backup file named in the error should actually exist on disk"
        );

        std::fs::remove_file(&path).ok();
        std::fs::remove_file(&backup_path).ok();
    }

    #[test]
    fn reopening_an_up_to_date_db_does_not_lose_data() {
        let path = temp_db_path();
        {
            let store = HistoryStore::open_at(&path).unwrap();
            store.insert(&sample_entry("hello world", 3.0)).unwrap();
        }
        {
            let store = HistoryStore::open_at(&path).unwrap();
            let metrics = store.metrics(DEFAULT_TYPING_WPM).unwrap();
            assert_eq!(metrics.total_transcriptions, 1);
        }
        std::fs::remove_file(&path).ok();
    }

    /// Regression test for the unbounded-backup-accumulation bug: reopening
    /// a DB that's already at the latest schema version must NOT create a
    /// new backup file every time — only an actual migration should.
    #[test]
    fn reopening_an_up_to_date_db_does_not_create_a_backup_file() {
        let path = temp_db_path();
        HistoryStore::open_at(&path).unwrap();
        HistoryStore::open_at(&path).unwrap();

        let backup_exists = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(path.file_name().unwrap().to_str().unwrap())
                    && e.path() != path
            });
        assert!(
            !backup_exists,
            "reopening an up-to-date DB should not create a backup file"
        );

        std::fs::remove_file(&path).ok();
    }
}
