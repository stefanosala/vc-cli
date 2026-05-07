use anyhow::{Result, anyhow};

use crate::commands::vehicle_shared::{VehicleVinOutput, build_request_context, resolve_vehicle};
use crate::store::sqlite::{Profile, Store};

#[derive(Debug, Clone)]
pub struct LocationGetArgs {
    pub vin: Option<String>,
    pub api_key: Option<String>,
}

pub type LocationGetOutput = VehicleVinOutput;

pub async fn execute(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: LocationGetArgs,
) -> Result<LocationGetOutput> {
    let context = build_request_context(store, profile, base_url, args.api_key).await?;
    let vin = resolve_vehicle(store, profile, &context, args.vin).await?;
    let data = context
        .client
        .get_vehicle_location(&vin, &context.access_token)
        .await
        .map_err(|err| anyhow!("failed to fetch latest location for VIN `{vin}`: {err:#}"))?;

    Ok(LocationGetOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: context.client.base_url().to_owned(),
        vin,
        data,
    })
}
