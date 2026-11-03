//! Dependency-light HTTP and local control wire contracts for Condite.

pub mod control;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct HealthStatus {
    pub healthy: bool,
    pub started_at: String,
    pub runtime: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct PresignRequest {
    pub size_bytes: u64,
    pub mime_type: String,
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub retention: Option<Retention>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct PresignResponse {
    pub request_id: String,
    pub uri: String,
    pub headers: std::collections::HashMap<String, String>,
    pub expires_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Upload {
    pub id: String,
    #[serde(default)]
    pub owner_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub status: UploadStatus,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub visibility: Visibility,
    pub deletion_pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ListUploadsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub owner_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ListUploadsResponse {
    pub uploads: Vec<Upload>,
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ConfirmResponse {
    pub upload: Upload,
    pub new_storage_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct UploadResult {
    pub request_id: String,
    pub confirmation: ConfirmResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub enum UploadStatus {
    Pending,
    Confirmed,
}

/// Omit retention on creation to use the default; a null deadline means Never.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Retention {
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub enum Visibility {
    Private,
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct DownloadResponse {
    pub uri: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct CreateShareRequest {
    pub expires_at: Option<String>,
    pub follows_upload: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub enum ShareStatus {
    Creating,
    Active,
    Revoking,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Share {
    pub id: String,
    pub upload_id: String,
    pub url: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub follows_upload: bool,
    pub status: ShareStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ListSharesResponse {
    pub shares: Vec<Share>,
    pub next_after: Option<String>,
}

/// Current key identity. Only core decides roles and ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum AccountRole {
    Key,
    Root,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Usage {
    pub confirmed_bytes: u64,
    pub reserved_bytes: u64,
    pub charged_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct AccountLimits {
    pub per_file_bytes: Option<u64>,
    pub storage_bytes: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Account {
    pub id: String,
    pub role: AccountRole,
    pub usage: Usage,
    pub limits: AccountLimits,
    pub global_usage: Option<Usage>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct KeySummary {
    pub id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub revoked: bool,
    pub limits: AccountLimits,
    pub usage: Usage,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ListKeysResponse {
    pub keys: Vec<KeySummary>,
    pub next_after: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub api_key: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct SessionResponse {
    pub token: String,
    pub expires_at: String,
}
