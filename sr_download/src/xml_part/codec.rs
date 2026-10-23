//! 私有 Serde 编解码适配；保持 quick_xml 的默认接受范围和既有错误映射。
//!
//! 这里只在 wire 值与 XML 字符串之间转换，不探测根名、不做整篇预校验，
//! 也不承担读写文件、业务归一化或结构校验。

use serde::{Serialize, de::DeserializeOwned};

use super::error::{XmlError, XmlResult};

pub(super) fn decode<T: DeserializeOwned>(data: &str) -> XmlResult<T> {
    quick_xml::de::from_str(data).map_err(XmlError::from)
}

pub(super) fn encode<T: Serialize>(value: &T) -> XmlResult<String> {
    quick_xml::se::to_string(value).map_err(|error| XmlError::Serialize(error.to_string()))
}
