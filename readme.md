# SR 存档下载计划

## V2

Rewritten in Rust !

可以使用 `cargo build --release --target x86_64-unknown-linux-musl` 来构建 musl 版本的二进制文件

### 运行模式

- `sr_download --serve`：持续下载并可选启动 Web 服务。
- `sr_download --fast`：批量同步指定 ID 区间。
- `sr_download --maintenance`：显式执行缺失数据补全和 `xml_tested` 修复；该模式不会启动下载或 Web 服务。维护按每页 100 条顺序执行，以限制上游请求和数据库写锁。下载或保存失败时保留原数据、继续处理其他页面，并以错误状态结束，`xml_tested` 修复不会在缺失数据补全失败后运行。收到 Ctrl-C 后会完成当前页、不再开始下一页或后续修复任务，并关闭连接池退出。

普通 `--serve` 和 `--fast` 只初始化所需 schema，不会执行全表修复。数据库连接池可在 `[db]` 中通过 `acquire_timeout_secs`、`idle_timeout_secs` 和 `statement_timeout_secs` 配置超时；`config_template.toml` 仅包含占位凭据，复制后请替换。

### API

现在支持提供 api 了

具体 API 请参考 [这个页面](./pages.md)

## V1

已经删掉了
