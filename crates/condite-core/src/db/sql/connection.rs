use anyhow::Result;
use toasty::Db;

use crate::state::app::AppRuntimeConfig;

static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!();

#[tracing::instrument(skip(config), err)]
pub async fn obtain_sql_conn(config: &AppRuntimeConfig) -> Result<Db> {
    tracing::info!("Connecting to SQL database");
    let db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect(&config.sql_conn_uri)
        .await?;

    tracing::debug!("SQL connection established; applying migrations");
    let report = MIGRATIONS.apply(&db).await?;
    tracing::info!(
        applied = report.applied(),
        skipped = report.skipped(),
        "SQL migrations completed"
    );

    Ok(db)
}
