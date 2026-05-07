use anyhow::{Result, anyhow};
use std::path::PathBuf;

pub const DEFAULT_PROFILE_NAME: &str = "default";
pub const DEFAULT_API_HOST: &str = "https://api.volvocars.com";
pub const DEFAULT_AUTH_ISSUER: &str = "https://volvoid.eu.volvocars.com";
pub const DEFAULT_AUTH_BRIDGE_URL: &str = "https://vc-cli.com";
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

pub fn resolve_config_dir() -> Result<PathBuf> {
    if let Ok(override_dir) = std::env::var("VOLVO_CONFIG_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    let base = dirs::config_dir().ok_or_else(|| anyhow!("failed to resolve config directory"))?;
    Ok(base.join("vc-cli"))
}
