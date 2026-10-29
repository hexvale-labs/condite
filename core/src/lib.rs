use axum::{Router, routing};

pub mod db;
pub mod state;
pub mod v1;

pub async fn start_app() {
    let state = state::app::AppState::init().await.expect("the state to be initialized");

    let app = Router::new()
        .route("/", routing::get(|| async { "Hello, world!" }))
        .with_state(state);

    let lis = tokio::net::TcpListener::bind("0.0.0.0:8080").await.expect("the TCP listener to be created");
    axum::serve(lis, app).await.expect("the app to start");
}