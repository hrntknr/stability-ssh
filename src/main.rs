use clap::{Parser, Subcommand};
use stablessh::{client, ctl, server};

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Server(server::Opt),
    Client(client::Opt),
    Ctl(ctl::Opt),
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = Cli::parse();
    match args.command {
        Commands::Server(opt) => match server::run(opt).await {
            Ok(_) => {}
            Err(e) => log::error!("{:?}", e),
        },
        Commands::Client(opt) => match client::run(opt).await {
            Ok(_) => {}
            Err(e) => log::error!("{:?}", e),
        },
        Commands::Ctl(opt) => match ctl::run(opt).await {
            Ok(_) => {}
            Err(e) => log::error!("{:?}", e),
        },
    }
    std::process::exit(0);
}
