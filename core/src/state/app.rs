use std::sync::Arc;

use anyhow::Result;

use crate::{db::{s3::obtain_s3_conn, sql::obtain_sql_conn}, state::env::{EnvironmentVariable, env_var_required}};

pub struct AppRuntimeConfig {
    pub sql_conn_url: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppRuntimeConfig>,
    pub sql: sea_orm::DatabaseConnection,
    pub s3: aws_sdk_s3::Client
}

impl AppState {
    pub async fn init() -> Result<Self> {
        let config = AppRuntimeConfig {
            sql_conn_url: env_var_required(EnvironmentVariable::SqlConnUrl),
            s3_endpoint: env_var_required(EnvironmentVariable::S3Endpoint),
            s3_region: env_var_required(EnvironmentVariable::S3Region),
            s3_bucket: env_var_required(EnvironmentVariable::S3Bucket),
            s3_access_key: env_var_required(EnvironmentVariable::S3AccessKey),
            s3_secret_key: env_var_required(EnvironmentVariable::S3SecretKey)
        };

        let (sql, s3) = tokio::join!(
            obtain_sql_conn(&config),
            obtain_s3_conn(&config)
        );

        Ok(Self {
            config: Arc::new(config),
            sql: sql.unwrap(),
            s3: s3.unwrap()
        })
    }
}