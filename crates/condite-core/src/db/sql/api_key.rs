use jiff::Timestamp;
use uuid::Uuid;

#[derive(toasty::Model)]
#[table = "api_keys"]
pub struct ApiKeyRecord {
    #[key]
    #[auto(uuid(v7))]
    pub id: Uuid,
    #[unique]
    pub digest: String,
    pub per_upload_quota: u64,
    pub storage_quota: u64,
    #[default(false)]
    pub revoked: bool,
    pub lock_nonce: String,
    #[auto]
    pub created_at: Timestamp,
}

pub fn validate_limits(per_upload: u64, storage: u64) -> anyhow::Result<()> {
    anyhow::ensure!(
        per_upload <= i64::MAX as u64 && storage <= i64::MAX as u64,
        "limits exceed the supported byte range"
    );
    Ok(())
}

/// The first statement must be a write: it serializes this key across connections
/// before any transaction snapshot is used for quota or ownership decisions.
pub async fn lock_key(tx: &mut toasty::Transaction<'_>, id: Uuid) -> toasty::Result<ApiKeyRecord> {
    ApiKeyRecord::filter_by_id(id)
        .update()
        .lock_nonce(Uuid::now_v7().to_string())
        .exec(tx)
        .await?;
    ApiKeyRecord::get_by_id(tx, &id).await
}

pub async fn storage_size(
    executor: &mut dyn toasty::Executor,
    id: Uuid,
    confirmed_only: bool,
) -> anyhow::Result<u64> {
    use super::{UploadRecord, UploadStatus};
    let mut after = Uuid::nil();
    let mut total = 0u64;
    loop {
        let mut query = UploadRecord::filter(UploadRecord::fields().owner_id().eq(id))
            .filter(UploadRecord::fields().id().gt(after));
        if confirmed_only {
            query = query.filter(UploadRecord::fields().status().eq(UploadStatus::Confirmed));
        }
        let batch = query
            .order_by(UploadRecord::fields().id().asc())
            .limit(500)
            .select((
                UploadRecord::fields().id(),
                UploadRecord::fields().size_bytes(),
            ))
            .exec(executor)
            .await?;
        for (id, size) in &batch {
            after = *id;
            total = total
                .checked_add(*size)
                .ok_or_else(|| anyhow::anyhow!("storage size overflow"))?;
        }
        if batch.len() < 500 {
            return Ok(total);
        }
    }
}
