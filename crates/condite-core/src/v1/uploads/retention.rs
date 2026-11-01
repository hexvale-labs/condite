use crate::{
    auth::AuthenticatedKey,
    db::sql::{ShareRecord, UploadRecord, UploadStatus},
    state::app::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
};
use axum_anyhow::{ApiResult, bad_request, not_found};
use jiff::{Span, Timestamp};
use uuid::Uuid;

pub fn parse(value: Option<Option<&str>>, now: Timestamp) -> ApiResult<Option<Timestamp>> {
    match value {
        None => Ok(Some(now.checked_add(Span::new().hours(14 * 24))?)),
        Some(None) => Ok(None),
        Some(Some(value)) => {
            let at: Timestamp = value.parse().map_err(|_| {
                bad_request("invalid expiry", "expires_at must be an RFC 3339 timestamp")
            })?;
            if at <= now {
                return Err(bad_request(
                    "invalid expiry",
                    "expiry must be in the future; use delete to remove a file now",
                ));
            }
            Ok(Some(at))
        }
    }
}

pub fn ensure_available(upload: &UploadRecord) -> ApiResult<()> {
    if upload.deletion_pending || upload.expires_at.is_some_and(|t| t <= Timestamp::now()) {
        return Err(bad_request(
            "upload expired",
            "upload is expired or being deleted",
        ));
    }
    Ok(())
}

pub async fn owned(
    tx: &mut toasty::Transaction<'_>,
    auth: AuthenticatedKey,
    id: Uuid,
) -> ApiResult<UploadRecord> {
    auth.lock(tx).await?;
    match UploadRecord::filter_by_id(id)
        .filter(UploadRecord::fields().owner_id().eq(auth.0))
        .get(tx)
        .await
    {
        Ok(upload) => Ok(upload),
        Err(e) if e.is_record_not_found() => Err(not_found(
            "upload not found",
            "upload record does not exist",
        )),
        Err(e) => Err(e.into()),
    }
}

pub async fn handler(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path(id): Path<Uuid>,
    Json(payload): Json<condite_shared::Retention>,
) -> ApiResult<Json<condite_shared::Upload>> {
    crate::db::sql::retry::api(|| attempt(&state, auth, id, &payload)).await
}
async fn attempt(
    state: &AppState,
    auth: AuthenticatedKey,
    id: Uuid,
    payload: &condite_shared::Retention,
) -> ApiResult<Json<condite_shared::Upload>> {
    let expiry = parse(Some(payload.expires_at.as_deref()), Timestamp::now())?;
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    let mut upload = owned(&mut tx, auth, id).await?;
    if upload.deletion_pending {
        return Err(bad_request(
            "deletion pending",
            "cannot change retention after deletion starts",
        ));
    }
    upload.update().expires_at(expiry).exec(&mut tx).await?;
    if upload.status == UploadStatus::Confirmed {
        upload.update().cleanup_at(expiry).exec(&mut tx).await?;
    }
    let shares = ShareRecord::filter(ShareRecord::fields().upload_id().eq(id))
        .filter(ShareRecord::fields().status().eq("Active"))
        .limit(20)
        .exec(&mut tx)
        .await?;
    for mut share in shares {
        if share.status == "Active" {
            let deadline = share.expires_at(&upload);
            share.update().cleanup_at(deadline).exec(&mut tx).await?;
        }
    }
    tx.commit().await?;
    Ok(Json(upload.into()))
}
