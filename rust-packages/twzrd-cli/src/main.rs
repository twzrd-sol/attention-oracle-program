//! TWZRD CLI - Command-line interface for TWZRD Attention Oracle
//!
//! Open-core Solana primitive for tokenized attention.

use clap::{Parser, Subcommand};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use twzrd_sdk::TwzrdClient;

#[derive(Parser)]
#[command(name = "twzrd")]
#[command(about = "TWZRD Attention Oracle CLI", long_about = None)]
#[command(version)]
struct Cli {
    /// RPC URL
    #[arg(short, long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get channel state for a streamer
    Channel {
        /// Streamer public key
        streamer: String,
    },
    /// Claim tokens
    Claim {
        /// User public key
        user: String,
        /// Channel public key
        channel: String,
    },
    /// Show program information
    Info,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = TwzrdClient::new(&cli.rpc_url);

    match cli.command {
        Commands::Channel { streamer } => {
            let pubkey = Pubkey::from_str(&streamer)?;
            println!("Fetching channel state for {}...", pubkey);
            // TODO: Implement
            println!("Not implemented yet");
        }
        Commands::Claim { user, channel } => {
            let user_pubkey = Pubkey::from_str(&user)?;
            let channel_pubkey = Pubkey::from_str(&channel)?;
            println!("Claiming tokens for {} from {}...", user_pubkey, channel_pubkey);
            // TODO: Implement
            println!("Not implemented yet");
        }
        Commands::Info => {
            println!("TWZRD Attention Oracle");
            println!("Program ID: {}", twzrd_sdk::PROGRAM_ID);
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
            println!("Website: https://twzrd.xyz");
        }
    }

    Ok(())
}
