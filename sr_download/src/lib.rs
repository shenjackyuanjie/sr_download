use std::{sync::OnceLock, time::SystemTime};

pub mod config;
pub mod db_part;
pub mod fast_mode;
pub mod net;
pub mod serve_mode;
pub mod web_part;
pub mod xml_part;

pub use db_part::SaveId;
pub use net::Downloader;

pub static START_TIME: OnceLock<SystemTime> = OnceLock::new();
