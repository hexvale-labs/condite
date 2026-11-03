//! Process-wide logging control, independent of request state and storage config.
use std::sync::OnceLock;

use anyhow::{Context, Result, ensure};
use tracing_subscriber::{
    EnvFilter, Registry, fmt, layer::SubscriberExt, reload, util::SubscriberInitExt,
};

type FilterHandle = reload::Handle<EnvFilter, Registry>;
static FILTER: OnceLock<FilterHandle> = OnceLock::new();

pub fn init() -> Result<()> {
    let _ = dotenv::dotenv();
    let config = crate::state::app::AppRuntimeConfig::load(&crate::state::app::config_path())?;
    init_with_filter(&config.log_filter)
}

pub fn init_with_filter(value: &str) -> Result<()> {
    let filter = parse_filter(value)?;
    let (layer, handle) = reload::Layer::new(filter);
    tracing_subscriber::registry()
        .with(layer)
        .with(fmt::layer())
        .try_init()
        .map_err(|error| anyhow::anyhow!("could not initialize logging: {error}"))?;
    FILTER
        .set(handle)
        .map_err(|_| anyhow::anyhow!("logging already initialized"))?;
    Ok(())
}

fn parse_filter(value: &str) -> Result<EnvFilter> {
    ensure!(!value.trim().is_empty(), "log filter must not be empty");
    EnvFilter::try_new(value).context("invalid log filter")
}

fn apply(filter: EnvFilter) -> Result<()> {
    FILTER
        .get()
        .context("reloadable logging is not initialized")?
        .reload(filter)
        .context("could not change log filter")
}

pub fn set_filter(value: &str) -> Result<()> {
    apply(parse_filter(value)?)
}
