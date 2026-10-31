use axum::{Router, routing};

pub mod db;
pub mod state;
pub mod v1;

pub async fn start_app() -> anyhow::Result<()> {
    let state = state::app::AppState::init().await?;

    let app = app_router(state);

    let lis = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(lis, app).await?;
    Ok(())
}

pub fn app_router(state: state::app::AppState) -> Router {
    Router::new()
        .route("/", routing::get(|| async { "Hello, world!" }))
        .route(
            "/v1/uploads/presign",
            routing::post(v1::uploads::presign::presign),
        )
        .with_state(state)
}
