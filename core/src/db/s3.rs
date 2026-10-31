use anyhow::{Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{Client, config::Credentials};

use crate::state::app::AppRuntimeConfig;

pub fn load_credentials(config: &AppRuntimeConfig) -> Result<Credentials> {
    Ok(Credentials::new(
        &config.s3_access_key,
        &config.s3_secret_key,
        None,
        None,
        "custom-s3-provider",
    ))
}

pub async fn obtain_s3_conn(config: &AppRuntimeConfig) -> Result<Client> {
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(config.s3_region.to_owned()))
        .endpoint_url(&config.s3_endpoint)
        .credentials_provider(
            load_credentials(config).with_context(|| "Failed loading all credentials")?,
        )
        .load()
        .await;

    Ok(Client::new(&aws_config))
}
