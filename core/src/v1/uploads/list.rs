use crate::{db::sql::UploadRecord, state::app::AppState};
use axum::{
    Json,
    extract::{Query, State},
};
use axum_anyhow::{ApiResult, bad_request};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct Parameters {
    limit: Option<usize>,
    after: Option<Uuid>,
}
#[derive(Serialize)]
pub struct Response {
    uploads: Vec<UploadRecord>,
    next_after: Option<Uuid>,
}

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
    let mut query = UploadRecord::all();
    if let Some(after) = params.after {
        query = query.filter(UploadRecord::fields().id().gt(after));
    }
    let mut uploads = query
        .order_by(UploadRecord::fields().id().asc())
        .limit(limit + 1)
        .exec(&mut state.sql)
        .await?;
    let has_more = uploads.len() > limit;
    uploads.truncate(limit);
    let next_after = if has_more {
        uploads.last().map(|u| u.id)
    } else {
        None
    };
    Ok(Json(Response {
        uploads,
        next_after,
    }))
}
