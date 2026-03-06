use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "fin-api", about = "Read-only local daemon for fin", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Start(StartArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TransportKind {
    Unix,
    Tcp,
}

#[derive(Debug, Clone, Default, Args)]
pub struct StartArgs {
    #[arg(long)]
    pub config_path: Option<PathBuf>,
    #[arg(long)]
    pub db_path: Option<PathBuf>,
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    #[arg(long)]
    pub tcp_addr: Option<SocketAddr>,
    #[arg(long)]
    pub transport: Option<TransportKind>,
    #[arg(long, default_value_t = false)]
    pub check_runtime: bool,
}
