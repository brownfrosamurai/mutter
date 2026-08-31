//! Versioned schema migrations via `rusqlite_migration`, with a
//! backup-then-migrate failure path. See ../mod.rs and
//! docs/mutter-project-plan.md Section 11 for the full contract.

use rusqlite_migration::{Migrations, M};

/// Number of `M::up(...)` entries in [`migrations`] — used by
/// `history::open_at` to decide whether a migration will actually run
/// before paying the cost of a pre-migration backup copy. Bump this
/// alongside adding a new migration; `migration_set_validates` below
/// doesn't catch a mismatch, so it's a manual invariant.
pub const LATEST_VERSION: usize = 3;

/// `aggregates` is a singleton row (Section 8's running-aggregate design —
/// time-saved/total-count/WPM are updated on each `history` insert, not
/// recomputed by scanning the whole table on every dashboard open).
pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                duration_secs REAL NOT NULL,
                text TEXT NOT NULL,
                language TEXT NOT NULL,
                engine TEXT NOT NULL
            );
            CREATE INDEX idx_history_timestamp ON history(timestamp DESC);

            CREATE TABLE aggregates (
                id INTEGER PRIMARY KEY CHECK (id = 0),
                total_count INTEGER NOT NULL DEFAULT 0,
                total_word_count INTEGER NOT NULL DEFAULT 0,
                total_dictation_seconds REAL NOT NULL DEFAULT 0
            );
            INSERT INTO aggregates (id, total_count, total_word_count, total_dictation_seconds)
                VALUES (0, 0, 0, 0);
            ",
        )
        .down("DROP TABLE history; DROP TABLE aggregates;"),
        // Stores each row's own word count instead of recomputing it by
        // tokenizing `text` at query time. `insert()` already computed this
        // once (for the running aggregate) and now just writes it down too.
        // What this unlocks: per-language and per-day WPM (the dashboard's
        // 2026-08-30 Stats redesign) as a cheap `SUM(word_count)` GROUP BY,
        // instead of `recompute_aggregates()`'s per-row tokenization — the
        // exact cost `language_breakdown`'s own doc comment already named
        // as the thing worth avoiding on a routine dashboard-open path.
        //
        // Backfill uses a space-count approximation
        // (`LENGTH(text) - LENGTH(REPLACE(text, ' ', '')) + 1`), not
        // `split_whitespace()`'s exact tokenization — SQLite has no
        // built-in whitespace tokenizer. Approximate only for rows that
        // predate this migration; every row inserted from here on gets the
        // real Rust-computed count. Safe in practice: `grammar::clean()`
        // already collapses transcripts to single-space-separated text
        // before it's ever stored, so single-space counting matches
        // `split_whitespace()` exactly for every row this app itself wrote.
        M::up(
            "ALTER TABLE history ADD COLUMN word_count INTEGER NOT NULL DEFAULT 0;
            UPDATE history SET word_count = CASE
                WHEN text = '' THEN 0
                ELSE LENGTH(text) - LENGTH(REPLACE(text, ' ', '')) + 1
            END;
            ",
        )
        .down("ALTER TABLE history DROP COLUMN word_count;"),
        // Backs the Stats page's Latency table (frontend-rewrite plan,
        // 2026-08-31). Both nullable, no backfill: existing rows genuinely
        // have no latency data (it was never captured before this
        // migration), and NULL correctly means "not measured" rather than
        // a fabricated 0 — `latency_stats()` excludes NULLs from its
        // percentile calculation the same way it excludes rows outside its
        // trailing window.
        M::up(
            "ALTER TABLE history ADD COLUMN recording_latency_ms REAL;
            ALTER TABLE history ADD COLUMN inference_latency_ms REAL;
            ",
        )
        .down(
            "ALTER TABLE history DROP COLUMN recording_latency_ms;
            ALTER TABLE history DROP COLUMN inference_latency_ms;
            ",
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_set_validates() {
        migrations()
            .validate()
            .expect("migration set should be internally consistent");
    }

    #[test]
    fn fresh_connection_migrates_to_latest() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrations().to_latest(&mut conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT total_count FROM aggregates WHERE id = 0", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    /// Migration 2's backfill formula must match `split_whitespace().count()`
    /// for the single-space-separated text this app actually stores (see
    /// migration 2's doc comment) — checked here at the raw-SQL level,
    /// independent of `HistoryStore::insert()`.
    #[test]
    fn word_count_backfill_matches_split_whitespace_for_normalized_text() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrations().to_latest(&mut conn).unwrap();

        conn.execute(
            "INSERT INTO history (timestamp, duration_secs, text, language, engine, word_count)
             VALUES (0, 1.0, 'one two three four five.', 'en', 'whisper-small', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE history SET word_count = CASE
                WHEN text = '' THEN 0
                ELSE LENGTH(text) - LENGTH(REPLACE(text, ' ', '')) + 1
             END",
            [],
        )
        .unwrap();

        let word_count: i64 = conn
            .query_row("SELECT word_count FROM history WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            word_count,
            "one two three four five.".split_whitespace().count() as i64
        );
    }
}
