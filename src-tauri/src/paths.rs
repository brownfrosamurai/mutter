//! Shared filesystem locations. `~/Library/Application Support/Mutter/` is
//! the single root for everything this app persists — downloaded models,
//! the history database, and (per logging.rs's TODO) log files — kept in
//! one small module rather than each caller re-deriving `$HOME`. Same DRY
//! reasoning as `permissions.rs`'s `PermissionGate<T>`: near-identical logic
//! duplicated across call sites is a violation waiting to happen.

use std::io;
use std::path::PathBuf;

/// `~/Library/Application Support/Mutter`. Created if missing.
pub fn app_support_dir() -> io::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "$HOME not set"))?;
    let dir = PathBuf::from(home).join("Library/Application Support/Mutter");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// A named subdirectory under the app support root (e.g. `"models"`).
/// Created if missing.
pub fn app_support_subdir(name: &str) -> io::Result<PathBuf> {
    let dir = app_support_dir()?.join(name);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
