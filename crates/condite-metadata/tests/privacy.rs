use condite_metadata::{Config, Error, PrivacyOptions, assess, decrypt, prepare};

#[test]
fn size_preflight_does_not_need_file_bytes() {
    let config = Config::default();
    assert_eq!(
        assess(config.max_input_bytes + 1, b"", &config).reason,
        Some(Error::TooLarge)
    );
    assert_ne!(
        assess(config.max_input_bytes, b"", &config).reason,
        Some(Error::TooLarge)
    );
}
#[test]
fn required_cleaning_never_returns_original_unsupported_bytes() {
    let options = PrivacyOptions {
        strip_metadata: true,
        ..Default::default()
    };
    assert!(matches!(
        prepare(
            b"secret",
            "real name.txt",
            "text/plain",
            &options,
            &Config::default()
        ),
        Err(Error::UnsupportedFormat)
    ));
}
#[test]
fn anonymized_names_do_not_retain_original_stem_or_unsafe_extensions() {
    let options = PrivacyOptions {
        anonymize_filename: true,
        ..Default::default()
    };
    let a = prepare(
        b"hello",
        "Alice.jpg",
        "image/jpeg",
        &options,
        &Config::default(),
    )
    .unwrap();
    let b = prepare(
        b"hello",
        "Alice.jpg",
        "image/jpeg",
        &options,
        &Config::default(),
    )
    .unwrap();
    assert!(!a.filename.contains("Alice"));
    assert!(a.filename.ends_with(".jpg"));
    assert_ne!(a.filename, b.filename);
    assert_eq!(a.data, b"hello");
    let c = prepare(
        b"hello",
        "Alice.personal-secret",
        "text/plain",
        &options,
        &Config::default(),
    )
    .unwrap();
    assert!(!c.filename.contains("personal"));
}
#[test]
fn age_encryption_roundtrips_and_authenticates_without_exposing_filename() {
    let options = PrivacyOptions {
        passphrase: Some("a synthetic long test passphrase".into()),
        ..Default::default()
    };
    assert!(!format!("{options:?}").contains("synthetic"));
    let config = Config::default();
    let a = prepare(
        b"private bytes",
        "Alice-secret.txt",
        "text/plain",
        &options,
        &config,
    )
    .unwrap();
    let b = prepare(
        b"private bytes",
        "Alice-secret.txt",
        "text/plain",
        &options,
        &config,
    )
    .unwrap();
    assert!(a.filename.ends_with(".age"));
    assert_eq!(a.mime_type, "application/octet-stream");
    assert!(a.data.starts_with(b"age-encryption.org/v1\n"));
    assert_ne!(a.data, b.data);
    assert_eq!(
        decrypt(&a.data, "a synthetic long test passphrase", &config).unwrap(),
        b"private bytes"
    );
    assert_eq!(
        decrypt(&a.data, "wrong", &config).unwrap_err(),
        Error::DecryptionFailed
    );
    let mut corrupt = a.data;
    *corrupt.last_mut().unwrap() ^= 1;
    assert_eq!(
        decrypt(&corrupt, "a synthetic long test passphrase", &config).unwrap_err(),
        Error::DecryptionFailed
    );
}
#[test]
fn encrypted_overhead_and_empty_passphrases_are_checked() {
    let options = PrivacyOptions {
        passphrase: Some("".into()),
        ..Default::default()
    };
    assert_eq!(
        prepare(b"hi", "a", "text/plain", &options, &Config::default()).unwrap_err(),
        Error::InvalidPassphrase
    );
    let options = PrivacyOptions {
        passphrase: Some("test-only".into()),
        ..Default::default()
    };
    let config = Config {
        max_output_bytes: 2,
        ..Default::default()
    };
    assert_eq!(
        prepare(b"hi", "a", "text/plain", &options, &config).unwrap_err(),
        Error::ResourceLimit
    );
}

#[test]
fn media_is_unavailable_until_a_backend_is_configured() {
    let config = Config::default();
    let result = assess(4, b"fLaC", &config);
    assert!(!result.can_attempt);
    assert_eq!(result.reason, Some(Error::BackendUnavailable));
    let options = PrivacyOptions {
        strip_metadata: true,
        ..Default::default()
    };
    assert_eq!(
        prepare(b"fLaC", "a.flac", "audio/flac", &options, &config).unwrap_err(),
        Error::BackendUnavailable
    );
}
#[test]
fn media_output_budget_is_separate_from_original_input_budget() {
    struct Backend;
    impl condite_metadata::media::MediaBackend for Backend {
        fn strip(
            &self,
            _data: &[u8],
            _config: &Config,
        ) -> condite_metadata::Result<(Vec<u8>, Vec<String>)> {
            Ok((
                b"fLaC-cleaned".to_vec(),
                vec!["codec-bitstream-metadata-not-cleaned".into()],
            ))
        }
    }
    let config = Config {
        max_input_bytes: 4,
        max_output_bytes: 20,
        ..Default::default()
    };
    let result = condite_metadata::prepare_with_backend(
        b"fLaC",
        "a.flac",
        "audio/flac",
        &PrivacyOptions {
            strip_metadata: true,
            ..Default::default()
        },
        &config,
        Some(&Backend),
    )
    .unwrap();
    assert_eq!(result.data, b"fLaC-cleaned");
    assert!(result.report.metadata_stripped);
}
