use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use condite_core::{
    app_router,
    db::s3::obtain_s3_conn,
    state::app::{AppRuntimeConfig, AppState},
};
use tower::ServiceExt;

fn config() -> AppRuntimeConfig {
    AppRuntimeConfig {
        sql_conn_url: "sqlite::memory:".into(),
        s3_endpoint: "http://127.0.0.1:1".into(),
        s3_region: "us-east-1".into(),
        s3_bucket: "uploads".into(),
        s3_access_key: "test-key".into(),
        s3_secret_key: "test-secret".into(),
    }
}

async fn state() -> AppState {
    let config = config();
    let s3 = aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::config::Builder::new()
            .behavior_version_latest()
            .region(aws_config::Region::new("us-east-1"))
            .endpoint_url(&config.s3_endpoint)
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                "test-key",
                "test-secret",
                None,
                None,
                "test",
            ))
            .build(),
    );
    AppState {
        sql: sea_orm::Database::connect("sqlite::memory:").await.unwrap(),
        config: Arc::new(config),
        s3,
    }
}

async fn request(size: u64) -> axum::response::Response {
    app_router(state().await).oneshot(Request::post("/v1/uploads/presign")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"size_bytes": size, "mime_type": "text/plain", "filename": "test.txt"}).to_string())).unwrap()).await.unwrap()
}

#[tokio::test]
async fn upload_route_returns_signed_put_with_required_headers() {
    let response = request(1024).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = to_bytes(response.into_body(), 16384).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let uri = body["uri"].as_str().unwrap();
    assert!(uri.contains("pending/"), "{uri}");
    assert!(uri.contains("X-Amz-Signature="), "{uri}");
    assert!(uri.contains("content-length"), "size must be signed: {uri}");
    assert!(
        uri.contains("content-type"),
        "MIME type must be signed: {uri}"
    );
    assert_eq!(body["headers"]["content-length"], "1024");
    assert_eq!(body["headers"]["content-type"], "text/plain");
    let expires: chrono::DateTime<chrono::Utc> =
        body["expires_at_utc"].as_str().unwrap().parse().unwrap();
    assert!(expires > chrono::Utc::now());
}

#[tokio::test]
async fn rejects_oversized_uploads_including_integer_overflow() {
    for size in [104_857_601, i64::MAX as u64 + 1, u64::MAX] {
        let response = request(size).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "size {size}");
    }
}

#[tokio::test]
async fn accepts_upload_at_size_limit() {
    assert_eq!(request(104_857_600).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn s3_initialization_does_not_require_bucket_listing_or_network() {
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(1), obtain_s3_conn(&config())).await;
    assert!(
        result.is_ok(),
        "S3 initialization attempted a network probe"
    );
    assert!(result.unwrap().is_ok());
}

#[tokio::test]
async fn invalid_database_configuration_returns_error_instead_of_panicking() {
    let mut config = config();
    config.sql_conn_url = "unsupported://database".into();
    assert!(AppState::from_config(config).await.is_err());
}
