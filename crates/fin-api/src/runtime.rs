use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fin_sdk::config::resolve_fin_home;
use fin_sdk::runtime::{RuntimeContext, RuntimeContextOptions};

use crate::cli::StartArgs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapPlan {
    pub fin_home: PathBuf,
    pub socket_path: PathBuf,
    pub runtime_checked: bool,
    pub group_count: Option<usize>,
    pub db_path: Option<PathBuf>,
}

impl BootstrapPlan {
    #[must_use]
    pub fn status_line(&self) -> String {
        let socket = self.socket_path.display();
        if self.runtime_checked {
            let group_count = self.group_count.unwrap_or_default();
            let db_path = self
                .db_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "n/a".to_owned());
            return format!(
                "fin-api scaffold ready | transport pending | socket {socket} | groups {group_count} | db {db_path}"
            );
        }
        format!("fin-api scaffold ready | transport pending | socket {socket}")
    }
}

pub fn default_socket_path(fin_home: &Path) -> PathBuf {
    fin_home.join("run").join("fin-api.sock")
}

pub fn prepare_bootstrap(args: &StartArgs) -> Result<BootstrapPlan> {
    let fin_home = resolve_fin_home();
    let socket_path = args
        .socket_path
        .clone()
        .unwrap_or_else(|| default_socket_path(&fin_home));
    let mut plan = BootstrapPlan {
        fin_home,
        socket_path,
        runtime_checked: false,
        group_count: None,
        db_path: None,
    };

    if args.check_runtime {
        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: args.config_path.clone(),
            db_path: args.db_path.clone(),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .context("open read-only runtime context for fin-api scaffold")?;
        plan.runtime_checked = true;
        plan.group_count = Some(runtime.config().group_ids().len());
        plan.db_path = Some(runtime.db_path().to_path_buf());
    }

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{default_socket_path, prepare_bootstrap};
    use crate::cli::StartArgs;
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
    fn prepare_bootstrap_can_check_runtime_against_fixture() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");

        let plan = prepare_bootstrap(&StartArgs {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            socket_path: Some(temp.path().join("fin-api.sock")),
            check_runtime: true,
        })
        .expect("prepare bootstrap");

        assert!(plan.runtime_checked);
        assert_eq!(plan.group_count, Some(3));
        assert_eq!(
            plan.db_path.as_deref(),
            Some(fixture.paths.db_path.as_path())
        );
    }
}
