use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ControlCommand {
    Status,
    Reload,
    Shutdown,
    SetLogLevel {
        level: String,
    },
    DeleteUpload {
        id: uuid::Uuid,
    },
    GenerateApiKey {
        per_upload_quota: u64,
        storage_quota: u64,
    },
}

pub use condite_shared::control::ControlResponse;

pub struct CommandRequest {
    pub command: ControlCommand,
    pub reply: oneshot::Sender<Result<ControlResponse>>,
}
