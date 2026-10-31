use std::sync::Arc;

use anyhow::Result;

use crate::db::{s3::obtain_s3_conn, sql::obtain_sql_conn};

pub use super::config::AppRuntimeConfig;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppRuntimeConfig>,
    pub sql: sea_orm::DatabaseConnection,
    pub s3: aws_sdk_s3::Client
}

impl AppState {
    pub async fn init() -> Result<Self> {
        let config = AppRuntimeConfig::from_env();

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