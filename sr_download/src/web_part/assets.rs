use axum::response::IntoResponse;
use reqwest::header;

const FAVICON_FILE: &[u8] = include_bytes!("../../assets/favicon.ico");

pub async fn favicon() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/x-icon")], FAVICON_FILE)
}

const INFO_JS_FILE: &[u8] = include_bytes!("../../assets/info.js");

pub async fn info_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        INFO_JS_FILE,
    )
}

const DARK_JS_FILE: &[u8] = include_bytes!("../../assets/dark.js");

pub async fn dark_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        DARK_JS_FILE,
    )
}

const INFO_CSS_FILE: &[u8] = include_bytes!("../../assets/info.css");

pub async fn info_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], INFO_CSS_FILE)
}

const UA_DISPLAY: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>浏览器UA检测器</title>
    <style>
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            max-width: 800px;
            margin: 2rem auto;
            padding: 1rem;
            line-height: 1.6;
        }
        .ua-container {
            background-color: #f0f0f0;
            border-left: 4px solid #0078d7;
            padding: 1rem;
            margin: 1rem 0;
            overflow-wrap: break-word;
        }
        .highlight {
            color: #0078d7;
            font-weight: bold;
        }
    </style>
</head>
<body>
    <h1>浏览器User-Agent检测器</h1>
    <p>您的浏览器标识信息：</p>
    <div id="uaDisplay" class="ua-container">正在获取UA信息...</div>
    <p>这个信息可以帮助网站开发者了解您使用的<strong class="highlight">浏览器类型</strong>和<strong class="highlight">操作系统版本</strong>。</p>

    <script>
        document.addEventListener('DOMContentLoaded', function() {
            const ua = navigator.userAgent;
            const displayElement = document.getElementById('uaDisplay');

            // 添加UA信息到页面
            displayElement.textContent = ua;

            // 添加分析信息
            displayElement.insertAdjacentHTML('afterend',
                `<p>分析结果：您正在使用
                ${ua.includes('Windows') ? 'Windows' :
                 ua.includes('Mac') ? 'macOS' :
                 ua.includes('Linux') ? 'Linux' : '未知'} 操作系统</p>`);
        });
    </script>
</body>
</html>
"#;

pub async fn ua_display() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html")], UA_DISPLAY)
}
