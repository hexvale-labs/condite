use axum::{Json, extract::State};
use axum_anyhow::{ApiResult};

use crate::state::app::{AppState, HealthStatus};

pub async fn handler(
    State(state): State<AppState>
) -> ApiResult<Json<HealthStatus>> {
    Ok(Json(state.health()?))
}