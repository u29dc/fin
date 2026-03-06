use std::collections::HashSet;

use rusqlite::Connection;

use crate::db::schema::{
    MIGRATION_METADATA, MigrationMetadata, REQUIRED_TABLES, SCHEMA_SQL, SCHEMA_VERSION,
};
use crate::error::{FinError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStep {
    pub metadata: MigrationMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlan {
    pub current_version: i32,
    pub target_version: i32,
    pub steps: Vec<MigrationStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub current_version: i32,
    pub target_version: i32,
    pub already_current: bool,
    pub applied_steps: Vec<MigrationStep>,
}

const MIGRATION_V2_SQL: &str = r#"
DROP INDEX IF EXISTS idx_postings_provider_txn;
CREATE UNIQUE INDEX IF NOT EXISTS idx_postings_provider_txn
    ON postings(provider_txn_id, account_id)
    WHERE provider_txn_id IS NOT NULL;
"#;

const MIGRATION_V3_SQL: &str = r#"
INSERT OR IGNORE INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)
SELECT 'Expenses:Bills:Energy', 'Energy', 'expense', 'Expenses:Bills', 0
WHERE EXISTS (SELECT 1 FROM chart_of_accounts WHERE id = 'Expenses:Bills');

INSERT OR IGNORE INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)
SELECT 'Expenses:Bills:Water', 'Water', 'expense', 'Expenses:Bills', 0
WHERE EXISTS (SELECT 1 FROM chart_of_accounts WHERE id = 'Expenses:Bills');

INSERT OR IGNORE INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)
SELECT 'Expenses:Bills:CouncilTax', 'Council Tax', 'expense', 'Expenses:Bills', 0
WHERE EXISTS (SELECT 1 FROM chart_of_accounts WHERE id = 'Expenses:Bills');

INSERT OR IGNORE INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)
SELECT 'Expenses:Bills:Internet', 'Internet', 'expense', 'Expenses:Bills', 0
WHERE EXISTS (SELECT 1 FROM chart_of_accounts WHERE id = 'Expenses:Bills');

INSERT OR IGNORE INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)
SELECT 'Expenses:Bills:Insurance', 'Insurance', 'expense', 'Expenses:Bills', 0
WHERE EXISTS (SELECT 1 FROM chart_of_accounts WHERE id = 'Expenses:Bills');
"#;

const MIGRATION_V4_SQL: &str = r#"
UPDATE journal_entries
SET posted_date = date(posted_at)
WHERE posted_date IS NULL OR posted_date = '';

CREATE INDEX IF NOT EXISTS idx_journal_entries_posted_date ON journal_entries(posted_date);
CREATE INDEX IF NOT EXISTS idx_postings_journal_entry_account ON postings(journal_entry_id, account_id);
CREATE INDEX IF NOT EXISTS idx_journal_entries_source_file ON journal_entries(source_file);
"#;

const MIGRATION_V5_SQL: &str = r#"
UPDATE journal_entries
SET is_transfer = 1
WHERE id IN (
    SELECT p.journal_entry_id
    FROM postings p
    INNER JOIN chart_of_accounts coa ON p.account_id = coa.id
    GROUP BY p.journal_entry_id
    HAVING COUNT(*) = 2
        AND SUM(CASE WHEN coa.account_type = 'asset' THEN 1 ELSE 0 END) = 2
);

CREATE INDEX IF NOT EXISTS idx_journal_entries_is_transfer_posted ON journal_entries(is_transfer, posted_at);
"#;

const MIGRATION_V6_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_postings_journal_entry_id ON postings(journal_entry_id, id);
CREATE INDEX IF NOT EXISTS idx_postings_account_journal_entry_id ON postings(account_id, journal_entry_id, id);
CREATE INDEX IF NOT EXISTS idx_journal_entries_posted_id ON journal_entries(posted_at, id);
"#;

pub fn get_user_version(connection: &Connection) -> Result<i32> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get::<usize, i32>(0))
        .map_err(Into::into)
}

fn set_user_version(connection: &Connection, version: i32) -> Result<()> {
    connection.execute_batch(&format!("PRAGMA user_version = {version};"))?;
    Ok(())
}

fn column_exists(connection: &Connection, table_name: &str, column_name: &str) -> Result<bool> {
    let pragma = format!("PRAGMA table_info({table_name})");
    let mut statement = connection.prepare(&pragma)?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn add_column_if_missing(
    connection: &Connection,
    table: &str,
    column: &str,
    sql_type: &str,
) -> Result<()> {
    if column_exists(connection, table, column)? {
        return Ok(());
    }
    let alter = format!("ALTER TABLE {table} ADD COLUMN {column} {sql_type}");
    connection.execute_batch(&alter)?;
    Ok(())
}

#[must_use]
pub fn plan_migrations(current_version: i32) -> MigrationPlan {
    let steps = MIGRATION_METADATA
        .iter()
        .copied()
        .filter(|metadata| {
            metadata.to_version > current_version && metadata.to_version <= SCHEMA_VERSION
        })
        .map(|metadata| MigrationStep { metadata })
        .collect::<Vec<_>>();
    MigrationPlan {
        current_version,
        target_version: SCHEMA_VERSION,
        steps,
    }
}

pub fn missing_required_tables(connection: &Connection) -> Result<Vec<&'static str>> {
    let mut statement = connection.prepare("SELECT name FROM sqlite_master WHERE type='table'")?;
    let table_names = statement
        .query_map([], |row| row.get::<usize, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let table_set = table_names.into_iter().collect::<HashSet<_>>();
    Ok(REQUIRED_TABLES
        .iter()
        .copied()
        .filter(|name| !table_set.contains(*name))
        .collect())
}

pub fn migrate_to_latest(connection: &mut Connection) -> Result<MigrationReport> {
    let current_version = get_user_version(connection)?;
    if current_version >= SCHEMA_VERSION {
        return Ok(MigrationReport {
            current_version,
            target_version: SCHEMA_VERSION,
            already_current: true,
            applied_steps: vec![],
        });
    }

    let plan = plan_migrations(current_version);
    let transaction = connection
        .transaction()
        .map_err(|error| FinError::Migration {
            message: error.to_string(),
        })?;

    if current_version == 0 {
        transaction.execute_batch(SCHEMA_SQL)?;
    }
    if current_version < 2 {
        transaction.execute_batch(MIGRATION_V2_SQL)?;
    }
    if current_version < 3 {
        transaction.execute_batch(MIGRATION_V3_SQL)?;
    }
    if current_version < 4 {
        add_column_if_missing(
            &transaction,
            "journal_entries",
            "posted_date",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        transaction.execute_batch(MIGRATION_V4_SQL)?;
    }
    if current_version < 5 {
        add_column_if_missing(
            &transaction,
            "journal_entries",
            "is_transfer",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        transaction.execute_batch(MIGRATION_V5_SQL)?;
    }
    if current_version < 6 {
        transaction.execute_batch(MIGRATION_V6_SQL)?;
    }

    set_user_version(&transaction, SCHEMA_VERSION)?;
    transaction.commit().map_err(|error| FinError::Migration {
        message: error.to_string(),
    })?;

    Ok(MigrationReport {
        current_version,
        target_version: SCHEMA_VERSION,
        already_current: false,
        applied_steps: plan.steps,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{get_user_version, migrate_to_latest, missing_required_tables, plan_migrations};
    use crate::SCHEMA_VERSION;

    #[test]
    fn migration_plan_tracks_versions() {
        let plan = plan_migrations(2);
        assert_eq!(plan.current_version, 2);
        assert_eq!(plan.target_version, SCHEMA_VERSION);
        assert_eq!(plan.steps.len(), 4);
    }

    #[test]
    fn migrate_to_latest_initializes_schema() {
        let mut connection = Connection::open_in_memory().expect("open in-memory sqlite");
        let report = migrate_to_latest(&mut connection).expect("migrate");
        assert!(!report.already_current);
        assert_eq!(
            get_user_version(&connection).expect("version"),
            SCHEMA_VERSION
        );
        assert_eq!(
            missing_required_tables(&connection).expect("missing tables"),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn migrate_to_latest_adds_transaction_query_index() {
        let mut connection = Connection::open_in_memory().expect("open in-memory sqlite");
        migrate_to_latest(&mut connection).expect("migrate");

        let index_names = connection
            .prepare("PRAGMA index_list(postings)")
            .expect("prepare index list")
            .query_map([], |row| row.get::<usize, String>(1))
            .expect("query index list")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect index names");

        assert!(
            index_names
                .iter()
                .any(|name| name == "idx_postings_account_journal_entry_id")
        );
    }
}
