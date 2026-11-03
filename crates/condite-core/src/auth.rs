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

/// Server-authenticated identity and root flag; limits/revocation are rechecked under the write lock.
#[derive(Clone, Copy)]
pub struct AuthenticatedKey(pub Uuid, pub bool);
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
        if !scheme.eq_ignore_ascii_case("bearer") || secret.len() > 512 {
            return Err(unauthorized());
        }
        authenticate(state, secret).await.map_err(|status| {
            if status == StatusCode::UNAUTHORIZED {
                unauthorized()
            } else {
                status.into_response()
            }
        })
    }
}

pub async fn authenticate(state: &AppState, secret: &str) -> Result<AuthenticatedKey, StatusCode> {
    if secret.starts_with("condite_session_") {
        let mut db = state.sql.clone();
        let session =
            match crate::db::sql::SessionRecord::get_by_digest(&mut db, &digest(secret)).await {
                Ok(session) => session,
                Err(error) if error.is_record_not_found() => {
                    return authenticate_direct(state, secret).await;
                }
                Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            };
        if session.upload_id.is_some() || session.expires_at <= jiff::Timestamp::now() {
            return Err(StatusCode::UNAUTHORIZED);
        }
        if session.root {
            let root = state
                .root_keys
                .read()
                .map_err(|_| StatusCode::UNAUTHORIZED)?
                .iter()
                .find(|r| digest(&r.key) == session.credential_digest)
                .cloned()
                .ok_or(StatusCode::UNAUTHORIZED)?;
            let auth = authenticate_direct(state, &root.key).await?;
            return if auth.0 == session.owner_id {
                Ok(auth)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            };
        }
        let key = ApiKeyRecord::get_by_id(&mut db, &session.owner_id)
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        return if !key.revoked && key.digest == session.credential_digest {
            Ok(AuthenticatedKey(key.id, false))
        } else {
            Err(StatusCode::UNAUTHORIZED)
        };
    }
    authenticate_direct(state, secret).await
}

pub async fn authenticate_direct(
    state: &AppState,
    secret: &str,
) -> Result<AuthenticatedKey, StatusCode> {
    let fingerprint = digest(secret);
    let root = state
        .root_keys
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .iter()
        .find(|r| digest(&r.key) == fingerprint)
        .cloned();
    let mut db = state.sql.clone();
    if let Some(root) = root {
        let id = root_id(&root.name);
        let exists = ApiKeyRecord::get_by_id(&mut db, &id).await;
        match exists {
            Ok(_) => {}
            Err(e) if e.is_record_not_found() => {
                let created = toasty::create!(ApiKeyRecord {
                    id: id,
                    digest: format!("root:{}", root.name),
                    per_upload_quota: i64::MAX as u64,
                    storage_quota: i64::MAX as u64,
                    lock_nonce: Uuid::now_v7().to_string()
                })
                .exec(&mut db)
                .await;
                if created.is_err() && ApiKeyRecord::get_by_id(&mut db, &id).await.is_err() {
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
        return Ok(AuthenticatedKey(id, true));
    }
    match ApiKeyRecord::filter(ApiKeyRecord::fields().digest().eq(fingerprint))
        .get(&mut db)
        .await
    {
        Ok(key) if !key.revoked => Ok(AuthenticatedKey(key.id, false)),
        Ok(_) => Err(StatusCode::UNAUTHORIZED),
        Err(e) if e.is_record_not_found() => Err(StatusCode::UNAUTHORIZED),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub fn quota_error() -> axum_anyhow::ApiError {
    axum_anyhow::ApiError::builder()
        .status(StatusCode::PAYLOAD_TOO_LARGE)
        .title("API key upload or storage quota exceeded")
        .build()
}

pub fn root_id(name: &str) -> Uuid {
    let hash = Sha256::digest(format!("condite:root:{name}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    Uuid::from_bytes(bytes)
}
