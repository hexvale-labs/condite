use std::collections::HashMap;

use aws_sdk_s3::presigning::PresigningConfig;
use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::state::app::AppState;

#[derive(Deserialize)]
pub(crate) struct Payload {
    size_bytes: u64,
    mime_type: String,
    #[serde(rename = "filename")]
    _filename: String,
}

#[derive(Serialize)]
pub(crate) struct Response {
    uri: String,
    headers: HashMap<String, String>,
    expires_at_utc: DateTime<Utc>,
}

pub(crate) async fn presign(
    State(state): State<AppState>,
    Json(payload): Json<Payload>,
) -> Result<Json<Response>, (StatusCode, &'static str)> {
    let expires_in = Duration::minutes(2);
    let expires_at_utc = Utc::now() + expires_in;

    const MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024;
    if payload.size_bytes > MAX_UPLOAD_BYTES {
        return Err((StatusCode::BAD_REQUEST, "Upload exceeds the 100 MiB limit"));
    }
    let content_length = i64::try_from(payload.size_bytes)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid upload size"))?;

    let id = uuid::Uuid::now_v7();
    let presigned = state
        .s3
        .put_object()
        .bucket(&state.config.s3_bucket)
        .key(format!("pending/{}", id))
        .set_content_length(Some(content_length))
        .set_content_type(Some(payload.mime_type))
        .presigned(
            PresigningConfig::expires_in(expires_in.to_std().map_err(|_| internal_error())?)
                .map_err(|_| internal_error())?,
        )
        .await
        .map_err(|_| internal_error())?;

    let uri = presigned.uri().to_owned();
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in presigned.headers() {
        headers.insert(name.to_owned(), value.to_owned());
    }

    // TODO: Track in DB

    Ok(Json(Response {
        uri,
        headers,
        expires_at_utc,
    }))
}
fn internal_error() -> (StatusCode, &'static str) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to create upload URL",
    )
}
