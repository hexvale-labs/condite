use anyhow::{Result, anyhow};
use sea_orm::{Database, DatabaseConnection};

use crate::state::app::AppRuntimeConfig;

pub async fn obtain_sql_conn(config: &AppRuntimeConfig) -> Result<DatabaseConnection> {
    let db = Database::connect(&config.sql_conn_url).await.map_err(|e| anyhow!(e))?;
    
    Ok(db)
}