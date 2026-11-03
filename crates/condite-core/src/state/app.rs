use std::sync::Arc;

use anyhow::Result;
use jiff::Timestamp;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    control::CommandBus,
    db::{s3::connection::obtain_s3_conn, sql::connection::obtain_sql_conn},
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

pub use condite_config::CoreConfig as AppRuntimeConfig;

pub fn config_path() -> std::path::PathBuf {
    std::env::args_os()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--config")
        .map(|pair| pair[1].clone().into())
        .unwrap_or_else(|| "condite.toml".into())
}

#[derive(Clone)]
pub struct AppState {
    pub root_keys: Arc<std::sync::RwLock<Vec<condite_config::RootKey>>>,
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
        let _ = dotenv::dotenv();
        let config = AppRuntimeConfig::load(&config_path())?;
        let details = AppRuntimeDetails::new();

        let (sql, s3) = tokio::try_join!(obtain_sql_conn(&config), obtain_s3_conn(&config))?;

        let (bus, rx) = CommandBus::new();
        let ct = CancellationToken::new();
        let tasks = TaskTracker::new();

        let state = Self {
            root_keys: Arc::new(std::sync::RwLock::new(config.root_keys.clone())),
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
