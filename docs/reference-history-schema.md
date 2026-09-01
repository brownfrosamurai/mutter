# History database reference

`HistoryStore` (`src-tauri/src/history/mod.rs`) is a local-only SQLite database at `~/Library/Application Support/Mutter/history.sqlite`, opened via `rusqlite`. Every transcription is written here regardless of what happens downstream — this is what makes "copy it from history at any time" a real recovery path if injection fails, and it's what feeds every dashboard metric.

## Schema

### `history` — one row per transcript

| Column | Type | Since | Notes |
|---|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | migration 1 | |
| `timestamp` | `INTEGER NOT NULL` | migration 1 | Unix seconds |
| `duration_secs` | `REAL NOT NULL` | migration 1 | Audio duration |
| `text` | `TEXT NOT NULL` | migration 1 | The final (post-grammar-cleanup) transcript |
| `language` | `TEXT NOT NULL` | migration 1 | Whisper's auto-detected language code |
| `engine` | `TEXT NOT NULL` | migration 1 | e.g. `"whisper-small"` |
| `word_count` | `INTEGER NOT NULL DEFAULT 0` | migration 2 | Stored, not recomputed per query — see below |
| `recording_latency_ms` | `REAL` (nullable) | migration 3 | Hotkey-press → first-audio-frame. `NULL` for auto-continue segments and system-audio (not instrumented) |
| `inference_latency_ms` | `REAL` (nullable) | migration 3 | Wall-clock time inside `engine.transcribe()` |

Indexed on `timestamp DESC` (`idx_history_timestamp`) — every read query is either bounded by a `WHERE timestamp >= ?` window or explicitly paginated; nothing scans the whole table except the manual `recompute_aggregates()` action.

### `aggregates` — a singleton running-total row (`id = 0`)

| Column | Type |
|---|---|
| `total_count` | `INTEGER NOT NULL DEFAULT 0` |
| `total_word_count` | `INTEGER NOT NULL DEFAULT 0` |
| `total_dictation_seconds` | `REAL NOT NULL DEFAULT 0` |

Updated incrementally on every `insert()`, not recomputed by scanning `history` on every dashboard open. `recompute_aggregates()` is the manual full-scan drift-correction action, kept as an escape hatch, not called automatically anywhere.

## Migrations

`src-tauri/src/history/migrations/mod.rs`, via `rusqlite_migration`. `LATEST_VERSION` (currently `3`) must be bumped by hand alongside adding a migration — nothing validates that it matches the actual migration count.

**Backup-then-migrate contract** (`HistoryStore::open_at`): before running a migration that will actually change the schema, the existing DB file is copied to a timestamped `.sqlite.backup-<unix-seconds>` file. If the migration then fails, `open()` returns `HistoryError::MigrationFailed(backup_path)` and the app shows the [recovery window](reference-architecture.md#the-four-windows-at-a-glance) naming that exact path, rather than launching against a half-migrated schema.

The backup — and the migration call itself — is **skipped** whenever the DB is already at `LATEST_VERSION`. Every normal app launch checks `current_version(&conn) < LATEST_VERSION` first; only an actual schema upgrade pays the backup cost. (An earlier version of this code backed up unconditionally on every launch, accumulating one backup file per app start forever — fixed, with a regression test: `reopening_an_up_to_date_db_does_not_create_a_backup_file`.)

## Computed metrics (no separate tables — derived at query time from the two above)

| Type | Query surface | Source |
|---|---|---|
| `Metrics` (sessions/words/WPM/time-saved) | `metrics(assumed_typing_wpm)` | Reads the `aggregates` row directly — no scan |
| `LanguageStat[]` | `language_breakdown()` | `GROUP BY language` over `history` — cheap even at large row counts since language cardinality is tiny (six languages ever in scope) |
| `DailyActivity[]` | `daily_activity(days, now_unix)` | `GROUP BY date(timestamp, 'unixepoch', 'localtime')`, bounded to a trailing window. **Buckets in the OS's local timezone**, not UTC — a session at 11pm and one at 1am the next morning must land in different days the way the user actually experienced them |
| `LatencyStats` (p50/p95/samples × recording/inference) | `latency_stats(days, now_unix)` | SQLite has no percentile function — values are pulled sorted (`ORDER BY ... ASC`) and reduced in Rust via nearest-rank `percentile_of`. Rows with a `NULL` latency column are excluded, never counted as `0` |

`time_saved_minutes` is `(total_words / assumed_typing_wpm) - dictation_minutes`, using a configurable assumed typing speed (`DEFAULT_TYPING_WPM = 40.0`) — presented to the user as an assumption, never precise fact.

## Why `word_count` is a stored column, not computed on read

Migration 2 added `word_count` and backfilled it via a space-count approximation (`LENGTH(text) - LENGTH(REPLACE(text, ' ', '')) + 1`) — not `split_whitespace()`'s exact tokenization, since SQLite has no built-in whitespace tokenizer. This is exact for every row the app itself ever wrote (the grammar-cleanup pipeline always collapses transcripts to single-space-separated text before storage), and only approximate for hypothetical rows written before migration 2 existed. Storing the count is what makes per-language and per-day WPM a cheap `SUM(word_count) GROUP BY` instead of per-row tokenization on every dashboard open.

## Related

- [`reference-commands.md`](reference-commands.md) — the `get_metrics`/`get_language_breakdown`/`get_daily_activity`/`get_latency_stats`/`get_history_page` commands that expose this
- [`reference-architecture.md`](reference-architecture.md)
