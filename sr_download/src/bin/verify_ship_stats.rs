use std::{collections::BTreeMap, io::Write, path::PathBuf, time::Instant};

use clap::Parser;
use colored::Colorize;
use sqlx::{PgPool, Row};
use tracing::Level;

use sr_download::{
    config::ConfigFile,
    db_part::utils::{ShipVerifyState, connect_server, verify_ship},
};

#[derive(Parser, Debug)]
#[command(
    name = "verify_ship_stats",
    about = "Scan database XML/ship verification states and print a summary table"
)]
struct Cli {
    #[arg(short = 'c', long = "config", default_value = "./config.toml")]
    config: String,

    #[arg(short = 'b', long = "batch-size", default_value_t = 10_000)]
    batch_size: i64,
}

#[derive(Default)]
struct VerifyStats {
    total_rows: u64,
    rows_with_data: u64,
    missing_data: u64,
    valid_xml: u64,
    states: BTreeMap<&'static str, u64>,
}

impl VerifyStats {
    fn record_missing(&mut self) {
        self.total_rows += 1;
        self.missing_data += 1;
    }

    fn record_state(&mut self, state: ShipVerifyState) {
        self.total_rows += 1;
        self.rows_with_data += 1;
        if !matches!(state, ShipVerifyState::NotXml) {
            self.valid_xml += 1;
        }
        *self.states.entry(state.as_str()).or_default() += 1;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let cli = Cli::parse();
    ConfigFile::init_global(Some(PathBuf::from(cli.config)));
    let conf = ConfigFile::get_global();
    let db = connect_server(conf).await?;

    let started_at = Instant::now();
    let stats = scan_db(&db, cli.batch_size).await?;

    print_summary(&stats, started_at.elapsed());
    Ok(())
}

async fn scan_db(db: &PgPool, batch_size: i64) -> anyhow::Result<VerifyStats> {
    let mut stats = VerifyStats::default();
    let total_rows = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM full_data")
        .fetch_one(db)
        .await? as u64;
    let mut last_save_id = 0i32;
    let mut batch_index = 0u64;
    let started_at = Instant::now();

    loop {
        let rows = sqlx::query(
            "SELECT save_id, data
             FROM full_data
             WHERE save_id > $1
             ORDER BY save_id
             LIMIT $2",
        )
        .bind(last_save_id)
        .bind(batch_size)
        .fetch_all(db)
        .await?;

        if rows.is_empty() {
            break;
        }

        let batch_start_id = rows
            .first()
            .and_then(|row| row.try_get::<i32, _>("save_id").ok())
            .unwrap_or(last_save_id);
        for row in rows {
            last_save_id = row.try_get::<i32, _>("save_id")?;
            let data = row.try_get::<Option<String>, _>("data")?;
            match data {
                Some(text) if !text.is_empty() => {
                    let state = verify_ship(&text);
                    stats.record_state(state);
                }
                _ => {
                    stats.record_missing();
                }
            }
        }

        batch_index += 1;
        render_progress(
            batch_index,
            batch_start_id,
            last_save_id,
            total_rows,
            &stats,
            started_at.elapsed(),
        );
    }

    Ok(stats)
}

fn render_progress(
    batch_index: u64,
    batch_start_id: i32,
    last_save_id: i32,
    total_rows: u64,
    total_stats: &VerifyStats,
    elapsed: std::time::Duration,
) {
    const PROGRESS_LINES: usize = 14;

    if batch_index > 1 {
        print!("\r\x1B[{}A\x1B[J", PROGRESS_LINES);
    }
    let progress = if total_rows == 0 {
        0.0
    } else {
        total_stats.total_rows as f64 * 100.0 / total_rows as f64
    };
    let speed = if elapsed.as_secs_f64() <= f64::EPSILON {
        0.0
    } else {
        total_stats.total_rows as f64 / elapsed.as_secs_f64()
    };
    let eta = if speed <= f64::EPSILON || total_stats.total_rows >= total_rows {
        std::time::Duration::from_secs(0)
    } else {
        std::time::Duration::from_secs_f64((total_rows - total_stats.total_rows) as f64 / speed)
    };

    println!("{}", "飞船校验统计".bold().cyan());
    println!(
        "{} {}  {} {}",
        "批次".bold(),
        batch_index.to_string().yellow(),
        "区间".bold(),
        format!("{batch_start_id}..{last_save_id}").yellow()
    );
    println!(
        "{} {}/{}  {} {:>6.2}%",
        "已扫描".bold(),
        total_stats.total_rows.to_string().green(),
        total_rows.to_string().green(),
        "进度".bold(),
        progress
    );
    println!(
        "{} {}  {} {}/s  {} {}",
        "耗时".bold(),
        format_duration(elapsed).blue(),
        "速度".bold(),
        format_count(total_stats.total_rows as f64 / elapsed.as_secs_f64().max(0.001)),
        "剩余".bold(),
        format_duration(eta).blue()
    );
    println!();
    println!("{}", format_header_row("状态", "数量", "占比"));
    println!("{}", "-".repeat(42));
    print_row("缺失数据", total_stats.missing_data, total_stats.total_rows);
    print_row("合法XML", total_stats.valid_xml, total_stats.total_rows);
    print_row(
        "非XML",
        count_for(total_stats, ShipVerifyState::NotXml),
        total_stats.total_rows,
    );
    print_row(
        "非飞船",
        count_for(total_stats, ShipVerifyState::NotShip),
        total_stats.total_rows,
    );
    print_row(
        "伪飞船",
        count_for(total_stats, ShipVerifyState::FakeShip),
        total_stats.total_rows,
    );
    print_row(
        "损坏船",
        count_for(total_stats, ShipVerifyState::BrokenShip),
        total_stats.total_rows,
    );
    print_row(
        "有效船",
        count_for(total_stats, ShipVerifyState::VerifiedShip),
        total_stats.total_rows,
    );
    let _ = std::io::stdout().flush();
}

fn print_summary(stats: &VerifyStats, elapsed: std::time::Duration) {
    println!("{}", "飞船校验汇总".bold().cyan());
    println!("{} {}", "耗时".bold(), format_duration(elapsed).blue());
    println!();
    println!("{}", format_header_row("状态", "数量", "占比"));
    println!("{}", "-".repeat(42));
    print_row("总条数", stats.total_rows, stats.total_rows);
    print_row("缺失数据", stats.missing_data, stats.total_rows);
    print_row("合法XML", stats.valid_xml, stats.total_rows);
    print_row(
        "非XML",
        count_for(stats, ShipVerifyState::NotXml),
        stats.total_rows,
    );
    print_row(
        "非飞船",
        count_for(stats, ShipVerifyState::NotShip),
        stats.total_rows,
    );
    print_row(
        "伪飞船",
        count_for(stats, ShipVerifyState::FakeShip),
        stats.total_rows,
    );
    print_row(
        "损坏船",
        count_for(stats, ShipVerifyState::BrokenShip),
        stats.total_rows,
    );
    print_row(
        "有效船",
        count_for(stats, ShipVerifyState::VerifiedShip),
        stats.total_rows,
    );
}

fn count_for(stats: &VerifyStats, state: ShipVerifyState) -> u64 {
    stats.states.get(state.as_str()).copied().unwrap_or(0)
}

fn print_row(label: &str, count: u64, total: u64) {
    let percent = if total == 0 {
        0.0
    } else {
        count as f64 * 100.0 / total as f64
    };
    let count_text = format_count(count as f64);
    let percent_text = format!("{percent:>6.2}%");
    println!(
        "{} | {} | {}",
        pad_right(label, 12),
        pad_left(&count_text, 10),
        pad_left(&percent_text, 8)
    );
}

fn format_count(value: f64) -> String {
    if value >= 1_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else {
        format!("{:.0}", value)
    }
}

fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn format_header_row(left: &str, middle: &str, right: &str) -> String {
    format!(
        "{} | {} | {}",
        pad_right(left, 12),
        pad_left(middle, 10),
        pad_left(right, 8)
    )
}

fn pad_right(text: &str, width: usize) -> String {
    let pad = width.saturating_sub(display_width(text));
    format!("{text}{}", " ".repeat(pad))
}

fn pad_left(text: &str, width: usize) -> String {
    let pad = width.saturating_sub(display_width(text));
    format!("{}{text}", " ".repeat(pad))
}

fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

fn char_width(c: char) -> usize {
    if c.is_ascii() { 1 } else { 2 }
}
