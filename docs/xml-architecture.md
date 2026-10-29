# XML 架构与兼容契约

本轮目标是整理职责、消除重复模型与数据库反向依赖，同时保持已有 XML 公共 API、解析结果和写回格式。行为基线为提交 `15bfc09` 的 XML 实现与当时的 quick-xml 0.41.0；依赖已升级到 0.42.0，两者的唯一输出差异见下文。

## 分层与依赖方向

```text
XML 文本 ── parse ── 私有 codec ── raw 协议模型
                                     ↕ convert
                                  model 领域模型
                                     ↓
                                  write ── codec ── XML 文本

raw 协议模型 ── verify（事件扫描、格式分类、引用与连接图）
DB / HTTP / 统计入口 ── XML 模块
```

- [raw](../sr_download/src/xml_part/raw.rs) 按船只、部件、分级、存档拆分协议类型，定义 XML 字段名、数值类型、必需性与默认值；通过原模块重新导出，保留旧类型路径。
- [model](../sr_download/src/xml_part/model.rs) 是与 XML 字段拼写分离的领域数据，不增加游戏物理归一化。
- [convert](../sr_download/src/xml_part/convert.rs) 是唯一的 Raw↔Model 语义转换层，各领域的转换分别维护。
- [parse](../sr_download/src/xml_part/parse.rs) 负责文本解码、首根类型派发和 UTF-8 文件读取；Raw 固有解析方法作为兼容入口。
- [codec](../sr_download/src/xml_part/codec.rs) 私有适配 Serde 编解码及历史错误映射，不负责校验或业务修复。
- [write](../sr_download/src/xml_part/write.rs) 负责 Raw 输出及 Model→Raw→XML，不排序、不去重、不修正连接或默认游戏状态。
- [verify](../sr_download/src/xml_part/verify.rs) 独立拥有事件扫描和船只语义校验，不依赖数据库或网络。[数据库工具](../sr_download/src/db_part/utils.rs) 保留旧路径的重新导出。

## 有意保持的历史行为

### 读取与校验不是同一契约

1. typed Raw/Model 入口使用原有 Serde 解码策略，不额外强制外层根名。
2. 自动入口只探测第一个根元素，再按 Ship/Runtime 派发；不增加全文预校验，也不改变错误优先级。
3. 历史 verify_xml 检查事件读取与标签闭合，但接受空内容、文本或多个闭合根组成的片段。它不是严格 XML 文档标准验证器。
4. verify_ship 保留五档分类。读取成功不代表引用、ID 或连接图合法；验证根仍要求 DisconnectedParts，不能直接用带默认值的 RawShipDocument 替代。
5. 公开错误枚举、显示前缀和底层错误转换保留旧契约，包括底层 quick_xml::Error 映射到 Serialize 的历史行为。

若后续需要严格单根文档校验，应另立明确 API 与行为变更测试，而不是在重构中替换所有入口。

### 默认值与转换

- 缺失 version/liftedOff/touchingGround/editorAngle 分别保持 1/0/1/0。
- 数值布尔读取仍为 i8，领域层把任何非零值转为 true，写回统一为 0/1。
- flippedX、flippedY、activated、exploded 经 Model 后会由缺失变为显式 0。
- inflate、deployed、rope 保留 Option 缺失与显式 false 的区别。
- 部件、连接、断开组、阶段、激活项与混合存档节点保持原列表顺序。
- 不增加有限浮点、正数 ID、已知部件类型等业务约束。

### 写回不是原文保真

Raw 仅保存已建模字段；未知属性和扩展、注释、原始数值拼写、原始属性顺序与省略状态不保证保留。Model 写回还会执行既有的布尔归一化及默认字段实体化。

因此：

- 原始下载和原始数据接口继续使用原始字符串，不经过 Model 重写。
- Raw 往返、Model 往返、游戏加载再保存，是三种不同操作，不能混用等价性结论。
- 输出沿用 quick-xml 的 Serde 编码格式，不改成游戏的固定六位小数，避免额外精度损失。

## quick-xml 0.42 带来的唯一输出差异

0.42 把属性中的制表符、换行和回车从原字符改为字符引用（`&#9;`、`&#10;`、`&#13;`）。[依赖清单](../sr_download/Cargo.toml) 已升级到 0.42.0，本项目接受这一变化：

- 原行为写出的裸控制字符在重新解析时会被 XML 属性值规范化成空格，`solarSystem="A\tB"` 写出再读入会变成 `"A B"`；字符引用可以无损往返，[往返测试](../sr_download/src/xml_part/tests.rs) 已固定该行为。
- [黄金快照](../sr_download/tests/fixtures/xml_compat.snap) 中只有 `attribute-control-characters` 一组随之变化，其余 15 组输入输出逐字节不变。
- 0.42 的 `QName` 由字节切片改为 `&str`，[verify](../sr_download/src/xml_part/verify.rs)、[parse](../sr_download/src/xml_part/parse.rs)、[net](../sr_download/src/net.rs) 三处适配，不影响对外接口。

## 游戏本体核对结果

参考 PC 1.6.11 的资源和原生二进制静态逻辑。二进制没有可引用的 Ship/Runtime 源码行号，以下使用 VA；未运行或修改游戏。APK 原生库仅用作交叉参考，表中关键差异已由 PC 本体核实。

| 项目 | PC 游戏的读取/保存行为 | 本项目保持的既有行为 |
| --- | --- | --- |
| 缺失 Ship 字段 | Ship::LoadXml，VA 0x454680：version 缺省 0，touchingGround 缺省 false | version=1、touchingGround=true |
| 旧版分级 | PartTree::LoadXml，VA 0x4708B8：version=0 时将根 currentStage/Staging 迁入首个 Pod | 忽略未建模根字段，不自动迁移 |
| 布尔值 | XMLUtil::ToBool，VA 0x426F20：整数非零为真，也接受小写 true/false | i8 数字，不扩充接受范围 |
| 激活项 moved | ShipStaging::LoadXml，VA 0x4828D0：按 Id 查部件，内部 moved 置 true，不读取输入 moved | moved 必填，并保留其布尔语义 |
| 运行态 podId | Runtime::SaveXml，VA 0x450D90：无选中 Pod 时可省略 | 仍要求 podId；缺失仍解码失败 |
| 着陆架状态 | 保存分支 VA 0x4ADBC7 等：lower/raise/length/legAngle | 尚未建模，Raw 写回也会丢弃 |
| 挑战运行态属性 | 派生 Runtime 会保存 MoonTouchDown、FuelUsed/fuelUsed、maxSpeed 及 LowFlyer 进度等 | 当前仅建模基础 Runtime 头和 Nodes，额外状态会丢弃 |
| 浮点输出 | Ship/Runtime 保存使用 %f，即通常六位小数 | f64 与当前 Serde 输出，不改精度策略 |
| editorAngle | PC VA 0x46F560：四向枚举映射到 0、π/2、π、3π/2，并非度数 | 仍保留原始整数，不从 angle 推算或改写 |
| throttle/fuel | 游戏加载会按阈值或容量裁剪 | 仅读取和保留数值，不模拟物理归一化 |
| version 输出 | Ship::SaveXml，VA 0x4547D0：固定输出 1 | 保留输入 version |

游戏资源中的旧版 Ship 确有根级 Staging；名为 Runtime 的游戏资源实际是 TextureAtlas，不能作为存档模板。核对 Runtime 应使用游戏关卡存档或项目已有存档样例。

这些是后续游戏兼容功能的已知缺口，不在本轮保行为重构中默默修复。补齐旧分级、可选 podId、着陆架状态等，应分别定义 API/模型及迁移策略。

## 验证方法

[集成测试](../sr_download/tests/xml_compat.rs) 对多组输入同时检查 typed Raw、typed Model、自动入口、Raw/Model 写回、校验分类和错误类别。[黄金快照](../sr_download/tests/fixtures/xml_compat.snap) 由重构前实现和旧版依赖生成，不以重构后结果覆盖。它还固定控制字符、旧分级、未建模着陆架字段及缺失 podId 的现有行为。

其余离线测试覆盖复杂可选字段、混合连接/节点、断开组、特殊字符、大 ID、阶段顺序，以及两万条边的非递归连接图检测。

```powershell
cargo test --locked --test xml_compat
cargo test --locked --lib xml_part::
```
