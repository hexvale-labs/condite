use jiff::Timestamp;

use crate::db::sql::UploadStatus;

#[derive(Debug, serde::Serialize, toasty::Model)]
#[table = "uploads"]
pub struct UploadRecord {
    #[key]
    #[auto(uuid(v7))]
    pub id: uuid::Uuid,

    #[index]
    pub owner_id: uuid::Uuid,

    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,

    #[default(UploadStatus::Pending)]
    pub status: UploadStatus,

    #[auto]
    pub created_at: Timestamp,
    pub expires_at: Option<Timestamp>,
    #[index]
    pub cleanup_at: Option<Timestamp>,
    #[index]
    pub pending_cleanup_at: Option<Timestamp>,
    #[default(false)]
    pub deletion_pending: bool,
    #[default(false)]
    pub shared: bool,
}

pub fn get_s3_key(id: &uuid::Uuid, mime_type: &str, status: &UploadStatus) -> String {
    let ext = mime2ext::mime2ext(mime_type).unwrap_or("bin");
    match status {
        UploadStatus::Pending => format!("pending/{}.{}", id, ext),
        UploadStatus::Confirmed => format!("uploads/{}.{}", id, ext),
    }
}

impl UploadRecord {
    pub fn s3_key(&self) -> String {
        get_s3_key(&self.id, &self.mime_type, &self.status)
    }
}

impl From<UploadRecord> for condite_shared::Upload {
    fn from(record: UploadRecord) -> Self {
        Self {
            id: record.id.to_string(),
            filename: record.filename,
            mime_type: record.mime_type,
            size_bytes: record.size_bytes,
            status: match record.status {
                UploadStatus::Pending => condite_shared::UploadStatus::Pending,
                UploadStatus::Confirmed => condite_shared::UploadStatus::Confirmed,
            },
            created_at: record.created_at.to_string(),
            expires_at: record.expires_at.map(|t| t.to_string()),
            visibility: if record.shared {
                condite_shared::Visibility::Shared
            } else {
                condite_shared::Visibility::Private
            },
            deletion_pending: record.deletion_pending,
        }
    }
}
