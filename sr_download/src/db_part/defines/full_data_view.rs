use sea_orm::DeriveIden;

pub const FULL_DATA_SQL: &str = r"
CREATE OR REPLACE VIEW full_data as
SELECT
	md.save_id,
	md.save_type,
	md.blake_hash,
    md.xml_tested,
	md.len,
	CASE
		WHEN md.len > 1024 THEN
			ld.text
		ELSE md.short_data
	END AS data
FROM main_data md
LEFT JOIN long_data ld ON md.save_id = ld.save_id
";

#[derive(DeriveIden)]
pub enum FullData {
    /// 表 (实际上是个视图)
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
    Len,
    /// 完整的数据
    Data,
    /// 数据是不是合法的 XML 数据
    XmlTested,
}
