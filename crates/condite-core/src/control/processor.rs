use std::{io::ErrorKind, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

use crate::{
    control::ctl::{ControlCommand, ControlResponse},
    state::app::AppState,
};

pub async fn execute_command(state: &AppState, command: ControlCommand) -> Result<ControlResponse> {
    tracing::debug!(?command, "Executing control command");
    match command {
        ControlCommand::Status => Ok(ControlResponse::Status(state.health()?)),
        ControlCommand::Shutdown => {
            tracing::info!("Shutdown requested through control socket");
            state.ct.cancel();
            Ok(ControlResponse::Success)
        }
        _ => {
            tracing::warn!(?command, "Control command is not implemented");
            Ok(ControlResponse::CommandNotImplemented)
        }
    }
}

pub async fn handle_sock_request(state: AppState, stream: UnixStream) -> Result<()> {
    let (rx, mut tx) = stream.into_split();
    let mut lines = BufReader::new(rx).lines();

    let line = tokio::select! {
        biased;
        _ = state.ct.cancelled() => return Ok(()),
        line = lines.next_line() => line?.context("couldn't read line")?,
    };
    let command: ControlCommand = serde_json::from_str(line.as_str())?;
    let res = state.bus.execute(command).await?;
    let mut res_bytes = serde_json::to_vec(&res)?;

    res_bytes.push(b'\n');
    // Preserve the shutdown response, but don't let a stalled client hold up exit.
    tokio::time::timeout(Duration::from_secs(5), tx.write_all(&res_bytes))
        .await
        .context("timed out writing socket response")??;

    Ok(())
}

pub async fn serve_sock_supervisor(state: AppState) {
    const RESTART_TIME: u64 = 1;
    let ct = state.ct.clone();

    loop {
        if ct.is_cancelled() {
            return;
        }

        // Let the server finish its async socket cleanup before returning.
        let result = serve_sock(&state).await;
        if ct.is_cancelled() {
            return;
        }

        match result {
            Ok(_) => {
                tracing::error!("Control socket server exited unexpectedly");
            }
            Err(e) => {
                tracing::error!(error = %e, "Control socket server failed");
            }
        }

        // prevent CPU starving if for any reason it ceases to even initialize
        tracing::info!(
            delay_secs = RESTART_TIME,
            "Scheduling control socket server restart"
        );
        tokio::select! {
            biased;
            _ = ct.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(RESTART_TIME)) => {},
        }
    }
}

pub fn get_base_directory() -> PathBuf {
    PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/run/")))
}

async fn serve_sock(state: &AppState) -> Result<()> {
    if state.ct.is_cancelled() {
        return Ok(());
    }

    let directory = get_base_directory().join("condite");
    let sock_path = directory.join("condite.sock");
    let result = async {
        fs::create_dir_all(&directory)
            .await
            .with_context(|| format!("couldn't create {}", directory.display()))?;
        // Remove a socket left behind by a previous run.
        if let Err(e) = fs::remove_file(&sock_path).await
            && e.kind() != ErrorKind::NotFound
        {
            return Err(e)
                .with_context(|| format!("couldn't remove stale socket {}", sock_path.display()));
        }

        let listener = UnixListener::bind(&sock_path)
            .with_context(|| format!("couldn't bind socket in {}", sock_path.display()))?;
        tracing::info!(path = %sock_path.display(), "Control socket listening");

        loop {
            let (stream, _) = tokio::select! {
                biased;
                _ = state.ct.cancelled() => return Ok(()),
                accepted = listener.accept() => accepted?,
            };
            let state = state.clone();
            let tasks = state.tasks.clone();
            tasks.spawn(async move {
                if let Err(e) = handle_sock_request(state, stream).await {
                    tracing::warn!(error = %e, "Control socket request failed");
                }
            });
        }
    }
    .await;

    tracing::debug!(path = %sock_path.display(), "Removing control socket");
    if let Err(e) = fs::remove_file(&sock_path).await
        && e.kind() != ErrorKind::NotFound
    {
        tracing::warn!(path = %sock_path.display(), error = %e, "Failed to remove control socket");
    }
    if let Err(e) = fs::remove_dir(&directory).await
        && e.kind() != ErrorKind::NotFound
    {
        tracing::warn!(
            path = %directory.display(),
            error = %e,
            "Failed to remove control socket directory"
        );
    }

    result
}
