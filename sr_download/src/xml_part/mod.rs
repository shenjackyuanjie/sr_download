//! SimpleRockets XML 的兼容读写边界。
//!
//! - [`raw`]：按领域组织的 Serde wire 模型，以及历史 `from_str` 转发入口。
//! - [`parse`]：直接 Serde 解码、首根类型分派与文件读取，不附加全篇门禁。
//! - [`convert`]：wire 与 [`model`] 的值转换，集中维护默认/数值布尔规则。
//! - [`write`]：经私有 codec 序列化同一套 wire schema，不复制 writer schema。
//! - [`verify`]：业务结构校验，与读取/写入保持独立。
//!
//! raw 往返不是原 XML 字节保真；默认值会写出，未知结构体字段会丢弃。
//! model 往返还会规范化数值布尔、补写已折叠为 bool 的部件状态，
//! 其余 None 字段按原 schema 省略。游戏参考用于核对，不作为补字段或改 schema 的依据。
//! 所有既有 public 模块路径、类型字段和读写入口保持可用。

mod codec;

pub mod convert;
pub mod error;
pub mod model;
pub mod parse;
pub mod raw;
pub mod verify;
pub mod write;

pub use error::{XmlError, XmlResult};

#[cfg(test)]
mod tests;
