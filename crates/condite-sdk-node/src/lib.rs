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
    pub retention: Option<Retention>,
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
    #[napi(js_name = "owner_id")]
    pub owner_id: String,
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
    #[napi(js_name = "expires_at")]
    pub expires_at: Either<String, Null>,
    pub visibility: String,
    #[napi(js_name = "deletion_pending")]
    pub deletion_pending: bool,
}
impl From<sdk::Upload> for Upload {
    fn from(value: sdk::Upload) -> Self {
        Self {
            id: value.id,
            owner_id: value.owner_id,
            filename: value.filename,
            mime_type: value.mime_type,
            size_bytes: value.size_bytes.into(),
            status: match value.status {
                sdk::UploadStatus::Pending => "Pending".into(),
                sdk::UploadStatus::Confirmed => "Confirmed".into(),
            },
            created_at: value.created_at,
            expires_at: value.expires_at.map(Either::A).unwrap_or(Either::B(Null)),
            visibility: format!("{:?}", value.visibility),
            deletion_pending: value.deletion_pending,
        }
    }
}

#[napi(object)]
pub struct ListUploadsRequest {
    #[napi(js_name = "owner_id")]
    pub owner_id: Option<String>,
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
    pub upload: Upload,
    #[napi(js_name = "new_storage_size_bytes")]
    pub new_storage_size_bytes: BigInt,
}
impl From<sdk::ConfirmResponse> for ConfirmResponse {
    fn from(value: sdk::ConfirmResponse) -> Self {
        Self {
            upload: value.upload.into(),
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
            retention: v.retention.map(|r| sdk::Retention {
                expires_at: nullable_string(r.expires_at),
            }),
        })
    }
}
fn nullable_string(value: Either<String, Null>) -> Option<String> {
    match value {
        Either::A(value) => Some(value),
        Either::B(_) => None,
    }
}

#[napi(object)]
pub struct Retention {
    #[napi(js_name = "expires_at")]
    pub expires_at: Either<String, Null>,
}
#[napi(object)]
pub struct CreateShareRequest {
    #[napi(js_name = "expires_at")]
    pub expires_at: Either<String, Null>,
    #[napi(js_name = "follows_upload")]
    pub follows_upload: bool,
}
#[napi(object)]
pub struct DownloadResponse {
    pub uri: String,
    #[napi(js_name = "expires_at")]
    pub expires_at: String,
}
impl From<sdk::DownloadResponse> for DownloadResponse {
    fn from(v: sdk::DownloadResponse) -> Self {
        Self {
            uri: v.uri,
            expires_at: v.expires_at,
        }
    }
}
#[napi(object)]
pub struct Share {
    pub id: String,
    #[napi(js_name = "upload_id")]
    pub upload_id: String,
    pub url: String,
    #[napi(js_name = "created_at")]
    pub created_at: String,
    #[napi(js_name = "expires_at")]
    pub expires_at: Either<String, Null>,
    #[napi(js_name = "follows_upload")]
    pub follows_upload: bool,
    pub status: String,
}
impl From<sdk::Share> for Share {
    fn from(v: sdk::Share) -> Self {
        Self {
            id: v.id,
            upload_id: v.upload_id,
            url: v.url,
            created_at: v.created_at,
            expires_at: v.expires_at.map(Either::A).unwrap_or(Either::B(Null)),
            follows_upload: v.follows_upload,
            status: format!("{:?}", v.status),
        }
    }
}
#[napi(object)]
pub struct ListSharesResponse {
    pub shares: Vec<Share>,
    #[napi(js_name = "next_after")]
    pub next_after: Either<String, Null>,
}
impl From<sdk::ListSharesResponse> for ListSharesResponse {
    fn from(v: sdk::ListSharesResponse) -> Self {
        Self {
            shares: v.shares.into_iter().map(Into::into).collect(),
            next_after: v.next_after.map(Either::A).unwrap_or(Either::B(Null)),
        }
    }
}
#[napi]
pub struct NativeClient {
    inner: sdk::Client,
}
#[napi]
impl NativeClient {
    #[napi(constructor)]
    pub fn new(base: String, api_key: Option<String>) -> Result<Self> {
        Ok(Self {
            inner: match api_key {
                Some(key) => sdk::Client::with_api_key(base, &key),
                None => sdk::Client::new(base),
            }
            .map_err(error)?,
        })
    }
    #[napi]
    pub async fn account(&self) -> Result<Account> {
        self.inner.account().await.map(Into::into).map_err(error)
    }
    /// Raw JSON retains u64 precision; use bigint-aware parsing at the call site.
    #[napi]
    pub async fn control_json(&self, command: String) -> Result<String> {
        let command = serde_json::from_str(&command)
            .map_err(|_| napi::Error::from_reason("invalid control command"))?;
        let response = self.inner.control(&command).await.map_err(error)?;
        serde_json::to_string(&response)
            .map_err(|_| napi::Error::from_reason("invalid control response"))
    }
    #[napi]
    pub async fn login(&self, api_key: String) -> Result<SessionResponse> {
        self.inner
            .login(&api_key)
            .await
            .map(|v| SessionResponse {
                token: v.token,
                expires_at: v.expires_at,
            })
            .map_err(error)
    }
    #[napi]
    pub async fn logout(&self) -> Result<()> {
        self.inner.logout().await.map_err(error)
    }
    #[napi]
    pub async fn download_bytes(&self, id: String) -> Result<Buffer> {
        self.inner
            .download_bytes(&id)
            .await
            .map(Buffer::from)
            .map_err(error)
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
                owner_id: request.owner_id,
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
    pub async fn download(&self, id: String) -> Result<DownloadResponse> {
        self.inner
            .download(&id)
            .await
            .map(Into::into)
            .map_err(error)
    }
    #[napi]
    pub async fn update_retention(&self, id: String, retention: Retention) -> Result<Upload> {
        self.inner
            .update_retention(
                &id,
                &sdk::Retention {
                    expires_at: nullable_string(retention.expires_at),
                },
            )
            .await
            .map(Into::into)
            .map_err(error)
    }
    #[napi]
    pub async fn create_share(&self, id: String, request: CreateShareRequest) -> Result<Share> {
        self.inner
            .create_share(
                &id,
                &sdk::CreateShareRequest {
                    expires_at: nullable_string(request.expires_at),
                    follows_upload: request.follows_upload,
                },
            )
            .await
            .map(Into::into)
            .map_err(error)
    }
    #[napi]
    pub async fn list_shares(
        &self,
        id: String,
        query: ListUploadsRequest,
    ) -> Result<ListSharesResponse> {
        self.inner
            .list_shares(
                &id,
                &sdk::ListUploadsRequest {
                    owner_id: query.owner_id,
                    limit: query.limit,
                    after: query.after,
                },
            )
            .await
            .map(Into::into)
            .map_err(error)
    }
    #[napi]
    pub async fn revoke_share(&self, id: String, share_id: String) -> Result<()> {
        self.inner.revoke_share(&id, &share_id).await.map_err(error)
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

#[napi(object)]
pub struct Usage {
    #[napi(js_name = "confirmed_bytes")]
    pub confirmed_bytes: BigInt,
    #[napi(js_name = "reserved_bytes")]
    pub reserved_bytes: BigInt,
    #[napi(js_name = "charged_bytes")]
    pub charged_bytes: BigInt,
}
impl From<sdk::Usage> for Usage {
    fn from(v: sdk::Usage) -> Self {
        Self {
            confirmed_bytes: v.confirmed_bytes.into(),
            reserved_bytes: v.reserved_bytes.into(),
            charged_bytes: v.charged_bytes.into(),
        }
    }
}
#[napi(object)]
pub struct AccountLimits {
    #[napi(js_name = "per_file_bytes")]
    pub per_file_bytes: Either<BigInt, Null>,
    #[napi(js_name = "storage_bytes")]
    pub storage_bytes: Either<BigInt, Null>,
}
#[napi(object)]
pub struct Account {
    pub id: String,
    pub role: String,
    pub usage: Usage,
    pub limits: AccountLimits,
    #[napi(js_name = "global_usage")]
    pub global_usage: Either<Usage, Null>,
}
impl From<sdk::Account> for Account {
    fn from(v: sdk::Account) -> Self {
        Self {
            id: v.id,
            role: match v.role {
                sdk::AccountRole::Key => "key".into(),
                sdk::AccountRole::Root => "root".into(),
            },
            usage: v.usage.into(),
            limits: AccountLimits {
                per_file_bytes: v
                    .limits
                    .per_file_bytes
                    .map(|n| Either::A(n.into()))
                    .unwrap_or(Either::B(Null)),
                storage_bytes: v
                    .limits
                    .storage_bytes
                    .map(|n| Either::A(n.into()))
                    .unwrap_or(Either::B(Null)),
            },
            global_usage: v
                .global_usage
                .map(|u| Either::A(u.into()))
                .unwrap_or(Either::B(Null)),
        }
    }
}
#[napi(object)]
pub struct SessionResponse {
    pub token: String,
    #[napi(js_name = "expires_at")]
    pub expires_at: String,
}
