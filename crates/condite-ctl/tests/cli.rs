use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::Command,
};

#[test]
fn generate_sends_defaults_and_prints_secret() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        let command: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(command["command"], "generate_api_key");
        assert_eq!(command["per_upload_quota"], 104_857_600);
        assert_eq!(command["storage_quota"], 1_073_741_824);
        stream
            .write_all(b"{\"type\":\"api_key\",\"id\":\"test-id\",\"api_key\":\"secret-value\"}\n")
            .unwrap();
    });
    let output = Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
        .args(["--socket", path.to_str().unwrap(), "apikey", "generate"])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("secret-value"));
}

#[test]
fn help_and_validation_work_without_a_server() {
    for args in [vec!["--help"], vec!["apikey", "generate", "--help"]] {
        assert!(
            Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    for args in [
        vec!["apikey", "generate", "--max-per-upload-size=100"],
        vec!["apikey", "update", "bad-id"],
        vec!["apikey", "generate", "--max-total-storage-size=-1mb"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn xdg_discovery_and_server_errors() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("condite")).unwrap();
    let listener = UnixListener::bind(dir.path().join("condite/condite.sock")).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).unwrap()["command"],
            "status"
        );
        stream
            .write_all(
                b"{\"type\":\"error\",\"code\":\"test_error\",\"message\":\"visible failure\"}\n",
            )
            .unwrap();
    });
    let output = Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
        .env("XDG_RUNTIME_DIR", dir.path())
        .arg("status")
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("visible failure"));
}

#[test]
fn stalled_server_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));
        drop(stream);
    });
    let start = std::time::Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
        .args([
            "--socket",
            path.to_str().unwrap(),
            "--timeout",
            "1",
            "status",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
    server.join().unwrap();
}

#[test]
fn absent_xdg_socket_falls_back_to_system_socket() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
        .env("XDG_RUNTIME_DIR", dir.path())
        .arg("status")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("/run/condite/condite.sock"));
}

#[test]
fn malformed_or_oversized_response_fails() {
    for response in [
        b"not json\n".to_vec(),
        vec![b'x'; 16_385],
        b"{\"type\":\"success\"}".to_vec(),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let _ = stream.write_all(&response);
        });
        let output = Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
            .args(["--socket", path.to_str().unwrap(), "status"])
            .output()
            .unwrap();
        server.join().unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn trickling_server_cannot_extend_the_frame_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        for _ in 0..10 {
            if stream.write_all(b" ").is_err() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    });
    let start = std::time::Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
        .args([
            "--socket",
            path.to_str().unwrap(),
            "--timeout",
            "1",
            "status",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let elapsed = start.elapsed();
    server.join().unwrap();
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "deadline took {elapsed:?}"
    );
}

#[test]
fn update_and_delete_commands_send_key_ids_and_preserve_unspecified_limits() {
    let cases = [
        (
            vec!["apikey", "update", "key-id", "--max-per-upload-size=200mb"],
            serde_json::json!({"command":"update_api_key","id":"key-id","per_upload_quota":209_715_200,"storage_quota":null}),
        ),
        (
            vec!["apikey", "delete", "key-id"],
            serde_json::json!({"command":"delete_api_key","id":"key-id"}),
        ),
        (
            vec!["upload", "delete", "upload-id"],
            serde_json::json!({"command":"delete_upload","id":"upload-id"}),
        ),
    ];
    for (args, expected) in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&line).unwrap(),
                expected
            );
            stream.write_all(b"{\"type\":\"success\"}\n").unwrap();
        });
        let output = Command::new(env!("CARGO_BIN_EXE_condite-ctl"))
            .args(["--socket", path.to_str().unwrap(), "--json"])
            .args(args)
            .output()
            .unwrap();
        server.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["type"],
            "success"
        );
    }
}
