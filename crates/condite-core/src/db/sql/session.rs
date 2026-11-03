use jiff::Timestamp;

/// Only token and credential digests are persisted; raw keys are never stored.
#[derive(toasty::Model)]
#[table = "sessions"]
pub struct SessionRecord {
    #[key]
    pub digest: String,
    pub owner_id: uuid::Uuid,
    pub credential_digest: String,
    pub root: bool,
    pub upload_id: Option<uuid::Uuid>,
    #[index]
    pub expires_at: Timestamp,
}
