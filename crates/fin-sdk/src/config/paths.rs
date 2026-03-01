use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinPaths {
    pub home: PathBuf,
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
    pub rules_file: PathBuf,
    pub legacy_rules_file: PathBuf,
    pub backups_dir: PathBuf,
    pub inbox_dir: PathBuf,
    pub archive_dir: PathBuf,
}

#[must_use]
pub fn resolve_fin_home() -> PathBuf {
    let fin_home = env::var_os("FIN_HOME").map(PathBuf::from);
    let tools_home = env::var_os("TOOLS_HOME").map(PathBuf::from);
    let home_dir = env::var_os("HOME").map(PathBuf::from);
    resolve_fin_home_with(
        fin_home.as_deref(),
        tools_home.as_deref(),
        home_dir.as_deref(),
    )
}

#[must_use]
pub fn resolve_fin_home_with(
    fin_home: Option<&Path>,
    tools_home: Option<&Path>,
    home_dir: Option<&Path>,
) -> PathBuf {
    if let Some(path) = fin_home {
        return path.to_path_buf();
    }
    if let Some(path) = tools_home {
        return path.join("fin");
    }
    let home = home_dir.unwrap_or_else(|| Path::new("."));
    home.join(".tools").join("fin")
}

#[must_use]
pub fn resolve_fin_paths() -> FinPaths {
    let home = resolve_fin_home();
    resolve_fin_paths_from_home(home)
}

#[must_use]
pub fn resolve_fin_paths_from_home(home: PathBuf) -> FinPaths {
    let data_dir = home.join("data");
    FinPaths {
        home: home.clone(),
        data_dir: data_dir.clone(),
        config_file: data_dir.join("fin.config.toml"),
        db_file: data_dir.join("fin.db"),
        rules_file: data_dir.join("fin.rules.toml"),
        legacy_rules_file: data_dir.join("fin.rules.ts"),
        backups_dir: data_dir.join("backups"),
        inbox_dir: home.join("imports").join("inbox"),
        archive_dir: home.join("imports").join("archive"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::resolve_fin_home_with;

    #[test]
    fn precedence_prefers_fin_home() {
        let resolved = resolve_fin_home_with(
            Some(Path::new("/tmp/fin-home")),
            Some(Path::new("/tmp/tools-home")),
            Some(Path::new("/tmp/user-home")),
        );
        assert_eq!(resolved, Path::new("/tmp/fin-home"));
    }

    #[test]
    fn falls_back_to_tools_home_fin() {
        let resolved = resolve_fin_home_with(
            None,
            Some(Path::new("/tmp/tools-home")),
            Some(Path::new("/tmp/user-home")),
        );
        assert_eq!(resolved, Path::new("/tmp/tools-home/fin"));
    }

    #[test]
    fn falls_back_to_home_tools_fin() {
        let resolved = resolve_fin_home_with(None, None, Some(Path::new("/tmp/user-home")));
        assert_eq!(resolved, Path::new("/tmp/user-home/.tools/fin"));
    }
}
