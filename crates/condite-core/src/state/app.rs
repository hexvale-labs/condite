use std::sync::Arc;

use anyhow::Result;
use jiff::{Span, Timestamp};
use serde::{Deserialize, Serialize};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    control::CommandBus,
    db::{s3::connection::obtain_s3_conn, sql::connection::obtain_sql_conn},
    state::env::{EnvironmentVariable, env_var_required},
};

#[derive(Debug)]
pub struct AppRuntimeDetails {
    pub started_at: jiff::Timestamp,
}

impl AppRuntimeDetails {
    pub fn new() -> Self {
        AppRuntimeDetails {
            started_at: jiff::Timestamp::now(),
        }
    }
}

impl Default for AppRuntimeDetails {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct AppRuntimeConfig {
    pub sql_conn_uri: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
}

impl AppRuntimeConfig {
    pub fn load_sync() -> Self {
        if dotenv::dotenv().is_ok() {
            tracing::info!("loaded .env file")
        }

        Self {
            sql_conn_uri: env_var_required(EnvironmentVariable::SqlConnUrl),
            s3_endpoint: env_var_required(EnvironmentVariable::S3Endpoint),
            s3_region: env_var_required(EnvironmentVariable::S3Region),
            s3_bucket: env_var_required(EnvironmentVariable::S3Bucket),
            s3_access_key: env_var_required(EnvironmentVariable::S3AccessKey),
            s3_secret_key: env_var_required(EnvironmentVariable::S3SecretKey),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppRuntimeConfig>,
    pub details: Arc<AppRuntimeDetails>,

    pub bus: CommandBus,
    /// Shared shutdown signal; background loops must observe cancellation.
    pub ct: CancellationToken,
    /// Tracks application tasks; close and wait after signalling shutdown.
    pub tasks: TaskTracker,

    pub sql: toasty::Db,
    pub s3: aws_sdk_s3::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    healthy: bool,
    started_at: Timestamp,
    runtime: Span,
}

impl AppState {
    pub async fn init() -> Result<Self> {
        let config = AppRuntimeConfig::load_sync();
        let details = AppRuntimeDetails::new();

        let (sql, s3) = tokio::try_join!(obtain_sql_conn(&config), obtain_s3_conn(&config))?;

        let (bus, rx) = CommandBus::new();
        let ct = CancellationToken::new();
        let tasks = TaskTracker::new();

        let state = Self {
            config: Arc::new(config),
            details: Arc::new(details),
            sql,
            tasks,
            s3,
            bus,
            ct,
        };

        CommandBus::start(rx, state.clone());

        Ok(state)
    }

    pub fn health(&self) -> Result<HealthStatus> {
        let started_at = self.details.started_at;

        Ok(HealthStatus {
            healthy: true,
            started_at,
            runtime: Timestamp::now().since(started_at)?,
        })
    }
}
