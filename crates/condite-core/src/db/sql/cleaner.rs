use crate::{
    db::sql::{ObjectCleanupRecord, ShareRecord, UploadRecord},
    state::app::AppState,
};
use anyhow::Result;
use jiff::Timestamp;
use std::time::Duration;
use uuid::Uuid;

pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
const BATCH_SIZE: usize = 100;

pub async fn cleaner_task_supervisor(state: AppState) {
    tracing::info!(
        interval_secs = CLEANUP_INTERVAL.as_secs(),
        "Expiry cleanup scheduler started"
    );
    loop {
        if state.ct.is_cancelled() {
            break;
        }
        if let Err(e) = clean_records(&state).await {
            tracing::error!(error = %e, "Expiry cleanup failed; retrying next minute");
        }
        tokio::select! {
            biased;
            _ = state.ct.cancelled() => break,
            _ = tokio::time::sleep(CLEANUP_INTERVAL) => {}
        }
    }
}

/// A cursor advances even after failures so one broken key cannot starve others.
pub async fn clean_records(state: &AppState) -> Result<()> {
    let now = Timestamp::now();
    let mut db = state.sql.clone();
    let mut after = Uuid::nil();
    loop {
        if state.ct.is_cancelled() {
            return Ok(());
        }
        let batch = ShareRecord::filter(ShareRecord::fields().cleanup_at().le(Some(now)))
            .filter(ShareRecord::fields().id().gt(after))
            .order_by(ShareRecord::fields().id().asc())
            .limit(BATCH_SIZE)
            .select(ShareRecord::fields().id())
            .exec(&mut db)
            .await?;
        for id in &batch {
            if state.ct.is_cancelled() {
                return Ok(());
            }
            after = *id;
            if crate::db::sql::retry::api(|| {
                crate::v1::uploads::shares::remove_copy(state, *id, true)
            })
            .await
            .is_err()
            {
                tracing::warn!(share_id = %id, "Share cleanup failed; retained for retry");
            }
        }
        if batch.len() < BATCH_SIZE {
            break;
        }
    }
    after = Uuid::nil();
    loop {
        if state.ct.is_cancelled() {
            return Ok(());
        }
        let batch = UploadRecord::filter(UploadRecord::fields().cleanup_at().le(Some(now)))
            .filter(UploadRecord::fields().id().gt(after))
            .order_by(UploadRecord::fields().id().asc())
            .limit(BATCH_SIZE)
            .select(UploadRecord::fields().id())
            .exec(&mut db)
            .await?;
        for id in &batch {
            if state.ct.is_cancelled() {
                return Ok(());
            }
            after = *id;
            if crate::v1::uploads::delete::remove_due(state, *id)
                .await
                .is_err()
            {
                tracing::warn!(upload_id = %id, "Upload cleanup failed; retained for retry");
            }
        }
        if batch.len() < BATCH_SIZE {
            break;
        }
    }
    after = Uuid::nil();
    loop {
        if state.ct.is_cancelled() {
            return Ok(());
        }
        let batch = UploadRecord::filter(UploadRecord::fields().pending_cleanup_at().le(Some(now)))
            .filter(UploadRecord::fields().id().gt(after))
            .order_by(UploadRecord::fields().id().asc())
            .limit(BATCH_SIZE)
            .select(UploadRecord::fields().id())
            .exec(&mut db)
            .await?;
        for id in &batch {
            if state.ct.is_cancelled() {
                return Ok(());
            }
            after = *id;
            if crate::db::sql::retry::api(|| cleanup_pending(state, *id))
                .await
                .is_err()
            {
                tracing::warn!(upload_id = %id, "Pending object cleanup failed; retained for retry");
            }
        }
        if batch.len() < BATCH_SIZE {
            break;
        }
    }
    after = Uuid::nil();
    loop {
        if state.ct.is_cancelled() {
            return Ok(());
        }
        let batch = ObjectCleanupRecord::filter(ObjectCleanupRecord::fields().cleanup_at().le(now))
            .filter(ObjectCleanupRecord::fields().id().gt(after))
            .order_by(ObjectCleanupRecord::fields().id().asc())
            .limit(BATCH_SIZE)
            .exec(&mut db)
            .await?;
        for job in &batch {
            if state.ct.is_cancelled() {
                return Ok(());
            }
            after = job.id;
            let result = async {
                for key in [&job.pending_key, &job.confirmed_key] {
                    state
                        .s3
                        .delete_object()
                        .bucket(&job.bucket)
                        .key(key)
                        .send()
                        .await?;
                }
                ObjectCleanupRecord::filter_by_id(job.id)
                    .update()
                    .cleanup_at(Timestamp::now().checked_add(jiff::Span::new().hours(24))?)
                    .exec(&mut db)
                    .await?;
                anyhow::Ok(())
            }
            .await;
            if result.is_err() {
                tracing::warn!(upload_id = %job.id, "Private object cleanup failed; retained for retry");
            }
        }
        if batch.len() < BATCH_SIZE {
            break;
        }
    }
    Ok(())
}

async fn cleanup_pending(state: &AppState, id: Uuid) -> axum_anyhow::ApiResult<()> {
    let mut db = state.sql.clone();
    let initial = match UploadRecord::get_by_id(&mut db, &id).await {
        Ok(row) => row,
        Err(e) if e.is_record_not_found() => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let mut tx = db.transaction().await?;
    super::api_key::lock_key(&mut tx, initial.owner_id).await?;
    let mut upload = match UploadRecord::get_by_id(&mut tx, &id).await {
        Ok(row) => row,
        Err(e) if e.is_record_not_found() => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if upload.status != super::UploadStatus::Confirmed
        || !upload
            .pending_cleanup_at
            .is_some_and(|at| at <= Timestamp::now())
    {
        return Ok(());
    }
    state
        .s3
        .delete_object()
        .bucket(&state.config.s3_bucket)
        .key(super::get_s3_key(
            &id,
            &upload.mime_type,
            &super::UploadStatus::Pending,
        ))
        .send()
        .await?;
    upload
        .update()
        .pending_cleanup_at(Some(
            Timestamp::now().checked_add(jiff::Span::new().hours(24))?,
        ))
        .exec(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
