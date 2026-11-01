use axum::{
    Router,
    body::to_bytes,
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::any,
};
use condite_sdk::{Client, Error, ListUploadsRequest, PresignRequest};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct Fixture {
    base: String,
    calls: Arc<Mutex<Vec<String>>>,
}
async fn handler(State(s): State<Fixture>, req: Request) -> axum::response::Response {
    let path = req.uri().path().to_owned();
    s.calls
        .lock()
        .unwrap()
        .push(format!("{} {}", req.method(), req.uri()));
    if path == "/storage" {
        assert!(req.headers().get("authorization").is_none());
        assert_eq!(req.headers()["content-type"], "text/plain");
        assert_eq!(to_bytes(req.into_body(), 1024).await.unwrap(), "abc");
        return StatusCode::OK.into_response();
    }
    let json = match (req.method().as_str(), path.as_str()) {
        ("GET", "/prefix/v1/health") => r#"{"healthy":true,"started_at":"now","runtime":"PT1S"}"#.to_owned(),
        ("GET", "/prefix/v1/uploads") => {
            assert_eq!(req.uri().query(), Some("limit=2&after=a%26b"));
            r#"{"uploads":[],"next_after":null}"#.to_owned()
        }
        ("POST", "/prefix/v1/uploads") => {
            let body: serde_json::Value = serde_json::from_slice(&to_bytes(req.into_body(), 1024).await.unwrap()).unwrap();
            assert_eq!(body["size_bytes"], 3);
            serde_json::json!({"request_id":if body["filename"] == "fail.txt" { "bad" } else { "id" }, "uri":format!("{}/storage", s.base),"headers":{"content-type":"text/plain","content-length":"3"},"expires_at_utc":"later"}).to_string()
        }
        ("POST", "/prefix/v1/uploads/id/confirm") => r#"{"upload":{"id":"id","filename":"a.txt","mime_type":"text/plain","size_bytes":3,"status":"Confirmed","created_at":"now","expires_at":null,"visibility":"Private","deletion_pending":false},"new_storage_size_bytes":18446744073709551615}"#.into(),
        ("POST", "/prefix/v1/uploads/id/shares") => {
            assert_eq!(req.headers()["authorization"], "Bearer api-only");
            let body: serde_json::Value = serde_json::from_slice(&to_bytes(req.into_body(), 1024).await.unwrap()).unwrap();
            assert_eq!(body, serde_json::json!({"expires_at":null,"follows_upload":true}));
            r#"{"id":"share","upload_id":"id","url":"https://storage.example/public","created_at":"now","expires_at":null,"follows_upload":true,"status":"Active"}"#.into()
        }
        ("GET", "/prefix/v1/uploads/id/shares") => r#"{"shares":[{"id":"share","upload_id":"id","url":"https://storage.example/public","created_at":"now","expires_at":null,"follows_upload":true,"status":"Active"}],"next_after":null}"#.into(),
        ("PATCH", "/prefix/v1/uploads/id/retention") => {
            let body: serde_json::Value = serde_json::from_slice(&to_bytes(req.into_body(), 1024).await.unwrap()).unwrap();
            assert_eq!(body, serde_json::json!({"expires_at":null}));
            r#"{"id":"id","filename":"a.txt","mime_type":"text/plain","size_bytes":3,"status":"Confirmed","created_at":"now","expires_at":null,"visibility":"Private","deletion_pending":false}"#.into()
        }
        ("POST", "/prefix/v1/uploads/id/download") => r#"{"uri":"https://storage.example/private","expires_at":"later"}"#.into(),
        ("DELETE", "/prefix/v1/uploads/id/shares/share") => return StatusCode::NO_CONTENT.into_response(),
        ("DELETE", "/prefix/v1/uploads/id") => return StatusCode::NO_CONTENT.into_response(),
        (_, "/prefix/v1/uploads/bad/confirm") => return (StatusCode::CONFLICT, "not ready").into_response(),
        (_, "/prefix/v1/uploads/fail/confirm") => return (StatusCode::OK, "invalid json").into_response(),
        _ => return (StatusCode::NOT_FOUND, "unknown").into_response(),
    };
    ([("content-type", "application/json")], json).into_response()
}
async fn fixture() -> (Client, Fixture, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let state = Fixture {
        base: format!("http://{}", listener.local_addr().unwrap()),
        ..Default::default()
    };
    let router = Router::new()
        .fallback(any(handler))
        .with_state(state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("authorization", "Bearer api-only".parse().unwrap());
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    (
        Client::with_client(format!("{}/prefix", state.base), http).unwrap(),
        state,
        server,
    )
}

#[tokio::test]
async fn all_operations_preserve_prefix_wire_types_and_storage_isolation() {
    let (client, _, server) = fixture().await;
    assert!(client.health().await.unwrap().healthy);
    let page = client
        .list_uploads(&ListUploadsRequest {
            limit: Some(2),
            after: Some("a&b".into()),
        })
        .await
        .unwrap();
    assert!(page.next_after.is_none());
    let result = client
        .upload(
            &PresignRequest {
                filename: "a.txt".into(),
                mime_type: "text/plain".into(),
                size_bytes: 3,
                retention: None,
            },
            b"abc".to_vec(),
        )
        .await
        .unwrap();
    assert_eq!(result.request_id, "id");
    assert_eq!(result.confirmation.new_storage_size_bytes, u64::MAX);
    client.delete("id").await.unwrap();
    server.abort();
}

#[tokio::test]
async fn errors_preserve_status_body_and_distinguish_decode_errors() {
    let (client, _, server) = fixture().await;
    assert!(
        matches!(client.confirm("bad").await.unwrap_err(), Error::Http { status:409, body, .. } if body == "not ready")
    );
    assert!(matches!(
        client.confirm("fail").await.unwrap_err(),
        Error::Decode { .. }
    ));
    assert!(matches!(
        client.delete("../escape").await.unwrap_err(),
        Error::Http { status: 404, .. }
    ));
    assert!(Client::new("file:///tmp").is_err());
    assert!(Client::new("https://example.com/?query=x").is_err());
    server.abort();
}

#[tokio::test]
async fn rejects_invalid_upload_and_page_inputs_before_network() {
    let (client, state, server) = fixture().await;
    assert!(
        client
            .list_uploads(&ListUploadsRequest {
                limit: Some(0),
                after: None
            })
            .await
            .is_err()
    );
    assert!(
        client
            .upload(
                &PresignRequest {
                    filename: "a".into(),
                    mime_type: "text/plain".into(),
                    size_bytes: 4,
                    retention: None,
                },
                b"abc".to_vec()
            )
            .await
            .is_err()
    );
    assert!(state.calls.lock().unwrap().is_empty());
    server.abort();
}

#[tokio::test]
async fn partial_upload_failure_preserves_identity_without_cleanup_or_retry() {
    let (client, state, server) = fixture().await;
    let error = client
        .upload(
            &PresignRequest {
                filename: "fail.txt".into(),
                mime_type: "text/plain".into(),
                size_bytes: 3,
                retention: None,
            },
            b"abc".to_vec(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.request_id(), Some("bad"));
    assert_eq!(error.status(), Some(409));
    assert!(matches!(
        error,
        Error::Upload {
            stage: "confirm",
            ..
        }
    ));
    let calls = state.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().all(|call| !call.starts_with("DELETE")));
    server.abort();
}

#[tokio::test]
async fn rejects_malformed_signed_uploads_before_network() {
    let (client, state, server) = fixture().await;
    for (suffix, headers) in [
        ("#fragment", vec![("content-type", "text/plain")]),
        ("", vec![("bad header", "value")]),
        ("", vec![("content-type", "text/plain\r\ninjected: value")]),
    ] {
        let signed = condite_sdk::PresignResponse {
            request_id: "id".into(),
            uri: format!("{}/storage{suffix}", state.base),
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            expires_at_utc: "later".into(),
        };
        assert!(matches!(
            client.put_upload(&signed, b"abc".to_vec()).await,
            Err(Error::Input(_))
        ));
    }
    assert!(state.calls.lock().unwrap().is_empty());
    server.abort();
}

#[tokio::test]
async fn api_key_constructor_sends_sensitive_header_only_to_api() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().fallback(any(|req: Request| async move {
        assert_eq!(req.headers()["authorization"], "Bearer test-secret");
        (
            [("content-type", "application/json")],
            r#"{"healthy":true,"started_at":"now","runtime":"PT1S"}"#,
        )
    }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::with_api_key(&base, "test-secret").unwrap();
    assert!(client.health().await.unwrap().healthy);
    assert!(!format!("{client:?}").contains("test-secret"));
    assert!(Client::with_api_key(&base, "bad\nheader").is_err());
    server.abort();
}

#[tokio::test]
async fn presign_defers_upload_limits_to_the_api_key_policy() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().fallback(any(|req: Request| async move {
        let payload: serde_json::Value = serde_json::from_slice(&to_bytes(req.into_body(), 1024).await.unwrap()).unwrap();
        assert_eq!(payload["size_bytes"], 104_857_601);
        axum::Json(serde_json::json!({"request_id":"id","uri":"http://example.test/storage","headers":{},"expires_at_utc":"later"}))
    }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let result = Client::new(base)
        .unwrap()
        .presign(&PresignRequest {
            size_bytes: 104_857_601,
            filename: "large.txt".into(),
            mime_type: "text/plain".into(),
            retention: None,
        })
        .await;
    assert!(result.is_ok(), "{result:?}");
    server.abort();
}

#[tokio::test]
async fn sharing_and_retention_use_authenticated_prefixed_routes() {
    let (client, _, server) = fixture().await;
    let share = client
        .create_share(
            "id",
            &condite_sdk::CreateShareRequest {
                expires_at: None,
                follows_upload: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(share.status, condite_sdk::ShareStatus::Active);
    assert!(share.follows_upload);
    let page = client
        .list_shares("id", &ListUploadsRequest::default())
        .await
        .unwrap();
    assert_eq!(page.shares.len(), 1);
    let upload = client
        .update_retention("id", &condite_sdk::Retention { expires_at: None })
        .await
        .unwrap();
    assert_eq!(upload.expires_at, None);
    assert_eq!(
        client.download("id").await.unwrap().uri,
        "https://storage.example/private"
    );
    client.revoke_share("id", "share").await.unwrap();
    server.abort();
}
