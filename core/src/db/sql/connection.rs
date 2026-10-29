use anyhow::{Result};
use toasty::Db;

use crate::state::app::AppRuntimeConfig;

static MIGRATIONS: toasty::migration::MigrationSet = 
    toasty::embed_migrations!();

#[tracing::instrument(skip(config))]
pub async fn obtain_sql_conn(config: &AppRuntimeConfig) -> Result<Db> {
    tracing::info!("connecting to database...");
    let db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect(&config.sql_conn_uri)
        .await?;

    let report = MIGRATIONS.apply(&db).await?;
    tracing::info!("applied {} migrations | skipped {}", report.applied(), report.skipped());

    Ok(db)
}