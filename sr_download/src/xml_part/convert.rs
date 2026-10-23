//! wire 与业务模型之间的值转换规则。
//!
//! 各领域分别实现既有 `From` 转换；这里不解析 XML、不访问文件、不执行校验。
//! 数值布尔按“非零为真”读取，写为 0/1。model 会失去部分字段的缺失状态：
//! 部件的 flippedX/flippedY/activated/exploded 缺失时为 false，写回显式 0；
//! 其他可选属性继续保持 None 并省略。未知 XML 字段不进入 model。
//!
//! 这些是现有 schema 的兼容规则，不根据游戏参考补字段或改变默认值。

mod part;
mod save;
mod ship;
mod staging;

fn i8_to_bool(value: i8) -> bool {
    value != 0
}

fn bool_to_i8(value: bool) -> i8 {
    if value { 1 } else { 0 }
}
