# 本轮验证记录

验证时间：2026-10-24；本机 Windows，rustc 1.99.0、Cargo 1.99.0。未读取本地数据库凭据，未连接生产数据库或执行实际迁移；游戏参考仅静态读取，未启动或修改游戏。

## 已通过

| 检查 | 结果 |
| --- | --- |
| 全项目 rustfmt | 通过 |
| 全目标 Clippy，警告作为错误 | 通过 |
| 不依赖真实上游的全目标回归 | 99 项通过：库 93、主程序 2、统计程序 3、集成快照 1；六项真实上游测试由命令显式筛除，源码未加 ignore |
| XML 精确快照 | 16 组输入，额外检查历史片段扫描与底层错误分类；与重构前源码生成的结果一致 |
| 文档测试 | 命令通过，当前无可执行文档示例 |
| 两个程序的 help 启动检查 | 通过，不读取配置、不连接数据库 |
| 原网络测试完整性 | 原有整个测试模块与提交 15bfc09 逐字一致（只规范化文件换行后比较） |
| Git 空白检查、文档内部链接 | 通过 |

主要命令：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked -- --skip net::tests::ship_ --skip net::tests::save_
cargo test --workspace --doc --locked
cargo run --locked --bin sr_download -- --help
cargo run --locked --bin verify_ship_stats -- --help
```

## 真实上游网络测试：并非全绿

完整测试命令已实际执行：

```powershell
cargo test --workspace --all-targets --locked
```

六项原网络测试中，两项不存在 ID 测试通过；四项需要下载实际 Ship/Save 的测试没有取得数据，在 body.is_some() 断言失败：

- ship_download_test
- save_download_test
- ship_as_any_download_test
- save_as_any_download_test

为区分本轮改动与环境问题，另以提交 `15bfc09` 的**原下载器源码和原测试**，配合原 reqwest 0.13.4 / Tokio 1.53.1 做了同机对照。相同四项测试失败、相同两项缺失测试通过。此对照沿用当前可解析的其他传递依赖，不宣称完全复刻旧锁文件环境。

这说明失败现象在旧下载逻辑中同样可复现；本次没有通过改测试、改预期、换目标或加 ignore 掩盖。上游可用性仍需要在可达环境独立复核，完整命令当前仍会因上述四项失败而返回非零。

## XML 行为基线

[兼容测试](../sr_download/tests/xml_compat.rs) 逐项比较 typed Raw/Model 读取、自动派发、Raw/Model 写回、验证分类及错误类别；[黄金快照](../sr_download/tests/fixtures/xml_compat.snap) 由上述旧提交的原 XML 实现及 quick-xml 0.41.0 生成，而非用新实现更新预期。

包括：最小 Ship/Runtime、宽松 typed 根名、尾随额外文档、未知扩展、非零布尔与可选缺失、旧根分级、特殊字符/大 ID/阶段顺序、属性控制字符、未建模着陆架/挑战状态、缺失 podId、混合存档节点及错误输入。

原始行为与游戏兼容缺口见 [XML 架构说明](xml-architecture.md)。图算法另外枚举三节点全部 512 种有向图与旧 DFS 对照，并测试两万条边的深链与循环。

## 本轮未执行的验证

- 真实 PostgreSQL DDL、查询计划、并发事务与维护集成。
- Linux/musl 产物构建。
- 游戏中实际加载/保存全部样例。二进制静态核对不是端到端游戏可加载证明。

下一步可在隔离 PostgreSQL 与可达上游环境运行上述检查，再单独定义旧分级、可选 podId、着陆架及挑战状态的游戏兼容改进。不要将这些新行为混入本轮结构重构。
