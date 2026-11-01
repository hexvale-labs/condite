//! Responses from the local administrative socket.
use crate::HealthStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Success,
    Status(HealthStatus),
    ApiKey { api_key: String },
    Error { code: String, message: String },
    CommandNotImplemented,
}
