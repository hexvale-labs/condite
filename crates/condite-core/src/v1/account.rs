use crate::{
    auth::AuthenticatedKey,
    db::sql::{UploadRecord, UploadStatus},
    state::app::AppState,
};
use axum::{Json, extract::State};
use axum_anyhow::ApiResult;

pub async fn usage(
    executor: &mut dyn toasty::Executor,
    owner: Option<uuid::Uuid>,
) -> anyhow::Result<condite_shared::Usage> {
    let mut usage = condite_shared::Usage::default();
    let mut after = uuid::Uuid::nil();
    loop {
        let mut query = UploadRecord::filter(UploadRecord::fields().id().gt(after));
        if let Some(owner) = owner {
            query = query.filter(UploadRecord::fields().owner_id().eq(owner));
        }
        let batch = query
            .order_by(UploadRecord::fields().id().asc())
            .limit(500)
            .exec(executor)
            .await?;
        for item in &batch {
            after = item.id;
            let count = if item.status == UploadStatus::Confirmed {
                &mut usage.confirmed_bytes
            } else {
                &mut usage.reserved_bytes
            };
            *count = count
                .checked_add(item.size_bytes)
                .ok_or_else(|| anyhow::anyhow!("usage overflow"))?;
        }
        if batch.len() < 500 {
            break;
        }
    }
    usage.charged_bytes = usage
        .confirmed_bytes
        .checked_add(usage.reserved_bytes)
        .ok_or_else(|| anyhow::anyhow!("usage overflow"))?;
    Ok(usage)
}
pub async fn handler(
    State(state): State<AppState>,
    auth: AuthenticatedKey,
) -> ApiResult<Json<condite_shared::Account>> {
    crate::db::sql::retry::api(|| account(&state, auth))
        .await
        .map(Json)
}
async fn account(state: &AppState, auth: AuthenticatedKey) -> ApiResult<condite_shared::Account> {
    let mut db = state.sql.clone();
    let mut tx = db.transaction().await?;
    let key = auth.lock(&mut tx).await?;
    let own = usage(&mut tx, Some(auth.0)).await?;
    let global_usage = if auth.1 {
        Some(usage(&mut tx, None).await?)
    } else {
        None
    };
    tx.commit().await?;
    Ok(condite_shared::Account {
        id: auth.0.to_string(),
        role: if auth.1 {
            condite_shared::AccountRole::Root
        } else {
            condite_shared::AccountRole::Key
        },
        limits: condite_shared::AccountLimits {
            per_file_bytes: (!auth.1).then_some(key.per_upload_quota),
            storage_bytes: (!auth.1).then_some(key.storage_quota),
        },
        usage: own,
        global_usage,
    })
}
