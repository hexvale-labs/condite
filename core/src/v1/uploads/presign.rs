use std::{collections::HashMap};

use anyhow::Result;
use aws_sdk_s3::presigning::PresigningConfig;
use axum::{Json, extract::State};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::state::app::AppState;

#[derive(Deserialize)]
struct Payload {
    size_bytes: u64,
    mime_type: String,
    filename: String
}

#[derive(Serialize)]
struct Response {
    uri: String,
    headers: HashMap<String, String>,
    expires_at_utc: DateTime<Utc>
}

async fn presign(
    State(state): State<AppState>,
    Json(payload): Json<Payload>
) -> Result<Response> {
    let expires_in = Duration::minutes(2);
    let expires_at_utc = Utc::now() + expires_in;

    // TODO: Limit file size

    let id = uuid::Uuid::now_v7();
    let presigned = state.s3
        .put_object()
        .bucket(&state.config.s3_bucket)
        .key(format!("pending/{}", id))
        .set_content_length(Some(payload.size_bytes as i64))
        .set_content_type(Some(payload.mime_type))
        .presigned(PresigningConfig::expires_in(expires_in.to_std()?)?)
        .await?;

    let uri = presigned.uri().to_owned();
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in presigned.headers() {
        headers.insert(name.to_owned(), value.to_owned());
    }

    // TODO: Track in DB

    Ok(Response {
        uri,
        headers,
        expires_at_utc
    })
}