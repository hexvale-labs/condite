//! Local administrative protocol. One JSON command and response per connection.
use crate::HealthStatus;
use serde::{Deserialize, Serialize};

pub const MAX_CONTROL_FRAME: u64 = 16_384;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlCommand {
    Status,
    Reload,
    Shutdown,
    SetLogLevel {
        level: String,
    },
    DeleteUpload {
        id: String,
    },
    GenerateApiKey {
        per_upload_quota: u64,
        storage_quota: u64,
    },
    UpdateApiKey {
        id: String,
        per_upload_quota: Option<u64>,
        storage_quota: Option<u64>,
    },
    ListApiKeys {
        limit: Option<u32>,
        after: Option<String>,
    },
    InspectApiKey {
        id: String,
    },
    DeleteApiKey {
        id: String,
    },
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Success,
    Status(HealthStatus),
    Keys(crate::ListKeysResponse),
    Key(crate::KeySummary),
    ApiKey { id: String, api_key: String },
    Error { code: String, message: String },
    CommandNotImplemented,
}
