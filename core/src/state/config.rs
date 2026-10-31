//! Runtime settings loaded before application connections are initialized.

use super::env::{EnvironmentVariable, env_var_required};

pub struct AppRuntimeConfig {
    pub sql_conn_url: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String
}

impl AppRuntimeConfig {
    /// Load required settings, panicking when a variable is missing or invalid Unicode.
    pub fn from_env() -> Self {
        Self {
            sql_conn_url: env_var_required(EnvironmentVariable::SqlConnUrl),
            s3_endpoint: env_var_required(EnvironmentVariable::S3Endpoint),
            s3_region: env_var_required(EnvironmentVariable::S3Region),
            s3_bucket: env_var_required(EnvironmentVariable::S3Bucket),
            s3_access_key: env_var_required(EnvironmentVariable::S3AccessKey),
            s3_secret_key: env_var_required(EnvironmentVariable::S3SecretKey)
        }
    }
}
