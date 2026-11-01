use std::{future::Future, time::Duration};

async fn backoff(attempt: u32) {
    tokio::time::sleep(Duration::from_millis((10u64 << attempt.min(5)).min(250))).await;
}

/// Only retry a database-declared serialization failure. Callers must keep their
/// external writes idempotent and commit before removing recovery metadata.
pub async fn api<T, F, Fut>(mut operation: F) -> axum_anyhow::ApiResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = axum_anyhow::ApiResult<T>>,
{
    for attempt in 0..10 {
        match operation().await {
            Err(e)
                if attempt < 9
                    && e.error()
                        .and_then(|e| e.downcast_ref::<toasty::Error>())
                        .is_some_and(|e| e.is_serialization_failure()) =>
            {
                backoff(attempt).await
            }
            result => return result,
        }
    }
    unreachable!()
}

pub async fn control<T, F, Fut>(mut operation: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    for attempt in 0..10 {
        match operation().await {
            Err(e)
                if attempt < 9
                    && e.downcast_ref::<toasty::Error>()
                        .is_some_and(|e| e.is_serialization_failure()) =>
            {
                backoff(attempt).await
            }
            result => return result,
        }
    }
    unreachable!()
}
