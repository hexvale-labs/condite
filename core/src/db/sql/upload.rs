use jiff::Timestamp;

use crate::db::sql::UploadStatus;

#[derive(Debug, toasty::Model)]
#[table = "uploads"]
pub struct UploadRecord {
    #[key]
    #[auto(uuid(v7))]
    pub id: uuid::Uuid,

    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    
    #[default(UploadStatus::Pending)]
    pub status: UploadStatus,

    #[auto]
    pub created_at: Timestamp,
}