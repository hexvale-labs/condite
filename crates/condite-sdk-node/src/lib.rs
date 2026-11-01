pub mod privacy;
use condite_sdk as sdk;
use napi::bindgen_prelude::*;
use napi_derive::napi;

fn error(e: sdk::Error) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, serde_json::json!({"code":e.code(),"message":e.to_string(),"status":e.status(),"body":e.body(),"request_id":e.request_id(),"stage":e.stage()}).to_string())
}
fn integer(value: BigInt) -> Result<u64> {
    let (negative, value, lossless) = value.get_u64();
    if negative || !lossless {
        return Err(error(sdk::Error::Input(
            "expected an unsigned 64-bit bigint".into(),
        )));
    }
    Ok(value)
}

#[napi(object)]
pub struct HealthStatus {
    #[napi(js_name = "healthy")]
    pub healthy: bool,
    #[napi(js_name = "started_at")]
    pub started_at: String,
    #[napi(js_name = "runtime")]
    pub runtime: String,
}
impl From<sdk::HealthStatus> for HealthStatus {
    fn from(value: sdk::HealthStatus) -> Self {
        Self {
            healthy: value.healthy,
            started_at: value.started_at,
            runtime: value.runtime,
        }
    }
}

#[napi(object)]
pub struct PresignRequest {
    #[napi(js_name = "size_bytes")]
    pub size_bytes: BigInt,
    #[napi(js_name = "mime_type")]
    pub mime_type: String,
    #[napi(js_name = "filename")]
    pub filename: String,
}

#[napi(object)]
pub struct PresignResponse {
    #[napi(js_name = "request_id")]
    pub request_id: String,
    #[napi(js_name = "uri")]
    pub uri: String,
    #[napi(js_name = "headers")]
    pub headers: std::collections::HashMap<String, String>,
    #[napi(js_name = "expires_at_utc")]
    pub expires_at_utc: String,
}
impl From<sdk::PresignResponse> for PresignResponse {
    fn from(value: sdk::PresignResponse) -> Self {
        Self {
            request_id: value.request_id,
            uri: value.uri,
            headers: value.headers,
            expires_at_utc: value.expires_at_utc,
        }
    }
}

#[napi(object)]
pub struct Upload {
    #[napi(js_name = "id")]
    pub id: String,
    #[napi(js_name = "filename")]
    pub filename: String,
    #[napi(js_name = "mime_type")]
    pub mime_type: String,
    #[napi(js_name = "size_bytes")]
    pub size_bytes: BigInt,
    #[napi(js_name = "status")]
    pub status: String,
    #[napi(js_name = "created_at")]
    pub created_at: String,
}
impl From<sdk::Upload> for Upload {
    fn from(value: sdk::Upload) -> Self {
        Self {
            id: value.id,
            filename: value.filename,
            mime_type: value.mime_type,
            size_bytes: value.size_bytes.into(),
            status: match value.status {
                sdk::UploadStatus::Pending => "Pending".into(),
                sdk::UploadStatus::Confirmed => "Confirmed".into(),
            },
            created_at: value.created_at,
        }
    }
}

#[napi(object)]
pub struct ListUploadsRequest {
    #[napi(js_name = "limit")]
    pub limit: Option<u32>,
    #[napi(js_name = "after")]
    pub after: Option<String>,
}

#[napi(object)]
pub struct ListUploadsResponse {
    #[napi(js_name = "uploads")]
    pub uploads: Vec<Upload>,
    #[napi(js_name = "next_after")]
    pub next_after: Either<String, Null>,
}
impl From<sdk::ListUploadsResponse> for ListUploadsResponse {
    fn from(value: sdk::ListUploadsResponse) -> Self {
        Self {
            uploads: value.uploads.into_iter().map(Into::into).collect(),
            next_after: value.next_after.map(Either::A).unwrap_or(Either::B(Null)),
        }
    }
}

#[napi(object)]
pub struct ConfirmResponse {
    #[napi(js_name = "public_uri")]
    pub public_uri: String,
    #[napi(js_name = "expires_at")]
    pub expires_at: String,
    #[napi(js_name = "new_storage_size_bytes")]
    pub new_storage_size_bytes: BigInt,
}
impl From<sdk::ConfirmResponse> for ConfirmResponse {
    fn from(value: sdk::ConfirmResponse) -> Self {
        Self {
            public_uri: value.public_uri,
            expires_at: value.expires_at,
            new_storage_size_bytes: value.new_storage_size_bytes.into(),
        }
    }
}

#[napi(object)]
pub struct UploadResult {
    #[napi(js_name = "request_id")]
    pub request_id: String,
    #[napi(js_name = "confirmation")]
    pub confirmation: ConfirmResponse,
}
impl From<sdk::UploadResult> for UploadResult {
    fn from(value: sdk::UploadResult) -> Self {
        Self {
            request_id: value.request_id,
            confirmation: value.confirmation.into(),
        }
    }
}

impl TryFrom<PresignRequest> for sdk::PresignRequest {
    type Error = napi::Error;
    fn try_from(v: PresignRequest) -> Result<Self> {
        Ok(Self {
            size_bytes: integer(v.size_bytes)?,
            mime_type: v.mime_type,
            filename: v.filename,
        })
    }
}
#[napi]
pub struct NativeClient {
    inner: sdk::Client,
}
#[napi]
impl NativeClient {
    #[napi(constructor)]
    pub fn new(base: String) -> Result<Self> {
        Ok(Self {
            inner: sdk::Client::new(base).map_err(error)?,
        })
    }
    #[napi]
    pub async fn health(&self) -> Result<HealthStatus> {
        self.inner.health().await.map(Into::into).map_err(error)
    }
    #[napi]
    pub async fn presign(&self, request: PresignRequest) -> Result<PresignResponse> {
        self.inner
            .presign(&request.try_into()?)
            .await
            .map(Into::into)
            .map_err(error)
    }
    #[napi]
    pub async fn list_uploads(&self, request: ListUploadsRequest) -> Result<ListUploadsResponse> {
        self.inner
            .list_uploads(&sdk::ListUploadsRequest {
                limit: request.limit,
                after: request.after,
            })
            .await
            .map(Into::into)
            .map_err(error)
    }
    #[napi]
    pub async fn confirm(&self, id: String) -> Result<ConfirmResponse> {
        self.inner.confirm(&id).await.map(Into::into).map_err(error)
    }
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        self.inner.delete(&id).await.map_err(error)
    }
    #[napi]
    pub async fn put_upload(&self, signed: PresignResponse, data: Uint8Array) -> Result<()> {
        self.inner
            .put_upload(
                &sdk::PresignResponse {
                    request_id: signed.request_id,
                    uri: signed.uri,
                    headers: signed.headers,
                    expires_at_utc: signed.expires_at_utc,
                },
                data.to_vec(),
            )
            .await
            .map_err(error)
    }
    #[napi]
    pub async fn upload(&self, request: PresignRequest, data: Uint8Array) -> Result<UploadResult> {
        self.inner
            .upload(&request.try_into()?, data.to_vec())
            .await
            .map(Into::into)
            .map_err(error)
    }
}
