use axum::{
    Json,
    extract::{Path, State},
};
use axum_anyhow::{ApiResult, internal_error, not_found};
use jiff::Timestamp;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::sql::{UploadRecord, UploadStatus, get_s3_key},
    state::app::AppState,
};

#[derive(Serialize, Debug)]
pub struct Response {
    public_uri: String,
    expires_at: Timestamp,
    new_storage_size_bytes: u64,
}

#[tracing::instrument(skip(state))]
pub async fn handler(
    State(mut state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Response>> {
    let mut upload = match UploadRecord::get_by_id(&mut state.sql, &id).await {
        Ok(v) => v,
        Err(e) if e.is_record_not_found() => {
            return Err(not_found("upload record not found", "it doesn't exist"));
        }
        Err(e) => {
            tracing::error!("couldn't hit up the DB: {}", e);
            return Err(internal_error(
                "internal server error",
                "couldn't hit up the database",
            ));
        }
    };

    if upload.status == UploadStatus::Confirmed {
        // TODO: Fill
        return Ok(Json(Response {
            public_uri: String::new(),
            expires_at: Timestamp::now(),
            new_storage_size_bytes: 0,
        }));
    }

    match state
        .s3
        .copy_object()
        .copy_source(format!("{}/{}", state.config.s3_bucket, upload.s3_key()))
        .bucket(&state.config.s3_bucket)
        .key(get_s3_key(&id, &upload.mime_type, &UploadStatus::Confirmed))
        .send()
        .await
    {
        Err(e) if e.raw_response().is_some_and(|e| e.status().as_u16() == 404) => {
            return Err(not_found("not found", "your upload was not found"));
        }
        Err(e) => {
            tracing::error!("failed copying: {}", e);
            return Err(internal_error(
                "internal server error",
                "s3 operation failed",
            ));
        }
        _ => {}
    };

    upload
        .update()
        .status(UploadStatus::Confirmed)
        .exec(&mut state.sql)
        .await?;

    // TODO: Custom expiry times
    // TODO: fill this out with real data
    Ok(Json(Response {
        public_uri: String::new(),
        expires_at: Timestamp::now(),
        new_storage_size_bytes: 0,
    }))
}
