//! Keyring operations for secure token storage.
//!
//! Provides functions to store, retrieve, and delete the Todoist API token
//! using the OS-native credential manager:
//! - macOS: Keychain
//! - Windows: Credential Manager
//! - Linux: Secret Service API (requires libsecret)

use keyring::Entry;

use super::{CommandError, Result};

/// Service name for keyring entries.
const SERVICE: &str = "td-todoist-cli";

/// Username for the token entry.
const USERNAME: &str = "api_token";

/// Stores token in OS keyring.
///
/// # Errors
///
/// Returns an error if the keyring is not available or the operation fails.
pub fn store_token(token: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)
        .map_err(|e| CommandError::Config(format!("Keyring error: {}", e)))?;
    entry
        .set_password(token)
        .map_err(|e| CommandError::Config(format!("Failed to store token in keyring: {}", e)))?;
    Ok(())
}

/// Retrieves token from OS keyring.
///
/// Returns `Ok(None)` if no token is stored, `Ok(Some(token))` if found.
///
/// # Errors
///
/// Returns an error if the keyring is not available or an unexpected error occurs.
pub fn get_token() -> Result<Option<String>> {
    let entry = Entry::new(SERVICE, USERNAME)
        .map_err(|e| CommandError::Config(format!("Keyring error: {}", e)))?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(keyring::Error::Ambiguous(_)) => Ok(None), // Multiple entries, treat as not found
        Err(e) => Err(CommandError::Config(format!(
            "Failed to read from keyring: {}",
            e
        ))),
    }
}

/// Deletes token from OS keyring.
///
/// Returns `Ok(())` even if no token was stored.
///
/// # Errors
///
/// Returns an error if the keyring is not available or an unexpected error occurs.
#[allow(dead_code)] // Available for future `td config migrate-token` command
pub fn delete_token() -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)
        .map_err(|e| CommandError::Config(format!("Keyring error: {}", e)))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
        Err(e) => Err(CommandError::Config(format!(
            "Failed to delete from keyring: {}",
            e
        ))),
    }
}

/// Checks if keyring is available on this system.
///
/// This does a lightweight check by attempting to create an entry.
/// Note: On some systems, the keyring might require user interaction
/// (e.g., unlocking) which only happens on actual read/write operations.
pub fn is_available() -> bool {
    Entry::new(SERVICE, "test").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available() {
        // This should succeed on macOS, Windows, and Linux with Secret Service
        // It may fail in CI environments without a keyring daemon
        let available = is_available();
        // Just verify it doesn't panic
        println!("Keyring available: {}", available);
    }

    // Note: We can't reliably test store/get/delete in unit tests because:
    // 1. CI environments may not have a keyring daemon
    // 2. Tests would leave state in the user's keyring
    // 3. Some systems require user interaction to unlock the keyring
    //
    // These functions should be tested manually on each target platform.
}
