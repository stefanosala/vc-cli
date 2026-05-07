use anyhow::Result;
use serde::Serialize;

use crate::commands::vehicle_shared::{build_request_context, extract_vehicle_vins};
use crate::store::sqlite::{Profile, Store, StoredVin};

#[derive(Debug, Serialize)]
pub struct VehicleVinMutateOutput {
    pub ok: bool,
    pub profile: String,
    pub default_vin: Option<String>,
    pub vins: Vec<StoredVin>,
}

pub async fn set_default(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    api_key: Option<String>,
    vin: &str,
) -> Result<VehicleVinMutateOutput> {
    sync_available_vins(store, profile, base_url, api_key, Some(vin)).await?;
    response_after_mutation(store, profile)
}

async fn sync_available_vins(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    api_key: Option<String>,
    default_vin: Option<&str>,
) -> Result<()> {
    let context = build_request_context(store, profile, base_url, api_key).await?;
    let data = context
        .client
        .get_vehicle_list(&context.access_token)
        .await?;
    let vins = extract_vehicle_vins(&data);
    store.sync_vins(profile.id, &vins, default_vin)
}

fn response_after_mutation(store: &Store, profile: &Profile) -> Result<VehicleVinMutateOutput> {
    let vins = store.list_vins(profile.id)?;
    let default_vin = vins
        .iter()
        .find(|item| item.is_default)
        .map(|item| item.vin.clone());
    Ok(VehicleVinMutateOutput {
        ok: true,
        profile: profile.name.clone(),
        default_vin,
        vins,
    })
}
