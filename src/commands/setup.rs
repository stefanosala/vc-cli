use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use std::io::{self, Write};

use crate::store::sqlite::{Profile, Store, StoredVin};

#[derive(Debug, Clone)]
pub struct SetupArgs {
    pub api_host: Option<String>,
    pub vins: Vec<String>,
    pub default_vin: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SetupOutput {
    pub ok: bool,
    pub profile: String,
    pub base_url: String,
    pub default_vin: Option<String>,
    pub vins: Vec<StoredVin>,
}

pub fn execute(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: SetupArgs,
) -> Result<SetupOutput> {
    if let Some(new_base_url) = args.api_host.as_deref() {
        store.set_profile_base_url(profile.id, new_base_url)?;
    }

    let mut vins = args.vins;
    if vins.is_empty()
        && let Some(prompted) = prompt_for_vins()?
    {
        vins = prompted;
    }

    let mut normalized_vins = Vec::new();
    for vin in vins {
        let normalized = vin.trim().to_uppercase();
        if !normalized.is_empty() && !normalized_vins.contains(&normalized) {
            normalized_vins.push(normalized);
        }
    }

    if !normalized_vins.is_empty() {
        let default_vin = args
            .default_vin
            .map(|vin| vin.trim().to_uppercase())
            .or_else(|| normalized_vins.first().cloned())
            .ok_or_else(|| anyhow!("failed to resolve default VIN from setup input"))?;
        for vin in &normalized_vins {
            store.upsert_vin(profile.id, vin, vin == &default_vin)?;
        }
        store.set_default_vin(profile.id, &default_vin)?;
    } else if let Some(default_vin) = args.default_vin {
        store.set_default_vin(profile.id, &default_vin)?;
    }

    let vins = store.list_vins(profile.id)?;
    let default_vin = vins
        .iter()
        .find(|vin| vin.is_default)
        .map(|vin| vin.vin.clone());
    Ok(SetupOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: base_url.to_owned(),
        default_vin,
        vins,
    })
}

fn prompt_for_vins() -> Result<Option<Vec<String>>> {
    eprint!("Enter VINs (comma-separated, leave blank to skip): ");
    io::stderr()
        .flush()
        .context("failed to flush prompt to stderr")?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed reading VIN input from stdin")?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let values = trimmed
        .split(',')
        .map(str::trim)
        .filter(|vin| !vin.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    Ok(Some(values))
}
