use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(M20260609_000001CreateSessions)]
    }

    fn migration_table_name() -> sea_orm::DynIden {
        "seaql_migrations_sessions".into_iden()
    }
}

#[derive(DeriveMigrationName)]
pub struct M20260609_000001CreateSessions;

#[async_trait::async_trait]
impl MigrationTrait for M20260609_000001CreateSessions {
    async fn up(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Sessions::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Sessions::AgentId).string().not_null())
                    .col(ColumnDef::new(Sessions::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Sessions::UpdatedAt).big_integer().not_null())
                    .col(
                        ColumnDef::new(Sessions::EntryCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Sessions::Metadata).text().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SessionEntries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SessionEntries::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SessionEntries::SessionId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SessionEntries::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SessionEntries::ParentId).string().null())
                    .col(
                        ColumnDef::new(SessionEntries::EntryType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SessionEntries::Data).text().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sessions_agent_id_updated_at")
                    .table(Sessions::Table)
                    .col(Sessions::AgentId)
                    .col(Sessions::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sessions_updated_at")
                    .table(Sessions::Table)
                    .col(Sessions::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_session_entries_session_id_created_at")
                    .table(SessionEntries::Table)
                    .col(SessionEntries::SessionId)
                    .col(SessionEntries::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_session_entries_parent_id")
                    .table(SessionEntries::Table)
                    .col(SessionEntries::ParentId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_session_entries_parent_id")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_session_entries_session_id_created_at")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(Index::drop().name("idx_sessions_updated_at").to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_sessions_agent_id_updated_at")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(SessionEntries::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
    AgentId,
    CreatedAt,
    UpdatedAt,
    EntryCount,
    Metadata,
}

#[derive(DeriveIden)]
enum SessionEntries {
    Table,
    Id,
    SessionId,
    CreatedAt,
    ParentId,
    EntryType,
    Data,
}
