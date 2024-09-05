use sea_orm::sea_query::{ColumnDef, Table};
use sea_orm::{DatabaseBackend, DeriveIden, EnumIter, Iterable, Statement};

use super::TEXT_DATA_MAX_LEN;

pub fn main_table() -> Statement {
    let mut table = Table::create();
    table
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
        .col(ColumnDef::new(MainData::Len).big_integer().not_null())
        .col(ColumnDef::new(MainData::ShortData).string_len(TEXT_DATA_MAX_LEN as u32))
        .col(ColumnDef::new(MainData::XmlTested).boolean().null())
        .col(
            ColumnDef::new(MainData::Time)
                .timestamp_with_time_zone()
                .not_null(),
        );
    DatabaseBackend::Postgres.build(&table)
}

#[derive(DeriveIden)]
#[sea_orm(iden = "save_type")]
pub struct SaveTypeEnum;

#[derive(DeriveIden, EnumIter)]
pub enum SaveTypeVariants {
    #[sea_orm(iden = "ship")]
    /// 飞船
    Ship,
    #[sea_orm(iden = "save")]
    /// 存档
    Save,
    #[sea_orm(iden = "unknown")]
    /// 未知 (预计用作下载中的占位符)
    Unknown,
    #[sea_orm(iden = "none")]
    /// 没有存档 (这个 Id 为 空)
    None,
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
    /// 数据是不是合法的 XML 数据
    XmlTested,
    /// 加入数据的时间
    /// 用于记录一些有趣的东西
    /// 加入于 版本号 2
    Time,
}
