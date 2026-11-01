use std::process::Command;

#[test]
fn live_logging_commands() {
    if std::env::var_os("CONDITE_LOG_TEST_CHILD").is_none() {
        for filter in [Some("warn"), Some("invalid["), None] {
            let mut child = Command::new(std::env::current_exe().unwrap());
            child
                .args(["--exact", "live_logging_commands", "--nocapture"])
                .env("CONDITE_LOG_TEST_CHILD", "1");
            if let Some(filter) = filter {
                child.env("RUST_LOG", filter);
            } else {
                child.env_remove("RUST_LOG");
            }
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    condite_core::logging::init().unwrap();
    condite_core::logging::set_filter("debug").unwrap();
    assert!(tracing::enabled!(tracing::Level::DEBUG));
    assert!(condite_core::logging::set_filter("invalid[").is_err());
    assert!(tracing::enabled!(tracing::Level::DEBUG));
    assert!(condite_core::logging::set_filter("").is_err());
    let result = condite_core::logging::reload_from_env();
    if std::env::var("RUST_LOG").as_deref() == Ok("invalid[") {
        assert!(result.is_err());
        assert!(tracing::enabled!(tracing::Level::DEBUG));
    } else {
        result.unwrap();
        assert!(!tracing::enabled!(tracing::Level::DEBUG));
        assert!(tracing::enabled!(tracing::Level::WARN));
        assert_eq!(
            tracing::enabled!(tracing::Level::INFO),
            std::env::var_os("RUST_LOG").is_none()
        );
    }
}
