// use axum::{Json, extract::State};
// use axum_anyhow::ApiResult;
// use jiff::Timestamp;
// use serde::{Deserialize, Serialize};

// use crate::state::app::AppState;

// #[derive(Deserialize)]
// struct Payload {
//     request_id: String,
// }

// #[derive(Serialize)]
// struct Response {
//     public_uri: String,
//     expires_at: Timestamp,
//     new_storage_size_bytes: u64
// }

// pub async fn handler(
//     State(state): State<AppState>,
//     Json(payload): Json<Payload>
// ) -> ApiResult<Json<Response>> {
//     unreachable!("todo")
// }