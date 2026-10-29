# SR 存档下载计划

下载、归档并查询 SimpleRockets 的 Ship / Runtime XML。数据落在 PostgreSQL，可选开启 Web 仪表板、市场页和 HTTP API，XML 读写与语义校验由独立的 `xml_part` 模块承担。

当前版本 1.3.x，行为变更见 [版本日志](update.md)。

## 功能

- **下载归档**：按 ID 抓取 `ship` / `save` 数据，hash 未变则跳过；短数据内联、长数据分表存储。
- **三种运行模式**：持续轮询的 serve 模式、区间批量的 fast 模式、只做数据修复的 maintenance 模式。
- **Web**：仪表板、记录详情、原始 XML 查看、飞船工程分析，以及可按类型、大小、时间、字段条件筛选的市场页。
- **API**：查询、下载、重同步与市场分页检索，见 [API 文档](pages.md)。
- **XML**：结构化的 Raw / Model 双层模型、解析、写回与五档语义校验，见 [XML 架构与兼容契约](docs/xml-architecture.md)。

## 快速开始

需要支持 Rust 2024 edition 的工具链（1.85+，开发与验证使用 1.99.0）和 PostgreSQL。

```powershell
cargo build --release --locked
Copy-Item config_template.toml config.toml
# 编辑 config.toml 填入数据库连接后：
cargo run --release --locked -- --serve --config config.toml
```

`config.toml` 已被 Git 忽略，字段说明见 [配置模板](config_template.toml)。Linux musl 目标可直接交叉构建：

```sh
cargo build --release --locked --target x86_64-unknown-linux-musl
```

HTTP 与 PostgreSQL TLS 都走 rustls，构建不依赖 OpenSSL；交叉构建仍需安装对应的 Rust target 与链接器。

## 运行模式

| 命令 | 行为 |
| --- | --- |
| `--serve` / `-s` | 持续下载最新数据，并从下一轮开始抓取新 ID；开启 `serve.enable` 时同时启动 Web |
| `--fast` / `-f` | 批量同步 `[start_id, end_id)`，结束 ID 不含在区间内 |
| `--maintenance` | 只执行数据维护，不下载也不启动 Web |

三种模式互斥，`--threads` 必须为正整数；配置缺省字段取默认值，启动前校验超时、并发、批次、连接数、ID 范围与 Web 必需项。

共同行为：

- 只初始化所需的 schema；全表数据修复仅在 maintenance 中执行。
- 下载具有总请求超时、连接超时与 32 MiB 单响应上限；网络错误与"记录不存在"分开处理，失败不写空数据覆盖原记录。
- serve 只在保存成功后前移 ID，网络故障时重试同一条而不跳过；fast 复用连接池与 HTTP 客户端，单个任务失败后继续其余任务，最终返回错误状态，可重跑补齐。
- Ctrl-C 停止派发并回收批量任务，再关闭连接池；已开启的事务由 SQLx 回滚。服务模式先停止接收请求，最多等待 10 秒再取消剩余请求。
- maintenance 每页 100 条顺序处理，失败保留原数据并继续后续页面，最后返回错误；Ctrl-C 在完成当前页后退出。

## 配置

配置分四组，单位与取值见 [config_template.toml](config_template.toml)：

| 分组 | 关键字段 | 说明 |
| --- | --- | --- |
| `[db]` | `url`、`schema`、`max_connections` | 连接池上限、获取/空闲/语句超时（秒）、SQLx 日志开关 |
| `[sync]` | `max_timeout`、`serve_wait_time` | 单次请求总超时与 serve 轮询间隔（秒） |
| `[sync.fast]` | `start_id`、`end_id`、`worker_count`、`worker_size` | fast 模式的区间与并发批次 |
| `[serve]` | `host_with_port`、`enable`、`resync_token`、`refresh_interval` | 监听地址、Web 开关、重同步令牌、缓存刷新间隔（毫秒） |

开启 Web 前必须把 `serve.resync_token` 换成非空、非占位值；`/resync/{id}` 需要 `X-Resync-Token` 请求头。

## Web 与 API

主要路由：

| 路由 | 用途 |
| --- | --- |
| `/dashboard` | 仪表板页面（`/` 重定向到此） |
| `/api/overview`、`/api/service` | 总量统计与服务状态 |
| `/api/market` | 市场页分页查询，支持排序、大小/类型/XML 过滤与高级条件 |
| `/api/records/{id}`、`/api/records/{id}/raw`、`/api/records/{id}/analysis` | 记录详情、原始 XML、飞船工程分析 |
| `/last/data`、`/last/save`、`/last/ship`、`/info/{id}`、`/download/{id}` | 早期查询接口，保留原有响应结构 |
| `/resync/{id}` | 重新抓取指定 ID，hash 未变则不改库 |

所有响应沿用 `{"code", "msg", "data"}` 信封，参数、示例与错误码见 [API 文档](pages.md)。网页资源（`info.html`、`info.css`、`info.js`、`dark.js`）通过 `include_str!` 编译进二进制，改前端后需要重新构建才能生效。

## 数据库

```text
main_data     save_id(PK) | save_type(ship/save/unknown/none) | blake_hash | len | short_data(<=1024) | xml_tested | time
long_data     save_id(PK/FK) | len | text            -- 超过 1024 字节的正文
full_data     视图：按 len 选择 long_data.text 或 main_data.short_data
db_version    已应用的迁移版本
```

启动时按 `db_version` 补齐缺失结构，迁移在 advisory lock 下执行。`update_xml_tested()` 用数据库的 `xml_is_well_formed_document` 回填 `xml_tested`。需要整库重建或清理异常 hash 时使用 [重建脚本](rebuild_clean_db.sql)，它带前置校验与显式确认参数，默认只打印统计。

## XML 模块

`xml_part` 分四层：`parse` 解码与首根派发、`codec` 私有 Serde 适配、`raw` 协议模型、`convert` 到 `model` 领域模型；`write` 走同一份 wire schema 输出，`verify` 独立完成事件扫描与船只语义校验。切分原则与逐项保留的历史行为见 [XML 架构与兼容契约](docs/xml-architecture.md)。

已知边界：

- Raw / Model 写回都不是原文保真，未建模的字段、注释和原始数值拼写会丢失；原始数据接口始终返回下载原串。
- 读取成功不等于结构合法：`verify_ship` 保留 not xml / not ship / fake ship / broken ship / verified ship 五档分类。
- 与游戏本体的差异（旧版根级分级、缺失 podId、着陆架状态、挑战运行态字段）已逐项记录，属后续兼容工作，不在解析层静默修复。
- quick-xml 已升级到 0.42：属性内的制表符、换行和回车改为字符引用输出，这是可无损往返的修复；[黄金快照](sr_download/tests/fixtures/xml_compat.snap) 只有该一组随之更新。

## 测试与检查

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings

# 离线回归：本地 HTTP 模拟、XML 快照、调度与数据库边界
cargo test --workspace --all-targets --locked -- --skip net::tests::ship_ --skip net::tests::save_
cargo test --workspace --doc --locked

# 完整测试，包含六项访问 jundroo.com 的真实网络测试
cargo test --workspace --all-targets --locked
```

真实网络测试依赖上游可用性，结果随网络状态变化；离线回归不需要 PostgreSQL，数据库边界测试使用已关闭的 lazy pool。统计已有数据的校验状态分布：

```powershell
cargo run --release --locked --bin verify_ship_stats -- --config config.toml --batch-size 500
```

## 部署

生产环境用 systemd 托管 release 二进制：构建产物替换旧文件后重启服务，启动时会自动补齐数据库结构。升级前建议先备份 `main_data` / `long_data`，升级后访问 `/api/overview` 确认服务健康。

## 目录结构

```text
sr_download/
├── src/
│   ├── main.rs            CLI 与运行模式选择
│   ├── config.rs          配置解析、默认值与校验
│   ├── net.rs             上游下载、超时与响应判定
│   ├── serve_mode.rs      serve 主循环
│   ├── fast_mode.rs       fast 批量同步
│   ├── db_part/           连接池、schema 迁移、维护与查询
│   ├── web_part/          axum 路由、处理器、模型、飞船分析
│   ├── xml_part/          XML 解析、写回、校验
│   └── bin/verify_ship_stats.rs  校验状态统计工具
├── assets/                info.html / info.css / info.js / dark.js / favicon
└── tests/                 XML 兼容快照与黄金样本
docs/                      架构与验证记录
pages.md                   HTTP API 文档
config_template.toml       配置模板
rebuild_clean_db.sql       整库重建脚本
update.md                  版本日志
```

## 相关文档

- [API 文档](pages.md)
- [XML 架构与游戏兼容契约](docs/xml-architecture.md)
- [验证记录](docs/verification.md)
- [版本日志](update.md)
