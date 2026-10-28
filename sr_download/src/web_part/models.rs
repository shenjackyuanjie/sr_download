use std::io::Write;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::{
    SaveId,
    db_part::{DbData, SaveType, utils},
    net::DownloadFile,
    web_part::{api_request_counter, service_uptime, web_request_counter},
};

#[derive(Serialize, Deserialize)]
pub struct LastData {
    pub save_id: SaveId,
    pub save_type: String,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

impl LastData {
    pub async fn from_db_by_id(db: &PgPool, id: SaveId) -> Option<Self> {
        let data = DbData::from_db(id, db).await?;
        let xml_tested = data.verify_xml();
        Some(Self {
            save_id: data.save_id,
            save_type: data.save_type.to_string(),
            len: data.len,
            blake_hash: data.blake_hash,
            xml_tested,
        })
    }

    pub fn from_file(file: &DownloadFile, id: SaveId) -> Self {
        let xml_tested = utils::verify_xml(file.ref_data()).is_ok();
        let save_type = file.save_type().to_string();
        let len = file.len();
        let blake_hash = {
            let mut hasher = blake3::Hasher::new();
            let _ = hasher.write(file.ref_data().as_bytes());
            hasher.finalize().to_hex().to_string()
        };
        Self {
            save_id: id,
            save_type,
            len: len as i64,
            blake_hash,
            xml_tested,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LastSave {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

#[derive(Serialize, Deserialize)]
pub struct LastShip {
    pub save_id: SaveId,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
}

#[derive(Serialize, Deserialize)]
pub struct RawData {
    pub info: LastData,
    pub raw_data: String,
}

impl RawData {
    pub async fn from_db_by_id(db: &PgPool, id: SaveId) -> Option<Self> {
        let data = DbData::from_db(id, db).await?;
        let xml_tested = data.verify_xml();
        Some(Self {
            info: LastData {
                save_id: data.save_id,
                save_type: data.save_type.to_string(),
                len: data.len,
                blake_hash: data.blake_hash,
                xml_tested,
            },
            raw_data: data.text?,
        })
    }

    pub fn from_file(file: DownloadFile, id: SaveId) -> Self {
        let info = LastData::from_file(&file, id);
        Self {
            info,
            raw_data: file.take_data(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ServiceStatus {
    pub version: String,
    pub web_request_count: u64,
    pub api_request_count: u64,
    pub uptime_human: String,
    pub uptime_seconds: u64,
    pub min_lookup_id: SaveId,
}

impl ServiceStatus {
    pub fn collect() -> Self {
        let uptime = service_uptime();
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            web_request_count: web_request_counter(),
            api_request_count: api_request_counter(),
            uptime_human: humantime::format_duration(uptime).to_string(),
            uptime_seconds: uptime.as_secs(),
            min_lookup_id: 76858,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct DashboardOverview {
    pub latest_data: Option<LastData>,
    pub latest_ship: Option<LastShip>,
    pub latest_save: Option<LastSave>,
    pub service: ServiceStatus,
}

/// 市场列表默认每页条数。
pub const DEFAULT_MARKET_LIMIT: u16 = 25;
/// 单页最多返回的记录条数。
pub const MAX_MARKET_LIMIT: u16 = 100;
/// 一次请求最多接受的条件数量，避免拼接出过长的 SQL。
pub const MAX_MARKET_FILTERS: usize = 16;

/// 市场列表的原始查询参数，字符串在 MarketRequest::parse 中统一校验。
#[derive(Debug, Default, Deserialize)]
pub struct MarketQuery {
    pub page: Option<u32>,
    pub limit: Option<u16>,
    #[serde(rename = "type")]
    pub save_type: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
    pub min_len: Option<i64>,
    pub max_len: Option<i64>,
    pub xml: Option<String>,
    pub logic: Option<String>,
    /// JSON 数组：[{"field":"len","op":"gte","value":1024}, ...]
    pub filters: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketSort {
    Id,
    Len,
    Time,
}

impl MarketSort {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "" | "id" | "save_id" => Ok(Self::Id),
            "len" | "size" => Ok(Self::Len),
            "time" | "recorded_at" => Ok(Self::Time),
            other => Err(format!("不支持的排序字段: {other}，可用 id、len、time")),
        }
    }

    fn sql(self) -> &'static str {
        match self {
            Self::Id => "save_id",
            Self::Len => "len",
            Self::Time => "time",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketLogic {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketField {
    SaveId,
    SaveType,
    Len,
    XmlTested,
    BlakeHash,
    Time,
}

impl MarketField {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "save_id" => Ok(Self::SaveId),
            "save_type" => Ok(Self::SaveType),
            "len" => Ok(Self::Len),
            "xml_tested" => Ok(Self::XmlTested),
            "blake_hash" => Ok(Self::BlakeHash),
            "time" => Ok(Self::Time),
            other => Err(format!(
                "不支持的搜索字段: {other}，可用 save_id、save_type、len、xml_tested、blake_hash、time"
            )),
        }
    }

    fn sql(self) -> &'static str {
        match self {
            Self::SaveId => "save_id",
            // save_type 是 PostgreSQL 枚举，转成文本后与字符串参数比较。
            Self::SaveType => "save_type::text",
            Self::Len => "len",
            Self::XmlTested => "coalesce(xml_tested, false)",
            Self::BlakeHash => "blake_hash",
            Self::Time => "time",
        }
    }

    fn is_text(self) -> bool {
        matches!(self, Self::SaveType | Self::BlakeHash)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketOp {
    Eq,
    Ne,
    Ilike,
    NotIlike,
    Gt,
    Lt,
    Gte,
    Lte,
    IsNull,
    IsNotNull,
}

impl MarketOp {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "eq" => Ok(Self::Eq),
            "ne" => Ok(Self::Ne),
            "ilike" => Ok(Self::Ilike),
            "not_ilike" => Ok(Self::NotIlike),
            "gt" => Ok(Self::Gt),
            "lt" => Ok(Self::Lt),
            "gte" => Ok(Self::Gte),
            "lte" => Ok(Self::Lte),
            "is_null" => Ok(Self::IsNull),
            "is_not_null" => Ok(Self::IsNotNull),
            other => Err(format!(
                "不支持的搜索操作符: {other}，可用 eq、ne、ilike、not_ilike、gt、lt、gte、lte、is_null、is_not_null"
            )),
        }
    }

    fn sql(self) -> Option<&'static str> {
        match self {
            Self::Eq => Some(" = "),
            Self::Ne => Some(" <> "),
            Self::Ilike => Some(" ILIKE "),
            Self::NotIlike => Some(" NOT ILIKE "),
            Self::Gt => Some(" > "),
            Self::Lt => Some(" < "),
            Self::Gte => Some(" >= "),
            Self::Lte => Some(" <= "),
            Self::IsNull | Self::IsNotNull => None,
        }
    }

    fn needs_value(self) -> bool {
        self.sql().is_some()
    }

    fn is_pattern(self) -> bool {
        matches!(self, Self::Ilike | Self::NotIlike)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarketValue {
    Int(i64),
    Text(String),
    Bool(bool),
    Timestamp(DateTime<Utc>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketFilter {
    pub field: MarketField,
    pub op: MarketOp,
    pub value: Option<MarketValue>,
}

#[derive(Debug, Deserialize)]
struct RawFilter {
    field: String,
    op: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
}

/// 校验完成的市场查询条件，SQL 只从这个结构取值。
#[derive(Debug)]
pub struct MarketRequest {
    pub page: u32,
    pub limit: u16,
    pub save_type: Option<SaveType>,
    pub min_len: Option<i64>,
    pub max_len: Option<i64>,
    pub xml_valid: Option<bool>,
    pub sort: MarketSort,
    pub desc: bool,
    pub logic: MarketLogic,
    pub filters: Vec<MarketFilter>,
}

impl MarketRequest {
    pub fn parse(query: MarketQuery) -> Result<Self, String> {
        let page = query.page.unwrap_or(1).max(1);
        let limit = query
            .limit
            .unwrap_or(DEFAULT_MARKET_LIMIT)
            .clamp(1, MAX_MARKET_LIMIT);
        let save_type = match normalized(query.save_type.as_deref()) {
            None | Some("all") => None,
            Some("ship") => Some(SaveType::Ship),
            Some("save") => Some(SaveType::Save),
            Some("unknown") => Some(SaveType::Unknown),
            Some(other) => return Err(format!("不支持的 market 类型: {other}")),
        };
        let sort = MarketSort::parse(query.sort.as_deref().unwrap_or_default())?;
        let desc = match normalized(query.dir.as_deref()) {
            None | Some("desc") => true,
            Some("asc") => false,
            Some(other) => return Err(format!("不支持的排序方向: {other}，可用 asc、desc")),
        };
        let xml_valid = match normalized(query.xml.as_deref()) {
            None | Some("all") | Some("any") => None,
            Some("valid") | Some("true") => Some(true),
            Some("invalid") | Some("false") => Some(false),
            Some(other) => {
                return Err(format!(
                    "不支持的 xml 状态: {other}，可用 all、valid、invalid"
                ));
            }
        };
        for (name, value) in [("min_len", query.min_len), ("max_len", query.max_len)] {
            if value.is_some_and(|value| value < 0) {
                return Err(format!("{name} 不能是负数"));
            }
        }
        if let (Some(min), Some(max)) = (query.min_len, query.max_len)
            && min > max
        {
            return Err(format!("min_len({min}) 不能大于 max_len({max})"));
        }
        let logic = match normalized(query.logic.as_deref()) {
            None | Some("and") => MarketLogic::And,
            Some("or") => MarketLogic::Or,
            Some(other) => return Err(format!("不支持的条件组合方式: {other}，可用 and、or")),
        };
        let filters = match normalized(query.filters.as_deref()) {
            None => Vec::new(),
            Some(raw) => {
                let raw_filters: Vec<RawFilter> = serde_json::from_str(raw)
                    .map_err(|error| format!("filters 不是合法的 JSON 数组: {error}"))?;
                if raw_filters.len() > MAX_MARKET_FILTERS {
                    return Err(format!("一次最多提交 {MAX_MARKET_FILTERS} 个筛选条件"));
                }
                raw_filters
                    .into_iter()
                    .map(parse_filter)
                    .collect::<Result<Vec<_>, _>>()?
            }
        };
        Ok(Self {
            page,
            limit,
            save_type,
            min_len: query.min_len,
            max_len: query.max_len,
            xml_valid,
            sort,
            desc,
            logic,
            filters,
        })
    }
}

fn normalized(raw: Option<&str>) -> Option<&str> {
    raw.map(str::trim).filter(|value| !value.is_empty())
}

fn parse_filter(raw: RawFilter) -> Result<MarketFilter, String> {
    let field = MarketField::parse(&raw.field)?;
    let op = MarketOp::parse(&raw.op)?;
    if op.is_pattern() && !field.is_text() {
        return Err(format!(
            "字段 {} 不支持 {}，请使用 eq、ne、gt、lt、gte、lte",
            raw.field, raw.op
        ));
    }
    if !op.needs_value() {
        return Ok(MarketFilter {
            field,
            op,
            value: None,
        });
    }
    let text = raw
        .value
        .as_ref()
        .and_then(json_scalar)
        .ok_or_else(|| format!("条件 {} 缺少搜索值", raw.field))?;
    let value = match field {
        MarketField::SaveId | MarketField::Len => MarketValue::Int(parse_int(&text, &raw.field)?),
        MarketField::SaveType => {
            let normalized = text.to_lowercase();
            if !matches!(normalized.as_str(), "save" | "ship" | "unknown" | "none") {
                return Err(format!(
                    "数据类型只能是 save、ship、unknown 或 none，收到 {text}"
                ));
            }
            MarketValue::Text(normalized)
        }
        MarketField::BlakeHash => MarketValue::Text(text),
        MarketField::XmlTested => MarketValue::Bool(parse_bool(&text, &raw.field)?),
        MarketField::Time => MarketValue::Timestamp(parse_time(&text)?),
    };
    Ok(MarketFilter {
        field,
        op,
        value: Some(value),
    })
}

fn json_scalar(value: &serde_json::Value) -> Option<String> {
    let text = match value {
        serde_json::Value::String(text) => text.trim().to_string(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(boolean) => boolean.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

fn parse_int(raw: &str, field: &str) -> Result<i64, String> {
    raw.trim()
        .parse::<i64>()
        .map_err(|_| format!("字段 {field} 需要整数，收到 {raw}"))
}

fn parse_bool(raw: &str, field: &str) -> Result<bool, String> {
    match raw.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(format!("字段 {field} 需要 true 或 false，收到 {raw}")),
    }
}

fn parse_time(raw: &str) -> Result<DateTime<Utc>, String> {
    let raw = raw.trim();
    if let Ok(value) = DateTime::parse_from_rfc3339(raw) {
        return Ok(value.with_timezone(&Utc));
    }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(value) = chrono::NaiveDateTime::parse_from_str(raw, format) {
            return Ok(value.and_utc());
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Ok(date
            .and_hms_opt(0, 0, 0)
            .expect("合法日期总能构造午夜时间")
            .and_utc());
    }
    Err(format!(
        "时间格式无效: {raw}，请使用 2026-01-31、2026-01-31 12:00:00 或 RFC3339"
    ))
}

fn push_market_where(query: &mut QueryBuilder<Postgres>, request: &MarketRequest) {
    query.push(" WHERE len > 0 AND save_id >= 0");
    match request.save_type {
        Some(save_type) => {
            query.push(" AND save_type = ").push_bind(save_type);
        }
        None => {
            query.push(" AND save_type <> ").push_bind(SaveType::None);
        }
    }
    if let Some(min_len) = request.min_len {
        query.push(" AND len >= ").push_bind(min_len);
    }
    if let Some(max_len) = request.max_len {
        query.push(" AND len <= ").push_bind(max_len);
    }
    if let Some(xml_valid) = request.xml_valid {
        query
            .push(" AND coalesce(xml_tested, false) = ")
            .push_bind(xml_valid);
    }
    if request.filters.is_empty() {
        return;
    }
    let joiner = match request.logic {
        MarketLogic::And => " AND ",
        MarketLogic::Or => " OR ",
    };
    query.push(" AND (");
    for (index, filter) in request.filters.iter().enumerate() {
        if index > 0 {
            query.push(joiner);
        }
        query.push(filter.field.sql());
        match filter.op.sql() {
            Some(operator) => {
                query.push(operator);
                match filter.value.as_ref().expect("带值操作符一定有值") {
                    MarketValue::Int(value) => {
                        query.push_bind(*value);
                    }
                    MarketValue::Text(value) => {
                        query.push_bind(value.clone());
                    }
                    MarketValue::Bool(value) => {
                        query.push_bind(*value);
                    }
                    MarketValue::Timestamp(value) => {
                        query.push_bind(*value);
                    }
                }
            }
            None => {
                query.push(match filter.op {
                    MarketOp::IsNull => " IS NULL",
                    MarketOp::IsNotNull => " IS NOT NULL",
                    _ => unreachable!("带值操作符在 sql() 中返回操作符"),
                });
            }
        }
    }
    query.push(")");
}

fn market_list_query(request: &MarketRequest) -> QueryBuilder<Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT save_id, save_type, len, blake_hash, xml_tested, time FROM main_data",
    );
    push_market_where(&mut query, request);
    query
        .push(" ORDER BY ")
        .push(request.sort.sql())
        .push(if request.desc { " DESC" } else { " ASC" })
        .push(", save_id DESC LIMIT ")
        .push_bind(i64::from(request.limit))
        .push(" OFFSET ")
        .push_bind(
            i64::from(request.page.saturating_sub(1)).saturating_mul(i64::from(request.limit)),
        );
    query
}

fn market_stats_query(request: &MarketRequest) -> QueryBuilder<Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT count(*)::bigint AS total, COALESCE(sum(len), 0)::bigint AS total_len FROM main_data",
    );
    push_market_where(&mut query, request);
    query
}

#[derive(Debug, Serialize)]
pub struct MarketPage {
    pub records: Vec<MarketRecord>,
    pub page: u32,
    pub limit: u16,
    pub total: i64,
    pub total_len: i64,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct MarketRecord {
    pub save_id: SaveId,
    pub save_type: String,
    pub len: i64,
    pub blake_hash: String,
    pub xml_tested: bool,
    pub recorded_at: String,
}

#[derive(Debug, FromRow)]
struct MarketRow {
    save_id: i32,
    save_type: SaveType,
    len: i64,
    blake_hash: String,
    xml_tested: Option<bool>,
    time: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct MarketStats {
    total: i64,
    total_len: i64,
}

impl From<MarketRow> for MarketRecord {
    fn from(row: MarketRow) -> Self {
        Self {
            save_id: row.save_id as SaveId,
            save_type: row.save_type.to_string(),
            len: row.len,
            blake_hash: row.blake_hash,
            xml_tested: row.xml_tested.unwrap_or(false),
            recorded_at: row.time.to_rfc3339(),
        }
    }
}

impl MarketPage {
    pub async fn from_db(db: &PgPool, request: &MarketRequest) -> Result<Self, sqlx::Error> {
        let rows = market_list_query(request)
            .build_query_as::<MarketRow>()
            .fetch_all(db)
            .await?;
        let stats = market_stats_query(request)
            .build_query_as::<MarketStats>()
            .fetch_one(db)
            .await?;
        let records: Vec<MarketRecord> = rows.into_iter().map(Into::into).collect();
        let seen = i64::from(request.page.saturating_sub(1))
            .saturating_mul(i64::from(request.limit))
            .saturating_add(records.len() as i64);
        Ok(Self {
            records,
            page: request.page,
            limit: request.limit,
            total: stats.total,
            total_len: stats.total_len,
            has_more: seen < stats.total,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct RecordDetail {
    pub info: LastData,
    pub xml_status: String,
    pub raw_data: Option<String>,
}

#[cfg(test)]
mod market_query_tests {
    use super::{
        MarketField, MarketLogic, MarketOp, MarketQuery, MarketRequest, MarketSort, MarketValue,
        SaveType, market_list_query, market_stats_query,
    };
    use chrono::{DateTime, Utc};

    fn request(filters: Option<&str>) -> MarketRequest {
        MarketRequest::parse(MarketQuery {
            filters: filters.map(str::to_string),
            ..Default::default()
        })
        .expect("测试参数应当合法")
    }

    fn sql(request: &MarketRequest) -> String {
        market_list_query(request).sql().as_str().to_string()
    }

    #[test]
    fn defaults_to_first_page_of_newest_records() {
        let request = request(None);
        assert_eq!(request.page, 1);
        assert_eq!(request.limit, super::DEFAULT_MARKET_LIMIT);
        assert_eq!(request.sort, MarketSort::Id);
        assert!(request.desc);
        assert!(request.filters.is_empty());
        assert!(sql(&request).contains("ORDER BY save_id DESC, save_id DESC LIMIT $2 OFFSET $3"));
    }

    #[test]
    fn parses_type_sort_direction_and_size_bounds() {
        let request = MarketRequest::parse(MarketQuery {
            page: Some(4),
            limit: Some(250),
            save_type: Some("ship".to_string()),
            sort: Some("size".to_string()),
            dir: Some("asc".to_string()),
            min_len: Some(1024),
            max_len: Some(2048),
            xml: Some("valid".to_string()),
            ..Default::default()
        })
        .expect("参数应当合法");
        assert_eq!(request.limit, super::MAX_MARKET_LIMIT);
        assert_eq!(request.save_type, Some(SaveType::Ship));
        assert_eq!(request.sort, MarketSort::Len);
        assert!(!request.desc);
        assert_eq!(request.xml_valid, Some(true));
        let sql = sql(&request);
        assert!(sql.contains("save_type = $1"));
        assert!(sql.contains("len >= $2"));
        assert!(sql.contains("len <= $3"));
        assert!(sql.contains("coalesce(xml_tested, false) = $4"));
        assert!(sql.ends_with("ORDER BY len ASC, save_id DESC LIMIT $5 OFFSET $6"));
    }

    #[test]
    fn rejects_unsupported_enum_like_parameters() {
        for query in [
            MarketQuery {
                save_type: Some("planet".to_string()),
                ..Default::default()
            },
            MarketQuery {
                sort: Some("hash".to_string()),
                ..Default::default()
            },
            MarketQuery {
                dir: Some("up".to_string()),
                ..Default::default()
            },
            MarketQuery {
                xml: Some("maybe".to_string()),
                ..Default::default()
            },
            MarketQuery {
                logic: Some("xor".to_string()),
                ..Default::default()
            },
        ] {
            assert!(MarketRequest::parse(query).is_err());
        }
    }

    #[test]
    fn rejects_invalid_size_bounds() {
        let negative = MarketRequest::parse(MarketQuery {
            min_len: Some(-1),
            ..Default::default()
        });
        assert!(negative.unwrap_err().contains("负数"));
        let flipped = MarketRequest::parse(MarketQuery {
            min_len: Some(2048),
            max_len: Some(1024),
            ..Default::default()
        });
        assert!(flipped.unwrap_err().contains("不能大于"));
    }

    #[test]
    fn parses_typed_filter_values() {
        let request = request(Some(
            r#"[
                {"field":"len","op":"gte","value":1024},
                {"field":"blake_hash","op":"ilike","value":"ab%"},
                {"field":"xml_tested","op":"eq","value":"true"},
                {"field":"time","op":"lt","value":"2026-01-31"},
                {"field":"save_id","op":"is_null"}
            ]"#,
        ));
        assert_eq!(request.filters.len(), 5);
        assert_eq!(request.filters[0].field, MarketField::Len);
        assert_eq!(request.filters[0].value, Some(MarketValue::Int(1024)));
        assert_eq!(
            request.filters[1].value,
            Some(MarketValue::Text("ab%".to_string()))
        );
        assert_eq!(request.filters[2].value, Some(MarketValue::Bool(true)));
        assert_eq!(
            request.filters[3].value,
            Some(MarketValue::Timestamp(
                DateTime::parse_from_rfc3339("2026-01-31T00:00:00Z")
                    .expect("合法时间")
                    .with_timezone(&Utc)
            ))
        );
        assert_eq!(request.filters[4].op, MarketOp::IsNull);
        assert_eq!(request.filters[4].value, None);
    }

    #[test]
    fn rejects_invalid_filters() {
        for filters in [
            r#"[{"field":"name","op":"eq","value":"x"}]"#,
            r#"[{"field":"len","op":"ilike","value":"x"}]"#,
            r#"[{"field":"len","op":"gt"}]"#,
            r#"[{"field":"len","op":"gt","value":"abc"}]"#,
            r#"[{"field":"xml_tested","op":"eq","value":"maybe"}]"#,
            r#"[{"field":"time","op":"eq","value":"昨天"}]"#,
            r#"[{"field":"save_type","op":"eq","value":"planet"}]"#,
            "not json",
        ] {
            assert!(
                MarketRequest::parse(MarketQuery {
                    filters: Some(filters.to_string()),
                    ..Default::default()
                })
                .is_err(),
                "应当拒绝条件：{filters}"
            );
        }
    }

    #[test]
    fn combines_filters_with_the_requested_logic() {
        let filters = Some(
            r#"[{"field":"save_id","op":"gt","value":1000},{"field":"len","op":"lt","value":2048}]"#,
        );
        let and_sql = sql(&request(filters));
        assert!(and_sql.contains("AND (save_id > $2 AND len < $3)"));
        let or_request = MarketRequest::parse(MarketQuery {
            filters: filters.map(str::to_string),
            logic: Some("or".to_string()),
            ..Default::default()
        })
        .expect("or 参数应当合法");
        assert_eq!(or_request.logic, MarketLogic::Or);
        assert!(
            market_list_query(&or_request)
                .sql()
                .as_str()
                .contains("AND (save_id > $2 OR len < $3)")
        );
    }

    #[test]
    fn stats_query_reuses_the_where_clause_without_paging() {
        let request = MarketRequest::parse(MarketQuery {
            save_type: Some("save".to_string()),
            filters: Some(r#"[{"field":"len","op":"gte","value":1024}]"#.to_string()),
            ..Default::default()
        })
        .expect("参数应当合法");
        let sql = market_stats_query(&request).sql().as_str().to_string();
        assert!(sql.starts_with("SELECT count(*)::bigint AS total"));
        assert!(sql.contains("save_type = $1"));
        assert!(sql.contains("AND (len >= $2)"));
        assert!(!sql.contains("ORDER BY"));
        assert!(!sql.contains("LIMIT"));
    }
}
