use std::{collections::HashMap, time::Duration};

use aws_sdk_s3::presigning::PresigningConfig;
use axum::{Json, extract::State};
use axum_anyhow::ApiResult;
use jiff::{Span, Timestamp};

use crate::{db::sql::UploadRecord, state::app::AppState};

pub use condite_shared::{PresignRequest as Payload, PresignResponse as Response};

#[tracing::instrument(name = "uploads.presign", skip_all, fields(size_bytes = payload.size_bytes, upload_id = tracing::field::Empty), err(Debug, level = "warn"))]
pub async fn handler(
    State(mut state): State<AppState>,
    Json(payload): Json<Payload>,
) -> ApiResult<Json<Response>> {
    let expires_in = Span::new().minutes(2);
    let expires_at_utc = Timestamp::now() + expires_in;

    if payload.size_bytes > 104_857_600
        || payload.filename.is_empty()
        || payload.filename.len() > 255
        || payload.mime_type.is_empty()
        || payload.mime_type.len() > 255
        || payload.mime_type.bytes().any(|b| b.is_ascii_control())
    {
        tracing::debug!("Upload request rejected: invalid size, filename, or MIME type");
        return Err(axum_anyhow::bad_request(
            "invalid upload",
            "maximum size is 100 MiB; filename and MIME type must be valid and at most 255 bytes",
        ));
    }

    let id = uuid::Uuid::now_v7();
    tracing::Span::current().record("upload_id", tracing::field::display(id));
    tracing::debug!("Generating signed upload authorization");
    let presigned = state
        .s3
        .put_object()
        .bucket(&state.config.s3_bucket)
        .key(crate::db::sql::get_s3_key(
            &id,
            &payload.mime_type,
            &crate::db::sql::UploadStatus::Pending,
        ))
        .set_content_length(Some(payload.size_bytes as i64))
        .set_content_type(Some(payload.mime_type.clone()))
        .presigned(PresigningConfig::expires_in(Duration::try_from(
            expires_in,
        )?)?)
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
        size_bytes: payload.size_bytes
    })
    .exec(&mut state.sql)
    .await?;

    tracing::info!(upload_id = %id, expires_at = %expires_at_utc, "Upload authorization created");
    Ok(Json(Response {
        request_id: id.to_string(),
        uri,
        headers,
        expires_at_utc: expires_at_utc.to_string(),
    }))
}
