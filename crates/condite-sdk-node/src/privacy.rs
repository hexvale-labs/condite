use condite_sdk::privacy as p;
use napi::{Task, bindgen_prelude::*};
use napi_derive::napi;

#[napi(object)]
#[derive(Default)]
pub struct PrivacyConfig {
    pub max_input_bytes: Option<BigInt>,
    pub max_output_bytes: Option<BigInt>,
    pub max_inflated_bytes: Option<BigInt>,
    pub max_entry_bytes: Option<BigInt>,
    pub max_entries: Option<u32>,
}
fn config(value: Option<PrivacyConfig>) -> Result<p::Config> {
    let mut c = p::Config::default();
    if let Some(v) = value {
        if let Some(n) = v.max_input_bytes {
            c.max_input_bytes = super::integer(n)?;
        }
        if let Some(n) = v.max_output_bytes {
            c.max_output_bytes = super::integer(n)?;
        }
        if let Some(n) = v.max_inflated_bytes {
            c.max_inflated_bytes = super::integer(n)?;
        }
        if let Some(n) = v.max_entry_bytes {
            c.max_entry_bytes = super::integer(n)?;
        }
        if let Some(n) = v.max_entries {
            c.max_entries = n as usize;
        }
    }
    c.validate().map_err(failure)?;
    Ok(c)
}
fn failure(e: p::Error) -> napi::Error {
    napi::Error::new(Status::InvalidArg, e.to_string())
}
#[napi(object)]
pub struct PreparedOutput {
    pub data: Buffer,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: BigInt,
    pub report_json: String,
}
pub struct PrepareTask {
    data: Vec<u8>,
    filename: String,
    mime: String,
    options: p::PrivacyOptions,
    config: p::Config,
    media: Option<p::media::FfmpegBackend>,
}
impl Task for PrepareTask {
    type Output = p::PreparedFile;
    type JsValue = PreparedOutput;
    fn compute(&mut self) -> Result<Self::Output> {
        p::prepare_with_backend(
            &self.data,
            &self.filename,
            &self.mime,
            &self.options,
            &self.config,
            self.media
                .as_ref()
                .map(|v| v as &dyn p::media::MediaBackend),
        )
        .map_err(failure)
    }
    fn resolve(&mut self, _env: Env, value: Self::Output) -> Result<Self::JsValue> {
        Ok(PreparedOutput {
            size_bytes: (value.data.len() as u64).into(),
            data: value.data.into(),
            filename: value.filename,
            mime_type: value.mime_type,
            report_json: serde_json::to_string(&value.report)
                .map_err(|_| failure(p::Error::Malformed))?,
        })
    }
}
#[napi(object)]
pub struct MediaPaths {
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
}
#[napi]
pub fn native_prepare(
    data: Uint8Array,
    filename: String,
    mime: String,
    options_json: String,
    limits: Option<PrivacyConfig>,
    media: Option<MediaPaths>,
) -> Result<AsyncTask<PrepareTask>> {
    let config = config(limits)?;
    config.check_input(data.len() as u64).map_err(failure)?;
    let secret_json = zeroize::Zeroizing::new(options_json);
    let options =
        serde_json::from_str(&secret_json).map_err(|_| failure(p::Error::InvalidConfig))?;
    Ok(AsyncTask::new(PrepareTask {
        data: data.to_vec(),
        filename,
        mime,
        options,
        config,
        media: media.map(|v| p::media::FfmpegBackend::new(v.ffmpeg_path, v.ffprobe_path)),
    }))
}
#[napi]
pub fn native_assess(
    size: BigInt,
    header: Uint8Array,
    limits: Option<PrivacyConfig>,
    media_available: Option<bool>,
) -> Result<String> {
    let assessment = p::assess_with_backend(
        super::integer(size)?,
        &header[..header.len().min(64)],
        &config(limits)?,
        media_available.unwrap_or(false),
    );
    serde_json::to_string(&assessment).map_err(|_| failure(p::Error::Malformed))
}
pub struct DecryptTask {
    data: Vec<u8>,
    passphrase: zeroize::Zeroizing<String>,
    config: p::Config,
}
impl Task for DecryptTask {
    type Output = Vec<u8>;
    type JsValue = Buffer;
    fn compute(&mut self) -> Result<Self::Output> {
        p::decrypt(&self.data, &self.passphrase, &self.config).map_err(failure)
    }
    fn resolve(&mut self, _env: Env, value: Self::Output) -> Result<Self::JsValue> {
        Ok(value.into())
    }
}
#[napi]
pub fn native_decrypt(
    data: Uint8Array,
    passphrase: String,
    limits: Option<PrivacyConfig>,
) -> Result<AsyncTask<DecryptTask>> {
    let config = config(limits)?;
    config.check_input(data.len() as u64).map_err(failure)?;
    Ok(AsyncTask::new(DecryptTask {
        data: data.to_vec(),
        passphrase: zeroize::Zeroizing::new(passphrase),
        config,
    }))
}
