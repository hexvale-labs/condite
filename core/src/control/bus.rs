use anyhow::{Context, Result};
use tokio::sync::{mpsc, oneshot};

use crate::{
    control::{
        ctl::{CommandRequest, ControlCommand, ControlResponse},
        processor::execute_command,
    },
    state::app::AppState,
};

#[derive(Debug, Clone)]
pub struct CommandBus {
    pub sender: mpsc::Sender<CommandRequest>,
}

impl CommandBus {
    const BUS_BUFFER_SIZE: usize = 256;

    pub async fn execute(&self, command: ControlCommand) -> Result<ControlResponse> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(CommandRequest { command, reply: tx })
            .await
            .context("failed sending command to the processor")?;

        rx.await.context("response channel closed")?
    }

    pub fn new() -> (Self, mpsc::Receiver<CommandRequest>) {
        let (tx, rx) = mpsc::channel::<CommandRequest>(Self::BUS_BUFFER_SIZE);

        (Self { sender: tx }, rx)
    }

    pub fn start(mut receiver: mpsc::Receiver<CommandRequest>, state: AppState) {
        let tasks = state.tasks.clone();
        tasks.spawn(async move {
            loop {
                let request = tokio::select! {
                    biased;
                    _ = state.ct.cancelled() => break,
                    request = receiver.recv() => match request {
                        Some(request) => request,
                        None => break,
                    }
                };

                // Finish an accepted command and reply even if it requests shutdown.
                let result = execute_command(&state, request.command).await;

                let _ = request.reply.send(result);
            }
        });
    }
}
