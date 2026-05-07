use anyhow::Result;

use crate::commands::vehicle_get::{VehicleReadEndpoint, execute_vehicle_read};
use crate::commands::vehicle_shared::{VehicleVinApiArgs, VehicleVinOutput};
use crate::store::sqlite::{Profile, Store};

#[derive(Debug, Clone)]
pub struct VehicleWindowsGetArgs {
    pub vin: Option<String>,
    pub api_key: Option<String>,
}

pub type VehicleWindowsGetOutput = VehicleVinOutput;

pub async fn execute(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: VehicleWindowsGetArgs,
) -> Result<VehicleWindowsGetOutput> {
    execute_vehicle_read(
        store,
        profile,
        base_url,
        VehicleVinApiArgs {
            vin: args.vin,
            api_key: args.api_key,
        },
        VehicleReadEndpoint::Windows,
    )
    .await
}
