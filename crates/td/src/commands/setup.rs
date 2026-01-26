//! First-run interactive setup.
//!
//! Handles initial configuration when no token is found:
//! 1. Detects first run (no config file, no token)
//! 2. Prompts user to enter API token
//! 3. Asks where to store token (keyring, config file, or env var)
//! 4. Writes config file with chosen settings
//! 5. Performs initial sync after setup

use std::io::{self, IsTerminal};

use dialoguer::{Input, Select};
use owo_colors::OwoColorize;
use todoist_cache::{CacheStore, SyncManager};

use super::config::{get_config_path, load_config, Config};
use super::keyring;
use super::{CommandContext, CommandError, Result};

/// Token storage options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenStorage {
    /// Store in OS keyring (most secure).
    Keyring,
    /// Store in config file.
    Config,
    /// Expect from environment variable (don't store).
    Env,
}

impl TokenStorage {
    fn as_str(&self) -> &'static str {
        match self {
            TokenStorage::Keyring => "keyring",
            TokenStorage::Config => "config",
            TokenStorage::Env => "env",
        }
    }
}

/// Checks if this is a first run (no token configured anywhere).
#[allow(dead_code)]
pub fn is_first_run(cli_token: Option<&String>) -> bool {
    // If token provided via flag/env, not a first run
    if cli_token.is_some() {
        return false;
    }

    // Check config file for token
    match load_config() {
        Ok(config) => config.token.is_none(),
        Err(_) => true, // Config doesn't exist or is invalid
    }
}

/// Runs the first-time setup wizard.
///
/// Returns the token on success.
pub async fn run_setup(ctx: &CommandContext) -> Result<String> {
    // Check if we're in a terminal
    if !io::stdin().is_terminal() {
        return Err(CommandError::Config(
            "No API token configured. Set TODOIST_TOKEN environment variable or run interactively.".to_string()
        ));
    }

    // Welcome message
    if !ctx.quiet {
        println!();
        if ctx.use_colors {
            println!("{}", "Welcome to td - Todoist CLI!".green().bold());
        } else {
            println!("Welcome to td - Todoist CLI!");
        }
        println!();
        println!("No API token found. Let's set one up.");
        println!();
        println!("You can get your API token from:");
        if ctx.use_colors {
            println!("  {}", "https://todoist.com/app/settings/integrations/developer".cyan());
        } else {
            println!("  https://todoist.com/app/settings/integrations/developer");
        }
        println!();
    }

    // Prompt for token
    let token: String = Input::new()
        .with_prompt("Enter your Todoist API token")
        .validate_with(|input: &String| -> std::result::Result<(), &str> {
            if input.trim().is_empty() {
                Err("Token cannot be empty")
            } else if input.len() < 20 {
                Err("Token seems too short - check your token")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| CommandError::Io(io::Error::other(e.to_string())))?;

    let token = token.trim().to_string();

    // Validate token by attempting a sync
    if !ctx.quiet {
        println!();
        println!("Validating token...");
    }

    let client = todoist_api::client::TodoistClient::new(&token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Try full sync to validate
    match manager.full_sync().await {
        Ok(cache) => {
            if !ctx.quiet {
                if ctx.use_colors {
                    println!("{}", "Token validated successfully!".green());
                } else {
                    println!("Token validated successfully!");
                }
                println!();

                // Show summary
                let tasks = cache.items.iter().filter(|i| !i.is_deleted && !i.checked).count();
                let projects = cache.projects.iter().filter(|p| !p.is_deleted).count();
                println!("Synced {} tasks in {} projects.", tasks, projects);
                println!();
            }
        }
        Err(e) => {
            return Err(CommandError::Config(format!(
                "Token validation failed: {}. Please check your token and try again.",
                e
            )));
        }
    }

    // Ask where to store token
    let keyring_available = keyring::is_available();
    let storage = if keyring_available {
        let storage_options = &[
            "OS Keychain (recommended - most secure)",
            "Config file",
            "Environment variable only",
        ];
        let storage_selection = Select::new()
            .with_prompt("Where should we store your token?")
            .items(storage_options)
            .default(0)
            .interact()
            .map_err(|e| CommandError::Io(io::Error::other(e.to_string())))?;

        match storage_selection {
            0 => TokenStorage::Keyring,
            1 => TokenStorage::Config,
            _ => TokenStorage::Env,
        }
    } else {
        let storage_options = &["Config file (recommended)", "Environment variable only"];
        let storage_selection = Select::new()
            .with_prompt("Where should we store your token?")
            .items(storage_options)
            .default(0)
            .interact()
            .map_err(|e| CommandError::Io(io::Error::other(e.to_string())))?;

        if storage_selection == 0 {
            TokenStorage::Config
        } else {
            TokenStorage::Env
        }
    };

    // Save config
    save_setup_config(&token, storage)?;

    // Final message
    if !ctx.quiet {
        println!();
        let config_path = get_config_path()?;
        match storage {
            TokenStorage::Keyring => {
                if ctx.use_colors {
                    println!("{}", "Setup complete!".green().bold());
                } else {
                    println!("Setup complete!");
                }
                println!("Token stored securely in OS keychain.");
                println!("Config saved to: {}", config_path.display());
            }
            TokenStorage::Config => {
                if ctx.use_colors {
                    println!("{}", "Setup complete!".green().bold());
                } else {
                    println!("Setup complete!");
                }
                println!("Token saved to: {}", config_path.display());
            }
            TokenStorage::Env => {
                if ctx.use_colors {
                    println!("{}", "Setup complete!".green().bold());
                } else {
                    println!("Setup complete!");
                }
                println!("Config saved to: {}", config_path.display());
                println!();
                println!("Remember to set TODOIST_TOKEN in your shell:");
                if ctx.use_colors {
                    println!("  {}", format!("export TODOIST_TOKEN=\"{}\"", token).cyan());
                } else {
                    println!("  export TODOIST_TOKEN=\"{}\"", token);
                }
            }
        }
        println!();
        println!("Run 'td list' to see your tasks, or 'td --help' for more commands.");
    }

    Ok(token)
}

/// Saves the configuration after setup.
fn save_setup_config(token: &str, storage: TokenStorage) -> Result<()> {
    use std::fs;

    let path = get_config_path()?;

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CommandError::Config(format!("Failed to create config directory: {}", e)))?;
    }

    // If using keyring, store the token there
    if storage == TokenStorage::Keyring {
        keyring::store_token(token)?;
    }

    // Build config (don't store token in config if using keyring or env)
    let config = Config {
        token: match storage {
            TokenStorage::Config => Some(token.to_string()),
            TokenStorage::Keyring | TokenStorage::Env => None,
        },
        token_storage: Some(storage.as_str().to_string()),
        ..Default::default()
    };

    // Serialize and save
    let content = toml::to_string_pretty(&config)
        .map_err(|e| CommandError::Config(format!("Failed to serialize config: {}", e)))?;

    fs::write(&path, content)
        .map_err(|e| CommandError::Config(format!("Failed to write config: {}", e)))?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, permissions)
            .map_err(|e| CommandError::Config(format!("Failed to set config permissions: {}", e)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_storage_as_str() {
        assert_eq!(TokenStorage::Keyring.as_str(), "keyring");
        assert_eq!(TokenStorage::Config.as_str(), "config");
        assert_eq!(TokenStorage::Env.as_str(), "env");
    }

    #[test]
    fn test_is_first_run_with_token_flag() {
        // When token is provided via flag, not a first run
        let token = Some(String::from("test-token"));
        assert!(!is_first_run(token.as_ref()));
    }

    #[test]
    fn test_is_first_run_without_token() {
        use std::env;
        use tempfile::TempDir;

        // Set up temporary config location with no token
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let original = env::var("TD_CONFIG").ok();
        env::set_var("TD_CONFIG", config_path.to_str().unwrap());

        // No token provided, no config file - should be first run
        assert!(is_first_run(None));

        // Restore
        if let Some(val) = original {
            env::set_var("TD_CONFIG", val);
        } else {
            env::remove_var("TD_CONFIG");
        }
    }

    #[test]
    fn test_is_first_run_with_config_token() {
        use std::env;
        use std::fs;
        use std::io::Write;
        use tempfile::TempDir;

        // Set up temporary config with token
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut file = fs::File::create(&config_path).unwrap();
        writeln!(file, r#"token = "existing-token""#).unwrap();

        let original = env::var("TD_CONFIG").ok();
        env::set_var("TD_CONFIG", config_path.to_str().unwrap());

        // Config has token - not a first run
        assert!(!is_first_run(None));

        // Restore
        if let Some(val) = original {
            env::set_var("TD_CONFIG", val);
        } else {
            env::remove_var("TD_CONFIG");
        }
    }

    #[test]
    fn test_save_setup_config_with_token() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let original = env::var("TD_CONFIG").ok();
        env::set_var("TD_CONFIG", config_path.to_str().unwrap());

        let result = save_setup_config("test-token-12345", TokenStorage::Config);
        assert!(result.is_ok());

        // Verify file exists and contains token
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("token = \"test-token-12345\""));
        assert!(content.contains("token_storage = \"config\""));

        // Restore
        if let Some(val) = original {
            env::set_var("TD_CONFIG", val);
        } else {
            env::remove_var("TD_CONFIG");
        }
    }

    #[test]
    fn test_save_setup_config_env_only() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let original = env::var("TD_CONFIG").ok();
        env::set_var("TD_CONFIG", config_path.to_str().unwrap());

        let result = save_setup_config("test-token-12345", TokenStorage::Env);
        assert!(result.is_ok());

        // Verify file exists but does NOT contain token
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(!content.contains("test-token-12345"));
        assert!(content.contains("token_storage = \"env\""));

        // Restore
        if let Some(val) = original {
            env::set_var("TD_CONFIG", val);
        } else {
            env::remove_var("TD_CONFIG");
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_save_setup_config_sets_permissions() {
        use std::env;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let original = env::var("TD_CONFIG").ok();
        env::set_var("TD_CONFIG", config_path.to_str().unwrap());

        save_setup_config("test-token", TokenStorage::Config).unwrap();

        // Verify permissions are 0600
        let metadata = fs::metadata(&config_path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        // Restore
        if let Some(val) = original {
            env::set_var("TD_CONFIG", val);
        } else {
            env::remove_var("TD_CONFIG");
        }
    }
}
