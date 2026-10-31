#[derive(Debug, serde::Serialize, PartialEq, toasty::Embed)]
#[column(type = text)]
pub enum UploadStatus {
    Pending,
    Confirmed,
}
