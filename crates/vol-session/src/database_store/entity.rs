use sea_orm::entity::prelude::*;

pub mod sessions {
    use super::{
        ActiveModelBehavior, DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EntityTrait,
        EnumIter, PrimaryKeyTrait, Related, RelationDef, RelationTrait,
    };

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "sessions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub agent_id: String,
        pub created_at: i64,
        pub updated_at: i64,
        pub entry_count: i32,
        pub metadata: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::session_entries::Entity")]
        SessionEntries,
    }

    impl Related<super::session_entries::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::SessionEntries.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod session_entries {
    use super::{
        ActiveModelBehavior, DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EntityTrait,
        EnumIter, PrimaryKeyTrait, Related, RelationDef, RelationTrait,
    };

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "session_entries")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub session_id: String,
        pub created_at: i64,
        pub parent_id: Option<String>,
        pub entry_type: String,
        pub data: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::sessions::Entity",
            from = "Column::SessionId",
            to = "super::sessions::Column::Id"
        )]
        Sessions,
    }

    impl Related<super::sessions::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Sessions.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}
