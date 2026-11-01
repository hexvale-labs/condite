use anyhow::Result;
pub use condite_shared::control::{ControlCommand, ControlResponse};
use tokio::sync::oneshot;

pub struct CommandRequest {
    pub command: ControlCommand,
    pub reply: oneshot::Sender<Result<ControlResponse>>,
}
