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
    match command {
        ControlCommand::Status => Ok(ControlResponse::Status(state.health()?)),
        ControlCommand::Shutdown => {
            state.ct.cancel();
            Ok(ControlResponse::Success)
        }
        _ => Ok(ControlResponse::CommandNotImplemented),
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
                tracing::error!("sock server somehow exited normally, restarting");
            }
            Err(e) => {
                tracing::error!("sock server error: {}", e);
            }
        }

        // prevent CPU starving if for any reason it ceases to even initialize
        tracing::info!("restarting sock server in {}s", RESTART_TIME);
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
        tracing::info!("created sock at {}", sock_path.display());

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
                    tracing::error!("failed handling sock request: {}", e);
                }
            });
        }
    }
    .await;

    tracing::info!("cleaning up socket");
    if let Err(e) = fs::remove_file(&sock_path).await
        && e.kind() != ErrorKind::NotFound
    {
        tracing::warn!("couldn't remove socket {}: {}", sock_path.display(), e);
    }
    if let Err(e) = fs::remove_dir(&directory).await
        && e.kind() != ErrorKind::NotFound
    {
        tracing::warn!(
            "couldn't remove socket directory {}: {}",
            directory.display(),
            e
        );
    }

    result
}
