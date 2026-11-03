use crate::{
    auth::{AuthenticatedKey, digest},
    db::sql::{SessionRecord, UploadRecord, UploadStatus},
    state::app::AppState,
};
use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use axum_anyhow::{ApiResult, bad_request};
use jiff::Timestamp;
use uuid::Uuid;

/// Compatibility metadata endpoint now issues a short-lived API ticket, never an S3 URL.
pub async fn handler(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<condite_shared::DownloadResponse>> {
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    let upload = super::retention::owned(&mut tx, auth, id).await?;
    ensure_confirmed(&upload)?;
    tx.commit().await?;
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|_| anyhow::anyhow!("ticket generation failed"))?;
    let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let expires_at = Timestamp::now().checked_add(jiff::Span::new().minutes(15))?;
    let expires_at = upload
        .expires_at
        .map(|at| at.min(expires_at))
        .unwrap_or(expires_at);
    let credential = if auth.1 {
        state
            .root_keys
            .read()
            .map_err(|_| anyhow::anyhow!("root lock poisoned"))?
            .iter()
            .find(|r| crate::auth::root_id(&r.name) == auth.0)
            .map(|r| digest(&r.key))
            .ok_or_else(|| anyhow::anyhow!("root removed"))?
    } else {
        crate::db::sql::ApiKeyRecord::get_by_id(&mut db, &auth.0)
            .await?
            .digest
    };
    toasty::create!(SessionRecord {
        digest: digest(&token),
        owner_id: auth.0,
        credential_digest: credential,
        root: auth.1,
        upload_id: Some(id),
        expires_at: expires_at
    })
    .exec(&mut db)
    .await?;
    Ok(Json(condite_shared::DownloadResponse {
        uri: format!(
            "{}/v1/downloads/{token}",
            state.config.public_api_url.trim_end_matches('/')
        ),
        expires_at: expires_at.to_string(),
    }))
}
fn ensure_confirmed(upload: &UploadRecord) -> ApiResult<()> {
    super::retention::ensure_available(upload)?;
    if upload.status != UploadStatus::Confirmed {
        return Err(bad_request(
            "upload pending",
            "confirm the upload before downloading",
        ));
    }
    Ok(())
}
pub async fn content(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path(id): Path<Uuid>,
    method: Method,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    let upload = super::retention::owned(&mut tx, auth, id).await?;
    ensure_confirmed(&upload)?;
    tx.commit().await?;
    stream(&state, upload, method, headers, false).await
}
pub async fn ticket(
    State(state): State<AppState>,
    Path(token): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let missing = || axum_anyhow::not_found("download unavailable", "ticket is invalid or expired");
    if token.len() != 64 {
        return Err(missing());
    }
    let mut db = state.sql.clone();
    let session = SessionRecord::get_by_digest(&mut db, &digest(&token))
        .await
        .map_err(|_| missing())?;
    if session.expires_at <= Timestamp::now() {
        return Err(missing());
    }
    let valid = if session.root {
        state
            .root_keys
            .read()
            .map_err(|_| anyhow::anyhow!("root lock poisoned"))?
            .iter()
            .any(|r| {
                crate::auth::root_id(&r.name) == session.owner_id
                    && digest(&r.key) == session.credential_digest
            })
    } else {
        let key = crate::db::sql::ApiKeyRecord::get_by_id(&mut db, &session.owner_id)
            .await
            .map_err(|_| missing())?;
        !key.revoked && key.digest == session.credential_digest
    };
    if !valid {
        return Err(missing());
    }
    let id = session.upload_id.ok_or_else(missing)?;
    let upload = UploadRecord::get_by_id(&mut db, &id)
        .await
        .map_err(|_| missing())?;
    if !session.root && upload.owner_id != session.owner_id {
        return Err(missing());
    }
    ensure_confirmed(&upload)?;
    stream(&state, upload, method, headers, false).await
}

/// Reads one range at a time with bounded buffering; the SQL transaction is already closed.
pub async fn stream(
    state: &AppState,
    upload: UploadRecord,
    method: Method,
    headers: HeaderMap,
    public: bool,
) -> ApiResult<Response> {
    let length = upload.size_bytes;
    let range = match headers.get(header::RANGE) {
        Some(value) => match value
            .to_str()
            .ok()
            .and_then(|value| byte_range(value, length))
        {
            Some(range) => Some(range),
            None => {
                return Ok((
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    [(header::CONTENT_RANGE, format!("bytes */{length}"))],
                )
                    .into_response());
            }
        },
        None => None,
    };
    let inline = public
        && matches!(
            upload.mime_type.as_str(),
            "image/png"
                | "image/jpeg"
                | "image/gif"
                | "image/webp"
                | "image/avif"
                | "image/bmp"
                | "image/x-icon"
        );
    let mime = if inline {
        upload.mime_type.as_str()
    } else {
        "application/octet-stream"
    };
    let mut response = Response::builder()
        .status(if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::ACCEPT_RANGES, "bytes")
        .header("x-content-type-options", "nosniff")
        .header("content-security-policy", "default-src 'none'; sandbox")
        .header(
            header::CONTENT_DISPOSITION,
            if inline { "inline" } else { "attachment" },
        )
        .header(header::CONTENT_TYPE, mime);
    if let Some((start, end)) = range {
        response = response
            .header(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{length}"),
            )
            .header(header::CONTENT_LENGTH, (end - start + 1).to_string());
    } else {
        response = response.header(header::CONTENT_LENGTH, length.to_string());
    }
    if method == Method::HEAD {
        let object = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            state
                .s3
                .head_object()
                .bucket(&state.config.s3_bucket)
                .key(upload.s3_key())
                .send(),
        )
        .await
        .map_err(|_| axum_anyhow::bad_gateway("storage unavailable", "storage timed out"))?
        .map_err(|_| {
            axum_anyhow::bad_gateway("storage unavailable", "could not inspect stored file")
        })?;
        if object.content_length() != Some(length as i64) {
            return Err(axum_anyhow::bad_gateway(
                "storage mismatch",
                "stored file has an unexpected size",
            ));
        }
        return Ok(response.body(Body::empty()).map_err(anyhow::Error::from)?);
    }
    let mut request = state
        .s3
        .get_object()
        .bucket(&state.config.s3_bucket)
        .key(upload.s3_key());
    if let Some((start, end)) = range {
        request = request.range(format!("bytes={start}-{end}"));
    }
    let object = tokio::time::timeout(std::time::Duration::from_secs(15), request.send())
        .await
        .map_err(|_| axum_anyhow::bad_gateway("storage unavailable", "storage timed out"))?
        .map_err(|_| {
            axum_anyhow::bad_gateway("storage unavailable", "could not open stored file")
        })?;
    let expected = range.map(|(a, b)| b - a + 1).unwrap_or(length);
    if object.content_length() != Some(expected as i64) {
        return Err(axum_anyhow::bad_gateway(
            "storage mismatch",
            "stored file has an unexpected size",
        ));
    }
    use tokio::io::AsyncReadExt;
    let reader = object.body.into_async_read().take(expected);
    use tokio_stream::StreamExt;
    let stream = tokio_util::io::ReaderStream::new(reader)
        .timeout(std::time::Duration::from_secs(15))
        .map(|chunk| {
            chunk.unwrap_or_else(|_| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "storage stream stalled",
                ))
            })
        });
    Ok(response
        .body(Body::from_stream(stream))
        .map_err(anyhow::Error::from)?)
}
fn byte_range(value: &str, length: u64) -> Option<(u64, u64)> {
    if length == 0 {
        return None;
    }
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let count = end.parse::<u64>().ok()?.min(length);
        return (count > 0).then_some((length - count, length - 1));
    }
    let start = start.parse::<u64>().ok()?;
    let end = if end.is_empty() {
        length - 1
    } else {
        end.parse::<u64>().ok()?.min(length - 1)
    };
    (start <= end && start < length).then_some((start, end))
}
