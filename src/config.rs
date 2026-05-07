use anyhow::{Result, anyhow};
use std::path::PathBuf;

pub const DEFAULT_PROFILE_NAME: &str = "default";
pub const DEFAULT_API_HOST: &str = "https://api.volvocars.com";
pub const DEFAULT_AUTH_ISSUER: &str = "https://volvoid.eu.volvocars.com";
pub const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:8787/callback";
pub const DEFAULT_SCOPES: &str = "openid";
pub const DEFAULT_AUTH_LISTEN_TIMEOUT_SECONDS: u64 = 180;

pub fn resolve_config_dir() -> Result<PathBuf> {
    if let Ok(override_dir) = std::env::var("VOLVO_CONFIG_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    let base = dirs::config_dir().ok_or_else(|| anyhow!("failed to resolve config directory"))?;
    Ok(base.join("vc-cli"))
}
