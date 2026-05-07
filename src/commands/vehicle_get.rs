use anyhow::{Result, anyhow};

use crate::commands::vehicle_shared::{
    VehicleApiArgs, VehicleOutput, VehicleVinApiArgs, VehicleVinOutput, build_request_context,
    resolve_vehicle, vehicle_list_output,
};
use crate::store::sqlite::{Profile, Store};

#[derive(Debug, Clone, Copy)]
pub enum VehicleReadEndpoint {
    Details,
    Windows,
    Doors,
    Warnings,
    Tyres,
    Statistics,
    Odometer,
    Fuel,
    Diagnostics,
    EngineDiagnostics,
    EngineStatus,
    Brakes,
}

impl VehicleReadEndpoint {
    fn label(self) -> &'static str {
        match self {
            Self::Details => "vehicle details",
            Self::Windows => "windows status",
            Self::Doors => "doors and lock status",
            Self::Warnings => "warnings",
            Self::Tyres => "tyre pressure values",
            Self::Statistics => "statistics",
            Self::Odometer => "odometer",
            Self::Fuel => "fuel amount",
            Self::Diagnostics => "diagnostics",
            Self::EngineDiagnostics => "engine diagnostics",
            Self::EngineStatus => "engine status",
            Self::Brakes => "brake status",
        }
    }
}

pub async fn execute_vehicle_list(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: VehicleApiArgs,
) -> Result<VehicleOutput> {
    let context = build_request_context(store, profile, base_url, args.api_key).await?;
    let data = context
        .client
        .get_vehicle_list(&context.access_token)
        .await
        .map_err(|err| anyhow!("failed to fetch vehicle list: {err:#}"))?;

    Ok(vehicle_list_output(profile, &context, data))
}

pub async fn execute_vehicle_read(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: VehicleVinApiArgs,
    endpoint: VehicleReadEndpoint,
) -> Result<VehicleVinOutput> {
    let context = build_request_context(store, profile, base_url, args.api_key).await?;
    let vin = resolve_vehicle(store, profile, &context, args.vin).await?;
    let data = match endpoint {
        VehicleReadEndpoint::Details => {
            context
                .client
                .get_vehicle_details(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Windows => {
            context
                .client
                .get_windows_status(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Doors => {
            context
                .client
                .get_doors_status(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Warnings => {
            context
                .client
                .get_warnings(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Tyres => {
            context
                .client
                .get_tyre_pressure_values(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Statistics => {
            context
                .client
                .get_statistics(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Odometer => {
            context
                .client
                .get_odometer(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Fuel => {
            context
                .client
                .get_fuel_amount(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Diagnostics => {
            context
                .client
                .get_diagnostics(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::EngineDiagnostics => {
            context
                .client
                .get_engine_diagnostics(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::EngineStatus => {
            context
                .client
                .get_engine_status(&vin, &context.access_token)
                .await
        }
        VehicleReadEndpoint::Brakes => {
            context
                .client
                .get_brake_status(&vin, &context.access_token)
                .await
        }
    }
    .map_err(|err| {
        anyhow!(
            "failed to fetch {} for VIN `{vin}`: {err:#}",
            endpoint.label()
        )
    })?;

    Ok(VehicleVinOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: context.client.base_url().to_owned(),
        vin,
        data,
    })
}
