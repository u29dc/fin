use serde_json::json;

use fin_sdk::sanitize::{
    discover_descriptions, discover_unmapped_descriptions, execute_migration, execute_recategorize,
    plan_migration, plan_recategorize,
};

use crate::commands::{CommandFailure, CommandResult, map_fin_error, open_runtime};
use crate::envelope::MetaExtras;
use crate::error::ExitCode;

pub fn run_discover(
    db: Option<&str>,
    unmapped: bool,
    min: usize,
    account: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("sanitize.discover", db, true)?;
    let rules = runtime
        .load_rules(None, None)
        .map_err(|error| map_fin_error("sanitize.discover", error))?;

    let descriptions = if unmapped {
        discover_unmapped_descriptions(runtime.connection(), &rules.config, min, account, 500)
    } else {
        discover_descriptions(runtime.connection(), min, account, 500)
    }
    .map_err(|error| map_fin_error("sanitize.discover", error))?;

    Ok(CommandResult {
        tool: "sanitize.discover",
        data: json!({
            "descriptions": descriptions,
            "count": descriptions.len(),
        }),
        text: format!("Found {} descriptions", descriptions.len()),
        meta: MetaExtras {
            count: Some(descriptions.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_migrate(db: Option<&str>, dry_run: bool) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("sanitize.migrate", db, dry_run)?;
    let rules = runtime
        .load_rules(None, None)
        .map_err(|error| map_fin_error("sanitize.migrate", error))?;

    let plan = plan_migration(runtime.connection(), &rules.config)
        .map_err(|error| map_fin_error("sanitize.migrate", error))?;
    let result = execute_migration(runtime.connection(), &plan, dry_run)
        .map_err(|error| map_fin_error("sanitize.migrate", error))?;

    let text = if dry_run {
        format!("Migration plan: {} updates (dry-run)", plan.to_update.len())
    } else {
        format!(
            "Migration result: {} updated, {} errors",
            result.updated,
            result.errors.len()
        )
    };

    Ok(CommandResult {
        tool: "sanitize.migrate",
        data: json!({
            "plan": {
                "toUpdate": plan.to_update.len(),
                "alreadyClean": plan.already_clean,
                "noMatch": plan.no_match,
            },
            "result": {
                "updated": result.updated,
                "skipped": result.skipped,
                "errors": result.errors,
            }
        }),
        text,
        meta: MetaExtras {
            count: Some(result.updated),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_recategorize(db: Option<&str>, dry_run: bool) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("sanitize.recategorize", db, dry_run)?;
    let rules = runtime
        .load_rules(None, None)
        .map_err(|error| map_fin_error("sanitize.recategorize", error))?;

    let plan = plan_recategorize(runtime.connection(), &rules.config)
        .map_err(|error| map_fin_error("sanitize.recategorize", error))?;
    let result = execute_recategorize(runtime.connection(), &plan, dry_run)
        .map_err(|error| map_fin_error("sanitize.recategorize", error))?;

    let text = if dry_run {
        format!(
            "Recategorize plan: {} updates (dry-run)",
            plan.to_update.len()
        )
    } else {
        format!(
            "Recategorize result: {} updated, {} errors",
            result.updated,
            result.errors.len()
        )
    };

    Ok(CommandResult {
        tool: "sanitize.recategorize",
        data: json!({
            "plan": {
                "toUpdate": plan.to_update.len(),
                "alreadyCategorized": plan.already_categorized,
                "noMatch": plan.no_match,
            },
            "result": {
                "updated": result.updated,
                "skipped": result.skipped,
                "errors": result.errors,
            }
        }),
        text,
        meta: MetaExtras {
            count: Some(result.updated),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}
