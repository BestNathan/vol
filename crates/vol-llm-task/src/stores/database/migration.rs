//! SeaORM migration for persisted tasks.

use sea_orm_migration::prelude::*;

pub(super) struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateTasks)]
    }
}

struct CreateTasks;

impl MigrationName for CreateTasks {
    fn name(&self) -> &str {
        "m20260609_000001_create_tasks"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateTasks {
    async fn up(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tasks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Tasks::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Tasks::Status).string().not_null())
                    .col(ColumnDef::new(Tasks::Kind).string().not_null())
                    .col(ColumnDef::new(Tasks::Publisher).string())
                    .col(ColumnDef::new(Tasks::Assignee).string())
                    .col(ColumnDef::new(Tasks::Subject).string().not_null())
                    .col(ColumnDef::new(Tasks::Description).text().not_null())
                    .col(ColumnDef::new(Tasks::ActiveForm).string())
                    .col(ColumnDef::new(Tasks::DependenciesJson).text().not_null())
                    .col(ColumnDef::new(Tasks::BlocksJson).text().not_null())
                    .col(ColumnDef::new(Tasks::ResultJson).text())
                    .col(ColumnDef::new(Tasks::Summary).text())
                    .col(ColumnDef::new(Tasks::OutputFile).text())
                    .col(
                        ColumnDef::new(Tasks::CreatedAtSecs)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Tasks::StartedAtSecs).big_integer())
                    .col(ColumnDef::new(Tasks::CompletedAtSecs).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_status")
                    .if_not_exists()
                    .table(Tasks::Table)
                    .col(Tasks::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tasks_status")
                    .table(Tasks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Tasks::Table).if_exists().to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Tasks {
    Table,
    Id,
    Status,
    Kind,
    Publisher,
    Assignee,
    Subject,
    Description,
    ActiveForm,
    DependenciesJson,
    BlocksJson,
    ResultJson,
    Summary,
    OutputFile,
    CreatedAtSecs,
    StartedAtSecs,
    CompletedAtSecs,
}
