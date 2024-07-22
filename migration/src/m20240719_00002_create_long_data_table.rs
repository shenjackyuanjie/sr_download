use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

use crate::m20240719_00001_create_main_data_table::MainData;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(LongData::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(LongData::SaveId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(LongData::Len).big_integer().not_null())
                    .col(ColumnDef::new(LongData::Text).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("save_id")
                            .to(MainData::Table, MainData::SaveId)
                            .from(LongData::Table, LongData::SaveId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(LongData::Table).to_owned())
            .await
    }
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
