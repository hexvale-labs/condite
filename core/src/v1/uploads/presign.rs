use std::{collections::HashMap, time::Duration};

use aws_sdk_s3::presigning::PresigningConfig;
use axum::{Json, extract::State};
use axum_anyhow::ApiResult;
use jiff::{Span, Timestamp};
use serde::{Deserialize, Serialize};

use crate::{db::sql::UploadRecord, state::app::AppState};

#[derive(Deserialize, Debug)]
pub struct Payload {
    size_bytes: u64,
    mime_type: String,
    filename: String
}

#[derive(Serialize, Debug)]
pub struct Response {
    request_id: String,
    uri: String,
    headers: HashMap<String, String>,
    expires_at_utc: Timestamp
}

#[tracing::instrument(skip(state))]
pub async fn handler(
    State(mut state): State<AppState>,
    Json(payload): Json<Payload>
) -> ApiResult<Json<Response>> {
    let expires_in = Span::new().minutes(2);
    let expires_at_utc = Timestamp::now() + expires_in;

    // TODO: Limit file size, filename length

    let id = uuid::Uuid::now_v7();
    let presigned = state.s3
        .put_object()
        .bucket(&state.config.s3_bucket)
        .key(format!("pending/{}", id.to_string()))
        .set_content_length(Some(payload.size_bytes as i64))
        .set_content_type(Some(payload.mime_type.clone()))
        .presigned(PresigningConfig::expires_in(Duration::try_from(expires_in)?)?)
        .await?;

    let uri = presigned.uri().to_owned();
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in presigned.headers() {
        headers.insert(name.to_owned(), value.to_owned());
    }

    toasty::create!(UploadRecord {
        id: id,
        filename: payload.filename.clone(),
        mime_type: payload.mime_type.clone(),
        size_bytes: payload.size_bytes.clone()
    }).exec(&mut state.sql).await?;

    Ok(Json(Response {
        request_id: id.to_string(),
        uri,
        headers,
        expires_at_utc
    }))
}