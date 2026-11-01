use std::sync::Arc;

use anyhow::Result;
use jiff::Timestamp;
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
    pub cors_origins: Option<String>,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_public_bucket: Option<String>,
    pub public_object_base_url: Option<String>,
}

impl AppRuntimeConfig {
    /// Compare restart-only settings without exposing their values.
    pub fn restart_required(&self) -> Vec<&'static str> {
        [
            ("SQL_CONN_URL", &self.sql_conn_uri),
            ("S3_ENDPOINT", &self.s3_endpoint),
            ("S3_REGION", &self.s3_region),
            ("S3_BUCKET", &self.s3_bucket),
            ("S3_ACCESS_KEY", &self.s3_access_key),
            ("S3_SECRET_KEY", &self.s3_secret_key),
        ]
        .into_iter()
        .filter_map(|(name, current)| (std::env::var(name).as_ref() != Ok(current)).then_some(name))
        .chain(
            [
                ("S3_PUBLIC_BUCKET", &self.s3_public_bucket),
                ("CORS_ALLOWED_ORIGINS", &self.cors_origins),
                ("PUBLIC_OBJECT_BASE_URL", &self.public_object_base_url),
            ]
            .into_iter()
            .filter_map(|(name, value)| {
                (std::env::var(name).ok().as_ref() != value.as_ref()).then_some(name)
            }),
        )
        .collect()
    }

    pub fn load_sync() -> Self {
        if dotenv::dotenv().is_ok() {
            tracing::info!("loaded .env file")
        }

        Self {
            cors_origins: std::env::var("CORS_ALLOWED_ORIGINS").ok(),
            sql_conn_uri: env_var_required(EnvironmentVariable::SqlConnUrl),
            s3_endpoint: env_var_required(EnvironmentVariable::S3Endpoint),
            s3_region: env_var_required(EnvironmentVariable::S3Region),
            s3_bucket: env_var_required(EnvironmentVariable::S3Bucket),
            s3_access_key: env_var_required(EnvironmentVariable::S3AccessKey),
            s3_secret_key: env_var_required(EnvironmentVariable::S3SecretKey),
            s3_public_bucket: std::env::var("S3_PUBLIC_BUCKET").ok(),
            public_object_base_url: std::env::var("PUBLIC_OBJECT_BASE_URL").ok(),
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

pub use condite_shared::HealthStatus;

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
            started_at: started_at.to_string(),
            runtime: Timestamp::now().since(started_at)?.to_string(),
        })
    }
}
