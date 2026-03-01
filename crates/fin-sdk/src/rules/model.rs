use serde::{Deserialize, Serialize};

use crate::error::{FinError, Result};

fn default_true() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_contains(value: &MatchMode) -> bool {
    *value == MatchMode::Contains
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    #[default]
    Contains,
    Regex,
    Exact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameMappingRule {
    pub patterns: Vec<String>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub case_sensitive: bool,
    #[serde(default, skip_serializing_if = "is_contains")]
    pub match_mode: MatchMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameMappingConfig {
    #[serde(default = "default_true", alias = "warnOnUnmapped")]
    pub warn_on_unmapped: bool,
    #[serde(default = "default_true", alias = "fallbackToRaw")]
    pub fallback_to_raw: bool,
    #[serde(default)]
    pub rules: Vec<NameMappingRule>,
}

impl Default for NameMappingConfig {
    fn default() -> Self {
        default_name_mapping_config()
    }
}

#[must_use]
pub fn default_name_mapping_config() -> NameMappingConfig {
    NameMappingConfig {
        warn_on_unmapped: true,
        fallback_to_raw: true,
        rules: vec![],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesOverrides {
    pub warn_on_unmapped: Option<bool>,
    pub fallback_to_raw: Option<bool>,
    pub rules: Vec<NameMappingRule>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawRulesConfig {
    #[serde(default, alias = "warnOnUnmapped")]
    warn_on_unmapped: Option<bool>,
    #[serde(default, alias = "fallbackToRaw")]
    fallback_to_raw: Option<bool>,
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawRule {
    #[serde(default)]
    patterns: Vec<String>,
    #[serde(rename = "match")]
    match_pattern: Option<String>,
    target: Option<String>,
    replace: Option<String>,
    category: Option<String>,
    #[serde(default, alias = "caseSensitive")]
    case_sensitive: Option<bool>,
    #[serde(default, alias = "matchMode")]
    match_mode: Option<MatchMode>,
}

impl RawRule {
    fn into_rule(self, index: usize) -> Result<NameMappingRule> {
        let mut patterns = self.patterns;
        if let Some(pattern) = self.match_pattern {
            patterns.push(pattern);
        }
        if patterns.is_empty() {
            return Err(FinError::RulesInvalid {
                path: "<inline>".into(),
                message: format!("rule[{index}] must define patterns or match"),
            });
        }
        let target = self.target.or(self.replace).ok_or(FinError::RulesInvalid {
            path: "<inline>".into(),
            message: format!("rule[{index}] must define target or replace"),
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

pub fn parse_toml_rules(raw: &str) -> Result<RulesOverrides> {
    let parsed: RawRulesConfig = toml::from_str(raw).map_err(|error| FinError::Parse {
        context: "fin.rules.toml",
        message: error.to_string(),
    })?;
    let rules = parsed
        .rules
        .into_iter()
        .enumerate()
        .map(|(idx, rule)| rule.into_rule(idx))
        .collect::<Result<Vec<_>>>()?;

    Ok(RulesOverrides {
        warn_on_unmapped: parsed.warn_on_unmapped,
        fallback_to_raw: parsed.fallback_to_raw,
        rules,
    })
}

#[must_use]
pub fn merge_rule_overrides(
    base: &NameMappingConfig,
    overrides: RulesOverrides,
) -> NameMappingConfig {
    NameMappingConfig {
        warn_on_unmapped: overrides.warn_on_unmapped.unwrap_or(base.warn_on_unmapped),
        fallback_to_raw: overrides.fallback_to_raw.unwrap_or(base.fallback_to_raw),
        rules: overrides
            .rules
            .into_iter()
            .chain(base.rules.iter().cloned())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::model::{
        default_name_mapping_config, merge_rule_overrides, parse_toml_rules,
    };

    #[test]
    fn parses_rules_toml_with_match_replace_shape() {
        let parsed = parse_toml_rules(
            r#"
warn_on_unmapped = true
fallback_to_raw = false

[[rules]]
match = "AMAZON"
replace = "Amazon"
category = "Expenses:Shopping"
"#,
        )
        .expect("rules parse");

        assert_eq!(parsed.rules.len(), 1);
        assert_eq!(parsed.rules[0].patterns, vec!["AMAZON"]);
        assert_eq!(parsed.rules[0].target, "Amazon");
        assert_eq!(parsed.fallback_to_raw, Some(false));
    }

    #[test]
    fn merge_places_external_rules_first() {
        let base = default_name_mapping_config();
        let overrides = parse_toml_rules(
            r#"
[[rules]]
patterns = ["WISE"]
target = "Wise"
"#,
        )
        .expect("rules parse");

        let merged = merge_rule_overrides(&base, overrides);
        assert_eq!(merged.rules.len(), 1);
        assert_eq!(merged.rules[0].target, "Wise");
    }
}
