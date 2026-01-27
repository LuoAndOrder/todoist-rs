//! Keyring operations for secure token storage.
//!
//! Provides functions to store, retrieve, and delete the Todoist API token
//! using the OS-native credential manager:
//! - macOS: Keychain
//! - Windows: Credential Manager
//! - Linux: Secret Service API (requires libsecret)

use keyring::Entry;

use super::{CommandError, Result};

/// Provides platform-specific hints for keyring errors.
///
/// Returns a helpful message explaining how to fix keyring issues on the current platform.
fn platform_hint(error: &keyring::Error) -> String {
    let base_error = format!("{}", error);

    let hint = match error {
        keyring::Error::NoStorageAccess(_) => platform_access_hint(),
        keyring::Error::PlatformFailure(_) => platform_failure_hint(),
        _ => return base_error,
    };

    format!("{}\n\nHint: {}", base_error, hint)
}

/// Returns platform-specific hint for storage access errors.
#[cfg(target_os = "linux")]
fn platform_access_hint() -> &'static str {
    "On Linux, td uses the Secret Service API (libsecret) for secure storage.\n\
     To fix this:\n\
     1. Install a secret service provider:\n\
        - GNOME/GTK: gnome-keyring (usually pre-installed)\n\
        - KDE: kwallet or ksecretservice\n\
        - Headless: Install 'gnome-keyring' and run 'dbus-run-session -- gnome-keyring-daemon --unlock'\n\
     2. Ensure the keyring daemon is running:\n\
        - Check: 'systemctl --user status gnome-keyring-daemon'\n\
        - Start: 'systemctl --user start gnome-keyring-daemon'\n\
     3. For SSH sessions, ensure D-Bus is available\n\
     \n\
     Alternative: Set TODOIST_API_TOKEN environment variable or use 'td config set token <TOKEN>'"
}

#[cfg(target_os = "macos")]
fn platform_access_hint() -> &'static str {
    "On macOS, td uses the Keychain for secure storage.\n\
     To fix this:\n\
     1. Check if Keychain Access is locked (open Keychain Access app)\n\
     2. If prompted, allow 'td' to access the keychain\n\
     3. Try unlocking your login keychain: 'security unlock-keychain ~/Library/Keychains/login.keychain-db'\n\
     \n\
     Alternative: Set TODOIST_API_TOKEN environment variable or use 'td config set token <TOKEN>'"
}

#[cfg(target_os = "windows")]
fn platform_access_hint() -> &'static str {
    "On Windows, td uses Credential Manager for secure storage.\n\
     To fix this:\n\
     1. Open Credential Manager (search in Start menu)\n\
     2. Check if there are issues with Windows Credentials\n\
     3. Try running td as administrator if access is denied\n\
     \n\
     Alternative: Set TODOIST_API_TOKEN environment variable or use 'td config set token <TOKEN>'"
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_access_hint() -> &'static str {
    "Your platform's credential storage is not accessible.\n\
     Alternative: Set TODOIST_API_TOKEN environment variable or use 'td config set token <TOKEN>'"
}

/// Returns platform-specific hint for platform failures.
#[cfg(target_os = "linux")]
fn platform_failure_hint() -> &'static str {
    "The Secret Service API encountered an error.\n\
     Common causes:\n\
     1. D-Bus session bus not available (common in containers/SSH)\n\
        - Try: 'eval $(dbus-launch --sh-syntax)'\n\
     2. No secret service provider installed\n\
        - Install: 'sudo apt install gnome-keyring' (Debian/Ubuntu)\n\
        - Install: 'sudo dnf install gnome-keyring' (Fedora)\n\
     3. Keyring daemon not running\n\
        - Start: 'gnome-keyring-daemon --start --components=secrets'\n\
     \n\
     Alternative: Set TODOIST_API_TOKEN environment variable or use 'td config set token <TOKEN>'"
}

#[cfg(target_os = "macos")]
fn platform_failure_hint() -> &'static str {
    "The Keychain encountered an error.\n\
     Common causes:\n\
     1. Keychain is corrupted - try 'Keychain First Aid' in Keychain Access\n\
     2. Code signing issue - the app may need to be re-signed\n\
     3. System Integrity Protection preventing access\n\
     \n\
     Alternative: Set TODOIST_API_TOKEN environment variable or use 'td config set token <TOKEN>'"
}

#[cfg(target_os = "windows")]
fn platform_failure_hint() -> &'static str {
    "Windows Credential Manager encountered an error.\n\
     Common causes:\n\
     1. Credential Manager service not running\n\
        - Open Services (services.msc) and start 'Credential Manager'\n\
     2. User profile issues\n\
        - Try logging out and back in\n\
     3. Antivirus blocking credential access\n\
     \n\
     Alternative: Set TODOIST_API_TOKEN environment variable or use 'td config set token <TOKEN>'"
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_failure_hint() -> &'static str {
    "Your platform's credential storage encountered an error.\n\
     Alternative: Set TODOIST_API_TOKEN environment variable or use 'td config set token <TOKEN>'"
}

/// Service name for keyring entries.
const SERVICE: &str = "td-todoist-cli";

/// Username for the token entry.
const USERNAME: &str = "api_token";

/// Stores token in OS keyring.
///
/// # Errors
///
/// Returns an error if the keyring is not available or the operation fails.
/// Error messages include platform-specific hints for common issues.
pub fn store_token(token: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)
        .map_err(|e| CommandError::Config(format!("Keyring error: {}", platform_hint(&e))))?;
    entry
        .set_password(token)
        .map_err(|e| CommandError::Config(format!("Failed to store token: {}", platform_hint(&e))))?;
    Ok(())
}

/// Retrieves token from OS keyring.
///
/// Returns `Ok(None)` if no token is stored, `Ok(Some(token))` if found.
///
/// # Errors
///
/// Returns an error if the keyring is not available or an unexpected error occurs.
/// Error messages include platform-specific hints for common issues.
pub fn get_token() -> Result<Option<String>> {
    let entry = Entry::new(SERVICE, USERNAME)
        .map_err(|e| CommandError::Config(format!("Keyring error: {}", platform_hint(&e))))?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(keyring::Error::Ambiguous(_)) => Ok(None), // Multiple entries, treat as not found
        Err(e) => Err(CommandError::Config(format!(
            "Failed to read token: {}",
            platform_hint(&e)
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
/// Error messages include platform-specific hints for common issues.
#[allow(dead_code)] // Available for future `td config migrate-token` command
pub fn delete_token() -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)
        .map_err(|e| CommandError::Config(format!("Keyring error: {}", platform_hint(&e))))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
        Err(e) => Err(CommandError::Config(format!(
            "Failed to delete token: {}",
            platform_hint(&e)
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

    #[test]
    fn test_platform_hint_no_entry_returns_base_error() {
        // NoEntry errors don't need platform hints - they're expected
        let error = keyring::Error::NoEntry;
        let hint = platform_hint(&error);
        // Should just return the base error message without extra hints
        assert!(!hint.contains("Hint:"));
    }

    #[test]
    fn test_platform_hint_bad_encoding_returns_base_error() {
        // BadEncoding errors don't need platform hints
        let error = keyring::Error::BadEncoding(vec![0x80, 0x81]);
        let hint = platform_hint(&error);
        assert!(!hint.contains("Hint:"));
    }

    #[test]
    fn test_platform_access_hint_contains_alternative() {
        // All platform access hints should mention the alternative
        let hint = platform_access_hint();
        assert!(hint.contains("TODOIST_API_TOKEN"));
        assert!(hint.contains("td config set token"));
    }

    #[test]
    fn test_platform_failure_hint_contains_alternative() {
        // All platform failure hints should mention the alternative
        let hint = platform_failure_hint();
        assert!(hint.contains("TODOIST_API_TOKEN"));
        assert!(hint.contains("td config set token"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_access_hint_mentions_secret_service() {
        let hint = platform_access_hint();
        assert!(hint.contains("Secret Service"));
        assert!(hint.contains("gnome-keyring"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_access_hint_mentions_keychain() {
        let hint = platform_access_hint();
        assert!(hint.contains("Keychain"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_access_hint_mentions_credential_manager() {
        let hint = platform_access_hint();
        assert!(hint.contains("Credential Manager"));
    }

    // Note: We can't reliably test store/get/delete in unit tests because:
    // 1. CI environments may not have a keyring daemon
    // 2. Tests would leave state in the user's keyring
    // 3. Some systems require user interaction to unlock the keyring
    //
    // These functions should be tested manually on each target platform.
}
