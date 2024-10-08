//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "long_data")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub save_id: i32,
    pub len: i64,
    pub text: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::main_data::Entity",
        from = "Column::SaveId",
        to = "super::main_data::Column::SaveId",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    MainData,
}

impl Related<super::main_data::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MainData.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
