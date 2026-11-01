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
        s3_public_bucket: Some("public".into()),
        public_object_base_url: Some("https://storage.example/public".into()),
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
    let mut state = AppState {
        sql: obtain_sql_conn(&config).await.unwrap(),
        details: Arc::new(AppRuntimeDetails::new()),
        bus,
        ct: tokio_util::sync::CancellationToken::new(),
        tasks: tokio_util::task::TaskTracker::new(),
        config: Arc::new(config),
        s3,
    };
    toasty::create!(condite_core::db::sql::ApiKeyRecord {
        id: TEST_OWNER.parse::<uuid::Uuid>().unwrap(),
        digest: condite_core::auth::digest(TEST_KEY),
        per_upload_quota: 104_857_600u64,
        storage_quota: 1_073_741_824u64,
        lock_nonce: "test"
    })
    .exec(&mut state.sql)
    .await
    .unwrap();
    state
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
    authenticated_router(state().await)
        .oneshot(request)
        .await
        .unwrap()
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
        assert_eq!(
            response.status(),
            if size <= i64::MAX as u64 {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            },
            "size {size}"
        );
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
    let app = authenticated_router(state().await);
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
            owner_id: TEST_OWNER.parse::<uuid::Uuid>().unwrap(),
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
    let app = authenticated_router(state.clone());
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
        owner_id: TEST_OWNER.parse::<uuid::Uuid>().unwrap(),
        filename: "a.txt",
        mime_type: "text/plain",
        size_bytes: 10u64
    })
    .exec(&mut state.sql)
    .await
    .unwrap();
    let app = authenticated_router(state.clone());
    for _ in 0..2 {
        let body = json_request(&app, "POST", &format!("/v1/uploads/{id}/confirm")).await;
        assert!(body.get("public_uri").is_none());
        assert_eq!(body["upload"]["visibility"], "Private");
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
            owner_id: TEST_OWNER.parse::<uuid::Uuid>().unwrap(),
            filename: "a.txt",
            mime_type: "text/plain",
            size_bytes: 10u64
        })
        .exec(&mut state.sql)
        .await
        .unwrap();
        let app = authenticated_router(state.clone());
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
    let app = authenticated_router(state().await);
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

#[tokio::test]
async fn sdk_uploads_lists_and_deletes_through_the_real_core_router() {
    use axum::{Router, response::IntoResponse, routing::any};
    let storage = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut config = config();
    config.s3_endpoint = format!("http://{}", storage.local_addr().unwrap());
    let storage_task = tokio::spawn(async move {
        axum::serve(
            storage,
            Router::new().fallback(any(|request: Request<Body>| async move {
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
            })),
        )
        .await
        .unwrap();
    });
    let app = authenticated_router(state_with_config(config).await);
    let api = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = condite_sdk::Client::new(format!("http://{}", api.local_addr().unwrap())).unwrap();
    let api_task = tokio::spawn(async move {
        axum::serve(api, app).await.unwrap();
    });
    assert!(client.health().await.unwrap().healthy);
    let uploaded = client
        .upload(
            &condite_sdk::PresignRequest {
                filename: "sdk.txt".into(),
                mime_type: "text/plain".into(),
                size_bytes: 10,
                retention: None,
            },
            b"0123456789".to_vec(),
        )
        .await
        .unwrap();
    assert_eq!(uploaded.confirmation.new_storage_size_bytes, 10);
    let page = client.list_uploads(&Default::default()).await.unwrap();
    assert_eq!(page.uploads.len(), 1);
    assert_eq!(page.uploads[0].id, uploaded.request_id);
    assert_eq!(
        page.uploads[0].status,
        condite_shared::UploadStatus::Confirmed
    );
    client.delete(&uploaded.request_id).await.unwrap();
    assert!(
        client
            .list_uploads(&Default::default())
            .await
            .unwrap()
            .uploads
            .is_empty()
    );
    api_task.abort();
    storage_task.abort();
}

#[tokio::test]
async fn upload_routes_require_authentication() {
    let app = app_router(state().await);
    for (method, uri) in [
        ("GET", "/v1/uploads".to_owned()),
        ("POST", "/v1/uploads".to_owned()),
        ("DELETE", format!("/v1/uploads/{}", uuid::Uuid::now_v7())),
        (
            "POST",
            format!("/v1/uploads/{}/confirm", uuid::Uuid::now_v7()),
        ),
    ] {
        for authorization in [None, Some("Bearer invalid"), Some("Basic invalid")] {
            let mut req = Request::builder().method(method).uri(&uri);
            if let Some(value) = authorization {
                req = req.header("authorization", value);
            }
            let response = app
                .clone()
                .oneshot(req.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {uri}"
            );
        }
    }
}

#[tokio::test]
async fn health_does_not_require_a_key() {
    assert_eq!(
        app_router(state().await)
            .oneshot(Request::get("/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

async fn generate_key(
    state: &AppState,
    per_upload_quota: u64,
    storage_quota: u64,
) -> (String, String) {
    use condite_core::control::{ControlCommand, execute_command};
    let result = execute_command(
        state,
        ControlCommand::GenerateApiKey {
            per_upload_quota,
            storage_quota,
        },
    )
    .await
    .unwrap();
    let value = serde_json::to_value(result).unwrap();
    (
        value["id"].as_str().expect("key id").into(),
        value["api_key"].as_str().expect("generated secret").into(),
    )
}
async fn presign_with_key(app: &axum::Router, key: &str, size: u64) -> axum::response::Response {
    app.clone().oneshot(Request::post("/v1/uploads").header("authorization", format!("Bearer {key}")).header("content-type", "application/json").body(Body::from(serde_json::json!({"size_bytes":size,"filename":"test.txt","mime_type":"text/plain"}).to_string())).unwrap()).await.unwrap()
}

#[tokio::test]
async fn pending_uploads_reserve_quota_and_keys_are_isolated() {
    let state = state().await;
    let (_, a) = generate_key(&state, 10, 15).await;
    let (_, b) = generate_key(&state, 10, 15).await;
    let app = app_router(state);
    let response = presign_with_key(&app, &a, 10).await;
    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
    let id = payload["request_id"].as_str().unwrap();
    assert_eq!(
        presign_with_key(&app, &a, 6).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(presign_with_key(&app, &a, 5).await.status(), StatusCode::OK);
    assert_eq!(
        presign_with_key(&app, &b, 11).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        presign_with_key(&app, &b, 10).await.status(),
        StatusCode::OK
    );
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
                    .header("authorization", format!("Bearer {b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let response = app
        .oneshot(
            Request::get("/v1/uploads")
                .header("authorization", format!("Bearer {b}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
    assert_eq!(payload["uploads"].as_array().unwrap().len(), 1);
    assert_ne!(payload["uploads"][0]["id"], id);
}

#[tokio::test]
async fn concurrent_reservations_do_not_oversubscribe() {
    let state = state().await;
    let (_, key) = generate_key(&state, 10, 10).await;
    let app = app_router(state);
    let (a, b) = tokio::join!(
        presign_with_key(&app, &key, 10),
        presign_with_key(&app, &key, 10)
    );
    let mut statuses = [a.status().as_u16(), b.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 413]);
}

#[tokio::test]
async fn malformed_and_duplicate_bearer_headers_are_rejected() {
    let state = state().await;
    let (_, key) = generate_key(&state, 10, 10).await;
    let app = app_router(state);
    for value in [
        format!("Bearer {key} extra"),
        format!("Bearer\t{key}"),
        format!("Bearer {key}, Bearer {key}"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get("/v1/uploads")
                    .header("authorization", value)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = app
        .oneshot(
            Request::get("/v1/uploads")
                .header("authorization", format!("Bearer {key}"))
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

const TEST_OWNER: &str = "00000000-0000-0000-0000-000000000001";
const TEST_KEY: &str = "condite_0000000000000000000000000000000000000000000000000000000000000000";
fn authenticated_router(state: AppState) -> axum::Router {
    app_router(state).layer(axum::middleware::from_fn(
        |mut request: axum::extract::Request, next: axum::middleware::Next| async move {
            request.headers_mut().insert(
                "authorization",
                format!("Bearer {TEST_KEY}").parse().unwrap(),
            );
            next.run(request).await
        },
    ))
}

#[tokio::test]
async fn limits_can_be_raised_lowered_and_key_revoked_without_deleting_files() {
    use condite_core::control::{ControlCommand, ControlResponse, execute_command};
    let mut state = state().await;
    let (id, key) = generate_key(&state, 10, 10).await;
    let app = app_router(state.clone());
    assert_eq!(
        presign_with_key(&app, &key, 10).await.status(),
        StatusCode::OK
    );
    let result = execute_command(
        &state,
        ControlCommand::UpdateApiKey {
            id: id.clone(),
            per_upload_quota: Some(20),
            storage_quota: Some(30),
        },
    )
    .await
    .unwrap();
    assert!(matches!(result, ControlResponse::Success));
    assert_eq!(
        presign_with_key(&app, &key, 20).await.status(),
        StatusCode::OK
    );
    execute_command(
        &state,
        ControlCommand::UpdateApiKey {
            id: id.clone(),
            per_upload_quota: None,
            storage_quota: Some(5),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        presign_with_key(&app, &key, 1).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let record = condite_core::db::sql::ApiKeyRecord::get_by_id(
        &mut state.sql,
        &id.parse::<uuid::Uuid>().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(record.per_upload_quota, 20);
    assert_eq!(record.storage_quota, 5);
    assert_ne!(record.digest, key);
    assert_eq!(record.digest.len(), 64);
    execute_command(&state, ControlCommand::DeleteApiKey { id })
        .await
        .unwrap();
    assert_eq!(
        presign_with_key(&app, &key, 0).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        condite_core::db::sql::UploadRecord::all()
            .exec(&mut state.sql)
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn lowered_limits_block_pending_confirmation_before_s3() {
    use condite_core::control::{ControlCommand, execute_command};
    let state = state().await;
    let (id, key) = generate_key(&state, 10, 10).await;
    let app = app_router(state.clone());
    let response = presign_with_key(&app, &key, 10).await;
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
    execute_command(
        &state,
        ControlCommand::UpdateApiKey {
            id,
            per_upload_quota: Some(9),
            storage_quota: None,
        },
    )
    .await
    .unwrap();
    let response = app
        .oneshot(
            Request::post(format!(
                "/v1/uploads/{}/confirm",
                body["request_id"].as_str().unwrap()
            ))
            .header("authorization", format!("Bearer {key}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn database_rejects_ownerless_uploads() {
    let mut state = state().await;
    let result = toasty::sql::statement("INSERT INTO uploads (id, filename, mime_type, size_bytes, status, created_at) VALUES (X'00000000000000000000000000000002', 'ownerless.txt', 'text/plain', 10, 'pending', '2026-10-31T00:00:00Z')").exec(&mut state.sql).await;
    assert!(result.is_err(), "owner_id must be NOT NULL in the database");
}

#[tokio::test]
async fn independent_database_pools_serialize_reservations() {
    let directory = tempfile::tempdir().unwrap();
    let mut cfg = config();
    cfg.sql_conn_uri = format!("turso:{}", directory.path().join("test.db").display());
    let first = state_with_config(cfg).await;
    let (_, key) = generate_key(&first, 10, 10).await;
    let mut second = first.clone();
    second.sql = obtain_sql_conn(&second.config).await.unwrap();
    let (a, b) = (app_router(first), app_router(second));
    let (a, b) = tokio::join!(
        presign_with_key(&a, &key, 10),
        presign_with_key(&b, &key, 10)
    );
    let mut statuses = [a.status().as_u16(), b.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 413]);
}

async fn socket_request(state: &AppState, request: &[u8]) -> serde_json::Value {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (mut client, server) = tokio::net::UnixStream::pair().unwrap();
    let state = state.clone();
    let handler = tokio::spawn(async move {
        condite_core::control::handle_sock_request(state, server)
            .await
            .unwrap()
    });
    client.write_all(request).await.unwrap();
    client.shutdown().await.unwrap();
    let mut response = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        BufReader::new(client).read_line(&mut response),
    )
    .await
    .unwrap()
    .unwrap();
    handler.await.unwrap();
    serde_json::from_str(&response).unwrap()
}

#[tokio::test]
async fn socket_commands_generate_update_revoke_and_report_errors() {
    let mut state = state().await;
    let (bus, rx) = CommandBus::new();
    state.bus = bus;
    CommandBus::start(rx, state.clone());
    let key = socket_request(
        &state,
        b"{\"command\":\"generate_api_key\",\"per_upload_quota\":10,\"storage_quota\":20}\n",
    )
    .await;
    assert_eq!(key["type"], "api_key");
    let secret = key["api_key"].as_str().unwrap();
    assert_eq!(secret.len(), 72);
    let id = key["id"].as_str().unwrap();
    let update =
        format!("{{\"command\":\"update_api_key\",\"id\":\"{id}\",\"storage_quota\":30}}\n");
    assert_eq!(
        socket_request(&state, update.as_bytes()).await["type"],
        "success"
    );
    assert_eq!(
        presign_with_key(&app_router(state.clone()), secret, 10)
            .await
            .status(),
        StatusCode::OK
    );
    let revoke = format!("{{\"command\":\"delete_api_key\",\"id\":\"{id}\"}}\n");
    assert_eq!(
        socket_request(&state, revoke.as_bytes()).await["type"],
        "success"
    );
    assert_eq!(
        presign_with_key(&app_router(state.clone()), secret, 1)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    for request in [
        b"invalid\n".to_vec(),
        vec![b'x'; 16_385],
        b"{\"command\":\"status\"}".to_vec(),
        b"{\"command\":\"status\",\"unexpected\":1}\n".to_vec(),
    ] {
        assert_eq!(
            socket_request(&state, &request).await["code"],
            "invalid_request"
        );
    }
    assert_eq!(
        socket_request(
            &state,
            b"{\"command\":\"update_api_key\",\"id\":\"invalid\"}\n"
        )
        .await["code"],
        "invalid_limits"
    );
    state.ct.cancel();
    state.tasks.close();
    state.tasks.wait().await;
}

#[tokio::test]
async fn confirmation_cleanup_retry_preserves_quota_and_delete_releases_it() {
    use axum::{Router, response::IntoResponse, routing::any};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let fail_delete = Arc::new(AtomicBool::new(true));
    let copies = Arc::new(AtomicUsize::new(0));
    let fail = fail_delete.clone();
    let observed = copies.clone();
    let s3 = Router::new().fallback(any(move |request: Request<Body>| {
        let fail = fail.clone();
        let observed = observed.clone();
        async move {
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
                "PUT" => {
                    observed.fetch_add(1, Ordering::SeqCst);
                    (
                        [("content-type", "application/xml")],
                        "<CopyObjectResult><ETag>etag</ETag></CopyObjectResult>",
                    )
                        .into_response()
                }
                _ if fail.load(Ordering::SeqCst) => StatusCode::FORBIDDEN.into_response(),
                _ => StatusCode::NO_CONTENT.into_response(),
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = config();
    cfg.s3_endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, s3).await.unwrap() });
    let mut state = state_with_config(cfg).await;
    let (_, key) = generate_key(&state, 10, 10).await;
    let app = app_router(state.clone());
    let response = presign_with_key(&app, &key, 10).await;
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
    let id = body["request_id"].as_str().unwrap();
    let confirm = || {
        Request::post(format!("/v1/uploads/{id}/confirm"))
            .header("authorization", format!("Bearer {key}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        app.clone().oneshot(confirm()).await.unwrap().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        condite_core::db::sql::UploadRecord::get_by_id(
            &mut state.sql,
            &id.parse::<uuid::Uuid>().unwrap()
        )
        .await
        .unwrap()
        .status,
        condite_core::db::sql::UploadStatus::Confirmed
    );
    assert_eq!(
        presign_with_key(&app, &key, 1).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    fail_delete.store(false, Ordering::SeqCst);
    assert_eq!(
        app.clone().oneshot(confirm()).await.unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(copies.load(Ordering::SeqCst), 1);
    assert_eq!(
        app.clone()
            .oneshot(
                Request::delete(format!("/v1/uploads/{id}"))
                    .header("authorization", format!("Bearer {key}"))
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        presign_with_key(&app, &key, 10).await.status(),
        StatusCode::OK
    );
    server.abort();
}

#[tokio::test]
async fn expiration_rechecks_state_and_releases_pending_quota() {
    use axum::{Router, routing::any};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = config();
    cfg.s3_endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(any(|| async { StatusCode::NO_CONTENT })),
        )
        .await
        .unwrap()
    });
    let mut state = state_with_config(cfg).await;
    let (_, key) = generate_key(&state, 10, 10).await;
    let app = app_router(state.clone());
    let response = presign_with_key(&app, &key, 10).await;
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
    let id = body["request_id"]
        .as_str()
        .unwrap()
        .parse::<uuid::Uuid>()
        .unwrap();
    let cutoff = jiff::Timestamp::now()
        .checked_sub(jiff::Span::new().hours(1))
        .unwrap();
    condite_core::v1::uploads::delete::remove(&state, id, None, Some(cutoff))
        .await
        .unwrap();
    assert_eq!(
        presign_with_key(&app, &key, 1).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let mut record = condite_core::db::sql::UploadRecord::get_by_id(&mut state.sql, &id)
        .await
        .unwrap();
    record
        .update()
        .created_at(cutoff)
        .exec(&mut state.sql)
        .await
        .unwrap();
    condite_core::v1::uploads::delete::remove(&state, id, None, Some(cutoff))
        .await
        .unwrap();
    assert_eq!(
        presign_with_key(&app, &key, 10).await.status(),
        StatusCode::OK
    );
    server.abort();
}

#[tokio::test]
async fn migration_resets_database_and_requires_upload_owners() {
    use condite_core::db::sql::{ApiKeyRecord, UploadRecord};
    static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!();
    let mut db = toasty::Db::builder()
        .models(toasty::models!(condite_core::*))
        .connect("turso::memory:")
        .await
        .unwrap();
    toasty::migration::MigrationSet::new(&MIGRATIONS.migrations()[..4])
        .apply(&db)
        .await
        .unwrap();
    toasty::sql::statement("INSERT INTO api_keys (id, digest, per_upload_quota, storage_quota, revoked, lock_nonce, created_at) VALUES (X'00000000000000000000000000000001', 'legacy', 100, 1000, false, 'test', '2026-10-31T00:00:00Z')").exec(&mut db).await.unwrap();
    for owner in ["NULL", "X'00000000000000000000000000000001'"] {
        let id = if owner == "NULL" { 2 } else { 3 };
        toasty::sql::statement(format!("INSERT INTO uploads (id, owner_id, filename, mime_type, size_bytes, status, created_at) VALUES (X'{id:032x}', {owner}, 'old.txt', 'text/plain', 10, 'pending', '2026-10-31T00:00:00Z')")).exec(&mut db).await.unwrap();
    }
    let report = MIGRATIONS.apply(&db).await.unwrap();
    assert!(
        UploadRecord::all().exec(&mut db).await.unwrap().is_empty(),
        "reset must remove all uploads"
    );
    assert!(
        ApiKeyRecord::all().exec(&mut db).await.unwrap().is_empty(),
        "reset must remove all keys"
    );
    assert_eq!(report.applied(), 4);
    assert_eq!(report.skipped(), 4);
    let owner_index = toasty::sql::query("SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'index_uploads_by_owner_id'").exec(&mut db).await.unwrap();
    assert_eq!(
        owner_index.len(),
        1,
        "table rebuild must preserve the owner index"
    );
    let mut state = state().await;
    state.sql = db;
    let (_, key) = generate_key(&state, 100, 100).await;
    assert_eq!(
        presign_with_key(&app_router(state.clone()), &key, 10)
            .await
            .status(),
        StatusCode::OK
    );
    let rerun = MIGRATIONS.apply(&state.sql).await.unwrap();
    assert_eq!(rerun.applied(), 0);
    assert_eq!(rerun.skipped(), 8);
    assert_eq!(
        UploadRecord::all()
            .exec(&mut state.sql)
            .await
            .unwrap()
            .len(),
        1,
        "reset must run only once"
    );
    assert_eq!(
        ApiKeyRecord::all()
            .exec(&mut state.sql)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn concurrent_policy_change_and_reservation_use_current_database_limits() {
    use condite_core::control::{ControlCommand, ControlResponse, execute_command};
    let directory = tempfile::tempdir().unwrap();
    let mut cfg = config();
    cfg.sql_conn_uri = format!("turso:{}", directory.path().join("test.db").display());
    let first = state_with_config(cfg).await;
    let (id, key) = generate_key(&first, 10, 10).await;
    let mut second = first.clone();
    second.sql = obtain_sql_conn(&second.config).await.unwrap();
    let app = app_router(first);
    let (response, updated) = tokio::join!(
        presign_with_key(&app, &key, 10),
        execute_command(
            &second,
            ControlCommand::UpdateApiKey {
                id,
                per_upload_quota: Some(0),
                storage_quota: Some(0)
            }
        )
    );
    assert!(matches!(updated.unwrap(), ControlResponse::Success));
    assert!(matches!(
        response.status(),
        StatusCode::OK | StatusCode::PAYLOAD_TOO_LARGE
    ));
    assert_eq!(
        presign_with_key(&app, &key, 1).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn reload_reports_restart_required_variables_without_values() {
    use condite_core::control::ControlCommand;
    const VARIABLES: [(&str, &str); 8] = [
        ("SQL_CONN_URL", "turso::memory:"),
        ("S3_ENDPOINT", "http://127.0.0.1:1"),
        ("S3_REGION", "us-east-1"),
        ("S3_BUCKET", "uploads"),
        ("S3_ACCESS_KEY", "test-key"),
        ("S3_SECRET_KEY", "test-secret"),
        ("S3_PUBLIC_BUCKET", "public"),
        ("PUBLIC_OBJECT_BASE_URL", "https://storage.example/public"),
    ];
    let Ok(expected) = std::env::var("CONDITE_RELOAD_TEST_CHANGED") else {
        for changed in std::iter::once("").chain(VARIABLES.iter().map(|(name, _)| *name)) {
            for missing in [false, true] {
                let mut child = std::process::Command::new(std::env::current_exe().unwrap());
                child
                    .args([
                        "--exact",
                        "reload_reports_restart_required_variables_without_values",
                        "--nocapture",
                    ])
                    .envs(VARIABLES)
                    .env("RUST_LOG", "warn")
                    .env("CONDITE_RELOAD_TEST_CHANGED", changed);
                if !changed.is_empty() {
                    if missing {
                        child.env_remove(changed);
                    } else {
                        child.env(changed, "new-private-value");
                    }
                }
                let output = child.output().unwrap();
                assert!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        return;
    };
    condite_core::logging::init().unwrap();
    let state = state().await;
    let (bus, receiver) = CommandBus::new();
    CommandBus::start(receiver, state.clone());
    let response = bus
        .execute(ControlCommand::SetLogLevel {
            level: "debug".into(),
        })
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(response).unwrap()["type"], "success");
    assert!(tracing::enabled!(tracing::Level::DEBUG));
    let response = bus
        .execute(ControlCommand::SetLogLevel {
            level: "invalid[".into(),
        })
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(response).unwrap()["type"], "error");
    assert!(tracing::enabled!(tracing::Level::DEBUG));
    let response = bus.execute(ControlCommand::Reload).await.unwrap();
    let response = serde_json::to_value(response).unwrap();
    if expected.is_empty() {
        assert_eq!(response["type"], "success");
    } else {
        assert_eq!(response["type"], "error");
        assert_eq!(response["code"], "restart_required");
        let message = response["message"].as_str().unwrap();
        for (name, _) in VARIABLES {
            assert_eq!(message.contains(name), name == expected, "{message}");
        }
    }
    assert!(!response.to_string().contains("test-secret"));
    assert!(!response.to_string().contains("new-private-value"));
    assert!(!tracing::enabled!(tracing::Level::DEBUG));
    assert!(tracing::enabled!(tracing::Level::WARN));
    assert_eq!(state.config.s3_secret_key, "test-secret");
    state.ct.cancel();
    state.tasks.close();
    state.tasks.wait().await;
}

#[tokio::test]
async fn retention_defaults_to_fourteen_days_and_can_be_disabled() {
    let state = state().await;
    let app = authenticated_router(state.clone());
    for retention in [None, Some(serde_json::json!({"expires_at":null}))] {
        let mut payload =
            serde_json::json!({"size_bytes":1,"filename":"a.png","mime_type":"image/png"});
        if let Some(value) = &retention {
            payload["retention"] = value.clone();
        }
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/uploads")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let created: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        let page = json_request(&app, "GET", "/v1/uploads").await;
        let upload = page["uploads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["id"] == created["request_id"])
            .unwrap();
        assert_eq!(upload["visibility"], "Private");
        if retention.is_none() {
            let expiry: jiff::Timestamp = upload["expires_at"].as_str().unwrap().parse().unwrap();
            let days = expiry.duration_since(jiff::Timestamp::now()).as_secs() / 86400;
            assert_eq!(days, 13);
        } else {
            assert!(upload["expires_at"].is_null());
        }
    }
}

#[derive(Default)]
struct SharingStorage {
    objects: std::collections::HashSet<String>,
    blocked_deletes: std::collections::HashSet<String>,
    copies: Vec<(String, axum::http::HeaderMap)>,
    defer_public_copy: bool,
    deferred_copies: Vec<String>,
}

async fn sharing_fixture() -> (
    AppState,
    Arc<std::sync::Mutex<SharingStorage>>,
    tokio::task::JoinHandle<()>,
) {
    use axum::{Router, response::IntoResponse, routing::any};
    let storage = Arc::new(std::sync::Mutex::new(SharingStorage::default()));
    let observed = storage.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = config();
    cfg.s3_endpoint = format!("http://{}", listener.local_addr().unwrap());
    cfg.public_object_base_url = Some(format!("{}/public", cfg.s3_endpoint));
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(any(move |request: Request<Body>| {
                let observed = observed.clone();
                async move {
                    let mut storage = observed.lock().unwrap();
                    let path = request.uri().path().to_owned();
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
                        "PUT" => {
                            if path.starts_with("/public/") && storage.defer_public_copy {
                                storage.deferred_copies.push(path);
                                return StatusCode::BAD_GATEWAY.into_response();
                            }
                            storage.objects.insert(path.clone());
                            storage.copies.push((path, request.headers().clone()));
                            (
                                [("content-type", "application/xml")],
                                "<CopyObjectResult><ETag>etag</ETag></CopyObjectResult>",
                            )
                                .into_response()
                        }
                        "DELETE" if storage.blocked_deletes.contains(&path) => {
                            StatusCode::FORBIDDEN.into_response()
                        }
                        "DELETE" => {
                            storage.objects.remove(&path);
                            StatusCode::NO_CONTENT.into_response()
                        }
                        "GET" if storage.objects.contains(&path) => "file bytes".into_response(),
                        _ => StatusCode::NOT_FOUND.into_response(),
                    }
                }
            })),
        )
        .await
        .unwrap()
    });
    (state_with_config(cfg).await, storage, server)
}

async fn json_body(
    app: &axum::Router,
    method: &str,
    uri: &str,
    payload: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, body)
}

async fn confirmed_for_sharing(app: &axum::Router) -> String {
    let (status, created) = json_body(
        app,
        "POST",
        "/v1/uploads",
        serde_json::json!({"size_bytes":10,"filename":"a.txt","mime_type":"text/plain"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = created["request_id"].as_str().unwrap().to_owned();
    let confirmed = json_request(app, "POST", &format!("/v1/uploads/{id}/confirm")).await;
    assert_eq!(confirmed["upload"]["visibility"], "Private");
    assert!(confirmed.get("public_uri").is_none());
    id
}

#[tokio::test]
async fn multiple_direct_shares_revoke_individually_and_recover_failed_deletions() {
    let (state, storage, server) = sharing_fixture().await;
    let app = authenticated_router(state.clone());
    let id = confirmed_for_sharing(&app).await;
    let route = format!("/v1/uploads/{id}/shares");
    let mut shares = Vec::new();
    for _ in 0..2 {
        let (status, share) = json_body(
            &app,
            "POST",
            &route,
            serde_json::json!({"expires_at":null,"follows_upload":true}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{share}");
        assert_eq!(share["status"], "Active");
        let url = share["url"].as_str().unwrap();
        assert!(!url.contains("X-Amz"));
        assert_eq!(reqwest_get_status(url).await, StatusCode::OK);
        shares.push(share);
    }
    assert_ne!(shares[0]["url"], shares[1]["url"]);
    {
        let copied = storage.lock().unwrap();
        let (_, headers) = copied
            .copies
            .iter()
            .find(|(path, _)| path.starts_with("/public/"))
            .unwrap();
        assert_eq!(headers["cache-control"], "no-store");
        assert_eq!(headers["content-type"], "application/octet-stream");
        assert!(
            headers["content-disposition"]
                .to_str()
                .unwrap()
                .starts_with("attachment")
        );
    }
    let first_url = shares[0]["url"].as_str().unwrap();
    let path = first_url
        .strip_prefix(&state.config.s3_endpoint)
        .unwrap()
        .to_owned();
    storage.lock().unwrap().blocked_deletes.insert(path.clone());
    let revoke = format!("{route}/{}", shares[0]["id"].as_str().unwrap());
    let failed = app
        .clone()
        .oneshot(Request::delete(&revoke).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let list = json_request(&app, "GET", &route).await;
    assert_eq!(list["shares"][0]["status"], "Revoking");
    assert_eq!(reqwest_get_status(first_url).await, StatusCode::OK);
    storage.lock().unwrap().blocked_deletes.remove(&path);
    condite_core::db::sql::clean_records(&state).await.unwrap();
    assert_eq!(reqwest_get_status(first_url).await, StatusCode::NOT_FOUND);
    assert_eq!(
        reqwest_get_status(shares[1]["url"].as_str().unwrap()).await,
        StatusCode::OK
    );
    let list = json_request(&app, "GET", &route).await;
    assert_eq!(list["shares"][0]["status"], "Revoked");
    let again = app
        .clone()
        .oneshot(Request::delete(revoke).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::NO_CONTENT);
    let download = json_request(&app, "POST", &format!("/v1/uploads/{id}/download")).await;
    assert!(
        download["uri"]
            .as_str()
            .unwrap()
            .contains("X-Amz-Signature=")
    );
    server.abort();
}

async fn reqwest_get_status(url: &str) -> StatusCode {
    // The SDK's HTTP client is intentionally separate from the fake S3 listener.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let uri: axum::http::Uri = url.parse().unwrap();
    let mut stream = tokio::net::TcpStream::connect(uri.authority().unwrap().as_str())
        .await
        .unwrap();
    stream
        .write_all(
            format!(
                "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                uri.path(),
                uri.authority().unwrap()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    StatusCode::from_u16(
        String::from_utf8_lossy(&response)
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn retention_changes_preserve_explicit_share_deadlines_and_cleanup_rechecks_extensions() {
    use condite_core::db::sql::{ShareRecord, UploadRecord};
    let (mut state, _, server) = sharing_fixture().await;
    let app = authenticated_router(state.clone());
    let id = confirmed_for_sharing(&app).await;
    let route = format!("/v1/uploads/{id}/shares");
    let at = jiff::Timestamp::now()
        .checked_add(jiff::Span::new().hours(6))
        .unwrap();
    let (_, fixed) = json_body(
        &app,
        "POST",
        &route,
        serde_json::json!({"expires_at":at.to_string(),"follows_upload":false}),
    )
    .await;
    let (_, following) = json_body(
        &app,
        "POST",
        &route,
        serde_json::json!({"expires_at":null,"follows_upload":true}),
    )
    .await;
    let (status, upload) = json_body(
        &app,
        "PATCH",
        &format!("/v1/uploads/{id}/retention"),
        serde_json::json!({"expires_at":null}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(upload["expires_at"].is_null());
    let list = json_request(&app, "GET", &route).await;
    assert_eq!(list["shares"][0]["expires_at"], fixed["expires_at"]);
    assert!(list["shares"][1]["expires_at"].is_null());
    let id: uuid::Uuid = id.parse().unwrap();
    // A cleanup candidate selected before extension must recheck the current row.
    condite_core::v1::uploads::delete::remove_due(&state, id)
        .await
        .unwrap();
    assert!(UploadRecord::get_by_id(&mut state.sql, &id).await.is_ok());
    let past = jiff::Timestamp::now()
        .checked_sub(jiff::Span::new().seconds(1))
        .unwrap();
    let sid: uuid::Uuid = following["id"].as_str().unwrap().parse().unwrap();
    ShareRecord::filter_by_id(sid)
        .update()
        .cleanup_at(Some(past))
        .exec(&mut state.sql)
        .await
        .unwrap();
    condite_core::db::sql::clean_records(&state).await.unwrap();
    let list = json_request(&app, "GET", &route).await;
    assert_eq!(list["shares"][1]["status"], "Expired");
    assert_eq!(
        reqwest_get_status(following["url"].as_str().unwrap()).await,
        StatusCode::NOT_FOUND
    );
    assert!(UploadRecord::get_by_id(&mut state.sql, &id).await.is_ok());
    assert_eq!(condite_core::db::sql::CLEANUP_INTERVAL.as_secs(), 60);
    server.abort();
}

#[tokio::test]
async fn sharing_migration_preserves_existing_files_with_fourteen_day_grace() {
    use condite_core::db::sql::UploadRecord;
    static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!();
    let mut db = toasty::Db::builder()
        .models(toasty::models!(condite_core::*))
        .connect("turso::memory:")
        .await
        .unwrap();
    toasty::migration::MigrationSet::new(&MIGRATIONS.migrations()[..5])
        .apply(&db)
        .await
        .unwrap();
    toasty::sql::statement("INSERT INTO uploads (id, owner_id, filename, mime_type, size_bytes, status, created_at) VALUES (X'00000000000000000000000000000001', X'00000000000000000000000000000002', 'old.txt', 'text/plain', 10, 'confirmed', '2020-01-01T00:00:00Z')").exec(&mut db).await.unwrap();
    let before = jiff::Timestamp::now();
    MIGRATIONS.apply(&db).await.unwrap();
    let records = UploadRecord::all().exec(&mut db).await.unwrap();
    assert_eq!(records.len(), 1);
    let upload = &records[0];
    let expires = upload.expires_at.unwrap();
    assert!(
        expires
            >= before
                .checked_add(jiff::Span::new().hours(14 * 24 - 1))
                .unwrap()
    );
    assert!(
        expires
            <= jiff::Timestamp::now()
                .checked_add(jiff::Span::new().hours(14 * 24))
                .unwrap()
    );
    assert_eq!(upload.cleanup_at, Some(expires));
    assert!(!upload.shared);
    assert!(!upload.deletion_pending);
}

#[tokio::test]
async fn sharing_routes_enforce_owner_isolation_and_validate_expiry() {
    let (state, _, server) = sharing_fixture().await;
    let owner = authenticated_router(state.clone());
    let id = confirmed_for_sharing(&owner).await;
    let (_, share) = json_body(
        &owner,
        "POST",
        &format!("/v1/uploads/{id}/shares"),
        serde_json::json!({"expires_at":null,"follows_upload":true}),
    )
    .await;
    let (_, other_key) = generate_key(&state, 100, 100).await;
    let app = app_router(state);
    for (method, path, payload) in [
        (
            "GET",
            format!("/v1/uploads/{id}/shares"),
            serde_json::Value::Null,
        ),
        (
            "POST",
            format!("/v1/uploads/{id}/shares"),
            serde_json::json!({"expires_at":null,"follows_upload":true}),
        ),
        (
            "DELETE",
            format!("/v1/uploads/{id}/shares/{}", share["id"].as_str().unwrap()),
            serde_json::Value::Null,
        ),
        (
            "POST",
            format!("/v1/uploads/{id}/download"),
            serde_json::Value::Null,
        ),
        (
            "PATCH",
            format!("/v1/uploads/{id}/retention"),
            serde_json::json!({"expires_at":null}),
        ),
    ] {
        for key in [None, Some(other_key.as_str())] {
            let mut req = Request::builder()
                .method(method)
                .uri(&path)
                .header("content-type", "application/json");
            if let Some(key) = key {
                req = req.header("authorization", format!("Bearer {key}"));
            }
            let response = app
                .clone()
                .oneshot(req.body(Body::from(payload.to_string())).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if key.is_some() {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::UNAUTHORIZED
                },
                "{method} {path}"
            );
        }
    }
    for expires_at in ["not-a-date", "2000-01-01T00:00:00Z"] {
        let (status, _) = json_body(
            &owner,
            "PATCH",
            &format!("/v1/uploads/{id}/retention"),
            serde_json::json!({"expires_at":expires_at}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = json_body(
            &owner,
            "POST",
            &format!("/v1/uploads/{id}/shares"),
            serde_json::json!({"expires_at":expires_at,"follows_upload":false}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    server.abort();
}

#[tokio::test]
async fn cleanup_deletes_expired_files_and_continues_after_a_storage_failure() {
    use condite_core::db::sql::{ShareRecord, UploadRecord};
    let (mut state, storage, server) = sharing_fixture().await;
    let app = authenticated_router(state.clone());
    let past = jiff::Timestamp::now()
        .checked_sub(jiff::Span::new().minutes(1))
        .unwrap();
    let mut entries = Vec::new();
    for _ in 0..2 {
        let id = confirmed_for_sharing(&app).await;
        let (_, share) = json_body(
            &app,
            "POST",
            &format!("/v1/uploads/{id}/shares"),
            serde_json::json!({"expires_at":null,"follows_upload":true}),
        )
        .await;
        let uid: uuid::Uuid = id.parse().unwrap();
        UploadRecord::filter_by_id(uid)
            .update()
            .expires_at(Some(past))
            .cleanup_at(Some(past))
            .exec(&mut state.sql)
            .await
            .unwrap();
        entries.push((uid, share));
    }
    let blocked_path = entries[0].1["url"]
        .as_str()
        .unwrap()
        .strip_prefix(&state.config.s3_endpoint)
        .unwrap()
        .to_owned();
    storage
        .lock()
        .unwrap()
        .blocked_deletes
        .insert(blocked_path.clone());
    condite_core::db::sql::clean_records(&state).await.unwrap();
    assert!(
        UploadRecord::get_by_id(&mut state.sql, &entries[1].0)
            .await
            .unwrap_err()
            .is_record_not_found()
    );
    let retained = UploadRecord::get_by_id(&mut state.sql, &entries[0].0)
        .await
        .unwrap();
    assert!(retained.deletion_pending);
    assert!(retained.shared);
    let (status, _) = json_body(
        &app,
        "PATCH",
        &format!("/v1/uploads/{}/retention", entries[0].0),
        serde_json::json!({"expires_at":null}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "deletion intent cannot be resurrected"
    );
    storage
        .lock()
        .unwrap()
        .blocked_deletes
        .remove(&blocked_path);
    condite_core::db::sql::clean_records(&state).await.unwrap();
    assert!(
        UploadRecord::all()
            .exec(&mut state.sql)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        ShareRecord::all()
            .exec(&mut state.sql)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(storage.lock().unwrap().objects.is_empty());
    server.abort();
}

#[tokio::test]
async fn cleanup_reconciles_a_public_copy_left_by_an_unfinished_share() {
    use condite_core::db::sql::{ShareRecord, UploadRecord};
    let (mut state, storage, server) = sharing_fixture().await;
    let app = authenticated_router(state.clone());
    let id = confirmed_for_sharing(&app).await;
    let uid: uuid::Uuid = id.parse().unwrap();
    let (_, share) = json_body(
        &app,
        "POST",
        &format!("/v1/uploads/{id}/shares"),
        serde_json::json!({"expires_at":null,"follows_upload":true}),
    )
    .await;
    let sid: uuid::Uuid = share["id"].as_str().unwrap().parse().unwrap();
    let past = jiff::Timestamp::now()
        .checked_sub(jiff::Span::new().seconds(1))
        .unwrap();
    // Reproduce storage success followed by database rollback: intent survives as Creating.
    ShareRecord::filter_by_id(sid)
        .update()
        .status("Creating")
        .cleanup_at(Some(past))
        .exec(&mut state.sql)
        .await
        .unwrap();
    condite_core::db::sql::clean_records(&state).await.unwrap();
    let upload = UploadRecord::get_by_id(&mut state.sql, &uid).await.unwrap();
    assert!(
        upload.shared,
        "uncertain remote publication must not be reported private"
    );
    let share = ShareRecord::get_by_id(&mut state.sql, &sid).await.unwrap();
    assert_eq!(share.status, "Reconciling");
    assert!(share.cleanup_at.is_some());
    assert!(
        storage
            .lock()
            .unwrap()
            .objects
            .iter()
            .all(|key| !key.starts_with("/public/"))
    );
    server.abort();
}

#[tokio::test]
async fn cleanup_removes_recreated_pending_objects_without_deleting_kept_originals() {
    use condite_core::db::sql::UploadRecord;
    let (mut state, storage, server) = sharing_fixture().await;
    let app = authenticated_router(state.clone());
    let id = confirmed_for_sharing(&app).await;
    let uid: uuid::Uuid = id.parse().unwrap();
    let (_, upload) = json_body(
        &app,
        "PATCH",
        &format!("/v1/uploads/{id}/retention"),
        serde_json::json!({"expires_at":null}),
    )
    .await;
    assert!(upload["expires_at"].is_null());
    let initial = UploadRecord::get_by_id(&mut state.sql, &uid).await.unwrap();
    assert!(
        initial.pending_cleanup_at.is_some(),
        "confirmation must schedule final pending cleanup after PUT expiry"
    );
    let pending = format!("/uploads/pending/{id}.txt");
    storage.lock().unwrap().objects.insert(pending.clone());
    let past = jiff::Timestamp::now()
        .checked_sub(jiff::Span::new().seconds(1))
        .unwrap();
    UploadRecord::filter_by_id(uid)
        .update()
        .pending_cleanup_at(Some(past))
        .exec(&mut state.sql)
        .await
        .unwrap();
    condite_core::db::sql::clean_records(&state).await.unwrap();
    let retained = UploadRecord::get_by_id(&mut state.sql, &uid).await.unwrap();
    assert!(
        retained
            .pending_cleanup_at
            .is_some_and(|at| at > jiff::Timestamp::now())
    );
    assert_eq!(
        retained.status,
        condite_core::db::sql::UploadStatus::Confirmed
    );
    assert!(!storage.lock().unwrap().objects.contains(&pending));
    assert!(
        storage
            .lock()
            .unwrap()
            .objects
            .contains(&format!("/uploads/uploads/{id}.txt"))
    );
    server.abort();
}

#[tokio::test]
async fn ambiguous_copy_completion_is_reconciled_even_after_parent_deletion() {
    use condite_core::db::sql::{ShareRecord, UploadRecord};
    let (mut state, storage, server) = sharing_fixture().await;
    let app = authenticated_router(state.clone());
    let id = confirmed_for_sharing(&app).await;
    storage.lock().unwrap().defer_public_copy = true;
    let (status, _) = json_body(
        &app,
        "POST",
        &format!("/v1/uploads/{id}/shares"),
        serde_json::json!({"expires_at":null,"follows_upload":true}),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let share = ShareRecord::all()
        .exec(&mut state.sql)
        .await
        .unwrap()
        .pop()
        .unwrap();
    let list = json_request(&app, "GET", &format!("/v1/uploads/{id}/shares")).await;
    assert_eq!(
        list["shares"][0]["url"], "",
        "failed publication never reveals the bearer key"
    );
    assert_eq!(
        storage.lock().unwrap().deferred_copies.len(),
        1,
        "ambiguous copies must not be automatically retried"
    );
    let response = app
        .clone()
        .oneshot(
            Request::delete(format!("/v1/uploads/{id}/shares/{}", share.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "unknown remote writes remain in reconciliation"
    );
    assert_eq!(
        reqwest_get_status(&share.public_url).await,
        StatusCode::NOT_FOUND
    );
    // The storage provider completes its accepted copy after DELETE already returned.
    let path = storage.lock().unwrap().deferred_copies.pop().unwrap();
    storage.lock().unwrap().objects.insert(path.clone());
    assert_eq!(reqwest_get_status(&share.public_url).await, StatusCode::OK);
    let past = jiff::Timestamp::now()
        .checked_sub(jiff::Span::new().seconds(1))
        .unwrap();
    ShareRecord::filter_by_id(share.id)
        .update()
        .cleanup_at(Some(past))
        .exec(&mut state.sql)
        .await
        .unwrap();
    condite_core::db::sql::clean_records(&state).await.unwrap();
    assert_eq!(
        reqwest_get_status(&share.public_url).await,
        StatusCode::NOT_FOUND
    );
    let response = app
        .clone()
        .oneshot(
            Request::delete(format!("/v1/uploads/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        UploadRecord::all()
            .exec(&mut state.sql)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        ShareRecord::get_by_id(&mut state.sql, &share.id)
            .await
            .unwrap()
            .cleanup_at
            .is_some()
    );
    storage.lock().unwrap().objects.insert(path);
    condite_core::db::sql::clean_records(&state).await.unwrap();
    assert_eq!(
        reqwest_get_status(&share.public_url).await,
        StatusCode::NOT_FOUND
    );
    server.abort();
}

#[tokio::test]
async fn deleted_upload_keeps_cleanup_state_for_a_late_private_write() {
    use condite_core::db::sql::ObjectCleanupRecord;
    let (mut state, storage, server) = sharing_fixture().await;
    let app = authenticated_router(state.clone());
    let id = confirmed_for_sharing(&app).await;
    let uid: uuid::Uuid = id.parse().unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::delete(format!("/v1/uploads/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let pending = format!("/uploads/pending/{id}.txt");
    storage.lock().unwrap().objects.insert(pending.clone());
    let past = jiff::Timestamp::now()
        .checked_sub(jiff::Span::new().seconds(1))
        .unwrap();
    ObjectCleanupRecord::filter_by_id(uid)
        .update()
        .cleanup_at(past)
        .exec(&mut state.sql)
        .await
        .unwrap();
    condite_core::db::sql::clean_records(&state).await.unwrap();
    assert!(!storage.lock().unwrap().objects.contains(&pending));
    let job = ObjectCleanupRecord::get_by_id(&mut state.sql, &uid)
        .await
        .unwrap();
    assert!(
        job.cleanup_at > jiff::Timestamp::now(),
        "future late completions retain a low-frequency cleanup job"
    );
    server.abort();
}
