//! Config command implementation.
//!
//! View and manage configuration settings.
//! Config file is located at ~/.config/td/config.toml.

use std::env;
use std::fs;
use std::path::PathBuf;

use tokio::process::Command;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use super::{CommandContext, CommandError, Result};

/// Current config file version. Increment when making breaking changes to schema.
const CONFIG_VERSION: u32 = 1;

/// Minimum token length to apply masking (show first and last N characters).
const TOKEN_MASK_MIN_LENGTH: usize = 8;

/// Number of characters to show at start/end of a masked token.
const TOKEN_MASK_VISIBLE_CHARS: usize = 4;

/// Default config file contents.
const DEFAULT_CONFIG: &str = r#"# td - Todoist CLI Configuration
# https://github.com/your-org/todoist-rs

# Config schema version (do not modify)
version = 1

# API token (can also use TODOIST_TOKEN env var)
# token = "your-api-token-here"

# Token storage method: "config", "keyring", or "env"
# token_storage = "config"

# Output preferences
[output]
# color = true              # Enable colors (respects NO_COLOR env)
# date_format = "relative"  # "relative", "iso", "short"

# Cache settings
[cache]
# enabled = true
"#;

/// Configuration file structure.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Config schema version for migrations.
    /// Defaults to current version when not present in file.
    #[serde(default = "default_version")]
    pub version: u32,

    /// API token (optional, can use env var instead).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Token storage method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_storage: Option<String>,

    /// Output settings.
    #[serde(default)]
    pub output: OutputConfig,

    /// Cache settings.
    #[serde(default)]
    pub cache: CacheConfig,
}

/// Returns the current config version (used by serde default).
fn default_version() -> u32 {
    CONFIG_VERSION
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            token: None,
            token_storage: None,
            output: OutputConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}

/// Output configuration.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Enable colors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<bool>,

    /// Date format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_format: Option<String>,
}

/// Cache configuration.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Gets the config directory path.
/// Uses XDG-style paths: ~/.config/td/ on all platforms.
fn get_config_dir() -> Result<PathBuf> {
    // Check for override env var first
    if let Ok(path) = env::var("TD_CONFIG") {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            return Ok(parent.to_path_buf());
        }
    }

    // Use XDG_CONFIG_HOME if set, otherwise ~/.config/td
    if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg_config).join("td"));
    }

    BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".config").join("td"))
        .ok_or_else(|| {
            CommandError::Config("Could not determine config directory".to_string())
        })
}

/// Gets the config file path.
pub fn get_config_path() -> Result<PathBuf> {
    // Check for override env var first
    if let Ok(path) = env::var("TD_CONFIG") {
        return Ok(PathBuf::from(path));
    }

    let config_dir = get_config_dir()?;
    Ok(config_dir.join("config.toml"))
}

/// Loads the configuration from disk.
pub fn load_config() -> Result<Config> {
    let path = get_config_path()?;

    if !path.exists() {
        return Ok(Config::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| CommandError::Config(format!("Failed to read config: {}", e)))?;

    let config: Config = toml::from_str(&content)
        .map_err(|e| CommandError::Config(format!("Failed to parse config: {}", e)))?;

    // Migrate config if needed (stub for future migrations)
    migrate_config(config)
}

/// Migrates config to current version if needed.
/// Returns the config as-is if already at current version.
fn migrate_config(mut config: Config) -> Result<Config> {
    // No migrations needed yet - version 1 is the initial version
    // Future migrations would be handled here:
    //
    // if config.version < 2 {
    //     // Apply v1 -> v2 migration
    //     config.version = 2;
    // }
    // if config.version < 3 {
    //     // Apply v2 -> v3 migration
    //     config.version = 3;
    // }

    // Ensure version is current
    config.version = CONFIG_VERSION;
    Ok(config)
}

/// Saves the configuration to disk.
fn save_config(config: &Config) -> Result<()> {
    let path = get_config_path()?;

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CommandError::Config(format!("Failed to create config directory: {}", e)))?;
    }

    let content = toml::to_string_pretty(config)
        .map_err(|e| CommandError::Config(format!("Failed to serialize config: {}", e)))?;

    fs::write(&path, content)
        .map_err(|e| CommandError::Config(format!("Failed to write config: {}", e)))?;

    Ok(())
}

/// Executes the config show command.
pub fn execute_show(ctx: &CommandContext) -> Result<()> {
    let config = load_config()?;
    let path = get_config_path()?;

    if ctx.json_output {
        let output = serde_json::json!({
            "path": path.display().to_string(),
            "exists": path.exists(),
            "config": config,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !ctx.quiet {
        use owo_colors::OwoColorize;

        let header = "Configuration";
        if ctx.use_colors {
            println!("{}\n", header.green().bold());
        } else {
            println!("{}\n", header);
        }

        println!("File: {}", path.display());
        println!("Exists: {}\n", path.exists());

        if path.exists() {
            // Show current config values
            println!("Settings:");
            if let Some(ref storage) = config.token_storage {
                println!("  token_storage: {}", storage);
            }
            if let Some(ref token) = config.token {
                println!("  token: {}", mask_token(token));
            }

            println!("\n[output]");
            if let Some(color) = config.output.color {
                println!("  color: {}", color);
            }
            if let Some(ref format) = config.output.date_format {
                println!("  date_format: {}", format);
            }

            println!("\n[cache]");
            if let Some(enabled) = config.cache.enabled {
                println!("  enabled: {}", enabled);
            }
        } else {
            println!("(No config file exists. Run 'td config edit' to create one.)");
        }
    }

    Ok(())
}

/// Executes the config edit command.
pub async fn execute_edit(ctx: &CommandContext) -> Result<()> {
    let path = get_config_path()?;

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CommandError::Config(format!("Failed to create config directory: {}", e)))?;
    }

    // Create default config if it doesn't exist
    if !path.exists() {
        fs::write(&path, DEFAULT_CONFIG)
            .map_err(|e| CommandError::Config(format!("Failed to create config file: {}", e)))?;

        if !ctx.quiet && !ctx.json_output {
            eprintln!("Created default config at: {}", path.display());
        }
    }

    // Get editor from environment
    let editor = env::var("EDITOR")
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    if ctx.verbose {
        eprintln!("Opening {} with {}", path.display(), editor);
    }

    // Open editor (async to avoid blocking the tokio runtime)
    let status = Command::new(&editor)
        .arg(&path)
        .status()
        .await
        .map_err(|e| CommandError::Config(format!("Failed to open editor '{}': {}", editor, e)))?;

    if ctx.json_output {
        let output = serde_json::json!({
            "status": if status.success() { "success" } else { "error" },
            "editor": editor,
            "path": path.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !ctx.quiet {
        if status.success() {
            println!("Config saved.");
        } else {
            eprintln!("Editor exited with error");
        }
    }

    Ok(())
}

/// Options for the config set command.
pub struct ConfigSetOptions {
    /// Configuration key.
    pub key: String,
    /// Configuration value.
    pub value: String,
}

/// Executes the config set command.
pub fn execute_set(ctx: &CommandContext, opts: &ConfigSetOptions) -> Result<()> {
    let mut config = load_config()?;
    let path = get_config_path()?;

    // Parse and set the value based on key
    let (section, field) = if opts.key.contains('.') {
        let parts: Vec<&str> = opts.key.splitn(2, '.').collect();
        (Some(parts[0]), parts[1])
    } else {
        (None, opts.key.as_str())
    };

    match (section, field) {
        (None, "token") => {
            config.token = Some(opts.value.clone());
        }
        (None, "token_storage") => {
            let valid = ["config", "keyring", "env"];
            if !valid.contains(&opts.value.as_str()) {
                return Err(CommandError::Config(format!(
                    "Invalid token_storage value '{}'. Valid values: {}",
                    opts.value,
                    valid.join(", ")
                )));
            }
            config.token_storage = Some(opts.value.clone());
        }
        (Some("output"), "color") => {
            let value = parse_bool(&opts.value)?;
            config.output.color = Some(value);
        }
        (Some("output"), "date_format") => {
            let valid = ["relative", "iso", "short"];
            if !valid.contains(&opts.value.as_str()) {
                return Err(CommandError::Config(format!(
                    "Invalid date_format value '{}'. Valid values: {}",
                    opts.value,
                    valid.join(", ")
                )));
            }
            config.output.date_format = Some(opts.value.clone());
        }
        (Some("cache"), "enabled") => {
            let value = parse_bool(&opts.value)?;
            config.cache.enabled = Some(value);
        }
        _ => {
            return Err(CommandError::Config(format!(
                "Unknown config key '{}'. Valid keys: token, token_storage, output.color, output.date_format, cache.enabled",
                opts.key
            )));
        }
    }

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CommandError::Config(format!("Failed to create config directory: {}", e)))?;
    }

    save_config(&config)?;

    if ctx.json_output {
        let output = serde_json::json!({
            "status": "success",
            "key": opts.key,
            "value": opts.value,
            "path": path.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !ctx.quiet {
        println!("Set {} = {}", opts.key, opts.value);
    }

    Ok(())
}

/// Executes the config path command.
pub fn execute_path(ctx: &CommandContext) -> Result<()> {
    let path = get_config_path()?;

    if ctx.json_output {
        let output = serde_json::json!({
            "path": path.display().to_string(),
            "exists": path.exists(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", path.display());
    }

    Ok(())
}

/// Masks a token for display, showing only the first and last N characters.
///
/// Uses character-based (not byte-based) indexing to safely handle
/// multi-byte UTF-8 characters.
fn mask_token(token: &str) -> String {
    let char_count = token.chars().count();
    if char_count > TOKEN_MASK_MIN_LENGTH {
        let prefix: String = token.chars().take(TOKEN_MASK_VISIBLE_CHARS).collect();
        let suffix: String = token
            .chars()
            .skip(char_count - TOKEN_MASK_VISIBLE_CHARS)
            .collect();
        format!("{}...{}", prefix, suffix)
    } else {
        "****".to_string()
    }
}

/// Parses a boolean value from string.
fn parse_bool(s: &str) -> Result<bool> {
    match s.to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(CommandError::Config(format!(
            "Invalid boolean value '{}'. Use true/false, yes/no, 1/0, or on/off",
            s
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bool_true_values() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("True").unwrap());
        assert!(parse_bool("TRUE").unwrap());
        assert!(parse_bool("yes").unwrap());
        assert!(parse_bool("1").unwrap());
        assert!(parse_bool("on").unwrap());
    }

    #[test]
    fn test_parse_bool_false_values() {
        assert!(!parse_bool("false").unwrap());
        assert!(!parse_bool("False").unwrap());
        assert!(!parse_bool("FALSE").unwrap());
        assert!(!parse_bool("no").unwrap());
        assert!(!parse_bool("0").unwrap());
        assert!(!parse_bool("off").unwrap());
    }

    #[test]
    fn test_parse_bool_invalid() {
        assert!(parse_bool("maybe").is_err());
        assert!(parse_bool("").is_err());
        assert!(parse_bool("2").is_err());
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, CONFIG_VERSION);
        assert!(config.token.is_none());
        assert!(config.token_storage.is_none());
        assert!(config.output.color.is_none());
        assert!(config.cache.enabled.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            version: CONFIG_VERSION,
            token: None,
            token_storage: Some("config".to_string()),
            output: OutputConfig {
                color: Some(true),
                date_format: Some("relative".to_string()),
            },
            cache: CacheConfig {
                enabled: Some(true),
            },
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("version = 1"));
        assert!(toml_str.contains("token_storage"));
        assert!(toml_str.contains("[output]"));
        assert!(toml_str.contains("color = true"));
        assert!(toml_str.contains("[cache]"));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = r#"
version = 1
token_storage = "keyring"

[output]
color = false
date_format = "iso"

[cache]
enabled = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.token_storage, Some("keyring".to_string()));
        assert_eq!(config.output.color, Some(false));
        assert_eq!(config.output.date_format, Some("iso".to_string()));
        assert_eq!(config.cache.enabled, Some(true));
    }

    #[test]
    fn test_config_deserialization_empty() {
        let toml_str = "";
        let config: Config = toml::from_str(toml_str).unwrap();
        // Missing version defaults to current version
        assert_eq!(config.version, CONFIG_VERSION);
        assert!(config.token.is_none());
        assert!(config.token_storage.is_none());
    }

    #[test]
    fn test_config_deserialization_partial() {
        let toml_str = r#"
[output]
color = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // Missing version defaults to current version
        assert_eq!(config.version, CONFIG_VERSION);
        assert!(config.token.is_none());
        assert_eq!(config.output.color, Some(true));
        assert!(config.output.date_format.is_none());
        assert!(config.cache.enabled.is_none());
    }

    #[test]
    fn test_mask_token_ascii() {
        // Long token gets masked with first 4 and last 4 visible
        assert_eq!(mask_token("abcdefghijklmnop"), "abcd...mnop");
        // Token at threshold (exactly 9 chars = still masks)
        assert_eq!(mask_token("123456789"), "1234...6789");
        // Token at min length (8 chars) gets fully masked
        assert_eq!(mask_token("12345678"), "****");
        // Short token gets fully masked
        assert_eq!(mask_token("short"), "****");
    }

    #[test]
    fn test_mask_token_utf8_emoji() {
        // Emoji tokens (4 bytes per emoji, but counted as 1 character each)
        // "🔑🔐🔒🔓🎉🎊🎁🎄🎅" = 9 emoji characters -> should mask
        assert_eq!(mask_token("🔑🔐🔒🔓🎉🎊🎁🎄🎅"), "🔑🔐🔒🔓...🎊🎁🎄🎅");
        // "🔑🔐🔒🔓🎉🎊🎁🎄" = 8 emoji characters -> too short, fully mask
        assert_eq!(mask_token("🔑🔐🔒🔓🎉🎊🎁🎄"), "****");
    }

    #[test]
    fn test_mask_token_utf8_chinese() {
        // Chinese characters (3 bytes each, but counted as 1 character)
        // "密码钥匙令牌凭证安全" = 10 characters -> should mask
        // First 4: 密码钥匙, Last 4: 凭证安全
        assert_eq!(mask_token("密码钥匙令牌凭证安全"), "密码钥匙...凭证安全");
        // "密码钥匙令牌凭证" = 8 characters -> too short
        assert_eq!(mask_token("密码钥匙令牌凭证"), "****");
    }

    #[test]
    fn test_mask_token_mixed_utf8() {
        // Mixed ASCII, emoji, and Chinese
        // "key🔑密码token" = 11 characters -> should mask
        assert_eq!(mask_token("key🔑密码token"), "key🔑...oken");
    }

    #[test]
    fn test_config_version_constant() {
        // Verify the current version constant
        assert_eq!(CONFIG_VERSION, 1);
    }

    #[test]
    fn test_config_version_default_function() {
        // Verify the default version function returns current version
        assert_eq!(default_version(), CONFIG_VERSION);
    }

    #[test]
    fn test_migrate_config_preserves_data() {
        // Migration should preserve all config data
        let config = Config {
            version: 1,
            token: Some("test-token".to_string()),
            token_storage: Some("keyring".to_string()),
            output: OutputConfig {
                color: Some(true),
                date_format: Some("iso".to_string()),
            },
            cache: CacheConfig {
                enabled: Some(true),
            },
        };

        let migrated = migrate_config(config).unwrap();
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(migrated.token, Some("test-token".to_string()));
        assert_eq!(migrated.token_storage, Some("keyring".to_string()));
        assert_eq!(migrated.output.color, Some(true));
        assert_eq!(migrated.output.date_format, Some("iso".to_string()));
        assert_eq!(migrated.cache.enabled, Some(true));
    }

    #[test]
    fn test_config_deserialization_with_future_version() {
        // Config with a future version should still parse
        let toml_str = r#"
version = 999
token_storage = "env"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.version, 999);
        assert_eq!(config.token_storage, Some("env".to_string()));
    }
}
