//! Process-wide logging control, independent of request state and storage config.
use std::sync::OnceLock;

use anyhow::{Context, Result, ensure};
use tracing_subscriber::{
    EnvFilter, Registry, fmt, layer::SubscriberExt, reload, util::SubscriberInitExt,
};

type FilterHandle = reload::Handle<EnvFilter, Registry>;
static FILTER: OnceLock<FilterHandle> = OnceLock::new();

pub fn init() -> Result<()> {
    let filter = filter_from_env().unwrap_or_else(|_| EnvFilter::new("info"));
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

fn filter_from_env() -> Result<EnvFilter> {
    // Reload reads the process environment; a config.json source could be added here later.
    match std::env::var("RUST_LOG") {
        Ok(value) => parse_filter(&value),
        Err(std::env::VarError::NotPresent) => Ok(EnvFilter::new("info")),
        Err(error) => Err(error).context("RUST_LOG must be valid Unicode"),
    }
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

pub fn reload_from_env() -> Result<()> {
    apply(filter_from_env()?)
}
