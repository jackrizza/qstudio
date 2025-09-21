extern crate qstudio;

use clap::Parser;

use qstudio::server::QStudioServer;
use qstudio::Args;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    if !args.client && !args.server {
        log::error!("Please specify --client or --server to run the respective mode.");
        log::warn!("Also, you can run both with --client --server for a local only setup.");
        log::warn!("Refer to --help for more information.");
        std::process::exit(1);
    }

    if args.server.clone() {
        // server(args.clone());
        let server = QStudioServer::new(args.clone());
        server.start();
    }
    if args.client.clone() {
        client(args.clone());
    }

    std::thread::park();
}

fn client(args: Args) {
    log::info!("Starting UI...");
    match qstudio_ui::window(args.tx_address, args.rx_address) {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            log::error!("Error starting UI: {}", e);
            std::process::exit(0);
        }
    }
}
