use std::env;
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingVar(String),
}

#[derive(Debug)]
pub struct AppConfig {
    pub telegram_bot_token: SecretString,
    pub telegram_allowed_user_ids: Vec<u64>,
    pub groq_api_key: SecretString,
    pub openrouter_api_key: SecretString,
    pub openrouter_model: String,
    pub db_path: String,
}

#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

// Custom debug format to prevent secret leakage
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretString([REDACTED])")
    }
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    load_config_inner(true)
}

pub(crate) fn load_config_inner(use_dotenv: bool) -> Result<AppConfig, ConfigError> {
    if use_dotenv {
        dotenvy::dotenv().ok(); // Ignore error if .env file is missing, fallback to env vars
    }

    let telegram_allowed_user_ids = get_env("TELEGRAM_ALLOWED_USER_IDS")?
        .split(',')
        .filter_map(|s| s.trim().parse::<u64>().ok())
        .collect::<Vec<_>>();

    if telegram_allowed_user_ids.is_empty() {
        return Err(ConfigError::MissingVar(
            "TELEGRAM_ALLOWED_USER_IDS (must contain valid IDs)".to_string(),
        ));
    }

    Ok(AppConfig {
        telegram_bot_token: SecretString::new(get_env("TELEGRAM_BOT_TOKEN")?),
        telegram_allowed_user_ids,
        groq_api_key: SecretString::new(get_env("GROQ_API_KEY")?),
        openrouter_api_key: SecretString::new(get_env("OPENROUTER_API_KEY")?),
        openrouter_model: get_env("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "openrouter/free".to_string()),
        db_path: get_env("DB_PATH").unwrap_or_else(|_| "./memory.db".to_string()),
    })
}

fn get_env(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::MissingVar(key.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_secret_string_redaction() {
        let secret = SecretString::new("my-secret".to_string());
        let debug_str = format!("{:?}", secret);
        assert_eq!(debug_str, "SecretString([REDACTED])");
        assert_eq!(secret.expose_secret(), "my-secret");
    }

    #[test]
    #[serial]
    fn test_missing_env() {
        // Clear environment variable to test missing var
        env::remove_var("TELEGRAM_BOT_TOKEN");
        let result = load_config_inner(false);
        assert!(matches!(result, Err(ConfigError::MissingVar(_))));
    }

    /// Verifies that when TELEGRAM_ALLOWED_USER_IDS contains no valid IDs,
    /// load_config_inner returns a MissingVar error.
    #[test]
    #[serial]
    fn test_empty_allowed_user_ids() {
        env::set_var("TELEGRAM_ALLOWED_USER_IDS", "not_a_number");
        env::set_var("TELEGRAM_BOT_TOKEN", "token123");
        env::set_var("GROQ_API_KEY", "groq123");
        env::set_var("OPENROUTER_API_KEY", "or123");

        let result = load_config_inner(false);
        assert!(matches!(result, Err(ConfigError::MissingVar(_))));
    }

    #[test]
    #[serial]
    fn test_load_config_success() {
        // Set all required env variables
        env::set_var("TELEGRAM_ALLOWED_USER_IDS", "123,456");
        env::set_var("TELEGRAM_BOT_TOKEN", "token123");
        env::set_var("GROQ_API_KEY", "groq123");
        env::set_var("OPENROUTER_API_KEY", "or123");

        let config = load_config_inner(false).unwrap();
        assert_eq!(config.telegram_allowed_user_ids, vec![123, 456]);
        assert_eq!(config.telegram_bot_token.expose_secret(), "token123");
        assert_eq!(config.groq_api_key.expose_secret(), "groq123");
        assert_eq!(config.openrouter_api_key.expose_secret(), "or123");
    }

    #[test]
    #[serial]
    fn test_load_config_optional_defaults() {
        env::set_var("TELEGRAM_ALLOWED_USER_IDS", "1");
        env::set_var("TELEGRAM_BOT_TOKEN", "t");
        env::set_var("GROQ_API_KEY", "g");
        env::set_var("OPENROUTER_API_KEY", "o");
        env::remove_var("OPENROUTER_MODEL");
        env::remove_var("DB_PATH");

        let config = load_config_inner(false).unwrap();
        assert_eq!(config.openrouter_model, "openrouter/free");
        assert_eq!(config.db_path, "./memory.db");
    }

    #[test]
    #[serial]
    fn test_load_config_with_dotenv() {
        // Just verify it doesn't crash when trying to load .env
        let _ = load_config_inner(true);
    }

    #[test]
    #[serial]
    fn test_public_load_config() {
        env::set_var("TELEGRAM_ALLOWED_USER_IDS", "1");
        env::set_var("TELEGRAM_BOT_TOKEN", "t");
        env::set_var("GROQ_API_KEY", "g");
        env::set_var("OPENROUTER_API_KEY", "o");
        let _ = load_config();
    }
}
