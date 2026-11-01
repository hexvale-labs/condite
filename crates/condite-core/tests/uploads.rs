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
    assert_eq!(report.applied(), 1);
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
    assert_eq!(rerun.skipped(), 5);
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
    const VARIABLES: [(&str, &str); 6] = [
        ("SQL_CONN_URL", "turso::memory:"),
        ("S3_ENDPOINT", "http://127.0.0.1:1"),
        ("S3_REGION", "us-east-1"),
        ("S3_BUCKET", "uploads"),
        ("S3_ACCESS_KEY", "test-key"),
        ("S3_SECRET_KEY", "test-secret"),
    ];
    let Ok(expected) = std::env::var("CONDITE_RELOAD_TEST_CHANGED") else {
        for changed in [
            "",
            "SQL_CONN_URL",
            "S3_ENDPOINT",
            "S3_REGION",
            "S3_BUCKET",
            "S3_ACCESS_KEY",
            "S3_SECRET_KEY",
        ] {
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
