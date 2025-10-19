pub mod server;
pub mod utils;

use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// server recieving tcp stream address
    #[arg(long, default_value_t = String::from("127.0.0.1:7878"))]
    pub rx_address: String,
    /// server transmitting tcp stream address
    #[arg(long, default_value_t = String::from("127.0.0.1:7879"))]
    pub tx_address: String,

    /// run client
    #[arg(short, long, default_value_t = false)]
    pub client: bool,
    /// run server
    #[arg(short, long, default_value_t = false)]
    pub server: bool,

    /// Root directory for file system watcher
    #[arg(short, long, default_value_t = String::from("."))]
    pub root_dir: String,
}
