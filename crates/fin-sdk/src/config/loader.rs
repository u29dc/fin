use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::model::{FinConfig, parse_fin_config};
use crate::config::paths::resolve_fin_paths;
use crate::error::{FinError, Result};

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: FinConfig,
}

impl LoadedConfig {
    #[must_use]
    pub fn config_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    }

    #[must_use]
    pub fn rules_path(&self) -> Option<PathBuf> {
        self.config
            .rules_path()
            .map(PathBuf::from)
            .map(|path| resolve_relative_to_fin_home(&self.config_dir(), &path))
    }
}

#[must_use]
pub fn resolve_config_path(explicit_path: Option<&Path>) -> PathBuf {
    let env_path = env::var_os("FIN_CONFIG_PATH").map(PathBuf::from);
    let default_path = resolve_fin_paths().config_file;
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_config_path_with(explicit_path, env_path.as_deref(), &default_path, &cwd)
}

#[must_use]
pub fn resolve_config_path_with(
    explicit_path: Option<&Path>,
    env_path: Option<&Path>,
    default_path: &Path,
    cwd: &Path,
) -> PathBuf {
    if let Some(path) = explicit_path {
        return normalize_user_path(path, cwd);
    }
    if let Some(path) = env_path {
        return normalize_user_path(path, cwd);
    }
    default_path.to_path_buf()
}

#[must_use]
fn normalize_user_path(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[must_use]
pub fn resolve_relative_to_fin_home(config_dir: &Path, maybe_relative: &Path) -> PathBuf {
    if maybe_relative.is_absolute() {
        return maybe_relative.to_path_buf();
    }
    let home = config_dir.parent().unwrap_or(config_dir);
    home.join(maybe_relative)
}

pub fn load_config(explicit_path: Option<&Path>) -> Result<LoadedConfig> {
    let path = resolve_config_path(explicit_path);
    if !path.exists() {
        return Err(FinError::ConfigNotFound { path });
    }
    let raw = fs::read_to_string(&path).map_err(|error| FinError::Io {
        message: format!("{}: {}", path.display(), error),
    })?;
    let config = parse_fin_config(&raw).map_err(|error| FinError::ConfigInvalid {
        path: path.clone(),
        message: error.to_string(),
    })?;
    Ok(LoadedConfig { path, config })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{load_config, resolve_config_path_with, resolve_relative_to_fin_home};

    #[test]
    fn config_path_precedence_prefers_explicit() {
        let resolved = resolve_config_path_with(
            Some(Path::new("explicit.toml")),
            Some(Path::new("env.toml")),
            Path::new("/tmp/default.toml"),
            Path::new("/cwd"),
        );
        assert_eq!(resolved, PathBuf::from("/cwd/explicit.toml"));
    }

    #[test]
    fn config_path_precedence_uses_env_then_default() {
        let resolved = resolve_config_path_with(
            None,
            Some(Path::new("env.toml")),
            Path::new("/tmp/default.toml"),
            Path::new("/cwd"),
        );
        assert_eq!(resolved, PathBuf::from("/cwd/env.toml"));

        let resolved_default = resolve_config_path_with(
            None,
            None,
            Path::new("/tmp/default.toml"),
            Path::new("/cwd"),
        );
        assert_eq!(resolved_default, PathBuf::from("/tmp/default.toml"));
    }

    #[test]
    fn resolve_relative_path_from_fin_home() {
        let resolved = resolve_relative_to_fin_home(
            Path::new("/tmp/fin/data"),
            Path::new("data/fin.rules.toml"),
        );
        assert_eq!(resolved, Path::new("/tmp/fin/data/fin.rules.toml"));
    }

    #[test]
    fn load_config_reads_and_validates_file() {
        let temp = tempdir().expect("create tempdir");
        let config_path = temp.path().join("fin.config.toml");
        std::fs::write(
            &config_path,
            r#"
[financial]
corp_tax_rate = 0.25

[[accounts]]
id = "Assets:Personal:Monzo"
group = "personal"
type = "asset"
provider = "monzo"

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"
"#,
        )
        .expect("write config");

        let loaded = load_config(Some(&config_path)).expect("load config");
        assert_eq!(loaded.path, config_path);
        assert_eq!(loaded.config.accounts.len(), 1);
    }
}
