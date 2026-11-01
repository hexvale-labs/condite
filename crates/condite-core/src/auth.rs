use crate::{
    db::sql::{ApiKeyRecord, api_key::lock_key},
    state::app::AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn digest(secret: &str) -> String {
    Sha256::digest(secret.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub async fn generate(
    state: &AppState,
    per_upload: u64,
    storage: u64,
) -> anyhow::Result<(Uuid, String)> {
    crate::db::sql::api_key::validate_limits(per_upload, storage)?;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|e| anyhow::anyhow!("could not generate random API key: {e}"))?;
    let secret = format!(
        "condite_{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let mut db = state.sql.clone();
    let key = toasty::create!(ApiKeyRecord {
        digest: digest(&secret),
        per_upload_quota: per_upload,
        storage_quota: storage,
        lock_nonce: Uuid::now_v7().to_string()
    })
    .exec(&mut db)
    .await?;
    Ok((key.id, secret))
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        "valid Bearer API key required",
    )
        .into_response()
}

/// Holds only a key ID; limits and revocation are checked again under the write lock.
#[derive(Clone, Copy)]
pub struct AuthenticatedKey(pub Uuid);
impl AuthenticatedKey {
    pub async fn lock(
        &self,
        tx: &mut toasty::Transaction<'_>,
    ) -> axum_anyhow::ApiResult<ApiKeyRecord> {
        match lock_key(tx, self.0).await {
            Ok(key) if !key.revoked => Ok(key),
            Ok(_) => Err(axum_anyhow::unauthorized(
                "invalid API key",
                "key has been revoked",
            )),
            Err(e) if e.is_record_not_found() => Err(axum_anyhow::unauthorized(
                "invalid API key",
                "key does not exist",
            )),
            Err(e) => Err(e.into()),
        }
    }
}
impl FromRequestParts<AppState> for AuthenticatedKey {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        let mut values = parts.headers.get_all(header::AUTHORIZATION).iter();
        let value = values
            .next()
            .and_then(|v| v.to_str().ok())
            .ok_or_else(unauthorized)?;
        if values.next().is_some() {
            return Err(unauthorized());
        }
        let (scheme, secret) = value.split_once(' ').ok_or_else(unauthorized)?;
        if !scheme.eq_ignore_ascii_case("bearer")
            || secret.len() != 72
            || !secret.starts_with("condite_")
            || !secret[8..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(unauthorized());
        }
        let mut db = state.sql.clone();
        match ApiKeyRecord::filter(ApiKeyRecord::fields().digest().eq(digest(secret)))
            .get(&mut db)
            .await
        {
            Ok(key) if !key.revoked => Ok(Self(key.id)),
            Ok(_) => Err(unauthorized()),
            Err(e) if e.is_record_not_found() => Err(unauthorized()),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        }
    }
}

pub fn quota_error() -> axum_anyhow::ApiError {
    axum_anyhow::ApiError::builder()
        .status(StatusCode::PAYLOAD_TOO_LARGE)
        .title("API key upload or storage quota exceeded")
        .build()
}
