//! Configuration file parsing and validation.
//!
//! This module handles loading and validating diesel-guard.toml configuration files.

use camino::{Utf8Path, Utf8PathBuf};
use miette::Diagnostic;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use thiserror::Error;

/// Regex pattern for validating timestamp format
/// Accepts: YYYY_MM_DD_HHMMSS, YYYY-MM-DD-HHMMSS, or YYYYMMDDHHMMSS
/// All separators must be the same (all underscores, all dashes, or none)
static TIMESTAMP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}_\d{2}_\d{2}_\d{6}|\d{4}-\d{2}-\d{2}-\d{6}|\d{14})$").unwrap()
});

/// Generate help text for invalid check names from the registry
fn valid_check_names_help() -> String {
    format!(
        "Valid check names: {}",
        crate::checks::ALL_CHECK_NAMES.join(", ")
    )
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse config file")]
    ParseError(#[from] toml::de::Error),

    #[error("Invalid check name: {invalid_name}")]
    InvalidCheckName { invalid_name: String },

    #[error("Invalid timestamp format: {0}")]
    InvalidTimestampFormat(String),
}

impl Diagnostic for ConfigError {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        match self {
            Self::IoError(_) => Some(Box::new("diesel_guard::config::io_error")),
            Self::ParseError(_) => Some(Box::new("diesel_guard::config::parse_error")),
            Self::InvalidCheckName { .. } => Some(Box::new("diesel_guard::config::invalid_check")),
            Self::InvalidTimestampFormat(_) => {
                Some(Box::new("diesel_guard::config::invalid_timestamp"))
            }
        }
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        match self {
            Self::InvalidCheckName { .. } => Some(Box::new(valid_check_names_help())),
            Self::InvalidTimestampFormat(_) => Some(Box::new(
                "Expected format: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, or YYYY-MM-DD-HHMMSS (e.g., 20240101000000, 2024_01_01_000000, or 2024-01-01-000000)",
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Skip migrations before this timestamp
    /// Format: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, or YYYY-MM-DD-HHMMSS
    /// Examples: "20240101000000", "2024_01_01_000000", or "2024-01-01-000000"
    #[serde(default)]
    pub start_after: Option<String>,

    /// Whether to check down.sql files in addition to up.sql
    #[serde(default)]
    pub check_down: bool,

    /// List of check struct names to disable
    #[serde(default)]
    pub disable_checks: Vec<String>,
}

impl Config {
    /// Load config from diesel-guard.toml in current directory
    /// Returns default config if file doesn't exist
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Utf8PathBuf::from("diesel-guard.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        Self::load_from_path(&config_path)
    }

    /// Load config from specific path (useful for testing)
    pub fn load_from_path(path: &Utf8Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate timestamp format if present
        if let Some(ref timestamp) = self.start_after {
            Self::validate_timestamp(timestamp)?;
        }

        // Validate check names against the central registry
        for check_name in &self.disable_checks {
            if !crate::checks::ALL_CHECK_NAMES.contains(&check_name.as_str()) {
                return Err(ConfigError::InvalidCheckName {
                    invalid_name: check_name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Validate timestamp format: YYYY_MM_DD_HHMMSS
    fn validate_timestamp(timestamp: &str) -> Result<(), ConfigError> {
        if TIMESTAMP_REGEX.is_match(timestamp) {
            Ok(())
        } else {
            Err(ConfigError::InvalidTimestampFormat(timestamp.to_string()))
        }
    }

    /// Check if a specific check is enabled
    pub fn is_check_enabled(&self, check_name: &str) -> bool {
        !self.disable_checks.iter().any(|c| c == check_name)
    }

    /// Normalize timestamp by removing all non-digit characters
    /// Converts any format (YYYY_MM_DD_HHMMSS, YYYY-MM-DD-HHMMSS, YYYYMMDDHHMMSS) to YYYYMMDDHHMMSS
    fn normalize_timestamp(timestamp: &str) -> String {
        timestamp.chars().filter(|c| c.is_ascii_digit()).collect()
    }

    /// Check if migration should be checked based on start_after
    /// Returns true if migration timestamp is AFTER start_after (or if no filter set)
    pub fn should_check_migration(&self, migration_dir_name: &str) -> bool {
        if let Some(ref start_after) = self.start_after {
            // Normalize the start_after timestamp (remove separators)
            let normalized_start_after = Self::normalize_timestamp(start_after);

            // Extract and normalize timestamp from migration directory name
            // Migration formats can be: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, YYYY-MM-DD-HHMMSS
            // Extract first 14 digits from the directory name
            let normalized_migration = Self::normalize_timestamp(migration_dir_name);

            if normalized_migration.len() >= 14 {
                let migration_timestamp = &normalized_migration[..14];
                // Lexicographic comparison works for normalized timestamp format
                // Returns true only if migration_timestamp > start_after (strictly after)
                return migration_timestamp > normalized_start_after.as_str();
            }
        }
        true // Check by default if no filter or if dir name too short
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.start_after, None);
        assert!(!config.check_down);
        assert_eq!(config.disable_checks.len(), 0);
    }

    #[test]
    fn test_valid_timestamp() {
        // Format with underscores
        assert!(Config::validate_timestamp("2024_01_01_000000").is_ok());
        assert!(Config::validate_timestamp("2023_12_31_235959").is_ok());
        assert!(Config::validate_timestamp("2025_06_15_120000").is_ok());

        // Format with dashes
        assert!(Config::validate_timestamp("2024-01-01-000000").is_ok());
        assert!(Config::validate_timestamp("2023-12-31-235959").is_ok());
        assert!(Config::validate_timestamp("2025-06-15-120000").is_ok());

        // Format without separators
        assert!(Config::validate_timestamp("20240101000000").is_ok());
        assert!(Config::validate_timestamp("20231231235959").is_ok());
        assert!(Config::validate_timestamp("20250615120000").is_ok());
    }

    #[test]
    fn test_invalid_timestamp_format() {
        // Mixed separators
        assert!(Config::validate_timestamp("2024_01-01_000000").is_err());
        assert!(Config::validate_timestamp("2024-01_01-000000").is_err());

        // Wrong length
        assert!(Config::validate_timestamp("2024_01_01").is_err());
        assert!(Config::validate_timestamp("2024_01_01_00000").is_err());
        assert!(Config::validate_timestamp("2024010100000").is_err());

        // Non-numeric characters
        assert!(Config::validate_timestamp("202a_01_01_000000").is_err());
        assert!(Config::validate_timestamp("202a-01-01-000000").is_err());

        // Invalid separators
        assert!(Config::validate_timestamp("2024/01/01/000000").is_err());
        assert!(Config::validate_timestamp("2024.01.01.000000").is_err());
    }

    #[test]
    fn test_should_check_migration_no_filter() {
        let config = Config::default();
        assert!(config.should_check_migration("2024_01_01_000000_create_users"));
        assert!(config.should_check_migration("2020_01_01_000000_old_migration"));
    }

    #[test]
    fn test_should_check_migration_with_filter() {
        let config = Config {
            start_after: Some("2024_01_01_000000".to_string()),
            ..Default::default()
        };

        // Should check (strictly after threshold)
        assert!(config.should_check_migration("2024_01_02_000000_new_migration"));
        assert!(config.should_check_migration("2024_06_15_120000_another_migration"));

        // Should NOT check (before or equal to threshold)
        assert!(!config.should_check_migration("2024_01_01_000000_exact_match"));
        assert!(!config.should_check_migration("2023_12_31_235959_old_migration"));
        assert!(!config.should_check_migration("2020_01_01_000000_very_old"));
    }

    #[test]
    fn test_should_check_migration_mixed_formats() {
        // Test start_after with underscores against folders with different formats
        let config_underscores = Config {
            start_after: Some("2024_01_01_000000".to_string()),
            ..Default::default()
        };

        // Folder with dashes - should check (after threshold)
        assert!(config_underscores.should_check_migration("2024-01-02-000000_new_migration"));
        assert!(config_underscores.should_check_migration("2024-06-15-120000_another_migration"));

        // Folder with dashes - should NOT check (before or equal)
        assert!(!config_underscores.should_check_migration("2024-01-01-000000_exact_match"));
        assert!(!config_underscores.should_check_migration("2023-12-31-235959_old_migration"));

        // Folder without separators - should check (after threshold)
        assert!(config_underscores.should_check_migration("20240102000000_new_migration"));
        assert!(config_underscores.should_check_migration("20240615120000_another_migration"));

        // Folder without separators - should NOT check (before or equal)
        assert!(!config_underscores.should_check_migration("20240101000000_exact_match"));
        assert!(!config_underscores.should_check_migration("20231231235959_old_migration"));

        // Test start_after with dashes against folders with different formats
        let config_dashes = Config {
            start_after: Some("2024-01-01-000000".to_string()),
            ..Default::default()
        };

        // Folder with underscores - should check (after threshold)
        assert!(config_dashes.should_check_migration("2024_01_02_000000_new_migration"));
        assert!(!config_dashes.should_check_migration("2024_01_01_000000_exact_match"));

        // Folder without separators - should check (after threshold)
        assert!(config_dashes.should_check_migration("20240102000000_new_migration"));
        assert!(!config_dashes.should_check_migration("20240101000000_exact_match"));

        // Test start_after without separators against folders with different formats
        let config_no_sep = Config {
            start_after: Some("20240101000000".to_string()),
            ..Default::default()
        };

        // Folder with underscores - should check (after threshold)
        assert!(config_no_sep.should_check_migration("2024_01_02_000000_new_migration"));
        assert!(!config_no_sep.should_check_migration("2024_01_01_000000_exact_match"));

        // Folder with dashes - should check (after threshold)
        assert!(config_no_sep.should_check_migration("2024-01-02-000000_new_migration"));
        assert!(!config_no_sep.should_check_migration("2024-01-01-000000_exact_match"));
    }

    #[test]
    fn test_is_check_enabled() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string(), "DropColumnCheck".to_string()],
            ..Default::default()
        };

        assert!(!config.is_check_enabled("AddColumnCheck"));
        assert!(!config.is_check_enabled("DropColumnCheck"));
        assert!(config.is_check_enabled("AddIndexCheck"));
        assert!(config.is_check_enabled("AddNotNullCheck"));
    }

    #[test]
    fn test_invalid_check_name() {
        let config_str = r#"
            disable_checks = ["InvalidCheckName"]
        "#;

        let config: Config = toml::from_str(config_str).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_valid_check_names() {
        let config_str = r#"
            disable_checks = ["AddColumnCheck", "DropColumnCheck"]
        "#;

        let config: Config = toml::from_str(config_str).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_check_name_help_includes_all_checks() {
        use miette::Diagnostic;

        let error = ConfigError::InvalidCheckName {
            invalid_name: "FooCheck".to_string(),
        };

        let help = error.help().unwrap().to_string();

        // Verify help text includes all check names from the registry
        for &check_name in crate::checks::ALL_CHECK_NAMES {
            assert!(
                help.contains(check_name),
                "Help text should include '{}', got: {}",
                check_name,
                help
            );
        }

        // Verify format
        assert!(help.starts_with("Valid check names: "));
    }

    #[test]
    fn test_load_from_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("diesel-guard.toml");

        fs::write(
            &config_path,
            r#"
start_after = "2024_01_01_000000"
check_down = true
disable_checks = ["AddColumnCheck"]
            "#,
        )
        .unwrap();

        let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
        let config = Config::load_from_path(config_path_utf8).unwrap();
        assert_eq!(config.start_after, Some("2024_01_01_000000".to_string()));
        assert!(config.check_down);
        assert_eq!(config.disable_checks, vec!["AddColumnCheck".to_string()]);
    }
}
