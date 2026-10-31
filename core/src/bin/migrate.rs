use condite_core::{db::sql::connection::obtain_sql_conn, state::app::AppRuntimeConfig};
use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runtime_config = AppRuntimeConfig::load_sync();
    let config = Config::load()?;
    let db = obtain_sql_conn(&runtime_config).await?;

    ToastyCli::with_config(db, config).parse_and_run().await?;

    Ok(())
}
