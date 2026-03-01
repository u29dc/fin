use crate::commands::view::run_edit_transaction;
use crate::commands::{CommandFailure, CommandResult};

pub fn run_transaction(
    db: Option<&str>,
    id: &str,
    description: Option<&str>,
    account: Option<&str>,
    dry_run: bool,
) -> Result<CommandResult, CommandFailure> {
    run_edit_transaction(db, id, description, account, dry_run)
}
