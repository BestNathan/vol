//! SeaORM migration registry for persisted tasks.

use sea_orm_migration::prelude::*;

#[path = "migrations/mod.rs"]
mod migrations;

pub(super) struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(migrations::m0001_create_tasks::Migration)]
    }

    fn migration_table_name() -> sea_orm::DynIden {
        "seaql_migrations_tasks".into_iden()
    }
}
