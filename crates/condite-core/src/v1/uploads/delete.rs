use crate::{
    auth::AuthenticatedKey,
    db::sql::{
        ObjectCleanupRecord, ShareRecord, UploadRecord, UploadStatus, api_key::lock_key, get_s3_key,
    },
    state::app::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use axum_anyhow::{ApiResult, not_found};
use jiff::Timestamp;
use uuid::Uuid;

pub async fn handler(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    remove(&state, id, Some(&auth), None).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Clone, Copy)]
enum Condition {
    Explicit,
    Abandoned(Timestamp),
    Due,
}

async fn intent(
    state: &AppState,
    id: Uuid,
    auth: Option<&AuthenticatedKey>,
    condition: Condition,
) -> ApiResult<bool> {
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
    let mut upload = match UploadRecord::get_by_id(&mut tx, &id).await {
        Ok(upload) => upload,
        Err(e) if e.is_record_not_found() => return Err(missing()),
        Err(e) => return Err(e.into()),
    };
    // Once deletion is committed, retention updates cannot resurrect the upload.
    if !upload.deletion_pending {
        let due = match condition {
            Condition::Explicit => true,
            Condition::Abandoned(cutoff) => {
                upload.status == UploadStatus::Pending && upload.created_at <= cutoff
            }
            Condition::Due => upload.cleanup_at.is_some_and(|t| t <= Timestamp::now()),
        };
        if !due {
            return Ok(false);
        }
        upload
            .update()
            .deletion_pending(true)
            .cleanup_at(Some(Timestamp::now()))
            .exec(&mut tx)
            .await?;
    }
    match ObjectCleanupRecord::get_by_id(&mut tx, &id).await {
        Ok(_) => {}
        Err(e) if e.is_record_not_found() => {
            toasty::create!(ObjectCleanupRecord {
                id: id,
                bucket: state.config.s3_bucket.clone(),
                pending_key: get_s3_key(&id, &upload.mime_type, &UploadStatus::Pending),
                confirmed_key: get_s3_key(&id, &upload.mime_type, &UploadStatus::Confirmed),
                cleanup_at: Timestamp::now().checked_add(jiff::Span::new().minutes(1))?
            })
            .exec(&mut tx)
            .await?;
        }
        Err(e) => return Err(e.into()),
    }
    tx.commit().await?;
    Ok(true)
}

async fn finish(state: &AppState, id: Uuid) -> ApiResult<()> {
    let mut db = state.sql.clone();
    let initial = match UploadRecord::get_by_id(&mut db, &id).await {
        Ok(upload) => upload,
        Err(e) if e.is_record_not_found() => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let mut tx = db.transaction().await?;
    lock_key(&mut tx, initial.owner_id).await?;
    let upload = match UploadRecord::get_by_id(&mut tx, &id).await {
        Ok(upload) => upload,
        Err(e) if e.is_record_not_found() => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if !upload.deletion_pending {
        return Ok(());
    }
    let mut after = Uuid::nil();
    loop {
        let shares = ShareRecord::filter(ShareRecord::fields().upload_id().eq(id))
            .filter(ShareRecord::fields().id().gt(after))
            .order_by(ShareRecord::fields().id().asc())
            .limit(100)
            .exec(&mut tx)
            .await?;
        for share in &shares {
            if matches!(share.status.as_str(), "Creating" | "Reconciling") {
                ShareRecord::filter_by_id(share.id)
                    .update()
                    .status("Reconciling")
                    .cleanup_at(Some(Timestamp::now()))
                    .exec(&mut tx)
                    .await?;
            }
            state
                .s3
                .delete_object()
                .bucket(&share.public_bucket)
                .key(&share.object_key)
                .send()
                .await?;
            after = share.id;
        }
        if shares.len() < 100 {
            break;
        }
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
    ShareRecord::filter(ShareRecord::fields().upload_id().eq(id))
        .filter(ShareRecord::fields().status().ne("Reconciling"))
        .delete()
        .exec(&mut tx)
        .await?;
    UploadRecord::filter_by_id(id)
        .delete()
        .exec(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn remove_when(
    state: &AppState,
    id: Uuid,
    auth: Option<&AuthenticatedKey>,
    condition: Condition,
) -> ApiResult<()> {
    if crate::db::sql::retry::api(|| intent(state, id, auth, condition)).await? {
        crate::db::sql::retry::api(|| finish(state, id)).await?;
    }
    Ok(())
}

/// Preserves the existing administrative pending-cleanup interface.
pub async fn remove(
    state: &AppState,
    id: Uuid,
    auth: Option<&AuthenticatedKey>,
    expired_before: Option<Timestamp>,
) -> ApiResult<()> {
    remove_when(
        state,
        id,
        auth,
        expired_before
            .map(Condition::Abandoned)
            .unwrap_or(Condition::Explicit),
    )
    .await
}

pub async fn remove_due(state: &AppState, id: Uuid) -> ApiResult<()> {
    remove_when(state, id, None, Condition::Due).await
}
