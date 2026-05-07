pub mod client;

use anyhow::{Result, anyhow};

pub fn normalize_base_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(anyhow!("base URL cannot be empty"));
    }
    let parsed = url::Url::parse(trimmed).map_err(|_| anyhow!("invalid base URL: {trimmed}"))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(anyhow!("base URL scheme must be http or https"));
    }
    Ok(trimmed.to_owned())
}
