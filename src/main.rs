#[tokio::main]
async fn main() {
    if let Err(err) = vc_cli::cli::run().await {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
