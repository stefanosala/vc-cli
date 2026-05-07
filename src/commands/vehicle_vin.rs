use anyhow::Result;
use serde::Serialize;

use crate::store::sqlite::{Profile, Store, StoredVin};

#[derive(Debug, Serialize)]
pub struct VehicleVinListOutput {
    pub ok: bool,
    pub profile: String,
    pub vins: Vec<StoredVin>,
}

#[derive(Debug, Serialize)]
pub struct VehicleVinMutateOutput {
    pub ok: bool,
    pub profile: String,
    pub default_vin: Option<String>,
    pub vins: Vec<StoredVin>,
}

pub fn list(store: &Store, profile: &Profile) -> Result<VehicleVinListOutput> {
    Ok(VehicleVinListOutput {
        ok: true,
        profile: profile.name.clone(),
        vins: store.list_vins(profile.id)?,
    })
}

pub fn add(
    store: &Store,
    profile: &Profile,
    vin: &str,
    set_default: bool,
) -> Result<VehicleVinMutateOutput> {
    store.upsert_vin(profile.id, vin, set_default)?;
    if set_default {
        store.set_default_vin(profile.id, vin)?;
    }
    response_after_mutation(store, profile)
}

pub fn set_default(store: &Store, profile: &Profile, vin: &str) -> Result<VehicleVinMutateOutput> {
    store.set_default_vin(profile.id, vin)?;
    response_after_mutation(store, profile)
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
