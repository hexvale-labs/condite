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
    state_with_config(config()).await
}

async fn state_with_config(config: AppRuntimeConfig) -> AppState {
    let s3 = aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::config::Builder::new()
            .behavior_version_latest()
            .region(aws_config::Region::new("us-east-1"))
            .endpoint_url(&config.s3_endpoint)
            .force_path_style(true)
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
    assert!(
        uri.contains(&format!(
            "pending/{}.txt",
            body["request_id"].as_str().unwrap()
        )),
        "{uri}"
    );
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
async fn rejects_oversized_uploads_including_integer_overflow() {
    for size in [104_857_601, i64::MAX as u64 + 1, u64::MAX] {
        let response = request(size).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "size {size}");
    }
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
async fn listing_validates_limits_and_cursors() {
    let app = app_router(state().await);
    for query in ["limit=0", "limit=101", "after=invalid"] {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/v1/uploads?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = app
        .oneshot(
            Request::get("/v1/uploads?limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn uuid_cursor_pagination_has_no_gaps_and_handles_deleted_cursor() {
    let mut state = state().await;
    let mut ids = Vec::new();
    for i in 0..5 {
        let id = uuid::Uuid::now_v7();
        toasty::create!(condite_core::db::sql::UploadRecord {
            id: id,
            filename: format!("{i}.txt"),
            mime_type: "text/plain",
            size_bytes: 10u64
        })
        .exec(&mut state.sql)
        .await
        .unwrap();
        ids.push(id);
    }
    ids.sort();
    let app = app_router(state.clone());
    let first = json_request(&app, "GET", "/v1/uploads?limit=2").await;
    assert_eq!(first["uploads"].as_array().unwrap().len(), 2);
    assert_eq!(first["uploads"][0]["id"], ids[0].to_string());
    assert_eq!(first["next_after"], ids[1].to_string());
    condite_core::db::sql::UploadRecord::filter_by_id(ids[1])
        .delete()
        .exec(&mut state.sql)
        .await
        .unwrap();
    let second = json_request(
        &app,
        "GET",
        &format!("/v1/uploads?limit=2&after={}", ids[1]),
    )
    .await;
    assert_eq!(second["uploads"][0]["id"], ids[2].to_string());
    assert_eq!(second["next_after"], ids[3].to_string());
    let last = json_request(
        &app,
        "GET",
        &format!("/v1/uploads?limit=2&after={}", ids[3]),
    )
    .await;
    assert_eq!(last["uploads"][0]["id"], ids[4].to_string());
    assert!(last["next_after"].is_null());
}

async fn json_request(app: &axum::Router, method: &str, uri: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap()
}

#[tokio::test]
async fn confirmation_and_deletion_use_matching_object_keys() {
    use axum::{Router, response::IntoResponse, routing::any};
    let operations = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = operations.clone();
    let s3 = Router::new().fallback(any(move |request: Request<Body>| {
        let observed = observed.clone();
        async move {
            observed.lock().unwrap().push((
                request.method().to_string(),
                request.uri().path().to_owned(),
            ));
            match request.method().as_str() {
                "HEAD" => (
                    [
                        ("content-length", "10"),
                        ("content-type", "text/plain"),
                        ("etag", "\"etag\""),
                    ],
                    "",
                )
                    .into_response(),
                "PUT" => (
                    [("content-type", "application/xml")],
                    "<CopyObjectResult><ETag>etag</ETag></CopyObjectResult>",
                )
                    .into_response(),
                _ => StatusCode::NO_CONTENT.into_response(),
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut config = config();
    config.s3_endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, s3).await.unwrap() });
    let mut state = state_with_config(config).await;
    let id = uuid::Uuid::now_v7();
    toasty::create!(condite_core::db::sql::UploadRecord {
        id: id,
        filename: "a.txt",
        mime_type: "text/plain",
        size_bytes: 10u64
    })
    .exec(&mut state.sql)
    .await
    .unwrap();
    let app = app_router(state.clone());
    for _ in 0..2 {
        let body = json_request(&app, "POST", &format!("/v1/uploads/{id}/confirm")).await;
        assert!(
            body["public_uri"]
                .as_str()
                .unwrap()
                .contains("X-Amz-Signature=")
        );
        assert_eq!(body["new_storage_size_bytes"], 10);
    }
    let response = app
        .oneshot(
            Request::delete(format!("/v1/uploads/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        condite_core::db::sql::UploadRecord::get_by_id(&mut state.sql, &id)
            .await
            .unwrap_err()
            .is_record_not_found()
    );
    let ops = operations.lock().unwrap();
    assert!(ops.contains(&("HEAD".into(), format!("/uploads/pending/{id}.txt"))));
    assert_eq!(ops.iter().filter(|(method, _)| method == "PUT").count(), 1);
    assert!(ops.contains(&("DELETE".into(), format!("/uploads/uploads/{id}.txt"))));
    server.abort();
}

#[tokio::test]
async fn failed_storage_operations_preserve_pending_metadata() {
    use axum::{Router, routing::any};
    for status in [StatusCode::NOT_FOUND, StatusCode::FORBIDDEN, StatusCode::OK] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = config();
        config.s3_endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().fallback(any(move || async move { status })),
            )
            .await
            .unwrap();
        });
        let mut state = state_with_config(config).await;
        let id = uuid::Uuid::now_v7();
        toasty::create!(condite_core::db::sql::UploadRecord {
            id: id,
            filename: "a.txt",
            mime_type: "text/plain",
            size_bytes: 10u64
        })
        .exec(&mut state.sql)
        .await
        .unwrap();
        let app = app_router(state.clone());
        let response = app
            .clone()
            .oneshot(
                Request::post(format!("/v1/uploads/{id}/confirm"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if status == StatusCode::NOT_FOUND {
                StatusCode::NOT_FOUND
            } else if status == StatusCode::OK {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        );
        assert_eq!(
            condite_core::db::sql::UploadRecord::get_by_id(&mut state.sql, &id)
                .await
                .unwrap()
                .status,
            condite_core::db::sql::UploadStatus::Pending
        );
        if status == StatusCode::FORBIDDEN {
            let response = app
                .oneshot(
                    Request::delete(format!("/v1/uploads/{id}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert!(
                condite_core::db::sql::UploadRecord::get_by_id(&mut state.sql, &id)
                    .await
                    .is_ok()
            );
        }
        server.abort();
    }
}

#[tokio::test]
async fn missing_upload_returns_not_found() {
    let app = app_router(state().await);
    let id = uuid::Uuid::now_v7();
    for (method, uri) in [
        ("DELETE", format!("/v1/uploads/{id}")),
        ("POST", format!("/v1/uploads/{id}/confirm")),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
