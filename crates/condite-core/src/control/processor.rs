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

async fn execute_attempt(state: &AppState, command: ControlCommand) -> Result<ControlResponse> {
    tracing::debug!(?command, "Executing control command");
    match command {
        ControlCommand::Status => Ok(ControlResponse::Status(state.health()?)),
        ControlCommand::Shutdown => {
            tracing::info!("Shutdown requested through control socket");
            state.ct.cancel();
            Ok(ControlResponse::Success)
        }
        ControlCommand::GenerateApiKey {
            per_upload_quota,
            storage_quota,
        } => {
            if crate::db::sql::api_key::validate_limits(per_upload_quota, storage_quota).is_err() {
                return Ok(control_error(
                    "invalid_limits",
                    "limits exceed the supported byte range",
                ));
            }
            let (id, api_key) =
                crate::auth::generate(state, per_upload_quota, storage_quota).await?;
            Ok(ControlResponse::ApiKey {
                id: id.to_string(),
                api_key,
            })
        }
        ControlCommand::UpdateApiKey {
            id,
            per_upload_quota,
            storage_quota,
        } => {
            if per_upload_quota.is_none() && storage_quota.is_none() {
                return Ok(control_error(
                    "invalid_limits",
                    "specify at least one limit",
                ));
            }
            if crate::db::sql::api_key::validate_limits(
                per_upload_quota.unwrap_or(0),
                storage_quota.unwrap_or(0),
            )
            .is_err()
            {
                return Ok(control_error(
                    "invalid_limits",
                    "limits exceed the supported byte range",
                ));
            }
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok(control_error("invalid_id", "key ID must be a UUID"));
            };
            let mut db = state.sql.clone();
            let mut tx = db.transaction().await?;
            let mut key = match crate::db::sql::api_key::lock_key(&mut tx, id).await {
                Ok(key) if !key.revoked => key,
                Ok(_) => return Ok(control_error("not_found", "key not found")),
                Err(e) if e.is_record_not_found() => {
                    return Ok(control_error("not_found", "key not found"));
                }
                Err(e) => return Err(e.into()),
            };
            let per_upload = per_upload_quota.unwrap_or(key.per_upload_quota);
            let storage = storage_quota.unwrap_or(key.storage_quota);
            key.update()
                .per_upload_quota(per_upload)
                .storage_quota(storage)
                .exec(&mut tx)
                .await?;
            tx.commit().await?;
            Ok(ControlResponse::Success)
        }
        ControlCommand::DeleteApiKey { id } => {
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok(control_error("invalid_id", "key ID must be a UUID"));
            };
            let mut db = state.sql.clone();
            let mut tx = db.transaction().await?;
            let mut key = match crate::db::sql::api_key::lock_key(&mut tx, id).await {
                Ok(key) => key,
                Err(e) if e.is_record_not_found() => {
                    return Ok(control_error("not_found", "key not found"));
                }
                Err(e) => return Err(e.into()),
            };
            key.update().revoked(true).exec(&mut tx).await?;
            tx.commit().await?;
            Ok(ControlResponse::Success)
        }
        ControlCommand::DeleteUpload { id } => {
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok(control_error("invalid_id", "upload ID must be a UUID"));
            };
            match crate::v1::uploads::delete::remove(state, id, None, None).await {
                Ok(()) => Ok(ControlResponse::Success),
                Err(e) if e.status() == axum::http::StatusCode::NOT_FOUND => {
                    Ok(control_error("not_found", "upload not found"))
                }
                Err(e) => Err(anyhow::anyhow!("upload deletion failed: {e:?}")),
            }
        }
        _ => Ok(ControlResponse::CommandNotImplemented),
    }
}

fn control_error(code: &str, message: &str) -> ControlResponse {
    ControlResponse::Error {
        code: code.into(),
        message: message.into(),
    }
}

fn parse_command(line: &[u8]) -> Result<ControlCommand> {
    let command: ControlCommand = serde_json::from_slice(line)?;
    // Serde ignores extra fields on internally tagged unit variants even with
    // deny_unknown_fields; enforce their empty payload explicitly.
    if matches!(
        command,
        ControlCommand::Status | ControlCommand::Reload | ControlCommand::Shutdown
    ) {
        let value: serde_json::Value = serde_json::from_slice(line)?;
        anyhow::ensure!(
            value.as_object().is_some_and(|object| object.len() == 1),
            "unexpected command fields"
        );
    }
    Ok(command)
}

pub async fn handle_sock_request(state: AppState, stream: UnixStream) -> Result<()> {
    use condite_shared::control::MAX_CONTROL_FRAME;
    use tokio::io::AsyncReadExt;
    let (rx, mut tx) = stream.into_split();
    let mut reader = BufReader::new(rx).take(MAX_CONTROL_FRAME + 1);
    let mut line = Vec::new();
    let read = tokio::select! {
        biased;
        _ = state.ct.cancelled() => return Ok(()),
        read = tokio::time::timeout(Duration::from_secs(5), reader.read_until(b'\n', &mut line)) => read,
    };
    let res = match read {
        Err(_) => control_error("timeout", "timed out reading command"),
        Ok(Err(_)) => control_error("invalid_request", "could not read command"),
        Ok(Ok(_)) if line.len() as u64 > MAX_CONTROL_FRAME || line.last() != Some(&b'\n') => {
            control_error(
                "invalid_request",
                "command must be a bounded newline-delimited JSON frame",
            )
        }
        Ok(Ok(_)) => match parse_command(&line) {
            Err(_) => control_error("invalid_request", "invalid control command"),
            Ok(command) => match state.bus.execute(command).await {
                Ok(response) => response,
                Err(e) => {
                    tracing::error!(error = %e, "Control command failed");
                    control_error("internal_error", "control command failed; see server logs")
                }
            },
        },
    };
    let mut bytes = serde_json::to_vec(&res)?;
    bytes.push(b'\n');
    tokio::time::timeout(Duration::from_secs(5), tx.write_all(&bytes))
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
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/run"))
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
        use std::os::unix::fs::PermissionsExt;
        anyhow::ensure!(
            fs::symlink_metadata(&directory).await?.file_type().is_dir(),
            "control directory must not be a symlink"
        );
        fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).await?;
        // Remove a socket left behind by a previous run.
        if let Err(e) = fs::remove_file(&sock_path).await
            && e.kind() != ErrorKind::NotFound
        {
            return Err(e)
                .with_context(|| format!("couldn't remove stale socket {}", sock_path.display()));
        }

        let listener = UnixListener::bind(&sock_path)
            .with_context(|| format!("couldn't bind socket in {}", sock_path.display()))?;
        fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600)).await?;
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

pub async fn execute_command(state: &AppState, command: ControlCommand) -> Result<ControlResponse> {
    crate::db::sql::retry::control(|| execute_attempt(state, command.clone())).await
}
