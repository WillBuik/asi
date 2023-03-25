use clap::{Parser, Subcommand};

use crate::uds_proto::AsiClient;

pub mod uds_proto;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    pub command: AsiCommands,
}

#[derive(Subcommand)]
enum AsiCommands {
    /// Get the a-Si host version.
    Version,

    /// Start a process in an a-Si fabric.
    Run {
    },

    /// Shutdown the a-Si host.
    Shutdown,
}

fn main() {
    let args = Cli::parse();

    let client = match AsiClient::new("../asi-host/asi.sock") {
        Ok(client) => client,
        Err(err) => {
            eprintln!("Failed to connect to host: {}", err);
            return;
        },
    };

    match args.command {
        AsiCommands::Version => {
            match client.version() {
                Ok(version) => println!("Version: {}", version),
                Err(err) => eprintln!("Error: {}", err),
            }
        },

        AsiCommands::Run {  } => {
            let path = "../target/wasm32-wasi/release/userland.wasm";
            let wasm_bin = std::fs::read(path).expect("failed to load wasm");

            match client.run(&wasm_bin) {
                Ok(_) => println!("Started '{}'", path),
                Err(err) => eprintln!("Error: {}", err),
            }
        },

        AsiCommands::Shutdown => {
            match client.shutdown() {
                Ok(_) => println!("Host is shutting down"),
                Err(err) => eprintln!("Error: {}", err),
            }
        },
    }
}
