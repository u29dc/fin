use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use fin_sdk::config::resolve_fin_home;
use fin_sdk::runtime::{RuntimeContext, RuntimeContextOptions};

use crate::cli::{StartArgs, TransportKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportBinding {
    Unix { socket_path: PathBuf },
    Tcp { bind_addr: SocketAddr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPlan {
    pub fin_home: PathBuf,
    pub transport: TransportBinding,
    pub runtime_checked: bool,
    pub group_count: Option<usize>,
    pub config_path_override: Option<PathBuf>,
    pub db_path_override: Option<PathBuf>,
    pub db_path: Option<PathBuf>,
}

impl StartPlan {
    #[must_use]
    pub fn bootstrap_line(&self) -> String {
        let transport = match &self.transport {
            TransportBinding::Unix { socket_path } => {
                format!("transport=unix | socket={}", socket_path.display())
            }
            TransportBinding::Tcp { bind_addr } => {
                format!("transport=tcp | bind={bind_addr}")
            }
        };
        if self.runtime_checked {
            let groups = self.group_count.unwrap_or_default();
            let db_path = self
                .db_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "n/a".to_owned());
            return format!(
                "fin-api starting | {transport} | runtime=checked | groups={groups} | db={db_path}"
            );
        }
        format!("fin-api starting | {transport} | runtime=deferred")
    }
}

pub fn default_socket_path(fin_home: &Path) -> PathBuf {
    fin_home.join("run").join("fin-api.sock")
}

pub fn default_transport() -> TransportKind {
    #[cfg(unix)]
    {
        TransportKind::Unix
    }
    #[cfg(not(unix))]
    {
        TransportKind::Tcp
    }
}

pub fn default_tcp_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
}

pub fn prepare_start_plan(args: &StartArgs) -> Result<StartPlan> {
    let fin_home = resolve_fin_home();
    let transport = match args.transport.unwrap_or_else(default_transport) {
        TransportKind::Unix => {
            if args.tcp_addr.is_some() {
                bail!("--tcp-addr cannot be used with unix transport");
            }
            #[cfg(unix)]
            {
                let socket_path = args
                    .socket_path
                    .clone()
                    .unwrap_or_else(|| default_socket_path(&fin_home));
                TransportBinding::Unix { socket_path }
            }
            #[cfg(not(unix))]
            {
                bail!("unix transport is unavailable on this platform")
            }
        }
        TransportKind::Tcp => {
            if args.socket_path.is_some() {
                bail!("--socket-path cannot be used with tcp transport");
            }
            TransportBinding::Tcp {
                bind_addr: args.tcp_addr.unwrap_or_else(default_tcp_addr),
            }
        }
    };

    let mut plan = StartPlan {
        fin_home,
        transport,
        runtime_checked: false,
        group_count: None,
        config_path_override: args.config_path.clone(),
        db_path_override: args.db_path.clone(),
        db_path: None,
    };

    if args.check_runtime {
        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: args.config_path.clone(),
            db_path: args.db_path.clone(),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .context("open read-only runtime context for fin-api")?;
        plan.runtime_checked = true;
        plan.group_count = Some(runtime.config().group_ids().len());
        plan.db_path = Some(runtime.db_path().to_path_buf());
    }

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{TransportBinding, default_socket_path, default_transport, prepare_start_plan};
    use crate::cli::{StartArgs, TransportKind};
    use fin_sdk::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    #[test]
    fn default_socket_path_uses_run_directory() {
        let socket = default_socket_path(std::path::Path::new("/tmp/fin-home"));
        assert_eq!(
            socket,
            std::path::Path::new("/tmp/fin-home/run/fin-api.sock")
        );
    }

    #[test]
    fn prepare_start_plan_can_check_runtime_against_fixture() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        let transport = default_transport();

        let plan = prepare_start_plan(&StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: matches!(transport, TransportKind::Unix)
                .then(|| temp.path().join("fin-api.sock")),
            tcp_addr: matches!(transport, TransportKind::Tcp)
                .then(|| "127.0.0.1:0".parse().expect("tcp addr")),
            transport: Some(transport),
            check_runtime: true,
        })
        .expect("prepare plan");

        assert!(plan.runtime_checked);
        assert_eq!(plan.group_count, Some(3));
        assert_eq!(
            plan.db_path.as_deref(),
            Some(fixture.paths.db_path.as_path())
        );
    }

    #[test]
    fn prepare_start_plan_rejects_mismatched_transport_flags() {
        let error = prepare_start_plan(&StartArgs {
            config_path: None,
            db_path: None,
            socket_path: Some(PathBuf::from("/tmp/fin-api.sock")),
            tcp_addr: None,
            transport: Some(TransportKind::Tcp),
            check_runtime: false,
        })
        .expect_err("mismatched flags should fail");

        assert!(error.to_string().contains("--socket-path"));
    }

    #[cfg(unix)]
    #[test]
    fn prepare_start_plan_defaults_to_unix_on_unix_hosts() {
        let plan = prepare_start_plan(&StartArgs {
            config_path: None,
            db_path: None,
            socket_path: Some(PathBuf::from("/tmp/fin-api.sock")),
            tcp_addr: None,
            transport: None,
            check_runtime: false,
        })
        .expect("prepare default plan");

        assert!(matches!(plan.transport, TransportBinding::Unix { .. }));
    }
}
