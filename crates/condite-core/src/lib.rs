pub mod control;
pub mod db;
mod shutdown_waiter;
pub mod state;
pub mod v1;

use axum::{Router, routing};

use crate::v1::{
    health,
    uploads::{confirm, delete, list, presign},
};

pub async fn start_app() {
    tracing::info!("Starting Condite core");
    let state = state::app::AppState::init()
        .await
        .expect("the state to be initialized");

    state
        .tasks
        .spawn(db::sql::cleaner_task_supervisor(state.clone()));
    state
        .tasks
        .spawn(control::serve_sock_supervisor(state.clone()));
    state
        .tasks
        .spawn(shutdown_waiter::wait_for_shutdown(state.ct.clone()));

    if let Err(e) = serve_http(state.clone()).await {
        tracing::error!(error = %e, "HTTP server failed; initiating shutdown");
    }

    // Also stop background tasks if binding or serving HTTP failed.
    state.ct.cancel();
    state.tasks.close();
    tracing::info!("Waiting for background tasks to finish");
    state.tasks.wait().await;
    tracing::info!("Condite core shutdown completed");
}

pub fn app_router(state: state::app::AppState) -> Router {
    Router::new()
        .route(
            "/",
            routing::get(|| async { "online! inspect /v1/health for statistics." }),
        )
        .route(
            "/v1/uploads",
            routing::post(presign::handler).get(list::handler),
        )
        .route("/v1/uploads/{id}", routing::delete(delete::handler))
        .route("/v1/uploads/{id}/confirm", routing::post(confirm::handler))
        .route("/v1/health", routing::get(health::handler))
        .with_state(state)
}

async fn serve_http(state: state::app::AppState) -> std::io::Result<()> {
    let app = app_router(state.clone());
    let lis = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!(address = %lis.local_addr()?, "HTTP server listening");
    // Axum drains its own connection tasks before this returns.
    axum::serve(lis, app)
        .with_graceful_shutdown(state.ct.cancelled_owned())
        .await
}
