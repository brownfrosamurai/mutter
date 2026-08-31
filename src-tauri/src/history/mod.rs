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
    /// Hotkey-press -> first-audio-frame, in milliseconds. `None` for
    /// auto-continue segments and the system-audio capture path — see
    /// `session::SegmentJob::recording_latency_ms`.
    pub recording_latency_ms: Option<f64>,
    /// Wall-clock time spent in the engine's `transcribe()` call, in
    /// milliseconds. Captured for every segment that reaches this far.
    pub inference_latency_ms: Option<f64>,
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
    /// Raw total dictation time — distinct from `time_saved_minutes` (a
    /// *comparison* against assumed typing speed). The Stats page's "Time
    /// Spoken" tile (frontend-rewrite plan) wants this directly; it was
    /// already tracked in the `aggregates` row (`total_dictation_seconds`)
    /// but never surfaced through this struct before.
    pub total_dictation_minutes: f64,
}

pub const DEFAULT_TYPING_WPM: f64 = 40.0;

/// One row of [`HistoryStore::language_breakdown`].
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageStat {
    pub language: String,
    pub count: i64,
    pub average_wpm: f64,
}

/// One row of [`HistoryStore::daily_activity`] — a calendar day (the OS's
/// local timezone, `YYYY-MM-DD`) that had at least one session.
#[derive(Debug, Clone, PartialEq)]
pub struct DailyActivity {
    pub date: String,
    pub count: i64,
}

/// p50/p95/sample-count for one latency stage — both `None` when `samples`
/// is 0 (an empty trailing window), never a fabricated 0ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatencyPercentiles {
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub samples: i64,
}

/// [`HistoryStore::latency_stats`]'s result — the Stats page's Latency
/// table has exactly these two rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatencyStats {
    pub recording: LatencyPercentiles,
    pub inference: LatencyPercentiles,
}

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
            "INSERT INTO history (timestamp, duration_secs, text, language, engine, word_count,
                recording_latency_ms, inference_latency_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry.timestamp,
                entry.duration_secs,
                entry.text,
                entry.language,
                entry.engine,
                word_count,
                entry.recording_latency_ms,
                entry.inference_latency_ms,
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
                    // Not selected above — dashboard History rows don't
                    // display per-entry latency (only the Stats page's
                    // aggregate Latency table does, via latency_stats()).
                    recording_latency_ms: None,
                    inference_latency_ms: None,
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
            total_dictation_minutes: dictation_minutes,
        })
    }

    /// Per-language transcription counts and average WPM, most-used first.
    /// Not part of Section 8's metric table (which only specifies
    /// time-saved/total/WPM/activity feed) — added because the dashboard's
    /// Languages section (built to match a user-supplied reference mockup)
    /// needs real data rather than a permanent "no data yet" placeholder.
    /// A plain `GROUP BY` over `history`, not a running aggregate —
    /// language cardinality is tiny (six languages, per the plan's scope),
    /// so a full scan here is cheap even at large row counts. The average-
    /// WPM addition (2026-08-30 Stats redesign) is still cheap despite
    /// needing word counts, because `word_count` is now a stored column
    /// (migration 2) — a `SUM`, not per-row tokenization.
    pub fn language_breakdown(&self) -> Result<Vec<LanguageStat>, HistoryError> {
        let conn = self.conn.lock().expect("history db lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT language, COUNT(*) as cnt, SUM(word_count), SUM(duration_secs)
                 FROM history GROUP BY language ORDER BY cnt DESC",
            )
            .map_err(|e| HistoryError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let language: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                let words: i64 = row.get(2)?;
                let seconds: f64 = row.get(3)?;
                Ok((language, count, words, seconds))
            })
            .map_err(|e| HistoryError::Database(e.to_string()))?;

        rows.map(|r| {
            r.map(|(language, count, words, seconds)| {
                let minutes = seconds / 60.0;
                LanguageStat {
                    language,
                    count,
                    average_wpm: if minutes > 0.0 {
                        words as f64 / minutes
                    } else {
                        0.0
                    },
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| HistoryError::Database(e.to_string()))
    }

    /// Session counts per calendar day, in the OS's local timezone (SQL's
    /// `'localtime'` modifier — a session at 11pm and one at 1am the next
    /// morning must land in different buckets the way the user actually
    /// experienced them, not by UTC's clock, which is wrong for almost
    /// every timezone; fixed 2026-08-30), over the trailing `days`-day
    /// window, oldest first — powers the Stats page's activity chart
    /// (2026-08-30 redesign). `now_unix` is a parameter rather than read
    /// internally so tests can pin "now" deterministically. Bounded by the
    /// `WHERE` clause to just the requested window (not the whole table),
    /// so this stays cheap at any history size — the "activity" feature
    /// only asked for a real backend aggregate specifically to avoid
    /// pulling unboundedly many rows to the client for the same chart
    /// (Section 8's scaling note, same reasoning as `list_page`'s
    /// pagination). Days with zero sessions are simply absent from the
    /// result — the caller fills gaps against its own day axis, same
    /// division of labor as `language_breakdown`'s formatting.
    pub fn daily_activity(
        &self,
        days: u32,
        now_unix: i64,
    ) -> Result<Vec<DailyActivity>, HistoryError> {
        let cutoff = now_unix - i64::from(days) * 86_400;
        let conn = self.conn.lock().expect("history db lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT date(timestamp, 'unixepoch', 'localtime') as day, COUNT(*) as cnt
                 FROM history WHERE timestamp >= ?1 GROUP BY day ORDER BY day",
            )
            .map_err(|e| HistoryError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(params![cutoff], |row| {
                Ok(DailyActivity {
                    date: row.get(0)?,
                    count: row.get(1)?,
                })
            })
            .map_err(|e| HistoryError::Database(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| HistoryError::Database(e.to_string()))
    }

    /// Backs the Stats page's Latency table (frontend-rewrite plan,
    /// 2026-08-31) — p50/p95/sample-count per stage over the trailing
    /// `days`-day window, same bounded-window discipline as
    /// `daily_activity` (never a full-table scan). SQLite has no native
    /// percentile function, so each stage's values are pulled sorted and
    /// reduced in Rust via `percentile_of` — cheap at this app's realistic
    /// row counts (a percentile query, not a per-dashboard-open aggregate
    /// scan of the whole table). `now_unix` is a parameter, not read
    /// internally, for the same reason `daily_activity` takes it: tests can
    /// pin "now" deterministically.
    pub fn latency_stats(&self, days: u32, now_unix: i64) -> Result<LatencyStats, HistoryError> {
        let cutoff = now_unix - i64::from(days) * 86_400;
        let conn = self.conn.lock().expect("history db lock poisoned");

        let recording = Self::column_percentiles(
            &conn,
            "SELECT recording_latency_ms FROM history
             WHERE timestamp >= ?1 AND recording_latency_ms IS NOT NULL
             ORDER BY recording_latency_ms ASC",
            cutoff,
        )?;
        let inference = Self::column_percentiles(
            &conn,
            "SELECT inference_latency_ms FROM history
             WHERE timestamp >= ?1 AND inference_latency_ms IS NOT NULL
             ORDER BY inference_latency_ms ASC",
            cutoff,
        )?;

        Ok(LatencyStats {
            recording,
            inference,
        })
    }

    fn column_percentiles(
        conn: &Connection,
        sorted_ascending_sql: &str,
        cutoff: i64,
    ) -> Result<LatencyPercentiles, HistoryError> {
        let mut stmt = conn
            .prepare(sorted_ascending_sql)
            .map_err(|e| HistoryError::Database(e.to_string()))?;
        let values: Vec<f64> = stmt
            .query_map(params![cutoff], |row| row.get::<_, f64>(0))
            .map_err(|e| HistoryError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| HistoryError::Database(e.to_string()))?;
        Ok(percentiles(&values))
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

/// `sorted_ascending` must already be sorted — callers do this in SQL
/// (`ORDER BY ... ASC`) rather than re-sorting in Rust. Empty input is the
/// zero-sample case (`None`/`None`/`0`), never an index panic.
fn percentiles(sorted_ascending: &[f64]) -> LatencyPercentiles {
    if sorted_ascending.is_empty() {
        return LatencyPercentiles {
            p50_ms: None,
            p95_ms: None,
            samples: 0,
        };
    }
    LatencyPercentiles {
        p50_ms: Some(percentile_of(sorted_ascending, 0.50)),
        p95_ms: Some(percentile_of(sorted_ascending, 0.95)),
        samples: sorted_ascending.len() as i64,
    }
}

/// Nearest-rank percentile over an already-sorted-ascending, non-empty
/// slice. `p` in `[0.0, 1.0]`. A single sample returns that sample for
/// every percentile (p50 == p95), matching what "the only data point we
/// have" should mean.
fn percentile_of(sorted_ascending: &[f64], p: f64) -> f64 {
    let n = sorted_ascending.len();
    if n == 1 {
        return sorted_ascending[0];
    }
    let rank = (p * (n - 1) as f64).round() as usize;
    sorted_ascending[rank.min(n - 1)]
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
            recording_latency_ms: None,
            inference_latency_ms: None,
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
        let counts: Vec<(String, i64)> = breakdown
            .iter()
            .map(|s| (s.language.clone(), s.count))
            .collect();
        assert_eq!(counts, vec![("en".to_string(), 2), ("fr".to_string(), 1)]);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn language_breakdown_computes_average_wpm_per_language() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();

        // "one two three four" (4 words) in 2 seconds (1/30 min) => 120 WPM.
        store
            .insert(&sample_entry_in("one two three four", 2.0, "en"))
            .unwrap();
        // "un deux" (2 words) in 2 seconds (1/30 min) => 60 WPM.
        store
            .insert(&sample_entry_in("un deux", 2.0, "fr"))
            .unwrap();

        let breakdown = store.language_breakdown().unwrap();
        let en = breakdown.iter().find(|s| s.language == "en").unwrap();
        let fr = breakdown.iter().find(|s| s.language == "fr").unwrap();
        assert!((en.average_wpm - 120.0).abs() < 1e-9);
        assert!((fr.average_wpm - 60.0).abs() < 1e-9);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn daily_activity_groups_by_day_within_the_window_and_excludes_older_rows() {
        // `daily_activity` buckets by the OS's local timezone (`'localtime'`
        // in its SQL), not UTC — that's the whole point (2026-08-30: dates
        // were misaligned for anyone not in UTC). Pinning TZ=UTC for this
        // test's duration makes 'localtime' behave exactly like the plain
        // UTC math the assertions below assume, without weakening what's
        // actually exercised — the real localtime-aware code path still
        // runs, just under a known, deterministic timezone. Safe to mutate
        // process-global env here: this is the only test in the crate that
        // depends on TZ, so there's no other test to race with.
        let prev_tz = std::env::var("TZ").ok();
        std::env::set_var("TZ", "UTC");

        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();

        let now: i64 = 1_700_100_000; // an arbitrary fixed "now"
        let one_day = 86_400_i64;

        let mut today = sample_entry("today one", 1.0);
        today.timestamp = now;
        let mut today_again = sample_entry("today two", 1.0);
        today_again.timestamp = now - 60; // same day, earlier
        let mut yesterday = sample_entry("yesterday", 1.0);
        yesterday.timestamp = now - one_day;
        let mut too_old = sample_entry("too old", 1.0);
        too_old.timestamp = now - 30 * one_day;

        store.insert(&today).unwrap();
        store.insert(&today_again).unwrap();
        store.insert(&yesterday).unwrap();
        store.insert(&too_old).unwrap();

        let activity = store.daily_activity(14, now).unwrap();

        match prev_tz {
            Some(tz) => std::env::set_var("TZ", tz),
            None => std::env::remove_var("TZ"),
        }

        // Exactly two days in the window, oldest first, and the 30-day-old
        // row is excluded entirely — not just under-counted.
        assert_eq!(activity.len(), 2);
        assert_eq!(activity[0].count, 1); // yesterday
        assert_eq!(activity[1].count, 2); // today (two sessions)
        assert!(activity.iter().all(|d| d.count <= 2));

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

    fn entry_with_latency(
        timestamp: i64,
        recording_latency_ms: Option<f64>,
        inference_latency_ms: Option<f64>,
    ) -> HistoryEntry {
        let mut entry = sample_entry("word", 1.0);
        entry.timestamp = timestamp;
        entry.recording_latency_ms = recording_latency_ms;
        entry.inference_latency_ms = inference_latency_ms;
        entry
    }

    #[test]
    fn latency_stats_on_empty_window_returns_nulls_not_a_panic() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();

        let stats = store.latency_stats(14, 1_700_000_000).unwrap();

        assert_eq!(stats.recording.samples, 0);
        assert_eq!(stats.recording.p50_ms, None);
        assert_eq!(stats.recording.p95_ms, None);
        assert_eq!(stats.inference.samples, 0);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn latency_stats_with_one_sample_p50_equals_p95() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();
        let now = 1_700_000_000;

        store
            .insert(&entry_with_latency(now, Some(120.0), Some(450.0)))
            .unwrap();

        let stats = store.latency_stats(14, now).unwrap();
        assert_eq!(stats.recording.samples, 1);
        assert_eq!(stats.recording.p50_ms, Some(120.0));
        assert_eq!(stats.recording.p95_ms, Some(120.0));
        assert_eq!(stats.inference.p50_ms, Some(450.0));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn latency_stats_computes_percentiles_and_excludes_nulls_and_old_rows() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();
        let now = 1_700_000_000;
        let one_day = 86_400;

        // Five recent samples with recording latency: 10, 20, 30, 40, 100.
        for (i, ms) in [10.0, 20.0, 30.0, 40.0, 100.0].into_iter().enumerate() {
            store
                .insert(&entry_with_latency(
                    now - i as i64,
                    Some(ms),
                    Some(ms * 3.0),
                ))
                .unwrap();
        }
        // An auto-continue segment (no recording latency, only inference) —
        // must not be counted as a recording-latency sample, but must still
        // count for inference.
        store
            .insert(&entry_with_latency(now, None, Some(500.0)))
            .unwrap();
        // Outside the 14-day window entirely — must not affect either stat.
        store
            .insert(&entry_with_latency(
                now - 30 * one_day,
                Some(9999.0),
                Some(9999.0),
            ))
            .unwrap();

        let stats = store.latency_stats(14, now).unwrap();

        assert_eq!(stats.recording.samples, 5);
        // Sorted: [10, 20, 30, 40, 100] — nearest-rank p50 (index round(0.5*4)=2) -> 30.
        assert_eq!(stats.recording.p50_ms, Some(30.0));
        // p95 (index round(0.95*4)=4) -> 100.
        assert_eq!(stats.recording.p95_ms, Some(100.0));

        assert_eq!(stats.inference.samples, 6);
        assert!(stats.inference.p50_ms.unwrap() < 9999.0);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn insert_persists_latency_columns() {
        let path = temp_db_path();
        let store = HistoryStore::open_at(&path).unwrap();

        store
            .insert(&entry_with_latency(1_700_000_000, Some(75.0), Some(300.0)))
            .unwrap();

        let page = store.list_page(0, 1).unwrap();
        // list_page doesn't select latency columns (dashboard History rows
        // don't need them) — this test exists to confirm insert() itself
        // doesn't error on the new columns; latency_stats' own tests above
        // confirm the values round-trip correctly.
        assert_eq!(page.len(), 1);

        std::fs::remove_file(&path).ok();
    }
}
