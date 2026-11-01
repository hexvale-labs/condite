use condite_shared::*;

#[test]
fn preserves_core_wire_format_and_full_u64_range() {
    let json = r#"{"uploads":[{"id":"01900000-0000-7000-8000-000000000001","filename":"a.txt","mime_type":"text/plain","size_bytes":18446744073709551615,"status":"Confirmed","created_at":"2026-10-31T00:00:00Z","expires_at":null,"visibility":"Private","deletion_pending":false}],"next_after":null}"#;
    let page: ListUploadsResponse = serde_json::from_str(json).unwrap();
    assert_eq!(page.uploads[0].size_bytes, u64::MAX);
    assert_eq!(page.uploads[0].status, UploadStatus::Confirmed);
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let health: HealthStatus = serde_json::from_str(
        r#"{"healthy":true,"started_at":"2026-10-31T00:00:00Z","runtime":"PT2S"}"#,
    )
    .unwrap();
    assert_eq!(health.runtime, "PT2S");
    assert_eq!(
        serde_json::to_value(ListUploadsRequest::default()).unwrap(),
        serde_json::json!({})
    );
}

#[test]
fn retention_distinguishes_default_from_never() {
    let default: PresignRequest = serde_json::from_value(
        serde_json::json!({"filename":"a", "mime_type":"text/plain", "size_bytes":1}),
    )
    .unwrap();
    assert_eq!(default.retention, None);
    let never: PresignRequest = serde_json::from_value(serde_json::json!({"filename":"a", "mime_type":"text/plain", "size_bytes":1,"retention":{"expires_at":null}})).unwrap();
    assert_eq!(never.retention, Some(Retention { expires_at: None }));
}
