mod api;
mod cli;
mod runtime;
mod server;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::oneshot;

use crate::cli::{Cli, Command};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("fin-api: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    run_with(Cli::parse()).await
}

async fn run_with(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Start(args) => {
            let plan = runtime::prepare_start_plan(&args)?;
            eprintln!("{}", plan.bootstrap_line());

            let (ready_tx, ready_rx) = oneshot::channel();
            let server = tokio::spawn(async move {
                server::serve_with_shutdown(plan, shutdown_signal(), Some(ready_tx)).await
            });

            let endpoint = ready_rx
                .await
                .context("fin-api terminated before reporting readiness")?;
            eprintln!("fin-api listening | {}", endpoint.log_line());

            server.await.context("join fin-api server task")??;
            eprintln!("fin-api stopped");
            Ok(())
        }
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let ctrl_c = async {
            let _ = tokio::signal::ctrl_c().await;
        };
        let terminate = async {
            if let Ok(mut signal) = signal(SignalKind::terminate()) {
                let _ = signal.recv().await;
            }
        };
        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
