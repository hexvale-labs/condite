//! Local, format-specific file privacy transformations.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub max_inflated_bytes: u64,
    pub max_entry_bytes: u64,
    pub max_entries: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_input_bytes: 300 * 1024 * 1024,
            max_output_bytes: 300 * 1024 * 1024,
            max_inflated_bytes: 512 * 1024 * 1024,
            max_entry_bytes: 64 * 1024 * 1024,
            max_entries: 10_000,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Error {
    TooLarge,
    UnsupportedFormat,
    UnsupportedFeature,
    Malformed,
    Encrypted,
    Signed,
    ResourceLimit,
    BackendUnavailable,
    InvalidPassphrase,
    DecryptionFailed,
    InvalidConfig,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::TooLarge => "too-large",
                Self::UnsupportedFormat => "unsupported-format",
                Self::UnsupportedFeature => "unsupported-feature",
                Self::Malformed => "malformed",
                Self::Encrypted => "encrypted",
                Self::Signed => "signed",
                Self::ResourceLimit => "resource-limit",
                Self::BackendUnavailable => "backend-unavailable",
                Self::InvalidPassphrase => "invalid-passphrase",
                Self::DecryptionFailed => "decryption-failed",
                Self::InvalidConfig => "invalid-config",
            }
        )
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

mod crypto;
mod images;
pub mod media;
mod office;
mod pdf;
pub use crypto::decrypt;

#[derive(Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct PrivacyOptions {
    pub strip_metadata: bool,
    pub anonymize_filename: bool,
    pub passphrase: Option<secrecy::SecretString>,
}
impl std::fmt::Debug for PrivacyOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivacyOptions")
            .field("strip_metadata", &self.strip_metadata)
            .field("anonymize_filename", &self.anonymize_filename)
            .field("encryption", &self.passphrase.is_some())
            .finish()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub can_attempt: bool,
    pub format: Option<String>,
    pub reason: Option<Error>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub policy_version: String,
    pub metadata_stripped: bool,
    pub filename_anonymized: bool,
    pub encrypted: bool,
    pub changed: bool,
    pub retained: Vec<String>,
}
#[derive(Debug)]
pub struct PreparedFile {
    pub data: Vec<u8>,
    pub filename: String,
    pub mime_type: String,
    pub report: Report,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.max_input_bytes == 0
            || self.max_output_bytes == 0
            || self.max_inflated_bytes == 0
            || self.max_entry_bytes == 0
            || self.max_entries == 0
            || self.max_entries > 100_000
        {
            return Err(Error::InvalidConfig);
        }
        Ok(())
    }
    pub fn check_input(&self, size: u64) -> Result<()> {
        self.validate()?;
        if size > self.max_input_bytes {
            return Err(Error::TooLarge);
        }
        Ok(())
    }
}
fn format(data: &[u8]) -> Option<(&'static str, &'static str)> {
    if data.starts_with(b"\xff\xd8") {
        Some(("jpeg", "image/jpeg"))
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("png", "image/png"))
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        Some(("webp", "image/webp"))
    } else if data.starts_with(b"%PDF-") {
        Some(("pdf", "application/pdf"))
    } else if data.starts_with(b"PK\x03\x04") {
        Some(("office", "application/zip"))
    } else {
        None
    }
}
/// Bounded-header eligibility only. Full validation occurs during stripping.
pub fn assess(size: u64, header: &[u8], config: &Config) -> Assessment {
    assess_with_backend(size, header, config, false)
}
pub fn assess_with_backend(
    size: u64,
    header: &[u8],
    config: &Config,
    media_available: bool,
) -> Assessment {
    let media = media::plan(header).ok();
    let kind = format(header)
        .map(|v| v.0.to_owned())
        .or_else(|| media.as_ref().map(|m| m.format.clone()));
    let reason = config.check_input(size).err().or_else(|| {
        if kind.is_none() {
            Some(Error::UnsupportedFormat)
        } else if media.is_some() && !media_available {
            Some(Error::BackendUnavailable)
        } else {
            None
        }
    });
    Assessment {
        can_attempt: reason.is_none(),
        format: kind,
        reason,
    }
}
pub fn can_strip_metadata(size: u64, header: &[u8], config: &Config) -> bool {
    assess(size, header, config).can_attempt
}
pub fn strip(data: &[u8], config: &Config) -> Result<PreparedFile> {
    strip_with_backend(data, config, None)
}
pub fn strip_with_backend(
    data: &[u8],
    config: &Config,
    backend: Option<&dyn media::MediaBackend>,
) -> Result<PreparedFile> {
    config.check_input(data.len() as u64)?;
    if let Ok(plan) = media::plan(data) {
        let (output, retained) = backend
            .ok_or(Error::BackendUnavailable)?
            .strip(data, config)?;
        if output.len() as u64 > config.max_output_bytes {
            return Err(Error::ResourceLimit);
        }
        return Ok(PreparedFile {
            report: Report {
                policy_version: "1".into(),
                metadata_stripped: true,
                filename_anonymized: false,
                encrypted: false,
                changed: output != data,
                retained,
            },
            data: output,
            filename: String::new(),
            mime_type: plan.mime_type,
        });
    }
    let (kind, mut mime) = format(data).ok_or(Error::UnsupportedFormat)?;
    let (output, retained) = match kind {
        "jpeg" | "png" | "webp" => images::strip(data, config)?,
        "office" => office::strip(data, config)?,
        "pdf" => pdf::strip(data, config)?,
        _ => return Err(Error::UnsupportedFormat),
    };
    if output.len() as u64 > config.max_output_bytes {
        return Err(Error::ResourceLimit);
    }
    if kind == "office" {
        mime = office::mime_type(data, config)?;
    }
    Ok(PreparedFile {
        report: Report {
            policy_version: "1".into(),
            metadata_stripped: true,
            filename_anonymized: false,
            encrypted: false,
            changed: output != data,
            retained,
        },
        data: output,
        filename: String::new(),
        mime_type: mime.into(),
    })
}
/// Applies metadata cleaning, filename replacement, then authenticated age encryption.
pub fn prepare(
    data: &[u8],
    filename: &str,
    mime_type: &str,
    options: &PrivacyOptions,
    config: &Config,
) -> Result<PreparedFile> {
    prepare_with_backend(data, filename, mime_type, options, config, None)
}
pub fn prepare_with_backend(
    data: &[u8],
    filename: &str,
    mime_type: &str,
    options: &PrivacyOptions,
    config: &Config,
    backend: Option<&dyn media::MediaBackend>,
) -> Result<PreparedFile> {
    config.check_input(data.len() as u64)?;
    if filename.is_empty() || filename.len() > 255 || filename.chars().any(char::is_control) {
        return Err(Error::Malformed);
    }
    let mut result = if options.strip_metadata {
        strip_with_backend(data, config, backend)?
    } else {
        PreparedFile {
            data: data.to_vec(),
            filename: String::new(),
            mime_type: mime_type.into(),
            report: Report {
                policy_version: "1".into(),
                metadata_stripped: false,
                filename_anonymized: false,
                encrypted: false,
                changed: false,
                retained: vec!["metadata-not-cleaned".into()],
            },
        }
    };
    result.filename = if options.anonymize_filename {
        anonymize_filename(filename)?
    } else {
        filename.to_owned()
    };
    result.report.filename_anonymized = options.anonymize_filename;
    if let Some(secret) = &options.passphrase {
        use secrecy::ExposeSecret;
        let passphrase = secret.expose_secret();
        let encrypted = crypto::encrypt(&result.data, passphrase, config)?;
        use zeroize::Zeroize;
        result.data.zeroize();
        result.data = encrypted;
        result.filename.push_str(".age");
        result.mime_type = "application/octet-stream".into();
        result.report.encrypted = true;
    }
    if result.data.len() as u64 > config.max_output_bytes {
        return Err(Error::ResourceLimit);
    }
    if result.filename.len() > 255 {
        return Err(Error::Malformed);
    }
    result.report.changed = result.data != data;
    Ok(result)
}
pub fn anonymize_filename(filename: &str) -> Result<String> {
    let mut id = [0u8; 16];
    getrandom::getrandom(&mut id).map_err(|_| Error::BackendUnavailable)?;
    let mut name = id.iter().map(|b| format!("{b:02x}")).collect::<String>();
    if let Some((_, ext)) = filename.rsplit_once('.') {
        let ext = ext.to_ascii_lowercase();
        if [
            "jpg", "jpeg", "png", "webp", "pdf", "docx", "xlsx", "pptx", "mp3", "flac", "wav",
            "mp4", "mov", "txt", "csv", "json", "zip", "heic", "avif", "gif",
        ]
        .contains(&ext.as_str())
        {
            name.push('.');
            name.push_str(&ext);
        }
    }
    Ok(name)
}
