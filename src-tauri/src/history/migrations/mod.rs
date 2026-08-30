//! Versioned schema migrations via `rusqlite_migration`, with a
//! backup-then-migrate failure path. See ../mod.rs and
//! docs/mutter-project-plan.md Section 11 for the full contract.
//!
//! STUB — no migrations defined yet since there is no schema. The first
//! migration (creating the `history` table) is Phase 2 work.

// pub fn migrations() -> rusqlite_migration::Migrations<'static> { ... }
