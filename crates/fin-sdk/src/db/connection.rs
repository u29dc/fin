use std::env;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::config::paths::resolve_fin_paths;
use crate::db::migrate::{get_user_version, migrate_to_latest};
use crate::db::schema::SCHEMA_VERSION;
use crate::error::{FinError, Result};

#[derive(Debug, Clone)]
pub struct OpenDatabaseOptions {
    pub path: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub create: bool,
    pub readonly: bool,
    pub migrate: bool,
}

impl Default for OpenDatabaseOptions {
    fn default() -> Self {
        Self {
            path: None,
            config_dir: None,
            create: true,
            readonly: false,
            migrate: false,
        }
    }
}

pub fn apply_connection_pragmas(connection: &Connection, readonly: bool) -> Result<()> {
    connection.execute_batch(
        r#"
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA cache_size = -64000;
PRAGMA temp_store = MEMORY;
"#,
    )?;
    if !readonly {
        connection.execute_batch(
            r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
"#,
        )?;
    }
    Ok(())
}

#[must_use]
pub fn resolve_db_path(explicit_path: Option<&Path>, config_dir: Option<&Path>) -> PathBuf {
    let env_path = env::var_os("DB_PATH").map(PathBuf::from);
    let default_db_path = resolve_fin_paths().db_file;
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_db_path_with(
        explicit_path,
        env_path.as_deref(),
        config_dir,
        &default_db_path,
        &cwd,
    )
}

#[must_use]
pub fn resolve_db_path_with(
    explicit_path: Option<&Path>,
    env_db_path: Option<&Path>,
    config_dir: Option<&Path>,
    default_db_path: &Path,
    cwd: &Path,
) -> PathBuf {
    if let Some(path) = explicit_path {
        return normalize_user_path(path, cwd);
    }
    if let Some(path) = env_db_path {
        return normalize_user_path(path, cwd);
    }
    if let Some(dir) = config_dir {
        return dir.join("fin.db");
    }
    default_db_path.to_path_buf()
}

fn normalize_user_path(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn open_connection(path: &Path, readonly: bool, create: bool) -> Result<Connection> {
    let flags = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else if create {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };

    Connection::open_with_flags(path, flags).map_err(|error| FinError::Database {
        message: format!("{}: {}", path.display(), error),
    })
}

pub fn open_database(options: OpenDatabaseOptions) -> Result<Connection> {
    let db_path = resolve_db_path(options.path.as_deref(), options.config_dir.as_deref());
    if options.create
        && let Some(parent) = db_path.parent()
    {
        create_dir_all(parent).map_err(|error| FinError::Io {
            message: format!("{}: {}", parent.display(), error),
        })?;
    }

    if options.migrate && options.readonly {
        let needs_migration = {
            let probe = open_connection(&db_path, true, false)?;
            apply_connection_pragmas(&probe, true)?;
            get_user_version(&probe)? < SCHEMA_VERSION
        };

        if needs_migration {
            let mut rw = open_connection(&db_path, false, options.create)?;
            apply_connection_pragmas(&rw, false)?;
            migrate_to_latest(&mut rw)?;
        }

        let ro = open_connection(&db_path, true, false)?;
        apply_connection_pragmas(&ro, true)?;
        return Ok(ro);
    }

    let mut connection = open_connection(&db_path, options.readonly, options.create)?;
    apply_connection_pragmas(&connection, options.readonly)?;
    if options.migrate && !options.readonly {
        migrate_to_latest(&mut connection)?;
    }
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{OpenDatabaseOptions, open_database, resolve_db_path_with};
    use crate::SCHEMA_VERSION;
    use crate::db::migrate::get_user_version;

    #[test]
    fn db_path_precedence_explicit_overrides_all() {
        let resolved = resolve_db_path_with(
            Some(Path::new("explicit.db")),
            Some(Path::new("env.db")),
            Some(Path::new("/tmp/config")),
            Path::new("/tmp/default.db"),
            Path::new("/cwd"),
        );
        assert_eq!(resolved, PathBuf::from("/cwd/explicit.db"));
    }

    #[test]
    fn db_path_falls_back_to_config_dir_then_default() {
        let from_config = resolve_db_path_with(
            None,
            None,
            Some(Path::new("/tmp/config")),
            Path::new("/tmp/default.db"),
            Path::new("/cwd"),
        );
        assert_eq!(from_config, PathBuf::from("/tmp/config/fin.db"));

        let from_default = resolve_db_path_with(
            None,
            None,
            None,
            Path::new("/tmp/default.db"),
            Path::new("/cwd"),
        );
        assert_eq!(from_default, PathBuf::from("/tmp/default.db"));
    }

    #[test]
    fn open_database_with_migrate_bootstraps_schema() {
        let temp = tempdir().expect("create tempdir");
        let db_path = temp.path().join("data").join("fin.db");
        let options = OpenDatabaseOptions {
            path: Some(db_path.clone()),
            migrate: true,
            ..OpenDatabaseOptions::default()
        };
        let connection = open_database(options).expect("open and migrate db");
        assert_eq!(
            get_user_version(&connection).expect("user version"),
            SCHEMA_VERSION
        );
    }
}
