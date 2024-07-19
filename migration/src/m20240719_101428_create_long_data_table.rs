use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

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
                    .col(ColumnDef::new(LongData::Len).string().not_null())
                    .col(ColumnDef::new(LongData::Text).string().not_null())
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
enum LongData {
    Table,
    /// 存档 ID
    SaveId,
    /// 二次校验长度(?)
    Len,
    /// 1024 ~ 2MB 长度的数据
    Text,
}
