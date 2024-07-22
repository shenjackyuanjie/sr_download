pub use sea_orm_migration::prelude::*;

mod m20240719_00001_create_main_data_table;
mod m20240719_00002_create_long_data_table;
mod m20240721_221623_create_indexs;

pub use m20240721_221623_create_indexs::FULL_DATA_VIEW;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240719_00001_create_main_data_table::Migration),
            Box::new(m20240719_00002_create_long_data_table::Migration),
            Box::new(m20240721_221623_create_indexs::Migration),
        ]
    }
}
