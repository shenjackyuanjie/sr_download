use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait};
use tracing::{event, Level};

use crate::config::ConfigFile;

pub mod pre_local {
    use super::*;
    use crate::db_part::defines::check_sea_orm_exists;

    pub async fn try_merge(db: &DatabaseConnection, conf: &ConfigFile) {
        event!(Level::INFO, "尝试从 sea_orm 表迁移数据");
        if !check_sea_orm_exists(db, conf).await {
            // 如果没有这个表, 那就说明已经是 merge 过了
            event!(Level::DEBUG, "sea_orm 表不存在, 不需要迁移");
        }
        event!(Level::DEBUG, "sea_orm 表存在, 开始迁移");
        // 先开个事物
        let transaction = match db.begin().await {
            Ok(t) => t,
            Err(e) => {
                event!(Level::ERROR, "无法开启事务: {:?}", e);
                return;
            }
        };

        // 迁移数据

        // 提交事务
        if let Err(e) = transaction.commit().await {
            event!(Level::ERROR, "无法提交事务: {:?}", e);
        }
    }
}

pub async fn update_db(db: &DatabaseConnection, conf: &ConfigFile) {
    event!(Level::INFO, "开始更新数据库");

    // 开启全局事务
    let global_transaction = match db.begin().await {
        Ok(t) => t,
        Err(e) => {
            event!(Level::ERROR, "无法开启全局事务: {:?}", e);
            return;
        }
    };

    pre_local::try_merge(db, conf).await;

    // 提交全局事务
    if let Err(e) = global_transaction.commit().await {
        event!(Level::ERROR, "无法提交全局事务, 更新失败: {:?}", e);
    }

    event!(Level::INFO, "更新完成");
}
