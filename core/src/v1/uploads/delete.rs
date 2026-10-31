use crate::{
    db::sql::{UploadRecord, UploadStatus, get_s3_key},
    state::app::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use axum_anyhow::{ApiResult, not_found};
use uuid::Uuid;

pub async fn handler(
    State(mut state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let upload = match UploadRecord::get_by_id(&mut state.sql, &id).await {
        Ok(upload) => upload,
        Err(e) if e.is_record_not_found() => {
            return Err(not_found(
                "upload not found",
                "upload record does not exist",
            ));
        }
        Err(e) => return Err(e.into()),
    };
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
        .exec(&mut state.sql)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
