//! Versioned schema migrations via `rusqlite_migration`, with a
//! backup-then-migrate failure path. See ../mod.rs and
//! docs/mutter-project-plan.md Section 11 for the full contract.

use rusqlite_migration::{Migrations, M};

/// `aggregates` is a singleton row (Section 8's running-aggregate design —
/// time-saved/total-count/WPM are updated on each `history` insert, not
/// recomputed by scanning the whole table on every dashboard open).
pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(
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
    .down("DROP TABLE history; DROP TABLE aggregates;")])
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
}
