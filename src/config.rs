use anyhow::{Context, Result, anyhow};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE_NAME: &str = "default";
pub const DEFAULT_API_HOST: &str = "https://api.volvocars.com";
pub const DEFAULT_AUTH_ISSUER: &str = "https://volvoid.eu.volvocars.com";
pub const DEFAULT_AUTH_REDIRECT_URI: &str = "http://127.0.0.1:1410/callback";
pub const DEFAULT_SCOPES: &str = concat!(
    "openid ",
    "conve:battery_charge_level ",
    "conve:brake_status ",
    "conve:climatization_start_stop ",
    "conve:command_accessibility ",
    "conve:commands ",
    "conve:connectivity_status ",
    "conve:diagnostics_engine_status ",
    "conve:diagnostics_workshop ",
    "conve:doors_status ",
    "conve:engine_start_stop ",
    "conve:engine_status ",
    "conve:environment ",
    "conve:fuel_status ",
    "conve:honk_flash ",
    "conve:lock ",
    "conve:lock_status ",
    "conve:navigation ",
    "conve:odometer_status ",
    "conve:trip_statistics ",
    "conve:tyre_status ",
    "conve:unlock ",
    "conve:vehicle_relation ",
    "conve:warnings ",
    "conve:windows_status ",
    "energy:capability:read ",
    "energy:state:read ",
    "location:read",
);
pub const DEFAULT_AUTH_LISTEN_TIMEOUT_SECONDS: u64 = 180;
pub const CONFIG_FILE_NAME: &str = "config";

pub fn resolve_config_dir() -> Result<PathBuf> {
    if let Ok(override_dir) = std::env::var("VOLVO_CONFIG_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("failed to resolve home directory"))?;
    Ok(home.join(".config").join("vc-cli"))
}

pub fn resolve_config_file(config_dir: &Path) -> PathBuf {
    config_dir.join(CONFIG_FILE_NAME)
}

pub fn load_config_env(config_dir: &Path) -> Result<()> {
    let config_file = resolve_config_file(config_dir);
    if !config_file.exists() {
        return Ok(());
    }
    let contents = fs::read_to_string(&config_file)
        .with_context(|| format!("failed to read config file {}", config_file.display()))?;
    for (key, value) in parse_env_config(&contents)? {
        if std::env::var(&key)
            .map(|existing| existing.trim().is_empty())
            .unwrap_or(true)
        {
            // SAFETY: This is called at CLI startup before async runtime work begins.
            unsafe { std::env::set_var(key, value) };
        }
    }
    Ok(())
}

pub fn save_config_values(config_dir: &Path, values: &[(&str, &str)]) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }

    fs::create_dir_all(config_dir)
        .with_context(|| format!("failed to create config directory {}", config_dir.display()))?;
    #[cfg(unix)]
    set_private_dir_permissions(config_dir)?;

    let config_file = resolve_config_file(config_dir);
    let existing = match fs::read_to_string(&config_file) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read config file {}", config_file.display()));
        }
    };
    let updated = update_env_config_contents(&existing, values)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&config_file)
        .with_context(|| format!("failed to open config file {}", config_file.display()))?;
    #[cfg(unix)]
    set_private_file_permissions(&file)?;
    file.write_all(updated.as_bytes())
        .with_context(|| format!("failed to write config file {}", config_file.display()))?;
    Ok(())
}

fn parse_env_config(contents: &str) -> Result<Vec<(String, String)>> {
    let mut values = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let Some((key, value)) = parse_env_line(line)
            .with_context(|| format!("invalid config assignment on line {}", index + 1))?
        else {
            continue;
        };
        values.push((key, value));
    }
    Ok(values)
}

fn update_env_config_contents(contents: &str, values: &[(&str, &str)]) -> Result<String> {
    let updates = values
        .iter()
        .map(|(key, value)| {
            validate_env_key(key)?;
            Ok(((*key).to_owned(), (*value).to_owned()))
        })
        .collect::<Result<HashMap<String, String>>>()?;
    let mut written = HashSet::new();
    let mut lines = Vec::new();

    for line in contents.lines() {
        if let Some(key) = parse_env_line_key(line)?
            && let Some(value) = updates.get(&key)
        {
            lines.push(format!("{key}={}", quote_env_value(value)));
            written.insert(key);
            continue;
        }
        lines.push(line.to_owned());
    }

    for (key, value) in values {
        if !written.contains(*key) {
            lines.push(format!("{key}={}", quote_env_value(value)));
        }
    }

    let mut output = lines.join("\n");
    output.push('\n');
    Ok(output)
}

fn parse_env_line(line: &str) -> Result<Option<(String, String)>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }
    let assignment = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let Some((raw_key, raw_value)) = assignment.split_once('=') else {
        return Ok(None);
    };
    let key = raw_key.trim().to_owned();
    validate_env_key(&key)?;
    let value = parse_env_value(raw_value.trim())?;
    Ok(Some((key, value)))
}

fn parse_env_line_key(line: &str) -> Result<Option<String>> {
    Ok(parse_env_line(line)?.map(|(key, _)| key))
}

fn parse_env_value(value: &str) -> Result<String> {
    if let Some(stripped) = value.strip_prefix('"') {
        let Some(end) = stripped.rfind('"') else {
            return Err(anyhow!("unterminated double-quoted value"));
        };
        return Ok(stripped[..end].replace("\\\"", "\"").replace("\\\\", "\\"));
    }
    if let Some(stripped) = value.strip_prefix('\'') {
        let Some(end) = stripped.rfind('\'') else {
            return Err(anyhow!("unterminated single-quoted value"));
        };
        return Ok(stripped[..end].replace("'\\''", "'"));
    }
    Ok(value
        .split_once('#')
        .map(|(before_comment, _)| before_comment)
        .unwrap_or(value)
        .trim()
        .to_owned())
}

fn validate_env_key(key: &str) -> Result<()> {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return Err(anyhow!("environment variable name cannot be empty"));
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(anyhow!("invalid environment variable name `{key}`"));
    }
    if !chars.all(|c| c == '_' || c.is_ascii_alphanumeric()) {
        return Err(anyhow!("invalid environment variable name `{key}`"));
    }
    Ok(())
}

fn quote_env_value(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to set permissions on {}", path.display()))
}

#[cfg(unix)]
fn set_private_file_permissions(file: &fs::File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(fs::Permissions::from_mode(0o600))
        .context("failed to set config file permissions")
}

#[cfg(test)]
mod tests {
    use super::{parse_env_config, update_env_config_contents};

    #[test]
    fn parses_env_config_values() {
        let parsed = parse_env_config(
            r#"
# comment
VCC_API_KEY='key one'
export VOLVO_CLIENT_ID=client-a
VOLVO_CLIENT_SECRET="secret-a"
"#,
        )
        .expect("config should parse");

        assert!(
            parsed
                .iter()
                .any(|(key, value)| key == "VCC_API_KEY" && value == "key one")
        );
        assert!(
            parsed
                .iter()
                .any(|(key, value)| key == "VOLVO_CLIENT_ID" && value == "client-a")
        );
        assert!(
            parsed
                .iter()
                .any(|(key, value)| key == "VOLVO_CLIENT_SECRET" && value == "secret-a")
        );
    }

    #[test]
    fn updates_existing_env_config_values() {
        let updated = update_env_config_contents(
            "# existing\nVCC_API_KEY='old'\nOTHER=value\n",
            &[("VCC_API_KEY", "new key"), ("VOLVO_CLIENT_ID", "client'a")],
        )
        .expect("config should update");

        assert!(updated.contains("VCC_API_KEY='new key'"));
        assert!(updated.contains("OTHER=value"));
        assert!(updated.contains("VOLVO_CLIENT_ID='client'\\''a'"));
    }
}
