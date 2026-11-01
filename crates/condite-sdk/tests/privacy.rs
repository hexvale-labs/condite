use condite_sdk::{Client, Error, PresignRequest, privacy};

#[tokio::test]
async fn unsupported_required_cleaning_fails_before_network() {
    // A connection attempt would be a transport error. Privacy rejection must
    // happen first, even when the service is unreachable.
    let client = Client::new("http://127.0.0.1:1").unwrap();
    let request = PresignRequest {
        filename: "identity.txt".into(),
        mime_type: "text/plain".into(),
        size_bytes: 3,
    };
    let error = client
        .upload_sanitized(
            &request,
            b"abc",
            &privacy::PrivacyOptions {
                strip_metadata: true,
                ..Default::default()
            },
            &privacy::Config::default(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Input(ref text) if text == "unsupported-format"));
}
