pub mod connection;
pub mod migrate;
pub mod schema;
pub mod seed;

pub use connection::{
    OpenDatabaseOptions, apply_connection_pragmas, open_database, resolve_db_path,
    resolve_db_path_with,
};
pub use migrate::{
    MigrationPlan, MigrationReport, MigrationStep, get_user_version, migrate_to_latest,
    missing_required_tables,
};
pub use schema::{MIGRATION_METADATA, REQUIRED_TABLES, SCHEMA_SQL, SCHEMA_VERSION};
pub use seed::ensure_chart_of_accounts_seeded;
