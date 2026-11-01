use crate::{Config, Error, Result};

use serde::Serialize;

/// Candidate remux plan. Processing also requires successful stream probing.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPlan {
    pub format: String,
    pub mime_type: String,
    pub extension: String,
    pub args: Vec<String>,
    pub retained: Vec<String>,
}
pub trait MediaBackend {
    fn strip(&self, data: &[u8], config: &Config) -> Result<(Vec<u8>, Vec<String>)>;
}
pub fn plan(data: &[u8]) -> Result<MediaPlan> {
    let (format, mime, extension) = if data.starts_with(b"ID3")
        || (data.len() >= 2 && data[0] == 0xff && data[1] & 0xe0 == 0xe0 && data[1] & 0x06 != 0)
    {
        ("mp3", "audio/mpeg", "mp3")
    } else if data.starts_with(b"fLaC") {
        ("flac", "audio/flac", "flac")
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WAVE") {
        ("wav", "audio/wav", "wav")
    } else if data.get(4..8) == Some(b"ftyp") {
        match data.get(8..12) {
            Some(b"qt  ") => ("mov", "video/quicktime", "mov"),
            Some(b"isom" | b"iso2" | b"mp41" | b"mp42" | b"avc1" | b"M4A " | b"M4V ") => {
                ("mp4", "video/mp4", "mp4")
            }
            _ => return Err(Error::UnsupportedFeature),
        }
    } else {
        return Err(Error::UnsupportedFormat);
    };
    let input = format!("input.{extension}");
    let output = format!("output.{extension}");
    let mut args: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-y",
        "-protocol_whitelist",
        "file,pipe",
        "-f",
        demuxer(format),
    ]
    .into_iter()
    .map(String::from)
    .collect();
    if matches!(format, "mp4" | "mov") {
        args.extend(
            ["-enable_drefs", "0", "-use_absolute_path", "0"]
                .into_iter()
                .map(String::from),
        );
    }
    args.extend(
        [
            "-i",
            &input,
            "-map",
            "0:v?",
            "-map",
            "0:a?",
            "-map_metadata",
            "-1",
            "-map_metadata:s",
            "-1",
            "-map_chapters",
            "-1",
            "-c",
            "copy",
            "-dn",
            "-sn",
            "-fflags",
            "+bitexact",
            "-metadata",
            "encoder=",
        ]
        .into_iter()
        .map(String::from),
    );
    if format == "mp3" {
        args.extend(
            ["-id3v2_version", "0", "-write_id3v1", "0"]
                .into_iter()
                .map(String::from),
        );
    }
    args.push(output);
    Ok(MediaPlan {
        format: format.into(),
        mime_type: mime.into(),
        extension: extension.into(),
        args,
        retained: vec![
            "codec-bitstream-metadata-not-cleaned".into(),
            "rotation-color-playback-structures".into(),
            "ffmpeg-build-dependent-remux".into(),
        ],
    })
}
fn demuxer(format: &str) -> &str {
    if format == "mp4" { "mov" } else { format }
}

/// Browser and native adapters use the same conservative stream policy.
pub fn validate_probe(data: &[u8], config: &Config) -> Result<()> {
    if data.len() as u64 > config.max_entry_bytes {
        return Err(Error::ResourceLimit);
    }
    let probe: serde_json::Value = serde_json::from_slice(data).map_err(|_| Error::Malformed)?;
    if probe.get("error").is_some() {
        return Err(Error::Malformed);
    }
    let streams = probe
        .get("streams")
        .and_then(|v| v.as_array())
        .ok_or(Error::Malformed)?;
    if streams.is_empty() {
        return Err(Error::Malformed);
    }
    if streams.len() > config.max_entries {
        return Err(Error::ResourceLimit);
    }
    for stream in streams {
        match stream.get("codec_type").and_then(|v| v.as_str()) {
            Some("audio" | "video") => {}
            _ => return Err(Error::UnsupportedFeature),
        }
        if matches!(
            stream.get("codec_tag_string").and_then(|v| v.as_str()),
            Some("encv" | "enca")
        ) {
            return Err(Error::Encrypted);
        }
        if stream
            .pointer("/disposition/attached_pic")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            != 0
        {
            return Err(Error::UnsupportedFeature);
        }
        if let Some(side_data) = stream.get("side_data_list") {
            for side in side_data.as_array().ok_or(Error::Malformed)? {
                match side.get("side_data_type").and_then(|v| v.as_str()) {
                    Some(
                        "Display Matrix"
                        | "Mastering display metadata"
                        | "Content light level metadata",
                    ) => {}
                    _ => return Err(Error::UnsupportedFeature),
                }
            }
        }
    }
    if let Some(packets) = probe.get("packets") {
        let packets = packets.as_array().ok_or(Error::Malformed)?;
        if packets.len() > config.max_entries {
            return Err(Error::ResourceLimit);
        }
        for packet in packets {
            if let Some(sides) = packet.get("side_data_list") {
                for side in sides.as_array().ok_or(Error::Malformed)? {
                    match side.get("side_data_type").and_then(|v| v.as_str()) {
                        Some("Skip Samples") => {}
                        Some("Encryption info" | "Encryption initialization data") => {
                            return Err(Error::Encrypted);
                        }
                        _ => return Err(Error::UnsupportedFeature),
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    /// Caller-installed binaries only. No binary download or shell execution.
    #[derive(Clone, Debug)]
    pub struct FfmpegBackend {
        executable: PathBuf,
        probe_executable: PathBuf,
    }
    impl FfmpegBackend {
        pub fn new(ffmpeg: impl Into<PathBuf>, ffprobe: impl Into<PathBuf>) -> Self {
            Self {
                executable: ffmpeg.into(),
                probe_executable: ffprobe.into(),
            }
        }
    }
    fn run(command: &mut Command, watched: &Path, limit: u64, deadline: Instant) -> Result<()> {
        let mut child = command
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Error::BackendUnavailable)?;
        let result = loop {
            if Instant::now() >= deadline
                || fs::metadata(watched)
                    .map(|m| m.len() > limit)
                    .unwrap_or(false)
            {
                break Err(Error::ResourceLimit);
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        break Err(Error::UnsupportedFeature);
                    }
                    if fs::metadata(watched)
                        .map(|m| m.len() > limit)
                        .unwrap_or(false)
                    {
                        break Err(Error::ResourceLimit);
                    }
                    break Ok(());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => break Err(Error::BackendUnavailable),
            }
        };
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        result
    }
    fn probe(
        backend: &FfmpegBackend,
        p: &MediaPlan,
        dir: &Path,
        file: &str,
        config: &Config,
        deadline: Instant,
    ) -> Result<()> {
        let json_path = dir.join("probe.json");
        let output = fs::File::create(&json_path).map_err(|_| Error::BackendUnavailable)?;
        let mut cmd = Command::new(&backend.probe_executable);
        cmd.current_dir(dir).stdout(Stdio::from(output)).args([
            "-v",
            "error",
            "-max_alloc",
            &config.max_entry_bytes.to_string(),
            "-protocol_whitelist",
            "file,pipe",
            "-f",
            demuxer(&p.format),
        ]);
        if matches!(p.format.as_str(), "mp4" | "mov") {
            cmd.args(["-enable_drefs", "0", "-use_absolute_path", "0"]);
        }
        cmd.args([
            "-show_error",
            "-show_streams",
            "-show_packets",
            "-show_entries",
            "stream:packet=stream_index:packet_side_data",
            "-of",
            "json",
            file,
        ]);
        run(
            &mut cmd,
            &json_path,
            config.max_entry_bytes.min(8 * 1024 * 1024),
            deadline,
        )?;
        let json = fs::read(&json_path).map_err(|_| Error::BackendUnavailable)?;
        validate_probe(&json, config)
    }
    impl MediaBackend for FfmpegBackend {
        fn strip(&self, data: &[u8], config: &Config) -> Result<(Vec<u8>, Vec<String>)> {
            config.check_input(data.len() as u64)?;
            let p = plan(data)?;
            let dir = tempfile::tempdir().map_err(|_| Error::BackendUnavailable)?;
            let input = format!("input.{}", p.extension);
            let output = format!("output.{}", p.extension);
            fs::write(dir.path().join(&input), data).map_err(|_| Error::BackendUnavailable)?;
            let deadline = Instant::now() + Duration::from_secs(30);
            probe(self, &p, dir.path(), &input, config, deadline)?;
            let mut command = Command::new(&self.executable);
            command
                .current_dir(dir.path())
                .stdout(Stdio::null())
                .args(["-max_alloc", &config.max_entry_bytes.to_string()])
                .args(&p.args[..p.args.len() - 1]);
            // Polling may allow temporary disk overshoot; accepted bytes are checked
            // again before reading. -fs is avoided because it silently truncates.
            command.arg(&output);
            let output_path = dir.path().join(&output);
            run(
                &mut command,
                &output_path,
                config.max_output_bytes,
                deadline,
            )?;
            probe(self, &p, dir.path(), &output, config, deadline)?;
            let bytes = fs::read(&output_path).map_err(|_| Error::BackendUnavailable)?;
            if bytes.len() as u64 > config.max_output_bytes {
                return Err(Error::ResourceLimit);
            }
            Ok((bytes, p.retained))
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use native::FfmpegBackend;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plans_all_supported_media_with_explicit_metadata_mapping() {
        for (data, extension) in [
            (b"ID3\x04\0\0\0\0\0\0".as_slice(), "mp3"),
            (b"fLaC".as_slice(), "flac"),
            (b"RIFF\0\0\0\0WAVE".as_slice(), "wav"),
            (b"\0\0\0\x18ftypisom".as_slice(), "mp4"),
            (b"\0\0\0\x18ftypqt  ".as_slice(), "mov"),
        ] {
            let p = plan(data).unwrap();
            assert_eq!(p.extension, extension);
            assert!(p.args.windows(2).any(|a| a == ["-map_metadata:s", "-1"]));
            assert!(
                p.retained
                    .iter()
                    .any(|s| s == "codec-bitstream-metadata-not-cleaned")
            );
        }
    }
    #[test]
    fn rejects_spoofed_formats() {
        assert_eq!(plan(b"random.mp3").unwrap_err(), Error::UnsupportedFormat);
        assert_eq!(
            plan(b"RIFF\0\0\0\0WEBP").unwrap_err(),
            Error::UnsupportedFormat
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn missing_backend_returns_unavailable() {
        let backend = FfmpegBackend::new("/does-not-exist/ffmpeg", "/does-not-exist/ffprobe");
        assert_eq!(
            backend.strip(b"fLaC", &Config::default()),
            Err(Error::BackendUnavailable)
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires CONDITE_TEST_FFMPEG and CONDITE_TEST_FFPROBE installed locally"]
    fn actual_remux_removes_canaries_and_preserves_decoded_audio() {
        use std::{
            fs,
            process::{Command, Stdio},
        };
        let ffmpeg = std::env::var("CONDITE_TEST_FFMPEG").expect("configured FFmpeg path");
        let ffprobe = std::env::var("CONDITE_TEST_FFPROBE").expect("configured ffprobe path");
        let backend = FfmpegBackend::new(&ffmpeg, &ffprobe);
        let dir = tempfile::tempdir().unwrap();
        for (extension, codec) in [
            ("mp3", "libmp3lame"),
            ("flac", "flac"),
            ("wav", "pcm_s16le"),
            ("mp4", "aac"),
            ("mov", "aac"),
        ] {
            let input = dir.path().join(format!("input.{extension}"));
            assert!(
                Command::new(&ffmpeg)
                    .args([
                        "-v",
                        "error",
                        "-nostdin",
                        "-y",
                        "-f",
                        "lavfi",
                        "-i",
                        "sine=frequency=440:duration=0.2",
                        "-c:a",
                        codec,
                        "-metadata",
                        "artist=PRIVATE_CANARY",
                        "-metadata:s:a",
                        "title=PRIVATE_CANARY"
                    ])
                    .arg(&input)
                    .stdin(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
            let original = fs::read(&input).unwrap();
            let (cleaned, retained) = backend.strip(&original, &Config::default()).unwrap();
            assert!(
                !cleaned
                    .windows(b"PRIVATE_CANARY".len())
                    .any(|v| v == b"PRIVATE_CANARY"),
                "{extension}"
            );
            assert!(
                retained
                    .iter()
                    .any(|v| v == "codec-bitstream-metadata-not-cleaned")
            );
            let output = dir.path().join(format!("clean.{extension}"));
            fs::write(&output, cleaned).unwrap();
            let decode = |file: &std::path::Path| {
                let result = Command::new(&ffmpeg)
                    .args(["-v", "error", "-nostdin", "-i"])
                    .arg(file)
                    .args([
                        "-map",
                        "0:a:0",
                        "-f",
                        "s16le",
                        "-acodec",
                        "pcm_s16le",
                        "pipe:1",
                    ])
                    .stdin(Stdio::null())
                    .output()
                    .unwrap();
                assert!(result.status.success());
                result.stdout
            };
            assert_eq!(
                decode(&input),
                decode(&output),
                "decoded {extension} audio changed"
            );
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires CONDITE_TEST_FFMPEG and CONDITE_TEST_FFPROBE installed locally"]
    fn actual_video_remux_preserves_rotation_color_and_pixels() {
        use std::{
            fs,
            process::{Command, Stdio},
        };
        let ffmpeg = std::env::var("CONDITE_TEST_FFMPEG").unwrap();
        let ffprobe = std::env::var("CONDITE_TEST_FFPROBE").unwrap();
        let backend = FfmpegBackend::new(&ffmpeg, &ffprobe);
        let dir = tempfile::tempdir().unwrap();
        for extension in ["mp4", "mov"] {
            let base = dir.path().join(format!("base.{extension}"));
            assert!(
                Command::new(&ffmpeg)
                    .args([
                        "-v",
                        "error",
                        "-nostdin",
                        "-y",
                        "-f",
                        "lavfi",
                        "-i",
                        "testsrc2=size=16x16:rate=5:duration=0.4",
                        "-c:v",
                        "mpeg4",
                        "-color_primaries",
                        "bt709",
                        "-color_trc",
                        "bt709",
                        "-colorspace",
                        "bt709",
                        "-color_range",
                        "tv"
                    ])
                    .arg(&base)
                    .status()
                    .unwrap()
                    .success()
            );
            let input = dir.path().join(format!("input.{extension}"));
            assert!(
                Command::new(&ffmpeg)
                    .args([
                        "-v",
                        "error",
                        "-nostdin",
                        "-y",
                        "-display_rotation",
                        "90",
                        "-i"
                    ])
                    .arg(&base)
                    .args(["-c", "copy", "-metadata", "artist=PRIVATE_CANARY"])
                    .arg(&input)
                    .status()
                    .unwrap()
                    .success()
            );
            let original = fs::read(&input).unwrap();
            let (cleaned, _) = backend.strip(&original, &Config::default()).unwrap();
            let output = dir.path().join(format!("clean.{extension}"));
            fs::write(&output, cleaned).unwrap();
            let inspect = |file: &std::path::Path| {
                let output = Command::new(&ffprobe)
                    .args(["-v", "error", "-show_streams", "-of", "json"])
                    .arg(file)
                    .output()
                    .unwrap();
                assert!(output.status.success());
                serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
            };
            let before = inspect(&input);
            let after = inspect(&output);
            assert!(
                after["streams"][0]["side_data_list"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["side_data_type"] == "Display Matrix")
            );
            for field in [
                "side_data_list",
                "color_space",
                "color_primaries",
                "color_transfer",
                "color_range",
            ] {
                assert_eq!(
                    before["streams"][0][field], after["streams"][0][field],
                    "{extension} {field}"
                );
            }
            let decode = |file: &std::path::Path| {
                let output = Command::new(&ffmpeg)
                    .args(["-v", "error", "-nostdin", "-noautorotate", "-i"])
                    .arg(file)
                    .args([
                        "-map", "0:v:0", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1",
                    ])
                    .stdin(Stdio::null())
                    .output()
                    .unwrap();
                assert!(output.status.success());
                output.stdout
            };
            assert_eq!(
                decode(&input),
                decode(&output),
                "{extension} decoded video changed"
            );
            let config = Config {
                max_output_bytes: 1,
                ..Config::default()
            };
            assert_eq!(backend.strip(&original, &config), Err(Error::ResourceLimit));
        }
    }
    #[test]
    fn probe_policy_rejects_subtitles_artwork_and_unknown_side_data() {
        for input in [
            r#"{"streams":[{"codec_type":"subtitle"}]}"#,
            r#"{"streams":[{"codec_type":"data"}]}"#,
            r#"{"streams":[{"codec_type":"video","disposition":{"attached_pic":1}}]}"#,
            r#"{"streams":[{"codec_type":"video","side_data_list":[{"side_data_type":"GPS"}]}]}"#,
        ] {
            assert_eq!(
                validate_probe(input.as_bytes(), &Config::default()),
                Err(Error::UnsupportedFeature)
            );
        }
        assert_eq!(
            validate_probe(
                br#"{"streams":[{"codec_type":"audio"}]}"#,
                &Config::default()
            ),
            Ok(())
        );
    }
}
