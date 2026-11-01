use jiff::Timestamp;

/// Opaque keys survive logical deletion so late storage writes can be removed.
#[derive(Debug, toasty::Model)]
#[table = "object_cleanup"]
pub struct ObjectCleanupRecord {
    #[key]
    pub id: uuid::Uuid,
    pub bucket: String,
    pub pending_key: String,
    pub confirmed_key: String,
    #[index]
    pub cleanup_at: Timestamp,
}
