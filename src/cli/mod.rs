use anyhow::{Result, anyhow};
use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

use crate::commands::{
    auth_login, auth_logout, auth_token_set, auth_whoami, energy_get, location_get,
    vehicle_commands, vehicle_get, vehicle_shared, vehicle_vin, vehicle_windows_get,
};
use crate::config::{DEFAULT_API_HOST, DEFAULT_PROFILE_NAME, resolve_config_dir};
use crate::http::normalize_base_url;
use crate::store::sqlite::{Profile, Store};

#[derive(Debug, Parser)]
#[command(name = "vc-cli", about = "Volvo Cars CLI", version)]
struct Cli {
    #[arg(long, global = true, env = "VOLVO_API_HOST")]
    api_host: Option<String>,

    #[arg(long, global = true)]
    profile: Option<String>,

    #[command(subcommand)]
    command: TopLevelCommand,
}

#[derive(Debug, Subcommand)]
enum TopLevelCommand {
    Auth(AuthCommand),
    Energy(EnergyCommand),
    Location(LocationCommand),
    Vehicle(VehicleCommand),
}

#[derive(Debug, Args)]
struct AuthCommand {
    #[command(subcommand)]
    command: AuthSubcommand,
}

#[derive(Debug, Subcommand)]
enum AuthSubcommand {
    Login(AuthLoginCliArgs),
    TokenSet(AuthTokenSetCliArgs),
    Whoami,
    Logout,
}

#[derive(Debug, Args)]
struct EnergyCommand {
    #[command(subcommand)]
    command: EnergySubcommand,
}

#[derive(Debug, Subcommand)]
enum EnergySubcommand {
    State(EnergyStateCommand),
    Capabilities(EnergyCapabilitiesCommand),
}

#[derive(Debug, Args)]
struct EnergyStateCommand {
    #[command(subcommand)]
    command: EnergyStateSubcommand,
}

#[derive(Debug, Subcommand)]
enum EnergyStateSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct EnergyCapabilitiesCommand {
    #[command(subcommand)]
    command: EnergyCapabilitiesSubcommand,
}

#[derive(Debug, Subcommand)]
enum EnergyCapabilitiesSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct LocationCommand {
    #[command(subcommand)]
    command: LocationSubcommand,
}

#[derive(Debug, Subcommand)]
enum LocationSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct AuthLoginCliArgs {
    #[arg(long, env = "VOLVO_SCOPES", default_value = crate::config::DEFAULT_SCOPES)]
    scopes: String,

    #[arg(long, env = "VOLVO_AUTH_ISSUER", default_value = crate::config::DEFAULT_AUTH_ISSUER)]
    auth_issuer: Option<String>,

    #[arg(long)]
    client_id: Option<String>,

    #[arg(long)]
    client_secret: Option<String>,

    #[arg(long, env = "VOLVO_REDIRECT_URI", default_value = crate::config::DEFAULT_AUTH_REDIRECT_URI)]
    redirect_uri: Option<String>,

    #[arg(
        long,
        env = "VOLVO_AUTH_LISTEN_TIMEOUT_SECONDS",
        default_value_t = crate::config::DEFAULT_AUTH_LISTEN_TIMEOUT_SECONDS
    )]
    auth_listen_timeout_seconds: u64,

    #[arg(long)]
    headless: bool,
}

#[derive(Debug, Args)]
struct AuthTokenSetCliArgs {
    #[arg(long, env = "VOLVO_ACCESS_TOKEN")]
    access_token: String,

    #[arg(long, env = "VOLVO_REFRESH_TOKEN")]
    refresh_token: Option<String>,

    #[arg(long)]
    expires_in: Option<u64>,

    #[arg(long)]
    scope: Option<String>,

    #[arg(long, default_value = "Bearer")]
    token_type: String,

    #[arg(long, env = "VOLVO_TOKEN_ENDPOINT")]
    token_endpoint: Option<String>,
}

#[derive(Debug, Args)]
struct VehicleCommand {
    #[command(subcommand)]
    command: VehicleSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleSubcommand {
    List(VehicleListCliArgs),
    Details(VehicleDetailsCommand),
    Windows(VehicleWindowsCommand),
    Doors(VehicleDoorsCommand),
    Warnings(VehicleWarningsCommand),
    Tyres(VehicleTyresCommand),
    Statistics(VehicleStatisticsCommand),
    Odometer(VehicleOdometerCommand),
    Fuel(VehicleFuelCommand),
    Diagnostics(VehicleDiagnosticsCommand),
    Engine(VehicleEngineCommand),
    Brakes(VehicleBrakesCommand),
    Commands(VehicleCommandsCliCommand),
    Vin(VehicleVinCommand),
}

#[derive(Debug, Args)]
struct VehicleWindowsCommand {
    #[command(subcommand)]
    command: VehicleWindowsSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleWindowsSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleListCliArgs {
    #[arg(long)]
    api_key: Option<String>,
}

#[derive(Debug, Args)]
struct VehicleVinApiCliArgs {
    #[arg(long)]
    vin: Option<String>,

    #[arg(long)]
    api_key: Option<String>,
}

#[derive(Debug, Args)]
struct VehicleDetailsCommand {
    #[command(subcommand)]
    command: VehicleDetailsSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleDetailsSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleDoorsCommand {
    #[command(subcommand)]
    command: VehicleDoorsSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleDoorsSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleWarningsCommand {
    #[command(subcommand)]
    command: VehicleWarningsSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleWarningsSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleTyresCommand {
    #[command(subcommand)]
    command: VehicleTyresSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleTyresSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleStatisticsCommand {
    #[command(subcommand)]
    command: VehicleStatisticsSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleStatisticsSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleOdometerCommand {
    #[command(subcommand)]
    command: VehicleOdometerSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleOdometerSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleFuelCommand {
    #[command(subcommand)]
    command: VehicleFuelSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleFuelSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleDiagnosticsCommand {
    #[command(subcommand)]
    command: VehicleDiagnosticsSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleDiagnosticsSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleEngineCommand {
    #[command(subcommand)]
    command: VehicleEngineSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleEngineSubcommand {
    Diagnostics(VehicleEngineDiagnosticsCommand),
    Status(VehicleEngineStatusCommand),
}

#[derive(Debug, Args)]
struct VehicleEngineDiagnosticsCommand {
    #[command(subcommand)]
    command: VehicleEngineDiagnosticsSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleEngineDiagnosticsSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleEngineStatusCommand {
    #[command(subcommand)]
    command: VehicleEngineStatusSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleEngineStatusSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleBrakesCommand {
    #[command(subcommand)]
    command: VehicleBrakesSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleBrakesSubcommand {
    Get(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleCommandsCliCommand {
    #[command(subcommand)]
    command: VehicleCommandsSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleCommandsSubcommand {
    List(VehicleVinApiCliArgs),
    Accessibility(VehicleVinApiCliArgs),
    Unlock(VehicleVinApiCliArgs),
    Lock(VehicleVinApiCliArgs),
    LockReducedGuard(VehicleVinApiCliArgs),
    Honk(VehicleVinApiCliArgs),
    Flash(VehicleVinApiCliArgs),
    HonkFlash(VehicleVinApiCliArgs),
    EngineStart(VehicleCommandsEngineStartCliArgs),
    EngineStop(VehicleVinApiCliArgs),
    ClimatizationStart(VehicleVinApiCliArgs),
    ClimatizationStop(VehicleVinApiCliArgs),
}

#[derive(Debug, Args)]
struct VehicleCommandsEngineStartCliArgs {
    #[command(flatten)]
    request: VehicleVinApiCliArgs,

    #[arg(long)]
    runtime_minutes: i32,
}

#[derive(Debug, Args)]
struct VehicleVinCommand {
    #[command(subcommand)]
    command: VehicleVinSubcommand,
}

#[derive(Debug, Subcommand)]
enum VehicleVinSubcommand {
    Default(VehicleVinDefaultCliArgs),
}

#[derive(Debug, Args)]
struct VehicleVinDefaultCliArgs {
    #[arg(long)]
    vin: String,

    #[arg(long)]
    api_key: Option<String>,
}

pub async fn run() -> Result<()> {
    let config_dir = resolve_config_dir()?;
    run_with_config_dir(config_dir).await
}

pub async fn run_with_config_dir(config_dir: PathBuf) -> Result<()> {
    let cli = Cli::parse();
    let store_path = config_dir.join("state.db");
    let store = Store::open(&store_path)?;

    let profile_name = cli
        .profile
        .unwrap_or_else(|| DEFAULT_PROFILE_NAME.to_owned());
    let profile = store.get_or_create_profile(&profile_name, DEFAULT_API_HOST)?;
    store.set_active_profile(&profile.name)?;
    let profile = store
        .get_active_profile()?
        .ok_or_else(|| anyhow!("failed to resolve active profile"))?;

    let base_url = resolve_base_url(cli.api_host.as_deref(), &profile)?;

    match cli.command {
        TopLevelCommand::Auth(args) => match args.command {
            AuthSubcommand::Login(login) => {
                let output = auth_login::execute(
                    &store,
                    &profile,
                    &base_url,
                    &config_dir,
                    auth_login::AuthLoginArgs {
                        scopes: login.scopes,
                        auth_issuer: login.auth_issuer,
                        client_id: login.client_id,
                        client_secret: login.client_secret,
                        redirect_uri: login.redirect_uri,
                        auth_listen_timeout_seconds: login.auth_listen_timeout_seconds,
                        headless: login.headless,
                    },
                )
                .await?;
                print_json(&output)?;
            }
            AuthSubcommand::TokenSet(token_set) => {
                let output = auth_token_set::execute(
                    &store,
                    &profile,
                    &base_url,
                    auth_token_set::AuthTokenSetArgs {
                        access_token: token_set.access_token,
                        refresh_token: token_set.refresh_token,
                        expires_in: token_set.expires_in,
                        scope: token_set.scope,
                        token_type: Some(token_set.token_type),
                        token_endpoint: token_set.token_endpoint,
                    },
                )?;
                print_json(&output)?;
            }
            AuthSubcommand::Whoami => {
                let output = auth_whoami::execute(&store, &profile, &base_url)?;
                print_json(&output)?;
            }
            AuthSubcommand::Logout => {
                let output = auth_logout::execute(&store, &profile, &base_url)?;
                print_json(&output)?;
            }
        },
        TopLevelCommand::Energy(args) => match args.command {
            EnergySubcommand::State(state) => match state.command {
                EnergyStateSubcommand::Get(get_args) => {
                    let output = energy_get::execute(
                        &store,
                        &profile,
                        &base_url,
                        energy_get::EnergyGetArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        energy_get::EnergyReadEndpoint::State,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            EnergySubcommand::Capabilities(capabilities) => match capabilities.command {
                EnergyCapabilitiesSubcommand::Get(get_args) => {
                    let output = energy_get::execute(
                        &store,
                        &profile,
                        &base_url,
                        energy_get::EnergyGetArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        energy_get::EnergyReadEndpoint::Capabilities,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
        },
        TopLevelCommand::Location(args) => match args.command {
            LocationSubcommand::Get(get_args) => {
                let output = location_get::execute(
                    &store,
                    &profile,
                    &base_url,
                    location_get::LocationGetArgs {
                        vin: get_args.vin,
                        api_key: get_args.api_key,
                    },
                )
                .await?;
                print_json(&output)?;
            }
        },
        TopLevelCommand::Vehicle(args) => match args.command {
            VehicleSubcommand::List(list_args) => {
                let output = vehicle_get::execute_vehicle_list(
                    &store,
                    &profile,
                    &base_url,
                    vehicle_shared::VehicleApiArgs {
                        api_key: list_args.api_key,
                    },
                )
                .await?;
                print_json(&output)?;
            }
            VehicleSubcommand::Details(details) => match details.command {
                VehicleDetailsSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Details,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Windows(windows) => match windows.command {
                VehicleWindowsSubcommand::Get(get_args) => {
                    let output = vehicle_windows_get::execute(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_windows_get::VehicleWindowsGetArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Doors(doors) => match doors.command {
                VehicleDoorsSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Doors,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Warnings(warnings) => match warnings.command {
                VehicleWarningsSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Warnings,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Tyres(tyres) => match tyres.command {
                VehicleTyresSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Tyres,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Statistics(statistics) => match statistics.command {
                VehicleStatisticsSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Statistics,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Odometer(odometer) => match odometer.command {
                VehicleOdometerSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Odometer,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Fuel(fuel) => match fuel.command {
                VehicleFuelSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Fuel,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Diagnostics(diagnostics) => match diagnostics.command {
                VehicleDiagnosticsSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Diagnostics,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Engine(engine) => match engine.command {
                VehicleEngineSubcommand::Diagnostics(diagnostics) => match diagnostics.command {
                    VehicleEngineDiagnosticsSubcommand::Get(get_args) => {
                        let output = vehicle_get::execute_vehicle_read(
                            &store,
                            &profile,
                            &base_url,
                            vehicle_shared::VehicleVinApiArgs {
                                vin: get_args.vin,
                                api_key: get_args.api_key,
                            },
                            vehicle_get::VehicleReadEndpoint::EngineDiagnostics,
                        )
                        .await?;
                        print_json(&output)?;
                    }
                },
                VehicleEngineSubcommand::Status(status) => match status.command {
                    VehicleEngineStatusSubcommand::Get(get_args) => {
                        let output = vehicle_get::execute_vehicle_read(
                            &store,
                            &profile,
                            &base_url,
                            vehicle_shared::VehicleVinApiArgs {
                                vin: get_args.vin,
                                api_key: get_args.api_key,
                            },
                            vehicle_get::VehicleReadEndpoint::EngineStatus,
                        )
                        .await?;
                        print_json(&output)?;
                    }
                },
            },
            VehicleSubcommand::Brakes(brakes) => match brakes.command {
                VehicleBrakesSubcommand::Get(get_args) => {
                    let output = vehicle_get::execute_vehicle_read(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: get_args.vin,
                            api_key: get_args.api_key,
                        },
                        vehicle_get::VehicleReadEndpoint::Brakes,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Commands(commands) => match commands.command {
                VehicleCommandsSubcommand::List(list_args) => {
                    let output = vehicle_commands::execute_query(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: list_args.vin,
                            api_key: list_args.api_key,
                        },
                        vehicle_commands::VehicleCommandsQueryEndpoint::List,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::Accessibility(accessibility_args) => {
                    let output = vehicle_commands::execute_query(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: accessibility_args.vin,
                            api_key: accessibility_args.api_key,
                        },
                        vehicle_commands::VehicleCommandsQueryEndpoint::Accessibility,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::Unlock(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::Unlock,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::Lock(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::Lock,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::LockReducedGuard(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::LockReducedGuard,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::Honk(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::Honk,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::Flash(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::Flash,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::HonkFlash(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::HonkFlash,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::EngineStart(command_args) => {
                    let output = vehicle_commands::execute_engine_start(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.request.vin,
                            api_key: command_args.request.api_key,
                        },
                        command_args.runtime_minutes,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::EngineStop(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::EngineStop,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::ClimatizationStart(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::ClimatizationStart,
                    )
                    .await?;
                    print_json(&output)?;
                }
                VehicleCommandsSubcommand::ClimatizationStop(command_args) => {
                    let output = vehicle_commands::execute_invoke(
                        &store,
                        &profile,
                        &base_url,
                        vehicle_shared::VehicleVinApiArgs {
                            vin: command_args.vin,
                            api_key: command_args.api_key,
                        },
                        vehicle_commands::VehicleInvokeEndpoint::ClimatizationStop,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
            VehicleSubcommand::Vin(vin) => match vin.command {
                VehicleVinSubcommand::Default(default_args) => {
                    let output = vehicle_vin::set_default(
                        &store,
                        &profile,
                        &base_url,
                        default_args.api_key,
                        &default_args.vin,
                    )
                    .await?;
                    print_json(&output)?;
                }
            },
        },
    }

    Ok(())
}

fn resolve_base_url(api_host_override: Option<&str>, profile: &Profile) -> Result<String> {
    if let Some(host) = api_host_override {
        return normalize_base_url(host);
    }
    normalize_base_url(&profile.base_url)
}

fn print_json<T: Serialize>(payload: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(payload)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn windows_get_parses_without_vin() {
        let parsed = Cli::try_parse_from(["vc-cli", "vehicle", "windows", "get"]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn vehicle_list_parses_with_api_key() {
        let parsed = Cli::try_parse_from(["vc-cli", "vehicle", "list", "--api-key", "key"]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn details_get_parses_without_vin() {
        let parsed = Cli::try_parse_from(["vc-cli", "vehicle", "details", "get"]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn commands_tree_parses() {
        let list = Cli::try_parse_from(["vc-cli", "vehicle", "commands", "list"]);
        let accessibility = Cli::try_parse_from(["vc-cli", "vehicle", "commands", "accessibility"]);
        let unlock = Cli::try_parse_from(["vc-cli", "vehicle", "commands", "unlock"]);
        let engine_start = Cli::try_parse_from([
            "vc-cli",
            "vehicle",
            "commands",
            "engine-start",
            "--runtime-minutes",
            "5",
        ]);

        assert!(list.is_ok());
        assert!(accessibility.is_ok());
        assert!(unlock.is_ok());
        assert!(engine_start.is_ok());
    }

    #[test]
    fn auth_token_set_parses_required_access_token() {
        let parsed =
            Cli::try_parse_from(["vc-cli", "auth", "token-set", "--access-token", "abc123"]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn auth_login_parses_with_client_credentials() {
        let parsed = Cli::try_parse_from([
            "vc-cli",
            "auth",
            "login",
            "--client-id",
            "client-a",
            "--client-secret",
            "secret-a",
        ]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn auth_login_parses_with_default_issuer_and_redirect() {
        let parsed = Cli::try_parse_from(["vc-cli", "auth", "login"]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn auth_login_parses_with_custom_redirect_uri() {
        let parsed = Cli::try_parse_from([
            "vc-cli",
            "auth",
            "login",
            "--redirect-uri",
            "http://localtest.me:1410/callback",
        ]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn auth_login_parses_headless_mode() {
        let parsed = Cli::try_parse_from(["vc-cli", "auth", "login", "--headless"]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn energy_tree_parses() {
        let state = Cli::try_parse_from(["vc-cli", "energy", "state", "get"]);
        let capabilities = Cli::try_parse_from(["vc-cli", "energy", "capabilities", "get"]);
        assert!(state.is_ok());
        assert!(capabilities.is_ok());
    }

    #[test]
    fn location_tree_parses() {
        let get = Cli::try_parse_from(["vc-cli", "location", "get"]);
        assert!(get.is_ok());
    }

    #[test]
    fn vehicle_vin_subcommands_parse() {
        let list = Cli::try_parse_from(["vc-cli", "vehicle", "vin", "list"]);
        let add = Cli::try_parse_from(["vc-cli", "vehicle", "vin", "add", "--vin", "VIN1"]);
        let default = Cli::try_parse_from(["vc-cli", "vehicle", "vin", "default", "--vin", "VIN1"]);
        assert!(list.is_err());
        assert!(add.is_err());
        assert!(default.is_ok());
    }
}
