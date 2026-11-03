use crate::{
    auth::AuthenticatedKey,
    db::sql::{ShareRecord, UploadRecord, UploadStatus, api_key::lock_key},
    state::app::AppState,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_anyhow::{ApiResult, bad_request, not_found};
use jiff::Timestamp;
use uuid::Uuid;

const MAX_LIVE_SHARES: usize = 20;

fn missing() -> axum_anyhow::ApiError {
    not_found("share not found", "share record does not exist")
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path(id): Path<Uuid>,
    Json(payload): Json<condite_shared::CreateShareRequest>,
) -> ApiResult<Json<condite_shared::Share>> {
    let share_id = Uuid::now_v7();
    crate::db::sql::retry::api(|| create_intent(&state, auth, id, share_id, &payload)).await?;
    crate::db::sql::retry::api(|| publish(&state, auth, id, share_id))
        .await
        .map(Json)
}

async fn create_intent(
    state: &AppState,
    auth: AuthenticatedKey,
    id: Uuid,
    share_id: Uuid,
    payload: &condite_shared::CreateShareRequest,
) -> ApiResult<()> {
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    let mut upload = super::retention::owned(&mut tx, auth, id).await?;
    super::retention::ensure_available(&upload)?;
    if upload.status != UploadStatus::Confirmed {
        return Err(bad_request(
            "upload pending",
            "confirm the upload before sharing",
        ));
    }
    let base = &state.config.public_api_url;
    let bucket = "";
    if payload.follows_upload && payload.expires_at.is_some() {
        return Err(bad_request(
            "invalid expiry",
            "a following share cannot specify a separate expiry",
        ));
    }
    let requested = if payload.follows_upload {
        None
    } else {
        super::retention::parse(Some(payload.expires_at.as_deref()), Timestamp::now())?
    };
    let live = ShareRecord::filter(ShareRecord::fields().upload_id().eq(id))
        .filter(ShareRecord::fields().status().ne("Revoked"))
        .filter(ShareRecord::fields().status().ne("Expired"))
        .limit(MAX_LIVE_SHARES)
        .exec(&mut tx)
        .await?;
    if live.len() >= MAX_LIVE_SHARES {
        return Err(bad_request(
            "share limit reached",
            "revoke a link before creating more than 20 live shares",
        ));
    }
    let mut random = [0u8; 32];
    getrandom::fill(&mut random)
        .map_err(|e| anyhow::anyhow!("share key generation failed: {e}"))?;
    let token: String = random.iter().map(|v| format!("{v:02x}")).collect();
    let object_key = token;
    let url = format!("{}/v1/shares/{}", base.trim_end_matches('/'), object_key);
    toasty::create!(ShareRecord {
        id: share_id,
        owner_id: upload.owner_id,
        upload_id: id,
        object_key: object_key,
        public_url: url,
        public_bucket: bucket,
        requested_expires_at: requested,
        follows_upload: payload.follows_upload,
        status: "Creating",
        cleanup_at: Some(Timestamp::now().checked_add(jiff::Span::new().minutes(2))?)
    })
    .exec(&mut tx)
    .await?;
    // Conservative until recovery proves there is no reachable public copy.
    upload.update().shared(true).exec(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn publish(
    state: &AppState,
    auth: AuthenticatedKey,
    id: Uuid,
    share_id: Uuid,
) -> ApiResult<condite_shared::Share> {
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    let upload = super::retention::owned(&mut tx, auth, id).await?;
    super::retention::ensure_available(&upload)?;
    let mut share = ShareRecord::get_by_id(&mut tx, &share_id).await?;
    if share.status != "Creating" {
        return Err(bad_request(
            "share cancelled",
            "the unfinished share was cleaned up; create a new share",
        ));
    }
    let expiry = share.expires_at(&upload);
    // The intent was committed before CopyObject; rollback retains its cleanup key.
    share
        .update()
        .status("Active")
        .cleanup_at(expiry)
        .exec(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(share.wire(&upload))
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path(id): Path<Uuid>,
    Query(params): Query<condite_shared::ListUploadsRequest>,
) -> ApiResult<Json<condite_shared::ListSharesResponse>> {
    crate::db::sql::retry::api(|| list_attempt(&state, auth, id, &params))
        .await
        .map(Json)
}
async fn list_attempt(
    state: &AppState,
    auth: AuthenticatedKey,
    id: Uuid,
    params: &condite_shared::ListUploadsRequest,
) -> ApiResult<condite_shared::ListSharesResponse> {
    let limit = params.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(bad_request(
            "invalid limit",
            "limit must be between 1 and 100",
        ));
    }
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    let upload = super::retention::owned(&mut tx, auth, id).await?;
    let mut query = ShareRecord::filter(ShareRecord::fields().upload_id().eq(id));
    if let Some(after) = &params.after {
        query = query.filter(
            ShareRecord::fields().id().gt(after
                .parse::<Uuid>()
                .map_err(|_| bad_request("invalid cursor", "after must be a UUID"))?),
        );
    }
    let mut shares = query
        .order_by(ShareRecord::fields().id().asc())
        .limit(limit as usize + 1)
        .exec(&mut tx)
        .await?;
    let has_more = shares.len() > limit as usize;
    shares.truncate(limit as usize);
    let next_after = has_more
        .then(|| shares.last().map(|s| s.id.to_string()))
        .flatten();
    let shares = shares.iter().map(|s| s.wire(&upload)).collect();
    tx.commit().await?;
    Ok(condite_shared::ListSharesResponse { shares, next_after })
}

pub async fn revoke(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
    Path((id, share_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    crate::db::sql::retry::api(|| revoke_intent(&state, auth, id, share_id)).await?;
    crate::db::sql::retry::api(|| remove_copy(&state, share_id, false)).await?;
    let mut db = state.sql.clone();
    let uncertain = match ShareRecord::get_by_id(&mut db, &share_id).await {
        Ok(share) => share.status == "Reconciling",
        Err(e) if e.is_record_not_found() => false,
        Err(e) => return Err(e.into()),
    };
    Ok(if uncertain {
        StatusCode::ACCEPTED
    } else {
        StatusCode::NO_CONTENT
    })
}
async fn revoke_intent(
    state: &AppState,
    auth: AuthenticatedKey,
    id: Uuid,
    share_id: Uuid,
) -> ApiResult<()> {
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    super::retention::owned(&mut tx, auth, id).await?;
    let mut share = match ShareRecord::filter_by_id(share_id)
        .filter(ShareRecord::fields().upload_id().eq(id))
        .get(&mut tx)
        .await
    {
        Ok(s) => s,
        Err(e) if e.is_record_not_found() => return Err(missing()),
        Err(e) => return Err(e.into()),
    };
    if share.is_live() {
        let status = if matches!(share.status.as_str(), "Creating" | "Reconciling") {
            "Reconciling"
        } else {
            "Revoking"
        };
        share
            .update()
            .status(status)
            .cleanup_at(Some(Timestamp::now()))
            .exec(&mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Called only for an explicit revocation or a rechecked due cleanup candidate.
pub async fn remove_copy(state: &AppState, share_id: Uuid, only_due: bool) -> ApiResult<()> {
    let mut db = state.sql.clone();
    let initial = match ShareRecord::get_by_id(&mut db, &share_id).await {
        Ok(s) => s,
        Err(e) if e.is_record_not_found() => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let mut tx = db.transaction().await?;
    lock_key(&mut tx, initial.owner_id).await?;
    let mut share = match ShareRecord::get_by_id(&mut tx, &share_id).await {
        Ok(s) => s,
        Err(e) if e.is_record_not_found() => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if !share.is_live() || (only_due && !share.cleanup_at.is_some_and(|t| t <= Timestamp::now())) {
        return Ok(());
    }
    // A disconnected CopyObject can finish after DELETE. Keep its tombstone
    // scheduled even after parent deletion; only known-completed copies retire.
    let uncertain = !share.public_bucket.is_empty()
        && matches!(share.status.as_str(), "Creating" | "Reconciling");
    let terminal = if uncertain {
        "Reconciling"
    } else if share.status == "Active" {
        "Expired"
    } else {
        "Revoked"
    };
    let next_cleanup = if uncertain {
        Some(Timestamp::now().checked_add(jiff::Span::new().minutes(1))?)
    } else {
        None
    };
    if !share.public_bucket.is_empty() {
        state
            .s3
            .delete_object()
            .bucket(&share.public_bucket)
            .key(&share.object_key)
            .send()
            .await?;
    }
    share
        .update()
        .status(terminal)
        .cleanup_at(next_cleanup)
        .exec(&mut tx)
        .await?;
    let live = ShareRecord::filter(ShareRecord::fields().upload_id().eq(share.upload_id))
        .filter(ShareRecord::fields().status().ne("Revoked"))
        .filter(ShareRecord::fields().status().ne("Expired"))
        .limit(1)
        .exec(&mut tx)
        .await?;
    UploadRecord::filter_by_id(share.upload_id)
        .update()
        .shared(!live.is_empty())
        .exec(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Public tokens grant access only while the share and original remain available.
pub async fn download(
    State(state): State<AppState>,
    Path(token): Path<String>,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
) -> ApiResult<axum::response::Response> {
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(missing());
    }
    let mut db = state.sql.clone();
    let share = ShareRecord::filter(ShareRecord::fields().object_key().eq(token))
        .get(&mut db)
        .await
        .map_err(|_| missing())?;
    if share.status != "Active" || !share.public_bucket.is_empty() {
        return Err(missing());
    }
    let upload = UploadRecord::get_by_id(&mut db, &share.upload_id)
        .await
        .map_err(|_| missing())?;
    if share
        .expires_at(&upload)
        .is_some_and(|at| at <= Timestamp::now())
        || upload.status != UploadStatus::Confirmed
        || upload.deletion_pending
    {
        return Err(missing());
    }
    super::retention::ensure_available(&upload)?;
    super::download::stream(&state, upload, method, headers, true).await
}
