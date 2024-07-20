use reqwest::Client;
use std::time::Duration;

use crate::{model::sea_orm_active_enums::SaveType, SaveId};

pub struct Downloader {
    pub client: Client,
    timeout: Duration,
}

/// 使用 any 下载下来的文件
pub enum DownloadFile {
    /// 是艘船
    Ship(String),
    /// 是存档
    Save(String),
}

impl DownloadFile {
    pub fn as_ship(&self) -> Option<&str> {
        match self {
            DownloadFile::Ship(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_save(&self) -> Option<&str> {
        match self {
            DownloadFile::Save(s) => Some(s),
            _ => None,
        }
    }
    pub fn is_ship(&self) -> bool {
        matches!(self, DownloadFile::Ship(_))
    }
    pub fn is_save(&self) -> bool {
        matches!(self, DownloadFile::Save(_))
    }
    pub fn take_data(self) -> String {
        match self {
            DownloadFile::Ship(s) => s,
            DownloadFile::Save(s) => s,
        }
    }
    pub fn len(&self) -> usize {
        match self {
            DownloadFile::Ship(s) => s.len(),
            DownloadFile::Save(s) => s.len(),
        }
    }
}

impl From<&DownloadFile> for SaveType {
    fn from(file: &DownloadFile) -> Self {
        match file {
            DownloadFile::Ship(_) => SaveType::Ship,
            DownloadFile::Save(_) => SaveType::Save,
        }
    }
}

impl Downloader {
    pub fn new(timeout: Duration) -> Self {
        Self {
            client: Client::new(),
            timeout,
        }
    }

    pub fn as_ship_url(id: SaveId) -> String {
        format!(
            "http://jundroo.com/service/SimpleRockets/DownloadRocket?id={}",
            id
        )
    }

    pub fn as_save_url(id: SaveId) -> String {
        format!(
            "http://jundroo.com/service/SimpleRockets/DownloadSandBox?id={}",
            id
        )
    }

    /// 尝试用 ship 或者 save 的 API 下载文件
    /// 如果两个都没下载到，返回 None
    /// 如果下载到了，返回 Some(文件内容)
    pub async fn try_download_as_any(&self, id: SaveId) -> Option<DownloadFile> {
        // 先尝试用 ship 的 API 下载
        let ship_url = Self::as_ship_url(id);
        let ship_try = self
            .client
            .get(&ship_url)
            .timeout(self.timeout.clone())
            .send()
            .await;
        if let Ok(ship_try) = ship_try {
            if ship_try.status().is_success() {
                if let Ok(body) = ship_try.text().await {
                    // 再判空
                    if !(body.is_empty() || body == "0") {
                        return Some(DownloadFile::Ship(body));
                    }
                }
            }
        }
        // 否则尝试用 save 的 API 下载
        let save_url = Self::as_save_url(id);
        let save_try = self
            .client
            .get(&save_url)
            .timeout(self.timeout.clone())
            .send()
            .await;
        if let Ok(save_try) = save_try {
            if save_try.status().is_success() {
                if let Ok(body) = save_try.text().await {
                    // 再判空
                    if !(body.is_empty() || body == "0") {
                        return Some(DownloadFile::Save(body));
                    }
                }
            }
        }
        None
    }

    /// 尝试用 ship 的 API 下载文件
    pub async fn download_as_ship(&self, id: SaveId) -> Option<String> {
        let url = Self::as_ship_url(id);
        let try_res = self
            .client
            .get(&url)
            .timeout(self.timeout.clone())
            .send()
            .await;
        if let Ok(try_res) = try_res {
            if try_res.status().is_success() {
                if let Ok(body) = try_res.text().await {
                    if !(body.is_empty() || body == "0") {
                        return Some(body);
                    }
                }
            }
        }
        None
    }

    /// 尝试用 save 的 API 下载文件
    pub async fn download_as_save(&self, id: SaveId) -> Option<String> {
        let url = Self::as_save_url(id);
        let try_res = self
            .client
            .get(&url)
            .timeout(self.timeout.clone())
            .send()
            .await;
        if let Ok(try_res) = try_res {
            if try_res.status().is_success() {
                if let Ok(body) = try_res.text().await {
                    if !(body.is_empty() || body == "0") {
                        return Some(body);
                    }
                }
            }
        }
        None
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new(Duration::from_secs(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIP_144444: &str = r#"<Ship currentStage="0" throttle="0.000000" liftedOff="0">
    <Parts>
        <Part partType="pod-1" id="1" x="-4.000000" y="3.250000" angle="0.000000" angleV="0.000000" activated="0" exploded="0"/>
    </Parts>
    <Connections/>
    <Staging/>
</Ship>
"#;

    const SAVE_1294489: &str = include_str!("./save_1294489.xml");

    #[tokio::test]
    async fn ship_as_any_download_test() {
        let downloader = Downloader::default();
        let body = downloader.try_download_as_any(144444).await;
        assert!(body.is_some());
        let body = body.unwrap();
        assert!(body.is_ship());
        assert_eq!(body.as_ship().unwrap(), SHIP_144444);
    }

    #[tokio::test]
    async fn save_as_any_download_test() {
        let downloader = Downloader::default();
        let body = downloader.try_download_as_any(1294489).await;
        assert!(body.is_some());
        let body = body.unwrap();
        assert!(body.is_save());
        assert_eq!(body.as_save().unwrap(), SAVE_1294489);
    }

    #[tokio::test]
    async fn ship_download_test() {
        let downloader = Downloader::default();
        let body = downloader.download_as_ship(144444).await;
        assert!(body.is_some());
        let body = body.unwrap();
        assert_eq!(body, SHIP_144444);
    }

    #[tokio::test]
    async fn save_download_test() {
        let downloader = Downloader::default();
        let body = downloader.download_as_save(1294489).await;
        assert!(body.is_some());
        let body = body.unwrap();
        assert_eq!(body, SAVE_1294489);
    }

    #[tokio::test]
    async fn ship_faild_test() {
        let downloader = Downloader::default();
        let body = downloader.download_as_ship(0).await;
        assert!(body.is_none());
    }

    #[tokio::test]
    async fn save_faild_test() {
        let downloader = Downloader::default();
        let body = downloader.download_as_save(0).await;
        assert!(body.is_none());
    }
}
