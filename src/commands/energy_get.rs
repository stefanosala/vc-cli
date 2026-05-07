use anyhow::{Result, anyhow};

use crate::commands::vehicle_shared::{VehicleVinOutput, build_request_context, resolve_vin};
use crate::store::sqlite::{Profile, Store};

#[derive(Debug, Clone)]
pub struct EnergyGetArgs {
    pub vin: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum EnergyReadEndpoint {
    State,
    Capabilities,
}

impl EnergyReadEndpoint {
    fn label(self) -> &'static str {
        match self {
            Self::State => "energy state",
            Self::Capabilities => "energy capabilities",
        }
    }
}

pub type EnergyGetOutput = VehicleVinOutput;

pub async fn execute(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: EnergyGetArgs,
    endpoint: EnergyReadEndpoint,
) -> Result<EnergyGetOutput> {
    let vin = resolve_vin(store, profile, args.vin)?;
    let context = build_request_context(store, profile, base_url, args.api_key).await?;
    let data = match endpoint {
        EnergyReadEndpoint::State => {
            context
                .client
                .get_energy_state(&vin, &context.access_token)
                .await
        }
        EnergyReadEndpoint::Capabilities => {
            context
                .client
                .get_energy_capabilities(&vin, &context.access_token)
                .await
        }
    }
    .map_err(|err| {
        anyhow!(
            "failed to fetch {} for VIN `{vin}`: {err:#}",
            endpoint.label()
        )
    })?;

    Ok(EnergyGetOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: context.client.base_url().to_owned(),
        vin,
        data,
    })
}
