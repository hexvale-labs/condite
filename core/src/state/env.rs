use std::fmt;

pub enum EnvironmentVariable {
    SqlConnUrl,
    S3Endpoint,
    S3Region,
    S3Bucket,
    S3AccessKey,
    S3SecretKey,
}

impl fmt::Display for EnvironmentVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::SqlConnUrl => "SQL_CONN_URL",
            Self::S3Endpoint => "S3_ENDPOINT",
            Self::S3Region => "S3_REGION",
            Self::S3Bucket => "S3_BUCKET",
            Self::S3AccessKey => "S3_ACCESS_KEY",
            Self::S3SecretKey => "S3_SECRET_KEY",
        };

        f.write_str(name)
    }
}

pub fn env_var_required(var: EnvironmentVariable) -> String {
    std::env::var(var.to_string())
        .unwrap_or_else(|_| panic!("Environment variable '{}' not defined!", var))
}
