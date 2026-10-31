use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use condite_core::{
    app_router,
    control::CommandBus,
    db::{s3::connection::obtain_s3_conn, sql::connection::obtain_sql_conn},
    state::app::{AppRuntimeConfig, AppRuntimeDetails, AppState},
};
use tower::ServiceExt;

fn config() -> AppRuntimeConfig {
    AppRuntimeConfig {
        sql_conn_uri: "turso::memory:".into(),
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
    let (bus, _rx) = CommandBus::new();
    AppState {
        sql: obtain_sql_conn(&config).await.unwrap(),
        details: Arc::new(AppRuntimeDetails::new()),
        bus,
        ct: tokio_util::sync::CancellationToken::new(),
        tasks: tokio_util::task::TaskTracker::new(),
        config: Arc::new(config),
        s3,
    }
}

async fn request(size: u64) -> axum::response::Response {
    let payload = serde_json::json!({
        "size_bytes": size,
        "mime_type": "text/plain",
        "filename": "test.txt",
    });
    let request = Request::post("/v1/uploads")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    app_router(state().await).oneshot(request).await.unwrap()
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
    let expires: jiff::Timestamp = body["expires_at_utc"].as_str().unwrap().parse().unwrap();
    assert!(expires > jiff::Timestamp::now());
}

#[tokio::test]
async fn accepts_100_mib_upload() {
    assert_eq!(request(104_857_600).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn invalid_database_configuration_returns_error_instead_of_panicking() {
    let mut config = config();
    config.sql_conn_uri = "unsupported://database".into();
    assert!(obtain_sql_conn(&config).await.is_err());
}

#[tokio::test]
#[ignore = "Upload size validation is not implemented yet"]
async fn rejects_oversized_uploads_including_integer_overflow() {
    for size in [104_857_601, i64::MAX as u64 + 1, u64::MAX] {
        let response = request(size).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "size {size}");
    }
}

#[tokio::test]
#[ignore = "S3 initialization currently probes bucket connectivity"]
async fn s3_initialization_does_not_require_bucket_listing_or_network() {
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(1), obtain_s3_conn(&config())).await;
    assert!(
        result.is_ok(),
        "S3 initialization attempted a network probe"
    );
    assert!(result.unwrap().is_ok());
}
