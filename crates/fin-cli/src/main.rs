use anyhow::Result;
use clap::{Parser, Subcommand};
use fin_sdk::{sdk_banner, SDK_VERSION};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "fin", version = SDK_VERSION, about = "fin rust cli")]
struct Cli {
    #[arg(long)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print version and sdk information
    Version,
}

#[derive(Serialize)]
struct Envelope<T: Serialize> {
    ok: bool,
    data: T,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Version) | None => {
            if cli.json {
                let payload = Envelope {
                    ok: true,
                    data: serde_json::json!({
                        "tool": "version",
                        "sdk": sdk_banner(),
                    }),
                };
                println!("{}", serde_json::to_string(&payload)?);
            } else {
                println!("{}", sdk_banner());
            }
        }
    }

    Ok(())
}
