use jiff::Timestamp;

use crate::db::sql::UploadStatus;

#[derive(Debug, serde::Serialize, toasty::Model)]
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
