use crate::{
    db::sql::{UploadRecord, UploadStatus, get_s3_key},
    state::app::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
};
use axum_anyhow::{ApiResult, bad_request, not_found};
use uuid::Uuid;

pub use condite_shared::ConfirmResponse as Response;

pub async fn handler(
    State(state): State<AppState>,
    auth: crate::auth::AuthenticatedKey,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Response>> {
    crate::db::sql::retry::api(|| attempt(State(state.clone()), auth, Path(id))).await
}

#[tracing::instrument(name = "uploads.confirm", skip_all, fields(upload_id = %id), err(Debug, level = "warn"))]
async fn attempt(
    State(mut state): State<AppState>,
    auth: crate::auth::AuthenticatedKey,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Response>> {
    let mut tx = state.sql.transaction().await?;
    let key = auth.lock(&mut tx).await?;
    let mut upload = match UploadRecord::filter_by_id(id)
        .filter(UploadRecord::fields().owner_id().eq(key.id))
        .get(&mut tx)
        .await
    {
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
    super::retention::ensure_available(&upload)?;
    let pending = get_s3_key(&id, &upload.mime_type, &UploadStatus::Pending);
    let confirmed = get_s3_key(&id, &upload.mime_type, &UploadStatus::Confirmed);
    if upload.status == UploadStatus::Pending {
        let reserved = crate::db::sql::api_key::storage_size(&mut tx, key.id, false).await?;
        if upload.size_bytes > key.per_upload_quota || reserved > key.storage_quota {
            return Err(crate::auth::quota_error());
        }
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
        let cleanup_at = upload.expires_at;
        let pending_cleanup_at = upload
            .created_at
            .checked_add(jiff::Span::new().minutes(3))?;
        upload
            .update()
            .status(UploadStatus::Confirmed)
            .cleanup_at(cleanup_at)
            .pending_cleanup_at(Some(pending_cleanup_at))
            .exec(&mut tx)
            .await?;
    }
    let total = crate::db::sql::api_key::storage_size(&mut tx, key.id, true).await?;
    tx.commit().await?;
    tracing::debug!("Confirmed object available; removing pending object");
    // Retrying confirmation also retries cleanup after a partial failure.
    state
        .s3
        .delete_object()
        .bucket(&state.config.s3_bucket)
        .key(&pending)
        .send()
        .await?;
    tracing::info!(storage_size_bytes = total, "Upload confirmation completed");
    Ok(Json(Response {
        upload: upload.into(),
        new_storage_size_bytes: total,
    }))
}
