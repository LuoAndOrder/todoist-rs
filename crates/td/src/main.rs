use clap::Parser;
use std::process::ExitCode;

mod cli;
mod commands;
mod dispatch;
mod output;

use cli::Cli;
use commands::config::load_config;
use commands::{CommandContext, CommandError};
use dispatch::{AuthCommand, AuthDispatch, NoAuthCommand, NoAuthDispatch};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if cli.json {
                let error_json = serde_json::json!({
                    "error": {
                        "code": error_code(&e),
                        "message": e.to_string(),
                    }
                });
                eprintln!("{}", serde_json::to_string_pretty(&error_json).unwrap());
            } else {
                eprintln!("Error: {e}");
            }
            error_exit_code(&e)
        }
    }
}

async fn run(cli: &Cli) -> commands::Result<()> {
    let ctx = CommandContext::from_cli(cli);

    // Try no-auth commands first (config, completions, help)
    if let Some(dispatch) = NoAuthDispatch::try_from_cli(cli) {
        // Special case: config edit requires async context
        if matches!(
            &cli.command,
            Some(cli::Commands::Config {
                command: Some(cli::ConfigCommands::Edit)
            })
        ) {
            // Config edit needs token resolution for potential keyring access
            let token = resolve_token(cli).await?;
            if let Some(auth_dispatch) = AuthDispatch::from_cli(cli) {
                return auth_dispatch.execute(&ctx, &token).await;
            }
        }
        return dispatch.execute(&ctx);
    }

    // Get token for authenticated commands
    let token = resolve_token(cli).await?;

    // Dispatch authenticated commands
    if let Some(dispatch) = AuthDispatch::from_cli(cli) {
        return dispatch.execute(&ctx, &token).await;
    }

    // Fallback for any unhandled commands
    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "status": "not_implemented",
                "command": format!("{:?}", cli.command)
            })
        );
    } else if !cli.quiet {
        println!("Command not yet implemented: {:?}", cli.command);
    }
    Ok(())
}

/// Returns the error code string for JSON output.
fn error_code(e: &CommandError) -> &'static str {
    match e {
        CommandError::Sync(_) => "SYNC_ERROR",
        CommandError::CacheStore(_) => "CACHE_ERROR",
        CommandError::Filter(_) => "FILTER_ERROR",
        CommandError::Api(_) => "API_ERROR",
        CommandError::Config(_) => "CONFIG_ERROR",
        CommandError::Io(_) => "IO_ERROR",
        CommandError::Json(_) => "JSON_ERROR",
    }
}

/// Returns the exit code for an error.
fn error_exit_code(e: &CommandError) -> ExitCode {
    match e {
        CommandError::Config(_) => ExitCode::from(5),
        CommandError::Filter(_) => ExitCode::from(1),
        CommandError::Api(_) => ExitCode::from(2),
        CommandError::Sync(todoist_cache_rs::SyncError::Api(_)) => ExitCode::from(2),
        CommandError::Sync(todoist_cache_rs::SyncError::Cache(_)) => ExitCode::from(5),
        CommandError::Sync(todoist_cache_rs::SyncError::NotFound { .. }) => ExitCode::from(5),
        CommandError::Sync(todoist_cache_rs::SyncError::SyncTokenInvalid) => ExitCode::from(2),
        CommandError::CacheStore(_) => ExitCode::from(5),
        CommandError::Io(_) => ExitCode::from(3),
        CommandError::Json(_) => ExitCode::from(1),
    }
}

/// Resolves the API token with priority: flag > env > keyring > config.
///
/// The resolution order is:
/// 1. `--token` command line flag (highest priority)
/// 2. `TODOIST_TOKEN` environment variable
/// 3. OS keyring (if `token_storage == "keyring"` in config)
/// 4. Token from config file (`~/.config/td/config.toml`)
///
/// Returns `None` if no token is found (allowing caller to trigger setup).
fn resolve_token_optional(cli: &Cli) -> commands::Result<Option<String>> {
    // 1. Flag takes highest priority (clap already handles env via `env = "TODOIST_TOKEN"`)
    //    When cli.token is Some, it's either from --token flag OR from TODOIST_TOKEN env
    if let Some(token) = &cli.token {
        return Ok(Some(token.clone()));
    }

    // 2. Try config file and check storage method
    match load_config() {
        Ok(config) => {
            // 3. If token_storage == "keyring", try keyring
            if config.token_storage.as_deref() == Some("keyring") {
                if let Some(token) = commands::keyring::get_token()? {
                    return Ok(Some(token));
                }
            }

            // 4. Fall back to config file token
            if let Some(token) = config.token {
                return Ok(Some(token));
            }
        }
        Err(_) => {
            // Config loading failed, continue
        }
    }

    // No token found
    Ok(None)
}

/// Resolves the API token, running first-run setup if needed.
///
/// If no token is found and we're in an interactive terminal,
/// runs the setup wizard to configure the token.
async fn resolve_token(cli: &Cli) -> commands::Result<String> {
    // First, try the normal resolution chain
    if let Some(token) = resolve_token_optional(cli)? {
        return Ok(token);
    }

    // No token found - check if we should run setup
    let ctx = CommandContext::from_cli(cli);

    // Run interactive setup
    commands::setup::run_setup(&ctx).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use cli::Commands;
    use std::env;

    /// Helper to create a test CLI with specified token.
    fn cli_with_token(token: Option<String>) -> Cli {
        Cli {
            verbose: false,
            quiet: false,
            json: false,
            no_color: false,
            token,
            sync: false,
            command: Some(Commands::List {
                filter: None,
                project: None,
                label: None,
                priority: None,
                section: None,
                overdue: false,
                no_due: false,
                limit: 50,
                all: false,
                cursor: None,
                sort: None,
                reverse: false,
            }),
        }
    }

    #[test]
    fn test_resolve_token_optional_from_flag() {
        // Token from flag takes highest priority
        let cli = cli_with_token(Some("flag-token".to_string()));
        let result = resolve_token_optional(&cli);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("flag-token".to_string()));
    }

    #[test]
    fn test_resolve_token_optional_no_token() {
        // Clear env var to ensure clean test
        let original = env::var("TODOIST_TOKEN").ok();
        env::remove_var("TODOIST_TOKEN");

        // Set config to non-existent path to ensure no config token
        let original_config = env::var("TD_CONFIG").ok();
        env::set_var("TD_CONFIG", "/tmp/td-test-nonexistent/config.toml");

        let cli = cli_with_token(None);
        let result = resolve_token_optional(&cli);

        // Restore env vars
        if let Some(val) = original {
            env::set_var("TODOIST_TOKEN", val);
        }
        if let Some(val) = original_config {
            env::set_var("TD_CONFIG", val);
        } else {
            env::remove_var("TD_CONFIG");
        }

        // Should return Ok(None) when no token found (setup will handle it)
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_resolve_token_optional_from_config() {
        use std::fs;
        use std::io::Write;
        use tempfile::TempDir;

        // Create a temporary config file
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut file = fs::File::create(&config_path).unwrap();
        writeln!(file, r#"token = "config-token""#).unwrap();

        // Set TD_CONFIG to point to our temp config
        let original_config = env::var("TD_CONFIG").ok();
        env::set_var("TD_CONFIG", config_path.to_str().unwrap());

        // Clear TODOIST_TOKEN to ensure we're not picking it up
        let original_token = env::var("TODOIST_TOKEN").ok();
        env::remove_var("TODOIST_TOKEN");

        let cli = cli_with_token(None);
        let result = resolve_token_optional(&cli);

        // Restore env vars first (before assertions that might panic)
        if let Some(val) = original_config {
            env::set_var("TD_CONFIG", val);
        } else {
            env::remove_var("TD_CONFIG");
        }
        if let Some(val) = original_token {
            env::set_var("TODOIST_TOKEN", val);
        }

        assert!(result.is_ok(), "Expected Ok but got: {:?}", result);
        assert_eq!(result.unwrap(), Some("config-token".to_string()));
    }

    #[test]
    fn test_resolve_token_optional_flag_overrides_config() {
        use std::fs;
        use std::io::Write;
        use tempfile::TempDir;

        // Create a temporary config file with a token
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut file = fs::File::create(&config_path).unwrap();
        writeln!(file, r#"token = "config-token""#).unwrap();

        // Set TD_CONFIG to point to our temp config
        let original_config = env::var("TD_CONFIG").ok();
        env::set_var("TD_CONFIG", config_path.to_str().unwrap());

        // Clear TODOIST_TOKEN
        let original_token = env::var("TODOIST_TOKEN").ok();
        env::remove_var("TODOIST_TOKEN");

        // CLI has a token from flag, should override config
        let cli = cli_with_token(Some("flag-token".to_string()));
        let result = resolve_token_optional(&cli);

        // Restore env vars
        if let Some(val) = original_config {
            env::set_var("TD_CONFIG", val);
        } else {
            env::remove_var("TD_CONFIG");
        }
        if let Some(val) = original_token {
            env::set_var("TODOIST_TOKEN", val);
        }

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("flag-token".to_string()));
    }
}
