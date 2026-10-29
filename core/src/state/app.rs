use std::sync::Arc;

use anyhow::Result;

use crate::{db::{s3::connection::obtain_s3_conn, sql::connection::obtain_sql_conn}, state::env::{EnvironmentVariable, env_var_required}};

#[derive(Debug)]
pub struct AppRuntimeConfig {
    pub sql_conn_uri: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String
}

impl AppRuntimeConfig {
    pub fn load_sync() -> Self {
        if let Some(_) = dotenv::dotenv().ok() {
            tracing::info!("loaded .env file")
        }

        Self {
            sql_conn_uri: env_var_required(EnvironmentVariable::SqlConnUrl),
            s3_endpoint: env_var_required(EnvironmentVariable::S3Endpoint),
            s3_region: env_var_required(EnvironmentVariable::S3Region),
            s3_bucket: env_var_required(EnvironmentVariable::S3Bucket),
            s3_access_key: env_var_required(EnvironmentVariable::S3AccessKey),
            s3_secret_key: env_var_required(EnvironmentVariable::S3SecretKey)
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub sql: toasty::Db,
    pub config: Arc<AppRuntimeConfig>,
    pub s3: aws_sdk_s3::Client
}

impl AppState {
    pub async fn init() -> Result<Self> {
        let config = AppRuntimeConfig::load_sync();

        let (sql, s3) = tokio::try_join!(
            obtain_sql_conn(&config),
            obtain_s3_conn(&config)
        )?;

        Ok(Self {
            config: Arc::new(config),
            sql,
            s3
        })
    }
}