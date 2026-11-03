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
        ControlCommand::ListApiKeys { limit, after } => {
            let limit = limit.unwrap_or(50);
            if !(1..=100).contains(&limit) {
                return Ok(control_error(
                    "invalid_limit",
                    "limit must be between 1 and 100",
                ));
            }
            let after = match after {
                Some(id) => match id.parse::<uuid::Uuid>() {
                    Ok(id) => id,
                    Err(_) => return Ok(control_error("invalid_cursor", "after must be a UUID")),
                },
                None => uuid::Uuid::nil(),
            };
            let mut db = state.sql.clone();
            let mut keys = crate::db::sql::ApiKeyRecord::filter(
                crate::db::sql::ApiKeyRecord::fields().id().gt(after),
            )
            .order_by(crate::db::sql::ApiKeyRecord::fields().id().asc())
            .limit(limit as usize + 1)
            .exec(&mut db)
            .await?;
            let more = keys.len() > limit as usize;
            keys.truncate(limit as usize);
            let next_after = if more {
                keys.last().map(|key| key.id.to_string())
            } else {
                None
            };
            let mut summaries = Vec::new();
            for key in keys {
                summaries.push(key_summary(state, &mut db, key).await?);
            }
            Ok(ControlResponse::Keys(condite_shared::ListKeysResponse {
                keys: summaries,
                next_after,
            }))
        }
        ControlCommand::InspectApiKey { id } => {
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok(control_error("invalid_id", "key ID must be a UUID"));
            };
            let mut db = state.sql.clone();
            match crate::db::sql::ApiKeyRecord::get_by_id(&mut db, &id).await {
                Ok(key) => Ok(ControlResponse::Key(
                    key_summary(state, &mut db, key).await?,
                )),
                Err(e) if e.is_record_not_found() => {
                    Ok(control_error("not_found", "key not found"))
                }
                Err(e) => Err(e.into()),
            }
        }
        ControlCommand::Status => Ok(ControlResponse::Status(state.health()?)),
        ControlCommand::SetLogLevel { level } => Ok(match crate::logging::set_filter(&level) {
            Ok(()) => ControlResponse::Success,
            Err(_) => ControlResponse::Error {
                code: "log_filter_failed".into(),
                message:
                    "Could not apply log filter; check the filter syntax and logging initialization"
                        .into(),
            },
        }),
        ControlCommand::Reload => {
            let candidate =
                match crate::state::app::AppRuntimeConfig::load(&state.config.config_path) {
                    Ok(candidate) => candidate,
                    Err(_) => {
                        return Ok(control_error(
                            "reload_failed",
                            "configuration validation failed; active settings retained",
                        ));
                    }
                };
            let restart = state.config.differences(&candidate);
            if crate::logging::set_filter(&candidate.log_filter).is_err() {
                return Ok(control_error(
                    "reload_failed",
                    "invalid log filter; active settings retained",
                ));
            }
            *state
                .root_keys
                .write()
                .map_err(|_| anyhow::anyhow!("root configuration lock poisoned"))? =
                candidate.root_keys;
            if restart.is_empty() {
                Ok(ControlResponse::Success)
            } else {
                Ok(control_error(
                    "restart_required",
                    &format!(
                        "logging and root credentials reloaded; restart required for: {}",
                        restart.join(", ")
                    ),
                ))
            }
        }
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
            if key.digest.starts_with("root:") {
                return Ok(control_error(
                    "managed_by_config",
                    "root credentials are managed in TOML",
                ));
            }
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
            if key.digest.starts_with("root:") {
                return Ok(control_error(
                    "managed_by_config",
                    "root credentials are managed in TOML",
                ));
            }
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

async fn key_summary(
    state: &AppState,
    db: &mut toasty::Db,
    key: crate::db::sql::ApiKeyRecord,
) -> Result<condite_shared::KeySummary> {
    let root = key.digest.starts_with("root:");
    let configured = if let Some(name) = key.digest.strip_prefix("root:") {
        state
            .root_keys
            .read()
            .map_err(|_| anyhow::anyhow!("root configuration lock poisoned"))?
            .iter()
            .any(|root| root.name == name)
    } else {
        true
    };
    Ok(condite_shared::KeySummary {
        id: key.id.to_string(),
        label: key.digest.strip_prefix("root:").map(str::to_owned),
        created_at: key.created_at.to_string(),
        revoked: key.revoked || !configured,
        limits: condite_shared::AccountLimits {
            per_file_bytes: (!root).then_some(key.per_upload_quota),
            storage_bytes: (!root).then_some(key.storage_quota),
        },
        usage: crate::v1::account::usage(db, Some(key.id)).await?,
    })
}

pub async fn http_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: crate::auth::AuthenticatedKey,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !auth.1 {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let command = match parse_command(&body) {
        Ok(command) => command,
        Err(_) => return axum::http::StatusCode::BAD_REQUEST.into_response(),
    };
    match tokio::time::timeout(Duration::from_secs(15), state.bus.execute(command)).await {
        Ok(Ok(response)) => ([("cache-control", "no-store")], axum::Json(response)).into_response(),
        _ => axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
