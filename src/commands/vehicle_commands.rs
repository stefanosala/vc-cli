use anyhow::{Result, anyhow};
use serde_json::json;

use crate::commands::vehicle_shared::{
    VehicleVinApiArgs, VehicleVinOutput, build_request_context, resolve_vin,
};
use crate::store::sqlite::{Profile, Store};

#[derive(Debug, Clone, Copy)]
pub enum VehicleCommandsQueryEndpoint {
    List,
    Accessibility,
}

#[derive(Debug, Clone, Copy)]
pub enum VehicleInvokeEndpoint {
    Unlock,
    Lock,
    LockReducedGuard,
    Honk,
    Flash,
    HonkFlash,
    EngineStop,
    ClimatizationStart,
    ClimatizationStop,
}

pub async fn execute_query(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: VehicleVinApiArgs,
    endpoint: VehicleCommandsQueryEndpoint,
) -> Result<VehicleVinOutput> {
    let vin = resolve_vin(store, profile, args.vin)?;
    let context = build_request_context(store, profile, base_url, args.api_key).await?;
    let data = match endpoint {
        VehicleCommandsQueryEndpoint::List => {
            context
                .client
                .get_command_list(&vin, &context.access_token)
                .await
        }
        VehicleCommandsQueryEndpoint::Accessibility => {
            context
                .client
                .get_commands_accessibility(&vin, &context.access_token)
                .await
        }
    }
    .map_err(|err| anyhow!("failed to fetch commands metadata for VIN `{vin}`: {err:#}"))?;

    Ok(VehicleVinOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: context.client.base_url().to_owned(),
        vin,
        data,
    })
}

pub async fn execute_invoke(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: VehicleVinApiArgs,
    endpoint: VehicleInvokeEndpoint,
) -> Result<VehicleVinOutput> {
    let vin = resolve_vin(store, profile, args.vin)?;
    let context = build_request_context(store, profile, base_url, args.api_key).await?;
    let data = match endpoint {
        VehicleInvokeEndpoint::Unlock => {
            context
                .client
                .invoke_unlock(&vin, &context.access_token)
                .await
        }
        VehicleInvokeEndpoint::Lock => {
            context
                .client
                .invoke_lock(&vin, &context.access_token)
                .await
        }
        VehicleInvokeEndpoint::LockReducedGuard => {
            context
                .client
                .invoke_lock_reduced_guard(&vin, &context.access_token)
                .await
        }
        VehicleInvokeEndpoint::Honk => {
            context
                .client
                .invoke_honk(&vin, &context.access_token)
                .await
        }
        VehicleInvokeEndpoint::Flash => {
            context
                .client
                .invoke_flash(&vin, &context.access_token)
                .await
        }
        VehicleInvokeEndpoint::HonkFlash => {
            context
                .client
                .invoke_honk_flash(&vin, &context.access_token)
                .await
        }
        VehicleInvokeEndpoint::EngineStop => {
            context
                .client
                .invoke_engine_stop(&vin, &context.access_token)
                .await
        }
        VehicleInvokeEndpoint::ClimatizationStart => {
            context
                .client
                .invoke_climatization_start(&vin, &context.access_token)
                .await
        }
        VehicleInvokeEndpoint::ClimatizationStop => {
            context
                .client
                .invoke_climatization_stop(&vin, &context.access_token)
                .await
        }
    }
    .map_err(|err| anyhow!("failed to invoke vehicle command for VIN `{vin}`: {err:#}"))?;

    Ok(VehicleVinOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: context.client.base_url().to_owned(),
        vin,
        data,
    })
}

pub async fn execute_engine_start(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: VehicleVinApiArgs,
    runtime_minutes: i32,
) -> Result<VehicleVinOutput> {
    if !(0..=15).contains(&runtime_minutes) {
        return Err(anyhow!(
            "runtime minutes must be between 0 and 15 (received {runtime_minutes})"
        ));
    }

    let vin = resolve_vin(store, profile, args.vin)?;
    let context = build_request_context(store, profile, base_url, args.api_key).await?;
    let data = context
        .client
        .invoke_engine_start(
            &vin,
            &context.access_token,
            json!({ "runtimeMinutes": runtime_minutes }),
        )
        .await
        .map_err(|err| anyhow!("failed to invoke engine start for VIN `{vin}`: {err:#}"))?;

    Ok(VehicleVinOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: context.client.base_url().to_owned(),
        vin,
        data,
    })
}
