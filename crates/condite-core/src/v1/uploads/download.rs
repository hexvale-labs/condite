use crate::{auth::AuthenticatedKey, db::sql::UploadStatus, state::app::AppState};
use aws_sdk_s3::presigning::PresigningConfig;
use axum::{
    Json,
    extract::{Path, State},
};
use axum_anyhow::{ApiResult, bad_request};
use jiff::Timestamp;
use uuid::Uuid;

pub async fn handler(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<condite_shared::DownloadResponse>> {
    crate::db::sql::retry::api(|| attempt(&state, auth, id))
        .await
        .map(Json)
}
async fn attempt(
    state: &AppState,
    auth: AuthenticatedKey,
    id: Uuid,
) -> ApiResult<condite_shared::DownloadResponse> {
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    let upload = super::retention::owned(&mut tx, auth, id).await?;
    super::retention::ensure_available(&upload)?;
    if upload.status != UploadStatus::Confirmed {
        return Err(bad_request(
            "upload pending",
            "confirm the upload before downloading",
        ));
    }
    let now = Timestamp::now();
    let seconds = upload
        .expires_at
        .map(|at| at.duration_since(now).as_secs().clamp(1, 900) as u64)
        .unwrap_or(900);
    let duration = std::time::Duration::from_secs(seconds);
    let signed = state
        .s3
        .get_object()
        .bucket(&state.config.s3_bucket)
        .key(upload.s3_key())
        .response_cache_control("no-store")
        .response_content_disposition("attachment")
        .presigned(PresigningConfig::expires_in(duration)?)
        .await?;
    tx.commit().await?;
    Ok(condite_shared::DownloadResponse {
        uri: signed.uri().to_owned(),
        expires_at: now.checked_add(duration)?.to_string(),
    })
}
