use anyhow::{Context, ensure};
use quick_xml::{Reader, events::Event};
use reqwest::{Client, ClientBuilder, StatusCode};
use std::time::Duration;
use tracing::{Level, event};

use crate::xml_part::{XmlResult, model::SaveDocument, model::ShipDocument, model::XmlDocument};
use crate::{SaveId, db_part::SaveType};

#[derive(Debug, Clone)]
pub struct Downloader {
    pub client: Client,
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
    pub fn is_empty(&self) -> bool {
        match self {
            DownloadFile::Ship(s) => s.is_empty(),
            DownloadFile::Save(s) => s.is_empty(),
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            DownloadFile::Ship(_) => "Ship",
            DownloadFile::Save(_) => "Save",
        }
    }
    pub fn save_type(&self) -> SaveType {
        match self {
            DownloadFile::Ship(_) => SaveType::Ship,
            DownloadFile::Save(_) => SaveType::Save,
        }
    }

    pub fn ref_data(&self) -> &str {
        match self {
            DownloadFile::Ship(s) => s,
            DownloadFile::Save(s) => s,
        }
    }
    pub fn info(&self) -> String {
        format!("{}: {} bytes", self.type_name(), self.len(),)
    }

    pub fn parse_xml(&self) -> XmlResult<XmlDocument> {
        crate::xml_part::parse::parse_any_xml(self.ref_data())
    }

    pub fn parse_ship_xml(&self) -> XmlResult<ShipDocument> {
        match self.parse_xml()? {
            XmlDocument::Ship(doc) => Ok(doc),
            XmlDocument::Save(_) => Err(crate::xml_part::XmlError::UnexpectedDocumentType {
                expected: "Ship",
                found: "Save",
            }),
        }
    }

    pub fn parse_save_xml(&self) -> XmlResult<SaveDocument> {
        match self.parse_xml()? {
            XmlDocument::Save(doc) => Ok(doc),
            XmlDocument::Ship(_) => Err(crate::xml_part::XmlError::UnexpectedDocumentType {
                expected: "Save",
                found: "Ship",
            }),
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

/// 我也不知道存这么多 UA 干啥
pub const REQUEST_UA: [&str; 4] = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 Edg/138.0.0.0",
    "Mozilla/5.0 (Phone; OpenHarmony 5.1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36 ArkWeb/4.1.6.1 Mobile HuaweiBrowser/5.1.5.352",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.5735.196 Safari/537.36",
    "Mozilla/5.0 (Linux; Android 12;) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.5735.196 HuaweiBrowser/16.0.1.301 Mobile Safari/537.36",
];
pub const EMPTY_SHIP: &str = r#"<Ship version="1" liftedOff="0" touchingGround="0"><DisconnectedParts/><Parts><Part partType="pod-1" id="1" x="0.000000" y="0.750000" angle="0.000000" angleV="0.000000" editorAngle="0"><Pod throttle="0.000000" name=""><Staging currentStage="0"/></Pod></Part></Parts><Connections/></Ship>"#;

/// The upstream service sometimes returns a cookie-check HTML page with a
/// successful HTTP status instead of the requested XML document.
const BAD_HTML_RESPONSE_MARKER: &str = "var isSupportredirect = true;";

fn is_known_bad_html_response(body: &str) -> bool {
    let body = body.trim_start();
    body.starts_with("<html>") && body.contains(BAD_HTML_RESPONSE_MARKER)
}

fn log_rejected_response(endpoint: &str, body: &str) {
    if is_known_bad_html_response(body) {
        event!(
            Level::WARN,
            "rejected known HTML redirect response for {endpoint}, body length {}",
            body.len()
        );
    }
}

/// 单次响应上限，避免错误页面或异常响应无限占用内存。
pub const MAX_DOWNLOAD_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

impl Downloader {
    /// 兼容旧调用方；需要处理初始化错误时使用 [Self::try_new]。
    pub fn new(timeout: Option<Duration>) -> Self {
        Self::try_new(timeout).expect("无法创建 HTTP 下载客户端")
    }

    pub fn try_new(timeout: Option<Duration>) -> anyhow::Result<Self> {
        let timeout = timeout.unwrap_or(DEFAULT_TIMEOUT);
        ensure!(!timeout.is_zero(), "HTTP 超时必须大于 0");
        ensure!(
            std::time::Instant::now().checked_add(timeout).is_some(),
            "HTTP 超时超出平台计时范围"
        );
        let client = ClientBuilder::new()
            .user_agent(format!("sr_download/{}", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(10)))
            .build()
            .context("无法创建 HTTP 下载客户端")?;
        Ok(Self { client })
    }

    pub fn fmt_ship_url(id: SaveId) -> String {
        format!("http://jundroo.com/service/SimpleRockets/DownloadRocket?id={id}")
    }

    pub fn fmt_save_url(id: SaveId) -> String {
        format!("http://jundroo.com/service/SimpleRockets/DownloadSandBox?id={id}")
    }

    async fn fetch(&self, url: &str, root: &str) -> anyhow::Result<Option<String>> {
        let response = self.client.get(url).send().await.context("下载请求失败")?;
        if matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::GONE) {
            return Ok(None);
        }
        let response = response.error_for_status().context("上游返回错误状态")?;
        let body = read_limited_body(response, MAX_DOWNLOAD_BYTES).await?;
        if body.trim().is_empty() || body.trim() == "0" {
            return Ok(None);
        }
        if is_known_bad_html_response(&body) {
            log_rejected_response(root, &body);
            anyhow::bail!("上游返回 HTML 跳转页而非 {root} XML");
        }
        crate::xml_part::verify::verify_xml(&body).context("上游响应不是完整 XML")?;
        let mut reader = Reader::from_str(&body);
        loop {
            match reader.read_event().context("无法读取 XML 根元素")? {
                Event::Start(element) | Event::Empty(element) => {
                    ensure!(
                        element.name().as_ref() == root.as_bytes(),
                        "上游响应根元素不是 {root}"
                    );
                    break;
                }
                Event::Eof => anyhow::bail!("上游响应缺少 XML 根元素"),
                _ => {}
            }
        }
        Ok(Some(body))
    }

    /// 区分真实缺失与请求/响应错误；任一接口返回有效文档即可成功。
    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn download_any(&self, id: SaveId) -> anyhow::Result<Option<DownloadFile>> {
        self.download_any_from_urls(&Self::fmt_ship_url(id), &Self::fmt_save_url(id))
            .await
            .with_context(|| format!("下载 ID {id} 失败"))
    }

    async fn download_any_from_urls(
        &self,
        ship_url: &str,
        save_url: &str,
    ) -> anyhow::Result<Option<DownloadFile>> {
        let ship_error = match self.fetch(ship_url, "Ship").await {
            Ok(Some(body)) => return Ok(Some(DownloadFile::Ship(body))),
            Ok(None) => None,
            Err(error) => Some(error),
        };
        match self.fetch(save_url, "Runtime").await {
            Ok(Some(body)) => Ok(Some(DownloadFile::Save(body))),
            Ok(None) => match ship_error {
                Some(error) => Err(error.context("Ship 接口失败，Save 接口没有数据")),
                None => Ok(None),
            },
            Err(error) => Err(error.context("Save 接口下载失败")),
        }
    }

    /// 保留原 Option API；需要区分失败原因时使用 [Self::download_any]。
    pub async fn try_download_as_any(&self, id: SaveId) -> Option<DownloadFile> {
        match self.download_any(id).await {
            Ok(file) => file,
            Err(error) => {
                event!(Level::WARN, id, error = %error, "下载失败，保留数据库原数据");
                None
            }
        }
    }

    pub async fn download_as_ship(&self, id: SaveId) -> Option<String> {
        self.fetch(&Self::fmt_ship_url(id), "Ship")
            .await
            .unwrap_or_else(|error| {
                event!(Level::WARN, id, error = %error, "Ship 下载失败");
                None
            })
    }

    pub async fn download_as_save(&self, id: SaveId) -> Option<String> {
        self.fetch(&Self::fmt_save_url(id), "Runtime")
            .await
            .unwrap_or_else(|error| {
                event!(Level::WARN, id, error = %error, "Save 下载失败");
                None
            })
    }
}

async fn read_limited_body(
    mut response: reqwest::Response,
    limit: usize,
) -> anyhow::Result<String> {
    ensure!(
        response
            .content_length()
            .is_none_or(|length| length <= limit as u64),
        "上游响应超过 {limit} 字节限制"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.context("读取上游响应失败")? {
        ensure!(
            bytes.len().saturating_add(chunk.len()) <= limit,
            "上游响应超过 {limit} 字节限制"
        );
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).context("上游响应不是 UTF-8")
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new(Some(Duration::from_secs(1)))
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

    #[test]
    fn rejects_known_html_redirect_response() {
        let body = r#"
            <html>
                <head></head>
                <body>
                    <script type="text/javascript">
                        var isSupportredirect = true;
                    </script>
                </body>
            </html>
        "#;
        assert!(is_known_bad_html_response(body));
    }

    #[test]
    fn rejects_only_the_known_html_signature() {
        assert!(!is_known_bad_html_response(
            "<html><body>normal content</body></html>"
        ));
        assert!(!is_known_bad_html_response(
            "<Ship><Parts/><Connections/></Ship>"
        ));
        assert!(!is_known_bad_html_response(
            "prefix <html> var isSupportredirect = true;"
        ));
    }
}

// 与真实上游网络测试独立，测试错误与边界时只连接回环地址。
#[cfg(test)]
mod local_tests {
    use super::*;
    use axum::{Router, body::Body, response::Response, routing::get};
    use tokio::task::JoinHandle;

    struct MockServer {
        base: String,
        task: JoinHandle<()>,
    }
    impl Drop for MockServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    impl MockServer {
        async fn new(ship: (StatusCode, &'static str), save: (StatusCode, &'static str)) -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let app = Router::new()
                .route("/ship", get(move || async move { ship }))
                .route("/save", get(move || async move { save }))
                .route(
                    "/slow",
                    get(|| async {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        "0"
                    }),
                )
                .route(
                    "/invalid-utf8",
                    get(|| async { Response::new(Body::from(vec![0xff, 0xfe])) }),
                );
            let task = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            Self { base, task }
        }
        fn url(&self, path: &str) -> String {
            format!("{}{path}", self.base)
        }
        async fn download(&self, client: &Downloader) -> anyhow::Result<Option<DownloadFile>> {
            client
                .download_any_from_urls(&self.url("/ship"), &self.url("/save"))
                .await
        }
    }

    fn client(timeout: Duration) -> Downloader {
        Downloader {
            client: Client::builder()
                .no_proxy()
                .timeout(timeout)
                .build()
                .unwrap(),
        }
    }

    #[tokio::test]
    async fn local_download_returns_ship_and_falls_back_to_save() {
        let client = client(Duration::from_secs(2));
        let server = MockServer::new(
            (StatusCode::OK, "<Ship><Parts/><Connections/></Ship>"),
            (StatusCode::INTERNAL_SERVER_ERROR, "error"),
        )
        .await;
        assert!(server.download(&client).await.unwrap().unwrap().is_ship());
        let server =
            MockServer::new((StatusCode::OK, " 0\n"), (StatusCode::OK, "<Runtime/>")).await;
        assert!(server.download(&client).await.unwrap().unwrap().is_save());
    }

    #[tokio::test]
    async fn local_missing_is_distinct_from_server_errors() {
        let client = client(Duration::from_secs(2));
        let server =
            MockServer::new((StatusCode::NOT_FOUND, "missing"), (StatusCode::OK, "  \n")).await;
        assert!(server.download(&client).await.unwrap().is_none());
        for status in [
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::FORBIDDEN,
            StatusCode::INTERNAL_SERVER_ERROR,
        ] {
            let server = MockServer::new((status, "failure"), (StatusCode::GONE, "missing")).await;
            assert!(server.download(&client).await.is_err(), "{status}");
        }
    }

    #[tokio::test]
    async fn local_error_response_can_fall_back_to_a_valid_save() {
        let client = client(Duration::from_secs(2));
        for body in [
            "<html><body>maintenance</body></html>",
            "<html>var isSupportredirect = true;</html>",
            "<Runtime/>",
            "<Ship>",
        ] {
            let server =
                MockServer::new((StatusCode::OK, body), (StatusCode::OK, "<Runtime/>")).await;
            assert!(
                server.download(&client).await.unwrap().unwrap().is_save(),
                "{body}"
            );
        }
    }

    #[tokio::test]
    async fn local_html_and_wrong_root_are_not_missing_records() {
        let client = client(Duration::from_secs(2));
        for body in [
            "<html><body>failure</body></html>",
            "not XML",
            "<Runtime/>",
            "<Ship>",
        ] {
            let server = MockServer::new((StatusCode::OK, body), (StatusCode::OK, "0")).await;
            assert!(server.download(&client).await.is_err(), "{body}");
        }
    }

    #[tokio::test]
    async fn local_timeout_is_reported_as_an_error() {
        let client = client(Duration::from_millis(25));
        let server = MockServer::new((StatusCode::OK, "0"), (StatusCode::OK, "0")).await;
        assert!(client.fetch(&server.url("/slow"), "Ship").await.is_err());
    }

    #[tokio::test]
    async fn local_body_limits_and_invalid_encoding_are_rejected() {
        let client = client(Duration::from_secs(2));
        let server = MockServer::new((StatusCode::OK, "<Ship/>"), (StatusCode::OK, "0")).await;
        let response = client.client.get(server.url("/ship")).send().await.unwrap();
        assert!(read_limited_body(response, 3).await.is_err());
        assert!(
            client
                .fetch(&server.url("/invalid-utf8"), "Ship")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn chunked_body_is_limited_without_content_length() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = BufReader::new(socket);
            let mut line = String::new();
            loop {
                assert!(socket.read_line(&mut line).await.unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                line.clear();
            }
            socket.get_mut().write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\n1234\r\n4\r\n5678\r\n0\r\n\r\n").await.unwrap();
        });
        let response = client(Duration::from_secs(2))
            .client
            .get(url)
            .send()
            .await
            .unwrap();
        assert!(response.content_length().is_none());
        assert!(read_limited_body(response, 6).await.is_err());
        task.await.unwrap();
    }

    #[test]
    fn client_rejects_zero_timeout() {
        assert!(Downloader::try_new(Some(Duration::ZERO)).is_err());
        assert!(Downloader::try_new(Some(Duration::MAX)).is_err());
        assert!(Downloader::try_new(None).is_ok());
    }
}
