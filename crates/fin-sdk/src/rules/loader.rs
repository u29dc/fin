use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::loader::{LoadedConfig, resolve_relative_to_fin_home};
use crate::config::paths::resolve_fin_paths;
use crate::error::{FinError, Result};
use crate::rules::model::{
    NameMappingConfig, default_name_mapping_config, merge_rule_overrides, parse_toml_rules,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedRules {
    pub resolved_path: PathBuf,
    pub external_loaded: bool,
    pub config: NameMappingConfig,
}

#[must_use]
pub fn resolve_rules_path(
    explicit_path: Option<&Path>,
    loaded_config: Option<&LoadedConfig>,
) -> PathBuf {
    let env_path = env::var_os("FIN_RULES_PATH").map(PathBuf::from);
    let default_path = resolve_fin_paths().rules_file;
    let config_rules = loaded_config
        .and_then(|loaded| loaded.config.rules_path())
        .map(PathBuf::from);
    let config_dir = loaded_config.map(LoadedConfig::config_dir);
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_rules_path_with(
        explicit_path,
        env_path.as_deref(),
        config_rules.as_deref(),
        config_dir.as_deref(),
        &default_path,
        &cwd,
    )
}

#[must_use]
pub fn resolve_rules_path_with(
    explicit_path: Option<&Path>,
    env_rules_path: Option<&Path>,
    config_rules_path: Option<&Path>,
    config_dir: Option<&Path>,
    default_rules_path: &Path,
    cwd: &Path,
) -> PathBuf {
    if let Some(path) = explicit_path {
        return normalize_user_path(path, cwd);
    }
    if let Some(path) = env_rules_path {
        return normalize_user_path(path, cwd);
    }
    if let Some(path) = config_rules_path {
        if let Some(dir) = config_dir {
            return resolve_relative_to_fin_home(dir, path);
        }
        return normalize_user_path(path, cwd);
    }
    default_rules_path.to_path_buf()
}

fn normalize_user_path(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

pub fn load_rules(
    explicit_path: Option<&Path>,
    loaded_config: Option<&LoadedConfig>,
    base_config: Option<NameMappingConfig>,
) -> Result<LoadedRules> {
    let base = base_config.unwrap_or_else(default_name_mapping_config);
    let path = resolve_rules_path(explicit_path, loaded_config);
    if !path.exists() {
        return Ok(LoadedRules {
            resolved_path: path,
            external_loaded: false,
            config: base,
        });
    }

    if path.extension().and_then(|ext| ext.to_str()) == Some("ts") {
        return Err(FinError::RulesInvalid {
            path,
            message:
                "TypeScript rules are not supported directly; migrate to fin.rules.toml first."
                    .to_owned(),
        });
    }

    let raw = fs::read_to_string(&path).map_err(|error| FinError::Io {
        message: format!("{}: {}", path.display(), error),
    })?;
    let overrides = parse_toml_rules(&raw).map_err(|error| FinError::RulesInvalid {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let merged = merge_rule_overrides(&base, overrides);

    Ok(LoadedRules {
        resolved_path: path,
        external_loaded: true,
        config: merged,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{load_rules, resolve_rules_path_with};
    use crate::rules::model::default_name_mapping_config;

    #[test]
    fn rules_path_precedence_works() {
        let explicit = resolve_rules_path_with(
            Some(Path::new("explicit.toml")),
            Some(Path::new("env.toml")),
            Some(Path::new("config.toml")),
            Some(Path::new("/tmp/fin/data")),
            Path::new("/tmp/default.toml"),
            Path::new("/cwd"),
        );
        assert_eq!(explicit, PathBuf::from("/cwd/explicit.toml"));

        let from_env = resolve_rules_path_with(
            None,
            Some(Path::new("env.toml")),
            Some(Path::new("config.toml")),
            Some(Path::new("/tmp/fin/data")),
            Path::new("/tmp/default.toml"),
            Path::new("/cwd"),
        );
        assert_eq!(from_env, PathBuf::from("/cwd/env.toml"));

        let from_config = resolve_rules_path_with(
            None,
            None,
            Some(Path::new("data/fin.rules.toml")),
            Some(Path::new("/tmp/fin/data")),
            Path::new("/tmp/default.toml"),
            Path::new("/cwd"),
        );
        assert_eq!(from_config, PathBuf::from("/tmp/fin/data/fin.rules.toml"));
    }

    #[test]
    fn load_rules_merges_external_over_base() {
        let temp = tempdir().expect("tempdir");
        let rules_path = temp.path().join("fin.rules.toml");
        std::fs::write(
            &rules_path,
            r#"
warn_on_unmapped = false

[[rules]]
patterns = ["UBER"]
target = "Uber"
category = "Expenses:Travel"
"#,
        )
        .expect("write rules");

        let loaded = load_rules(Some(&rules_path), None, Some(default_name_mapping_config()))
            .expect("load rules");
        assert!(loaded.external_loaded);
        assert!(!loaded.config.warn_on_unmapped);
        assert_eq!(loaded.config.rules[0].target, "Uber");
    }

    #[test]
    fn missing_rules_falls_back_to_base() {
        let loaded = load_rules(Some(Path::new("/tmp/non-existent-rules.toml")), None, None)
            .expect("load base");
        assert!(!loaded.external_loaded);
        assert_eq!(loaded.config, default_name_mapping_config());
    }
}
