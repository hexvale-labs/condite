use std::process::Command;

#[test]
fn live_logging_commands() {
    let Ok(filter) = std::env::var("CONDITE_LOG_TEST_CHILD") else {
        for filter in [Some("warn"), Some("invalid["), None] {
            let directory = tempfile::tempdir().unwrap();
            let mut source = "sql_conn_uri='turso::memory:'\ns3_endpoint='https://s3.example'\ns3_region='test'\ns3_bucket='test'\ns3_access_key='test'\ns3_secret_key='test'\npublic_api_url='https://files.example'".to_owned();
            if let Some(filter) = filter {
                source.push_str(&format!("\nlog_filter='{filter}'"));
            }
            std::fs::write(directory.path().join("condite.toml"), source).unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "live_logging_commands", "--nocapture"])
                .current_dir(directory.path())
                .env("CONDITE_LOG_TEST_CHILD", filter.unwrap_or("info"))
                .env("RUST_LOG", "error") // Not an implicit TOML override.
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    let initialized = condite_core::logging::init();
    if filter == "invalid[" {
        assert!(initialized.is_err());
        return;
    }
    initialized.unwrap();
    assert_eq!(tracing::enabled!(tracing::Level::INFO), filter == "info");
    assert!(tracing::enabled!(tracing::Level::WARN));
    condite_core::logging::set_filter("debug").unwrap();
    assert!(tracing::enabled!(tracing::Level::DEBUG));
    assert!(condite_core::logging::set_filter("invalid[").is_err());
    assert!(tracing::enabled!(tracing::Level::DEBUG));
    assert!(condite_core::logging::set_filter("").is_err());
    condite_core::logging::set_filter("warn").unwrap();
    assert!(!tracing::enabled!(tracing::Level::DEBUG));
    assert!(!tracing::enabled!(tracing::Level::INFO));
    assert!(tracing::enabled!(tracing::Level::WARN));
}
