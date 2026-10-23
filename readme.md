# SR 存档下载计划

使用 Rust 下载、归档和查询 SimpleRockets 的 Ship / Runtime XML，数据库为 PostgreSQL，可选提供 Web 仪表板与 API。

## 构建与配置

使用支持 Rust 2024 edition 的稳定工具链：

```powershell
cargo build --release --locked
Copy-Item config_template.toml config.toml
```

复制 [配置模板](config_template.toml) 后，填写数据库连接信息；本地配置已由 Git 忽略。Linux musl 构建可使用：

```sh
cargo build --release --locked --target x86_64-unknown-linux-musl
```

HTTP 与 PostgreSQL TLS 使用 rustls；交叉构建仍需安装相应 Rust target、链接器及原生编译工具。

### 运行模式

```powershell
# 持续下载，可按配置启动 Web 服务
cargo run --release --locked -- --serve --config config.toml

# 批量同步 [start_id, end_id)，结束 ID 不包含在区间内
cargo run --release --locked -- --fast --config config.toml

# 仅显式执行数据维护，不启动下载或 Web 服务
cargo run --release --locked -- --maintenance --config config.toml
```

三个模式互斥；`--threads` 必须为正整数。配置支持省略字段并使用默认值，启动前校验超时、并发、批次、数据库连接数、ID 范围和 Web 必需项。时间单位见 [配置模板](config_template.toml)。

- 普通 serve / fast 只初始化所需 schema，不执行全表数据修复。
- fast 复用 HTTP 客户端，限制并发与实际批次边界；下载或保存失败后继续其余任务，最终返回错误状态，可重跑补齐。
- serve 只在成功保存后前移 ID，网络故障会等待后重试同一条，不跳过记录。
- 下载具有总请求/连接超时与 32 MiB 单响应限制；网络错误与真实缺失在新的 Result API 中区分，失败不写空数据覆盖原记录。
- Ctrl-C 会停止派发、取消并回收批量任务，再关闭连接池；取消的数据库事务由 SQLx 回滚。服务模式通知 Web 停止接收请求，最多等待 10 秒后取消剩余请求。
- maintenance 每页 100 条顺序处理；下载/保存失败保留原数据并继续其他页面，最终返回错误；缺失补全失败后不执行 xml_tested 修复。Ctrl-C 完成当前页后退出，不开始下一页或下一项修复。

### Web 与 API

开启 Web 前将 `serve.resync_token` 换成非空、非占位值；默认关闭 Web。重同步需要 `X-Resync-Token` 请求头。Web 刷新间隔单位为**毫秒**，数据库获取/空闲/语句超时单位为秒。

API 用法见 [API 文档](pages.md)。原有 JSON envelope、状态字段与路由继续保留；非法或超出 PostgreSQL integer 范围的 ID 在进入数据库前被拒绝。

## XML 架构与兼容性

重构将协议模型、领域模型、转换、编解码和语义验证分离，验证器不再由数据库层实现。旧的公开类型与调用路径保留。

- 维持既有默认值、宽松 typed 解析、首根派发、错误类别、字段顺序和布尔归一化。
- Raw / Model 写回均不是原文保真；未知字段、注释等未建模内容可能丢失。原始下载数据继续作为原始字符串存储和返回，不通过 Model 重写。
- quick-xml 保留在 0.41 兼容线，因为 0.42 改变属性控制字符的输出；其他适用依赖与锁文件已升级。
- 已参考游戏本体核对默认值、旧版分级、Pod、着陆架及运行态字段差异。现有游戏兼容缺口单独记录，不混入本轮保行为重构。

详见 [XML 架构与游戏兼容契约](docs/xml-architecture.md)。[行为集成测试](sr_download/tests/xml_compat.rs) 与 [黄金快照](sr_download/tests/fixtures/xml_compat.snap) 对照重构前源码的结果，覆盖读取、写回、错误类别与验证分类。

## 验证

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings

# 不依赖真实上游：含本地 HTTP 模拟、XML 快照、调度和数据库边界测试
cargo test --workspace --all-targets --locked -- --skip net::tests::ship_ --skip net::tests::save_
cargo test --workspace --doc --locked

# 完整测试，包含保留原样的六项真实上游网络测试
cargo test --workspace --all-targets --locked
```

真实网络测试会访问 jundroo.com，受上游内容和网络状态影响。本地回归不需要 PostgreSQL；数据库边界测试使用已关闭的 lazy pool，不会访问实际数据库。

真实 PostgreSQL 的迁移、查询计划与并发行为需要独立集成环境验证。本轮实测结果与原版网络对照见 [验证记录](docs/verification.md)。
