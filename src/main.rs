fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let config_dir = vc_cli::config::resolve_config_dir()?;
    vc_cli::config::load_config_env(&config_dir)?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(vc_cli::cli::run_with_config_dir(config_dir))
}
