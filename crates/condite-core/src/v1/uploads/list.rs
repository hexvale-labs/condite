use crate::{db::sql::UploadRecord, state::app::AppState};
use axum::{
    Json,
    extract::{Query, State},
};
use axum_anyhow::{ApiResult, bad_request};
use uuid::Uuid;

pub use condite_shared::{ListUploadsRequest as Parameters, ListUploadsResponse as Response};

#[tracing::instrument(name = "uploads.list", skip_all, err(Debug, level = "warn"))]
pub async fn handler(
    State(mut state): State<AppState>,
    Query(params): Query<Parameters>,
) -> ApiResult<Json<Response>> {
    let limit = params.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(bad_request(
            "invalid limit",
            "limit must be between 1 and 100",
        ));
    }
    let limit = limit as usize;
    let mut query = UploadRecord::all();
    if let Some(after) = params.after {
        query = query.filter(
            UploadRecord::fields().id().gt(after
                .parse::<Uuid>()
                .map_err(|_| bad_request("invalid cursor", "after must be a UUID"))?),
        );
    }
    let mut uploads = query
        .order_by(UploadRecord::fields().id().asc())
        .limit(limit + 1)
        .exec(&mut state.sql)
        .await?;
    let has_more = uploads.len() > limit;
    uploads.truncate(limit);
    let next_after = if has_more {
        uploads.last().map(|u| u.id.to_string())
    } else {
        None
    };
    tracing::debug!(count = uploads.len(), has_more, limit, "Uploads listed");
    Ok(Json(Response {
        uploads: uploads.into_iter().map(Into::into).collect(),
        next_after,
    }))
}
