use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ParameterMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputFieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

pub type OutputSchema = BTreeMap<String, OutputFieldSchema>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolMeta {
    pub name: String,
    pub command: String,
    pub category: String,
    pub description: String,
    pub parameters: Vec<ParameterMeta>,
    pub output_fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<OutputSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
    pub idempotent: bool,
    pub rate_limit: Option<String>,
    pub example: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalFlag {
    pub name: String,
    pub description: String,
}

pub fn global_flags() -> Vec<GlobalFlag> {
    vec![
        GlobalFlag {
            name: "--json".to_string(),
            description: "Output as JSON envelope".to_string(),
        },
        GlobalFlag {
            name: "--db".to_string(),
            description: "Override database path".to_string(),
        },
        GlobalFlag {
            name: "--format".to_string(),
            description: "Output format (table|tsv)".to_string(),
        },
    ]
}

fn config_show_tool() -> ToolMeta {
    let mut output_schema = OutputSchema::new();
    output_schema.insert(
        "groups".to_string(),
        OutputFieldSchema {
            field_type: "array".to_string(),
            items: Some("GroupMetadata".to_string()),
            description: Some("Group configurations".to_string()),
        },
    );
    output_schema.insert(
        "accounts".to_string(),
        OutputFieldSchema {
            field_type: "object".to_string(),
            items: None,
            description: Some("Accounts keyed by group ID".to_string()),
        },
    );
    output_schema.insert(
        "financial".to_string(),
        OutputFieldSchema {
            field_type: "object".to_string(),
            items: None,
            description: Some("Financial parameters (tax rates, reserves)".to_string()),
        },
    );
    output_schema.insert(
        "configPath".to_string(),
        OutputFieldSchema {
            field_type: "string".to_string(),
            items: None,
            description: Some("Resolved config file path".to_string()),
        },
    );

    ToolMeta {
        name: "config.show".to_string(),
        command: "fin config show".to_string(),
        category: "config".to_string(),
        description: "Show parsed configuration".to_string(),
        parameters: Vec::new(),
        output_fields: output_schema.keys().cloned().collect(),
        output_schema: Some(output_schema),
        input_schema: None,
        idempotent: true,
        rate_limit: None,
        example: "fin config show --json".to_string(),
    }
}

fn config_validate_tool() -> ToolMeta {
    let mut output_schema = OutputSchema::new();
    output_schema.insert(
        "valid".to_string(),
        OutputFieldSchema {
            field_type: "boolean".to_string(),
            items: None,
            description: Some("Whether config is valid".to_string()),
        },
    );
    output_schema.insert(
        "errors".to_string(),
        OutputFieldSchema {
            field_type: "array".to_string(),
            items: Some("ValidationError".to_string()),
            description: Some("Validation errors with path and message".to_string()),
        },
    );
    output_schema.insert(
        "configPath".to_string(),
        OutputFieldSchema {
            field_type: "string".to_string(),
            items: None,
            description: Some("Resolved config file path".to_string()),
        },
    );

    ToolMeta {
        name: "config.validate".to_string(),
        command: "fin config validate".to_string(),
        category: "config".to_string(),
        description: "Validate config file".to_string(),
        parameters: Vec::new(),
        output_fields: output_schema.keys().cloned().collect(),
        output_schema: Some(output_schema),
        input_schema: None,
        idempotent: true,
        rate_limit: None,
        example: "fin config validate --json".to_string(),
    }
}

pub fn tool_registry() -> Vec<ToolMeta> {
    let mut tools = vec![config_show_tool(), config_validate_tool()];
    tools.sort_by(|left, right| {
        let by_category = left.category.cmp(&right.category);
        if by_category == Ordering::Equal {
            left.name.cmp(&right.name)
        } else {
            by_category
        }
    });
    tools
}
