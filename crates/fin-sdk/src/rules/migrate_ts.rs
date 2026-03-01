use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{FinError, Result};
use crate::rules::model::{MatchMode, NameMappingConfig, NameMappingRule};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesMigrationSummary {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub rule_count: usize,
    pub warn_on_unmapped: bool,
    pub fallback_to_raw: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsNameMappingConfig {
    #[serde(default)]
    rules: Vec<TsRule>,
    #[serde(default = "default_true")]
    warn_on_unmapped: bool,
    #[serde(default = "default_true")]
    fallback_to_raw: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsRule {
    #[serde(default)]
    patterns: Vec<String>,
    #[serde(rename = "match")]
    match_pattern: Option<String>,
    target: Option<String>,
    replace: Option<String>,
    category: Option<String>,
    #[serde(default)]
    case_sensitive: Option<bool>,
    #[serde(default)]
    match_mode: Option<MatchMode>,
}

impl TsRule {
    fn into_rule(self, index: usize) -> Result<NameMappingRule> {
        let mut patterns = self.patterns;
        if let Some(pattern) = self.match_pattern {
            patterns.push(pattern);
        }
        if patterns.is_empty() {
            return Err(FinError::Parse {
                context: "fin.rules.ts",
                message: format!("rule[{index}] missing patterns/match"),
            });
        }
        let target = self.target.or(self.replace).ok_or(FinError::Parse {
            context: "fin.rules.ts",
            message: format!("rule[{index}] missing target/replace"),
        })?;

        Ok(NameMappingRule {
            patterns,
            target,
            category: self.category,
            case_sensitive: self.case_sensitive.unwrap_or(false),
            match_mode: self.match_mode.unwrap_or_default(),
        })
    }
}

pub fn extract_name_mapping_object(source: &str) -> Result<String> {
    let anchor = source.find("NAME_MAPPING_CONFIG").ok_or(FinError::Parse {
        context: "fin.rules.ts",
        message: "NAME_MAPPING_CONFIG export not found".to_owned(),
    })?;
    let source_after_anchor = &source[anchor..];
    let assignment = source_after_anchor.find('=').ok_or(FinError::Parse {
        context: "fin.rules.ts",
        message: "NAME_MAPPING_CONFIG assignment not found".to_owned(),
    })?;
    let assignment_start = anchor + assignment;
    let body_from_assignment = &source[assignment_start..];
    let open_brace_offset = body_from_assignment.find('{').ok_or(FinError::Parse {
        context: "fin.rules.ts",
        message: "NAME_MAPPING_CONFIG object body not found".to_owned(),
    })?;
    let object_start = assignment_start + open_brace_offset;

    let mut depth = 0_i32;
    let mut string_quote: Option<char> = None;
    let mut escaped = false;

    for (offset, ch) in source[object_start..].char_indices() {
        if let Some(quote) = string_quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                string_quote = None;
            }
            continue;
        }

        if ch == '\'' || ch == '"' || ch == '`' {
            string_quote = Some(ch);
            continue;
        }

        if ch == '{' {
            depth += 1;
            continue;
        }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let end = object_start + offset;
                return Ok(source[object_start..=end].to_owned());
            }
        }
    }

    Err(FinError::Parse {
        context: "fin.rules.ts",
        message: "Unterminated NAME_MAPPING_CONFIG object".to_owned(),
    })
}

pub fn parse_ts_rules(source: &str) -> Result<NameMappingConfig> {
    let object_literal = extract_name_mapping_object(source)?;
    let parsed: TsNameMappingConfig =
        json5::from_str(&object_literal).map_err(|error| FinError::Parse {
            context: "fin.rules.ts",
            message: format!("failed to parse NAME_MAPPING_CONFIG object: {error}"),
        })?;
    let rules = parsed
        .rules
        .into_iter()
        .enumerate()
        .map(|(idx, rule)| rule.into_rule(idx))
        .collect::<Result<Vec<_>>>()?;
    Ok(NameMappingConfig {
        warn_on_unmapped: parsed.warn_on_unmapped,
        fallback_to_raw: parsed.fallback_to_raw,
        rules,
    })
}

pub fn render_rules_json(config: &NameMappingConfig) -> Result<String> {
    let mut rendered = serde_json::to_string_pretty(config).map_err(|error| FinError::Parse {
        context: "fin.rules.json",
        message: format!("failed to render JSON: {error}"),
    })?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

pub fn migrate_ts_rules_file(
    source_path: &Path,
    target_path: &Path,
) -> Result<RulesMigrationSummary> {
    let source = fs::read_to_string(source_path).map_err(|error| FinError::Io {
        message: format!("{}: {}", source_path.display(), error),
    })?;
    let config = parse_ts_rules(&source)?;
    let rendered = render_rules_json(&config)?;
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| FinError::Io {
            message: format!("{}: {}", parent.display(), error),
        })?;
    }
    fs::write(target_path, rendered).map_err(|error| FinError::Io {
        message: format!("{}: {}", target_path.display(), error),
    })?;

    Ok(RulesMigrationSummary {
        source_path: source_path.to_path_buf(),
        target_path: target_path.to_path_buf(),
        rule_count: config.rules.len(),
        warn_on_unmapped: config.warn_on_unmapped,
        fallback_to_raw: config.fallback_to_raw,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        extract_name_mapping_object, migrate_ts_rules_file, parse_ts_rules, render_rules_json,
    };

    #[test]
    fn extracts_name_mapping_object() {
        let ts = r#"
export const NAME_MAPPING_CONFIG = {
  rules: [
    { patterns: ["AMAZON"], target: "Amazon" },
  ],
  warnOnUnmapped: true,
  fallbackToRaw: false,
};
"#;
        let object = extract_name_mapping_object(ts).expect("extract object");
        assert!(object.starts_with('{'));
        assert!(object.ends_with('}'));
    }

    #[test]
    fn parses_ts_object_and_renders_json() {
        let ts = r#"
export const NAME_MAPPING_CONFIG = {
  rules: [{ match: "UBER", replace: "Uber", category: "Expenses:Travel" }],
  warnOnUnmapped: false,
  fallbackToRaw: true,
};
"#;
        let parsed = parse_ts_rules(ts).expect("parse ts rules");
        assert_eq!(parsed.rules.len(), 1);
        assert!(!parsed.warn_on_unmapped);
        let json = render_rules_json(&parsed).expect("render json");
        assert!(json.contains("\"warn_on_unmapped\": false"));
        assert!(json.contains("\"target\": \"Uber\""));
    }

    #[test]
    fn migrates_rules_file_from_ts_to_json() {
        let temp = tempdir().expect("tempdir");
        let source = temp.path().join("fin.rules.ts");
        let target = temp.path().join("fin.rules.json");
        std::fs::write(
            &source,
            r#"
export const NAME_MAPPING_CONFIG = {
  rules: [{ patterns: ["WISE"], target: "Wise" }],
  warnOnUnmapped: true,
  fallbackToRaw: true,
};
"#,
        )
        .expect("write ts");

        let summary = migrate_ts_rules_file(&source, &target).expect("migrate file");
        assert_eq!(summary.rule_count, 1);
        let output = std::fs::read_to_string(&target).expect("read json");
        assert!(output.contains("\"rules\""));
        assert!(output.contains("\"target\": \"Wise\""));
    }
}
