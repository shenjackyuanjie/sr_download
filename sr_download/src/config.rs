use serde::Deserialize;

#[derive(Deserialize)]
pub struct ConfigFile {
    pub db_url: String,
    pub max_connections: u32,
    pub sqlx_logging: bool,
    pub schema_search_path: String,
}
