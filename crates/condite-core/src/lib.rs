pub mod auth;
pub mod control;
pub mod db;
pub mod logging;
mod shutdown_waiter;
pub mod state;
pub mod v1;

use axum::{Router, routing};

use crate::v1::{
    health,
    uploads::{confirm, delete, download, list, presign, retention, shares},
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
    let cors = cors_layer(state.config.cors_origins.as_deref());
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
        .route(
            "/v1/uploads/{id}/retention",
            routing::patch(retention::handler),
        )
        .route(
            "/v1/uploads/{id}/download",
            routing::post(download::handler),
        )
        .route(
            "/v1/uploads/{id}/shares",
            routing::post(shares::create).get(shares::list),
        )
        .route(
            "/v1/uploads/{id}/shares/{share_id}",
            routing::delete(shares::revoke),
        )
        .route("/v1/health", routing::get(health::handler))
        .with_state(state)
        .layer(cors)
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

fn cors_layer(origins: Option<&str>) -> tower_http::cors::CorsLayer {
    use axum::http::{HeaderValue, Method, header};
    let origins: Vec<HeaderValue> = origins
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|origin| {
            let uri: axum::http::Uri = origin.parse().expect("invalid CORS_ALLOWED_ORIGINS origin");
            assert!(
                matches!(uri.scheme_str(), Some("http" | "https"))
                    && uri.authority().is_some_and(|a| !a.as_str().contains('@'))
                    && uri.path() == "/"
                    && uri.query().is_none()
                    && !origin.contains('#'),
                "CORS_ALLOWED_ORIGINS must contain HTTP(S) origins without paths or credentials"
            );
            origin.parse().expect("invalid CORS_ALLOWED_ORIGINS header")
        })
        .collect();
    tower_http::cors::CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}
