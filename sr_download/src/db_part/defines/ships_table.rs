use sea_orm::sea_query::{ColumnDef, ForeignKey, Table};
use sea_orm::{DatabaseBackend, DeriveIden, ForeignKeyAction, Iden, Statement};

use super::main_data_table::MainData;

pub fn ships_table() -> Statement {
    let mut table = Table::create();
    table
        .table(Ships::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(Ships::SaveId)
                .integer()
                .not_null()
                .primary_key(),
        )
        .foreign_key(
            ForeignKey::create()
                .name(Ships::SaveId.to_string())
                .to(MainData::Table, MainData::SaveId)
                .from(Ships::Table, Ships::SaveId)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade),
        )
        .col(ColumnDef::new(Ships::Mass).big_integer().not_null())
        .col(ColumnDef::new(Ships::ModUsed).boolean().not_null())
        .col(
            ColumnDef::new(Ships::DocxConnectionUsed)
                .boolean()
                .not_null(),
        );
        // .col(ColumnDef::new(Ships::XmlData));
    DatabaseBackend::Postgres.build(&table)
}

#[derive(DeriveIden)]
pub enum Ships {
    Table,
    /// 存档 ID
    SaveId,
    /// 解析过的 xml 数据
    /// 这样就不用每次解析一遍了
    XmlData,
    /// 飞船质量
    /// (按照原版数据计算, 经过比例缩放)
    Mass,
    /// 是否检查到了使用 mod 的迹象
    /// 比如 多 pod 之类的
    ModUsed,
    /// 是否有 docxConnection (xml使用迹象)
    DocxConnectionUsed,
}
