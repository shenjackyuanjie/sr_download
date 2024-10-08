//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "save_type")]
pub enum SaveType {
    #[sea_orm(string_value = "none")]
    None,
    #[sea_orm(string_value = "save")]
    Save,
    #[sea_orm(string_value = "ship")]
    Ship,
    #[sea_orm(string_value = "unknown")]
    Unknown,
}
