use crate::{
    auth::{authenticate_direct, digest},
    db::sql::SessionRecord,
    state::app::AppState,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

// Bound password guessing and database growth. Deployment proxies should also
// rate-limit login per source; this process-wide cap cannot be bypassed by IP spoofing.
static LOGIN_WINDOW: std::sync::Mutex<(u64, u32)> = std::sync::Mutex::new((0, 0));
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<condite_shared::LoginRequest>,
) -> Response {
    let now = jiff::Timestamp::now();
    {
        let Ok(mut window) = LOGIN_WINDOW.lock() else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        let minute = (now.as_second() / 60) as u64;
        if window.0 != minute {
            *window = (minute, 0);
        }
        if window.1 >= 120 {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        window.1 += 1;
    }
    if payload.api_key.len() > 512 {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let auth = match authenticate_direct(&state, &payload.api_key).await {
        Ok(auth) => auth,
        Err(status) => return status.into_response(),
    };
    let mut random = [0u8; 32];
    if getrandom::fill(&mut random).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let token = format!(
        "condite_session_{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let expires_at = match now.checked_add(jiff::Span::new().hours(12)) {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let mut db = state.sql.clone();
    if toasty::create!(SessionRecord {
        digest: digest(&token),
        owner_id: auth.0,
        credential_digest: digest(&payload.api_key),
        root: auth.1,
        upload_id: None,
        expires_at: expires_at
    })
    .exec(&mut db)
    .await
    .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    (
        [("cache-control", "no-store")],
        Json(condite_shared::SessionResponse {
            token,
            expires_at: expires_at.to_string(),
        }),
    )
        .into_response()
}
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, token)| {
            scheme.eq_ignore_ascii_case("bearer") && token.starts_with("condite_session_")
        })
        .map(|(_, token)| token);
    let Some(token) = token else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let mut db = state.sql.clone();
    match SessionRecord::filter_by_digest(digest(token))
        .delete()
        .exec(&mut db)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
