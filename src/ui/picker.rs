use std::path::PathBuf;
use anyhow::Result;

/// Show interactive session picker. Returns selected path, or None for new session.
pub fn pick_session(_cwd: &str) -> Result<Option<PathBuf>> {
    // Stub — will be implemented in Task 4
    Ok(None)
}
