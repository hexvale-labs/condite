//! Typed TOML loading. Only explicit `${NAME}` references read the environment.
use serde::de::DeserializeOwned;
use std::{fmt, path::Path};

/// A configuration diagnostic which never includes source or resolved values.
#[derive(Debug)]
pub struct Error(String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// Parse before substituting, preventing environment values from injecting TOML.
pub fn parse_with<T: DeserializeOwned>(
    source: &str,
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<T, Error> {
    let mut value: toml::Value =
        toml::from_str(source).map_err(|_| Error("invalid TOML syntax".into()))?;
    expand_value(&mut value, &lookup, "config")?;
    value
        .try_into()
        .map_err(|_| Error("configuration fields or types are invalid".into()))
}

/// Load a TOML file without implicit environment overrides.
pub fn load<T: DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let source = std::fs::read_to_string(path)
        .map_err(|_| Error(format!("cannot read config file {}", path.display())))?;
    parse_with(&source, |name| std::env::var(name).ok())
}

fn expand_value(
    value: &mut toml::Value,
    lookup: &impl Fn(&str) -> Option<String>,
    path: &str,
) -> Result<(), Error> {
    match value {
        toml::Value::String(text) => {
            *text = expand(text, lookup).map_err(|e| Error(format!("{path}: {e}")))?
        }
        toml::Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                expand_value(item, lookup, &format!("{path}[{index}]"))?;
            }
        }
        toml::Value::Table(table) => {
            for (key, item) in table {
                expand_value(item, lookup, &format!("{path}.{key}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn expand(text: &str, lookup: &impl Fn(&str) -> Option<String>) -> Result<String, Error> {
    let mut output = String::new();
    let mut rest = text;
    while let Some(index) = rest.find('$') {
        output.push_str(&rest[..index]);
        rest = &rest[index..];
        if rest.starts_with("$${") {
            output.push_str("${");
            rest = &rest[3..];
        } else if rest.starts_with("${") {
            let end = rest
                .find('}')
                .ok_or_else(|| Error("unterminated environment reference".into()))?;
            let name = &rest[2..end];
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(Error("invalid environment reference".into()));
            }
            output.push_str(
                &lookup(name)
                    .ok_or_else(|| Error(format!("missing environment variable {name}")))?,
            );
            rest = &rest[end + 1..];
        } else {
            output.push('$');
            rest = &rest[1..];
        }
    }
    output.push_str(rest);
    Ok(output)
}

/// A named root credential. Debug intentionally omits the key.
#[derive(Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RootKey {
    pub name: String,
    pub key: String,
}
impl fmt::Debug for RootKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootKey")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Core deployment settings. Legacy public storage is cleanup-only.
#[derive(Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoreConfig {
    pub sql_conn_uri: String,
    pub cors_origins: Option<String>,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_public_bucket: Option<String>,
    pub public_object_base_url: Option<String>,
    pub public_api_url: String,
    #[serde(default = "default_bind")]
    pub bind: std::net::SocketAddr,
    #[serde(default = "default_log")]
    pub log_filter: String,
    #[serde(default)]
    pub root_keys: Vec<RootKey>,
    #[serde(skip)]
    pub config_path: std::path::PathBuf,
}
fn default_bind() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([0, 0, 0, 0], 8080))
}
fn default_log() -> String {
    "info".into()
}
impl fmt::Debug for CoreConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CoreConfig { secrets: [REDACTED] }")
    }
}
impl CoreConfig {
    pub fn parse(source: &str) -> Result<Self, Error> {
        let config: Self = parse_with(source, |name| std::env::var(name).ok())?;
        config.validate()?;
        Ok(config)
    }
    pub fn load(path: &Path) -> Result<Self, Error> {
        let mut config: Self = load(path)?;
        config.validate()?;
        config.config_path = path.to_path_buf();
        if let Some(relative) = config.sql_conn_uri.strip_prefix("turso:")
            && !relative.starts_with(':')
            && Path::new(relative).is_relative()
        {
            config.sql_conn_uri = format!(
                "turso:{}",
                path.parent()
                    .unwrap_or(Path::new("."))
                    .join(relative)
                    .display()
            );
        }
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), Error> {
        validate_http_url(&self.s3_endpoint).map_err(|_| {
            Error(
                "s3_endpoint must be an HTTP(S) URL without credentials, query or fragment".into(),
            )
        })?;
        validate_http_url(&self.public_api_url).map_err(|_| {
            Error(
                "public_api_url must be an HTTP(S) URL without credentials, query or fragment"
                    .into(),
            )
        })?;
        if [
            self.sql_conn_uri.as_str(),
            &self.s3_region,
            &self.s3_bucket,
            &self.s3_access_key,
            &self.s3_secret_key,
            &self.log_filter,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        {
            return Err(Error(
                "required configuration values must not be empty".into(),
            ));
        }
        if let Some(origins) = &self.cors_origins {
            for origin in origins.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                validate_http_url(origin)?;
                let parsed =
                    url::Url::parse(origin).map_err(|_| Error("invalid CORS origin".into()))?;
                if parsed.path() != "/" {
                    return Err(Error("CORS origins cannot have a path".into()));
                }
            }
        }
        let mut names = std::collections::HashSet::new();
        let mut keys = std::collections::HashSet::new();
        for root in &self.root_keys {
            if root.name.is_empty()
                || root.name.len() > 64
                || !root
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || root.key.len() < 32
                || root.key.len() > 512
                || root.key.bytes().any(|b| !b.is_ascii_graphic())
            {
                return Err(Error("root names must be identifiers and root keys must contain 32..512 printable ASCII bytes".into()));
            }
            if !names.insert(&root.name) || !keys.insert(&root.key) {
                return Err(Error("duplicate root name or credential".into()));
            }
        }
        Ok(())
    }
    /// Names only, never values: callers may report restart requirements safely.
    pub fn differences(&self, candidate: &Self) -> Vec<&'static str> {
        let mut changed = Vec::new();
        macro_rules! compare { ($($name:ident),*) => { $(if self.$name != candidate.$name { changed.push(stringify!($name)); })* }; }
        compare!(
            sql_conn_uri,
            cors_origins,
            s3_endpoint,
            s3_region,
            s3_bucket,
            s3_access_key,
            s3_secret_key,
            s3_public_bucket,
            public_object_base_url,
            public_api_url,
            bind
        );
        changed
    }
}
/// Validate deployment URLs without disclosing their values.
pub fn validate_http_url(value: &str) -> Result<(), Error> {
    if value.trim() != value || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(Error("invalid HTTP(S) URL".into()));
    }
    let url = url::Url::parse(value).map_err(|_| Error("invalid HTTP(S) URL".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error("invalid HTTP(S) URL".into()));
    }
    Ok(())
}
