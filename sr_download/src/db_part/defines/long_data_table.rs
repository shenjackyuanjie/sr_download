use sea_orm::sea_query::{ColumnDef, ForeignKey, Table};
use sea_orm::{DatabaseBackend, DeriveIden, ForeignKeyAction, Iden, Statement};

use super::main_data_table::MainData;

pub fn long_data_table() -> Statement {
    let mut table = Table::create();
    table
        .table(LongData::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(LongData::SaveId)
                .integer()
                .not_null()
                .primary_key(),
        )
        .foreign_key(
            ForeignKey::create()
                .name(LongData::SaveId.to_string())
                .to(MainData::Table, MainData::SaveId)
                .from(LongData::Table, LongData::SaveId)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade),
        )
        .col(ColumnDef::new(LongData::Len).big_integer().not_null())
        .col(ColumnDef::new(LongData::Text).string().not_null());
    DatabaseBackend::Postgres.build(&table)
}

#[derive(DeriveIden)]
pub enum LongData {
    Table,
    /// 存档 ID
    SaveId,
    /// 二次校验长度(?)
    Len,
    /// 1024 ~ 2MB 长度的数据
    Text,
}
