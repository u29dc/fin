use serde_json::json;

use fin_sdk::SDK_VERSION;

use crate::commands::{CommandFailure, CommandResult};
use crate::envelope::MetaExtras;
use crate::error::{CliError, ErrorCode, ExitCode};
use crate::registry::{ToolMeta, global_flags, tool_registry};

fn render_tool_detail(tool: &ToolMeta) -> String {
    let mut lines = vec![
        format!("{} -- {}", tool.name, tool.description),
        format!("  Command: {}", tool.command),
        format!("  Category: {}", tool.category),
        format!("  Idempotent: {}", tool.idempotent),
        format!("  Read only: {}", tool.read_only),
        format!(
            "  Default output: {}",
            if tool.interactive_only {
                "interactive"
            } else {
                "json"
            }
        ),
        format!("  Interactive only: {}", tool.interactive_only),
    ];
    if let Some(limit) = &tool.rate_limit {
        lines.push(format!("  Rate limit: {limit}"));
    }
    lines.push(format!("  Example: {}", tool.example));
    if !tool.parameters.is_empty() {
        lines.push("  Parameters:".to_string());
        for parameter in &tool.parameters {
            let required = if parameter.required { ", required" } else { "" };
            lines.push(format!(
                "    {} ({}{}): {}",
                parameter.name, parameter.param_type, required, parameter.description
            ));
        }
    }
    lines.join("\n")
}

fn render_tool_catalog(tools: &[ToolMeta]) -> String {
    let mut lines = Vec::new();
    let mut current_category = String::new();

    for tool in tools {
        if tool.category != current_category {
            if !current_category.is_empty() {
                lines.push(String::new());
            }
            lines.push(tool.category.to_uppercase());
            current_category = tool.category.clone();
        }
        lines.push(format!("  {:<32} {}", tool.command, tool.description));
    }

    lines.join("\n")
}

pub fn run(name: Option<&str>) -> Result<CommandResult, CommandFailure> {
    let tools = tool_registry();
    if let Some(name) = name {
        let Some(tool) = tools.iter().find(|candidate| candidate.name == name) else {
            return Err(CommandFailure {
                tool: "tools",
                error: CliError::new(
                    ErrorCode::NotFound,
                    format!("Tool \"{name}\" not found"),
                    "Run `fin tools` to list all available tools",
                ),
            });
        };

        return Ok(CommandResult {
            tool: "tools",
            data: json!({ "tool": tool }),
            text: render_tool_detail(tool),
            meta: MetaExtras::default(),
            exit_code: ExitCode::Success,
        });
    }

    let catalog_text = render_tool_catalog(&tools);
    let total = tools.len();

    Ok(CommandResult {
        tool: "tools",
        data: json!({
            "version": SDK_VERSION,
            "tools": tools,
            "globalFlags": global_flags(),
        }),
        text: catalog_text,
        meta: MetaExtras {
            count: Some(total),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}
