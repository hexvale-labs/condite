use axum::{Router, routing};

use crate::v1::uploads::presign;

pub mod db;
pub mod state;
pub mod v1;

pub async fn start_app() {
    tracing::info!("starting up...");
    let state = state::app::AppState::init().await.expect("the state to be initialized");
    let app = Router::new()
        .route("/", routing::get(|| async { "condite" }))
        .route("/v1/presign", routing::post(presign::handler))
        .with_state(state);

    let lis = tokio::net::TcpListener::bind("0.0.0.0:8080").await.expect("the TCP listener to be created");
    axum::serve(lis, app).await.expect("the app to start");
}