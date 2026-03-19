use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct WebResponse<T> {
    pub code: u32,
    pub msg: String,
    pub data: Option<T>,
}

impl<T> WebResponse<T> {
    pub fn new(status: StatusCode, msg: impl ToString, data: Option<T>) -> Self {
        Self {
            code: status.as_u16() as u32,
            msg: msg.to_string(),
            data,
        }
    }

    pub fn new_with_data(data: Option<T>) -> Self {
        match data {
            Some(data) => Self::new_normal(data),
            None => Self::new_missing("internal error?".to_string()),
        }
    }

    pub fn new_normal(data: T) -> Self {
        Self {
            code: StatusCode::OK.as_u16() as u32,
            msg: "ok".to_string(),
            data: Some(data),
        }
    }

    pub fn new_missing(msg: impl ToString) -> Self {
        Self {
            code: StatusCode::NOT_FOUND.as_u16() as u32,
            msg: msg.to_string(),
            data: None,
        }
    }

    pub fn new_error(status: StatusCode, msg: impl ToString) -> Self {
        Self {
            code: status.as_u16() as u32,
            msg: msg.to_string(),
            data: None,
        }
    }
}
