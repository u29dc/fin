use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::config::{FinConfig, LoadedConfig, load_config};
use crate::db::{OpenDatabaseOptions, open_database, resolve_db_path};
use crate::error::Result;
use crate::rules::{LoadedRules, NameMappingConfig, load_rules};

#[derive(Debug, Clone)]
pub struct RuntimeContextOptions {
    pub config_path: Option<PathBuf>,
    pub db_path: Option<PathBuf>,
    pub readonly: bool,
    pub create: bool,
    pub migrate: bool,
}

impl RuntimeContextOptions {
    #[must_use]
    pub fn read_only() -> Self {
        Self {
            readonly: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn writable() -> Self {
        Self {
            readonly: false,
            ..Self::default()
        }
    }
}

impl Default for RuntimeContextOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            db_path: None,
            readonly: true,
            create: true,
            migrate: true,
        }
    }
}

pub struct RuntimeContext {
    loaded: LoadedConfig,
    connection: Connection,
    db_path: PathBuf,
    readonly: bool,
}

impl RuntimeContext {
    pub fn open(options: RuntimeContextOptions) -> Result<Self> {
        let loaded = load_config(options.config_path.as_deref())?;
        let db_path = resolve_db_path(options.db_path.as_deref(), Some(&loaded.config_dir()));
        let connection = open_database(OpenDatabaseOptions {
            path: Some(db_path.clone()),
            config_dir: Some(loaded.config_dir()),
            readonly: options.readonly,
            create: options.create,
            migrate: options.migrate,
        })?;
        Ok(Self {
            loaded,
            connection,
            db_path,
            readonly: options.readonly,
        })
    }

    #[must_use]
    pub fn loaded_config(&self) -> &LoadedConfig {
        &self.loaded
    }

    #[must_use]
    pub fn config(&self) -> &FinConfig {
        &self.loaded.config
    }

    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    #[must_use]
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn load_rules(
        &self,
        explicit_path: Option<&Path>,
        base_config: Option<NameMappingConfig>,
    ) -> Result<LoadedRules> {
        load_rules(explicit_path, Some(&self.loaded), base_config)
    }
}

impl fmt::Debug for RuntimeContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeContext")
            .field("config_path", &self.loaded.path)
            .field("db_path", &self.db_path)
            .field("readonly", &self.readonly)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{RuntimeContext, RuntimeContextOptions};
    use crate::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    #[test]
    fn runtime_context_opens_materialized_fixture() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");

        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime");

        assert_eq!(runtime.config().group_ids().len(), 3);
        assert_eq!(runtime.db_path(), fixture.paths.db_path.as_path());
    }

    #[test]
    fn runtime_context_loads_rules_relative_to_loaded_config() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");

        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime");

        let rules = runtime.load_rules(None, None).expect("load rules");
        assert!(rules.external_loaded);
        assert!(!rules.config.rules.is_empty());
    }
}
