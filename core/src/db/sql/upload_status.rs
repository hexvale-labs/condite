#[derive(Debug, PartialEq, toasty::Embed)]
pub enum UploadStatus {
    Pending,
    Uploaded
}