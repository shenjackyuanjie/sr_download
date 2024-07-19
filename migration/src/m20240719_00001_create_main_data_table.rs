use sea_orm::{EnumIter, Iterable};
use sea_orm_migration::prelude::extension::postgres::Type;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
#[sea_orm(iden = "save_type")]
pub struct SaveTypeEnum;

#[derive(DeriveIden, EnumIter)]
pub enum SaveTypeVariants {
    #[sea_orm(iden = "ship")]
    Ship,
    #[sea_orm(iden = "save")]
    Save,
    #[sea_orm(iden = "unknown")]
    Unknown,
    #[sea_orm(iden = "none")]
    None,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_type(Type::drop().if_exists().name(SaveTypeEnum).to_owned())
            .await?;
        manager
            .create_type(
                Type::create()
                    .as_enum(SaveTypeEnum)
                    .values(SaveTypeVariants::iter())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MainData::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MainData::SaveId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MainData::SaveType)
                            .enumeration(SaveTypeEnum, SaveTypeVariants::iter())
                            .not_null(),
                    )
                    // blake hash:
                    // ad4a4c99162bac6766fa9a658651688c6db4955922f8e5447cb14ad1c1b05825
                    // len = 64
                    .col(ColumnDef::new(MainData::BlakeHash).char_len(64).not_null())
                    .col(ColumnDef::new(MainData::Len).integer().not_null())
                    .col(ColumnDef::new(MainData::ShortData).string_len(1024))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MainData::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum MainData {
    /// 表
    Table,
    /// 这个存档的 Id
    SaveId,
    /// 存档类型
    /// - ship: 船
    /// - save: 存档
    /// - unknown: 未知 (没下载呢)
    /// - none: 没有存档 (这个 Id 为 空)
    SaveType,
    /// blake3 hash
    /// len = 64
    /// 64 位的 blake3 hash
    BlakeHash,
    /// 存档的长度 (用来过滤太长的存档)
    /// 长度 > 1024 的存档会存在隔壁表
    Len,
    /// 如果长度 < 1024
    /// 那就直接存在这
    ShortData,
}
