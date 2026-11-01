use crate::{
    auth::AuthenticatedKey,
    db::sql::{UploadRecord, UploadStatus, api_key::lock_key, get_s3_key},
    state::app::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use axum_anyhow::{ApiResult, not_found};
use uuid::Uuid;

pub async fn handler(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    remove(&state, id, Some(&auth), None).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Administrative deletion and expiration share the HTTP path's recovery rules.
async fn remove_attempt(
    state: &AppState,
    id: Uuid,
    auth: Option<&AuthenticatedKey>,
    expired_before: Option<jiff::Timestamp>,
) -> ApiResult<()> {
    let missing = || not_found("upload not found", "upload record does not exist");
    let mut db = state.sql.clone();
    let initial = match UploadRecord::get_by_id(&mut db, &id).await {
        Ok(upload) => upload,
        Err(e) if e.is_record_not_found() => return Err(missing()),
        Err(e) => return Err(e.into()),
    };
    if auth.is_some_and(|a| a.0 != initial.owner_id) {
        return Err(missing());
    }
    let mut tx = db.transaction().await?;
    if let Some(auth) = auth {
        auth.lock(&mut tx).await?;
    } else {
        lock_key(&mut tx, initial.owner_id).await?;
    }
    let upload = match UploadRecord::get_by_id(&mut tx, &id).await {
        Ok(upload) => upload,
        Err(e) if e.is_record_not_found() => return Err(missing()),
        Err(e) => return Err(e.into()),
    };
    if let Some(cutoff) = expired_before
        && (upload.status != UploadStatus::Pending || upload.created_at > cutoff)
    {
        return Ok(());
    }
    for status in [UploadStatus::Pending, UploadStatus::Confirmed] {
        state
            .s3
            .delete_object()
            .bucket(&state.config.s3_bucket)
            .key(get_s3_key(&id, &upload.mime_type, &status))
            .send()
            .await?;
    }
    UploadRecord::filter_by_id(id)
        .delete()
        .exec(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn remove(
    state: &AppState,
    id: Uuid,
    auth: Option<&AuthenticatedKey>,
    expired_before: Option<jiff::Timestamp>,
) -> ApiResult<()> {
    crate::db::sql::retry::api(|| remove_attempt(state, id, auth, expired_before)).await
}
