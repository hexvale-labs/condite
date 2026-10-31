use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::state::app::HealthStatus;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ControlCommand {
    Status,
    Reload,
    Shutdown,
    SetLogLevel {
        level: String
    },
    DeleteUpload {
        id: uuid::Uuid
    },
    GenerateApiKey {
        per_upload_quota: u64,
        storage_quota: u64
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Success,
    Status(HealthStatus),
    ApiKey {
        api_key: String
    },
    CommandNotImplemented
}

pub struct CommandRequest {
    pub command: ControlCommand,
    pub reply: oneshot::Sender<Result<ControlResponse>>
}