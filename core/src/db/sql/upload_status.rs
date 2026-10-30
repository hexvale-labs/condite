#[derive(Debug, PartialEq, toasty::Embed)]
#[column(type = text)]
pub enum UploadStatus {
    Pending,
    Confirmed
}