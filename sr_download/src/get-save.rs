use std::path::Path;

#[allow(unused)]
mod config;
#[allow(unused)]
mod db_part;
#[allow(unused)]
mod model;

use migration::SaveId;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::ConfigFile::read_from_file(Path::new("config.toml")).unwrap();
    let db = db_part::connect(&config).await.unwrap();
    
    let want_get_id = std::env::args().nth(1).ok_or(anyhow::anyhow!("No input"))?.parse::<SaveId>()?;
    
    let data = db_part::get_raw_data(want_get_id, &db).await.ok_or(anyhow::anyhow!("No data"))?;

    println!("{}", data.text.ok_or(anyhow::anyhow!("No text"))?);

    Ok(())
}
