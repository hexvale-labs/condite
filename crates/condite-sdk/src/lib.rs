//! Async Condite HTTP client for native Rust and browser WebAssembly.
mod error;
pub use condite_metadata as privacy;
pub use condite_shared::*;
pub use error::Error;
use reqwest::{Method, Url};
use serde::de::DeserializeOwned;

#[derive(Clone, Debug)]
pub struct Client {
    base: Url,
    api: reqwest::Client,
    storage: reqwest::Client,
}
impl Client {
    pub fn new(base: impl AsRef<str>) -> Result<Self, Error> {
        Self::with_client(base, reqwest::Client::new())
    }
    /// Authenticate API calls; signed storage requests never receive this credential.
    pub fn with_api_key(base: impl AsRef<str>, key: &str) -> Result<Self, Error> {
        let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| Error::Input("invalid API key header".into()))?;
        value.set_sensitive(true);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, value);
        let builder = reqwest::Client::builder().default_headers(headers);
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.redirect(reqwest::redirect::Policy::none());
        let api = builder.build()?;
        Self::with_client(base, api)
    }
    /// The supplied client's default headers are used only for API requests.
    pub fn with_client(base: impl AsRef<str>, api: reqwest::Client) -> Result<Self, Error> {
        let mut base = Url::parse(base.as_ref()).map_err(|e| Error::Input(e.to_string()))?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.username().is_empty()
            || base.password().is_some()
        {
            return Err(Error::Input(
                "base URL must be HTTP(S), without credentials, query, or fragment".into(),
            ));
        }
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        Ok(Self {
            base,
            api,
            storage: reqwest::Client::new(),
        })
    }
    fn url(&self, parts: &[&str]) -> Result<Url, Error> {
        if parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == "..")
        {
            return Err(Error::Input(
                "path segments must not be empty or dot segments".into(),
            ));
        }
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|_| Error::Input("invalid base URL".into()))?
            .pop_if_empty()
            .extend(parts);
        Ok(url)
    }
    async fn checked(response: reqwest::Response) -> Result<reqwest::Response, Error> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status().as_u16();
        let body = response.text().await?;
        Err(Error::Http { status, body })
    }
    async fn json<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T, Error> {
        let response = Self::checked(request.send().await?).await?;
        serde_json::from_slice(&response.bytes().await?).map_err(|source| Error::Decode { source })
    }
    pub async fn health(&self) -> Result<HealthStatus, Error> {
        Self::json(self.api.get(self.url(&["v1", "health"])?)).await
    }
    pub async fn presign(&self, payload: &PresignRequest) -> Result<PresignResponse, Error> {
        if payload.size_bytes > i64::MAX as u64
            || payload.filename.is_empty()
            || payload.filename.len() > 255
            || payload.mime_type.is_empty()
            || payload.mime_type.len() > 255
            || payload.mime_type.bytes().any(|b| b.is_ascii_control())
        {
            return Err(Error::Input("size exceeds supported byte range; filename and MIME type must be nonempty and at most 255 bytes; MIME type must not contain control characters".into()));
        }
        Self::json(self.api.post(self.url(&["v1", "uploads"])?).json(payload)).await
    }
    pub async fn list_uploads(
        &self,
        query: &ListUploadsRequest,
    ) -> Result<ListUploadsResponse, Error> {
        if query.limit.is_some_and(|n| !(1..=100).contains(&n)) {
            return Err(Error::Input("limit must be between 1 and 100".into()));
        }
        Self::json(self.api.get(self.url(&["v1", "uploads"])?).query(query)).await
    }
    pub async fn confirm(&self, id: &str) -> Result<ConfirmResponse, Error> {
        Self::json(self.api.post(self.url(&["v1", "uploads", id, "confirm"])?)).await
    }
    pub async fn download(&self, id: &str) -> Result<DownloadResponse, Error> {
        Self::json(self.api.post(self.url(&["v1", "uploads", id, "download"])?)).await
    }
    pub async fn update_retention(&self, id: &str, retention: &Retention) -> Result<Upload, Error> {
        Self::json(
            self.api
                .patch(self.url(&["v1", "uploads", id, "retention"])?)
                .json(retention),
        )
        .await
    }
    pub async fn create_share(
        &self,
        id: &str,
        request: &CreateShareRequest,
    ) -> Result<Share, Error> {
        Self::json(
            self.api
                .post(self.url(&["v1", "uploads", id, "shares"])?)
                .json(request),
        )
        .await
    }
    pub async fn list_shares(
        &self,
        id: &str,
        query: &ListUploadsRequest,
    ) -> Result<ListSharesResponse, Error> {
        if query.limit.is_some_and(|n| !(1..=100).contains(&n)) {
            return Err(Error::Input("limit must be between 1 and 100".into()));
        }
        Self::json(
            self.api
                .get(self.url(&["v1", "uploads", id, "shares"])?)
                .query(query),
        )
        .await
    }
    pub async fn revoke_share(&self, id: &str, share_id: &str) -> Result<(), Error> {
        Self::checked(
            self.api
                .delete(self.url(&["v1", "uploads", id, "shares", share_id])?)
                .send()
                .await?,
        )
        .await?;
        Ok(())
    }
    pub async fn delete(&self, id: &str) -> Result<(), Error> {
        Self::checked(
            self.api
                .delete(self.url(&["v1", "uploads", id])?)
                .send()
                .await?,
        )
        .await?;
        Ok(())
    }
    /// Upload to the signed URL using an isolated client with no API credentials.
    pub async fn put_upload(&self, signed: &PresignResponse, data: Vec<u8>) -> Result<(), Error> {
        let url = Url::parse(&signed.uri).map_err(|e| Error::Input(e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Input(
                "signed URL must be HTTP(S) without credentials or fragment".into(),
            ));
        }
        let mut request = self.storage.request(Method::PUT, url);
        for (name, value) in &signed.headers {
            // Fetch sets these itself; signed content-length must still match the bytes.
            if name.eq_ignore_ascii_case("content-length")
                && value.parse::<u64>().ok() != Some(data.len() as u64)
            {
                return Err(Error::Input(
                    "signed content-length does not match data".into(),
                ));
            }
            #[cfg(target_arch = "wasm32")]
            if name.eq_ignore_ascii_case("host") || name.eq_ignore_ascii_case("content-length") {
                continue;
            }
            let name = name
                .parse::<reqwest::header::HeaderName>()
                .map_err(|_| Error::Input("invalid signed header name".into()))?;
            let value = value
                .parse::<reqwest::header::HeaderValue>()
                .map_err(|_| Error::Input("invalid signed header value".into()))?;
            request = request.header(name, value);
        }
        Self::checked(request.body(data).send().await?).await?;
        Ok(())
    }
    /// Transform locally before presigning; failures never upload original bytes.
    pub async fn upload_sanitized(
        &self,
        payload: &PresignRequest,
        data: &[u8],
        options: &privacy::PrivacyOptions,
        config: &privacy::Config,
    ) -> Result<(UploadResult, privacy::Report), Error> {
        if payload.size_bytes != data.len() as u64 {
            return Err(Error::Input("size_bytes does not match data".into()));
        }
        let prepared =
            privacy::prepare(data, &payload.filename, &payload.mime_type, options, config)
                .map_err(|e| Error::Input(e.to_string()))?;
        let request = PresignRequest {
            filename: prepared.filename,
            mime_type: prepared.mime_type,
            size_bytes: prepared.data.len() as u64,
            retention: payload.retention.clone(),
        };
        let uploaded = self.upload(&request, prepared.data).await?;
        Ok((uploaded, prepared.report))
    }
    pub async fn upload(
        &self,
        payload: &PresignRequest,
        data: Vec<u8>,
    ) -> Result<UploadResult, Error> {
        if payload.size_bytes != data.len() as u64 {
            return Err(Error::Input("size_bytes does not match data".into()));
        }
        let signed = self.presign(payload).await?;
        self.put_upload(&signed, data)
            .await
            .map_err(|source| Error::Upload {
                request_id: signed.request_id.clone(),
                stage: "put",
                source: Box::new(source),
            })?;
        let confirmation =
            self.confirm(&signed.request_id)
                .await
                .map_err(|source| Error::Upload {
                    request_id: signed.request_id.clone(),
                    stage: "confirm",
                    source: Box::new(source),
                })?;
        Ok(UploadResult {
            request_id: signed.request_id,
            confirmation,
        })
    }
}
