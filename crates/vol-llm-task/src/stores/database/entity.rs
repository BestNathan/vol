//! SeaORM entity for persisted tasks.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub status: String,
    pub kind: String,
    pub publisher: Option<String>,
    pub assignee: Option<String>,
    pub subject: String,
    pub description: String,
    pub active_form: Option<String>,
    pub dependencies_json: String,
    pub blocks_json: String,
    pub result_json: Option<String>,
    pub summary: Option<String>,
    pub output_file: Option<String>,
    pub created_at_secs: i64,
    pub started_at_secs: Option<i64>,
    pub completed_at_secs: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
