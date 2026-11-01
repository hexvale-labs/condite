use jiff::Timestamp;

#[derive(Debug, toasty::Model)]
#[table = "shares"]
pub struct ShareRecord {
    #[key]
    #[auto(uuid(v7))]
    pub id: uuid::Uuid,
    #[index]
    pub owner_id: uuid::Uuid,
    #[index]
    pub upload_id: uuid::Uuid,
    pub object_key: String,
    pub public_url: String,
    pub public_bucket: String,
    pub requested_expires_at: Option<Timestamp>,
    pub follows_upload: bool,
    pub status: String,
    #[index]
    pub cleanup_at: Option<Timestamp>,
    #[auto]
    pub created_at: Timestamp,
}

impl ShareRecord {
    pub fn is_live(&self) -> bool {
        matches!(
            self.status.as_str(),
            "Creating" | "Active" | "Revoking" | "Reconciling"
        )
    }
    pub fn expires_at(&self, upload: &super::UploadRecord) -> Option<Timestamp> {
        match (self.requested_expires_at, upload.expires_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    pub fn wire(&self, upload: &super::UploadRecord) -> condite_shared::Share {
        use condite_shared::ShareStatus;
        condite_shared::Share {
            id: self.id.to_string(),
            upload_id: self.upload_id.to_string(),
            // An ambiguous publication never exposes its bearer key.
            url: if matches!(self.status.as_str(), "Creating" | "Reconciling") {
                String::new()
            } else {
                self.public_url.clone()
            },
            created_at: self.created_at.to_string(),
            expires_at: self.expires_at(upload).map(|t| t.to_string()),
            follows_upload: self.follows_upload,
            status: match self.status.as_str() {
                "Creating" => ShareStatus::Creating,
                "Active" => ShareStatus::Active,
                "Revoking" | "Reconciling" => ShareStatus::Revoking,
                "Expired" => ShareStatus::Expired,
                _ => ShareStatus::Revoked,
            },
        }
    }
}
