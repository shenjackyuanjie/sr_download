use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

use crate::m20240719_00001_create_main_data_table::MainData;
use crate::m20240719_00002_create_long_data_table::LongData;

pub const MAIN_SAVETYPE_SAVEID_IDX: &str = "maindata_savetype_saveid_idx";
pub const LONG_SAVEID_IDX: &str = "longdata_saveid_idx";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_index("main_data", MAIN_SAVETYPE_SAVEID_IDX)
            .await?
        {
            let mut dropping_index = Index::drop();
            dropping_index
                .name(MAIN_SAVETYPE_SAVEID_IDX)
                .table(MainData::Table);
            manager.drop_index(dropping_index).await?;
        }

        if manager.has_index("long_data", LONG_SAVEID_IDX).await? {
            let mut dropping_index = Index::drop();
            dropping_index.name(LONG_SAVEID_IDX).table(LongData::Table);
            manager.drop_index(dropping_index).await?;
        }

        let mut savetype_saveid_idx = Index::create();
        savetype_saveid_idx
            .table(MainData::Table)
            .col(MainData::SaveType)
            .col(MainData::SaveId)
            .name(MAIN_SAVETYPE_SAVEID_IDX);
        manager.create_index(savetype_saveid_idx).await?;

        let mut save_type_idx = Index::create();
        save_type_idx
            .table(LongData::Table)
            .col(LongData::SaveId)
            .name(LONG_SAVEID_IDX);
        manager.create_index(save_type_idx).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_index("main_data", MAIN_SAVETYPE_SAVEID_IDX)
            .await?
        {
            let mut dropping_index = Index::drop();
            dropping_index
                .name(MAIN_SAVETYPE_SAVEID_IDX)
                .table(MainData::Table);
            manager.drop_index(dropping_index).await?;
        }

        Ok(())
    }
}
