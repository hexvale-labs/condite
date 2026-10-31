use crate::{
    db::sql::{UploadRecord, UploadStatus, get_s3_key},
    state::app::AppState,
};
use aws_sdk_s3::presigning::PresigningConfig;
use axum::{
    Json,
    extract::{Path, State},
};
use axum_anyhow::{ApiResult, bad_request, not_found};
use jiff::Timestamp;
use std::time::Duration;
use uuid::Uuid;

pub use condite_shared::ConfirmResponse as Response;

#[tracing::instrument(name = "uploads.confirm", skip_all, fields(upload_id = %id), err(Debug, level = "warn"))]
pub async fn handler(
    State(mut state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Response>> {
    let mut upload = match UploadRecord::get_by_id(&mut state.sql, &id).await {
        Ok(upload) => upload,
        Err(e) if e.is_record_not_found() => {
            tracing::debug!("Upload record not found");
            return Err(not_found(
                "upload not found",
                "upload record does not exist",
            ));
        }
        Err(e) => return Err(e.into()),
    };
    let pending = get_s3_key(&id, &upload.mime_type, &UploadStatus::Pending);
    let confirmed = get_s3_key(&id, &upload.mime_type, &UploadStatus::Confirmed);
    if upload.status == UploadStatus::Pending {
        tracing::debug!("Checking pending object metadata before confirmation");
        let head = state
            .s3
            .head_object()
            .bucket(&state.config.s3_bucket)
            .key(&pending)
            .send()
            .await;
        let head = match head {
            Ok(head) => head,
            Err(e) if e.raw_response().is_some_and(|r| r.status().as_u16() == 404) => {
                tracing::debug!("Pending upload object not found in storage");
                return Err(not_found(
                    "upload not found",
                    "upload the file before confirming",
                ));
            }
            Err(e) => return Err(e.into()),
        };
        if head.content_length() != Some(upload.size_bytes as i64)
            || head.content_type() != Some(upload.mime_type.as_str())
        {
            tracing::debug!(expected_size_bytes = upload.size_bytes, actual_size_bytes = ?head.content_length(), "Upload confirmation rejected: object metadata does not match");
            return Err(bad_request(
                "upload metadata mismatch",
                "uploaded size and MIME type must match the upload request",
            ));
        }
        state
            .s3
            .copy_object()
            .bucket(&state.config.s3_bucket)
            .key(&confirmed)
            .copy_source(format!("{}/{}", state.config.s3_bucket, pending))
            .set_copy_source_if_match(head.e_tag().map(str::to_owned))
            .send()
            .await?;
        upload
            .update()
            .status(UploadStatus::Confirmed)
            .exec(&mut state.sql)
            .await?;
    }
    tracing::debug!("Confirmed object available; removing pending object");
    // Retrying confirmation also retries cleanup after a partial failure.
    state
        .s3
        .delete_object()
        .bucket(&state.config.s3_bucket)
        .key(&pending)
        .send()
        .await?;
    let expires_at = Timestamp::now().checked_add(jiff::Span::new().minutes(15))?;
    let signed = state
        .s3
        .get_object()
        .bucket(&state.config.s3_bucket)
        .key(&confirmed)
        .presigned(PresigningConfig::expires_in(Duration::from_secs(900))?)
        .await?;
    let mut total = 0u64;
    let mut after = Uuid::nil();
    loop {
        let batch =
            UploadRecord::filter(UploadRecord::fields().status().eq(UploadStatus::Confirmed))
                .filter(UploadRecord::fields().id().gt(after))
                .order_by(UploadRecord::fields().id().asc())
                .limit(500)
                .select((
                    UploadRecord::fields().id(),
                    UploadRecord::fields().size_bytes(),
                ))
                .exec(&mut state.sql)
                .await?;
        for (id, size) in &batch {
            after = *id;
            total = total
                .checked_add(*size)
                .ok_or_else(|| anyhow::anyhow!("storage size overflow"))?;
        }
        if batch.len() < 500 {
            break;
        }
    }
    tracing::info!(storage_size_bytes = total, "Upload confirmation completed");
    Ok(Json(Response {
        public_uri: signed.uri().to_owned(),
        expires_at: expires_at.to_string(),
        new_storage_size_bytes: total,
    }))
}
