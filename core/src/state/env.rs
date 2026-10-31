pub enum EnvironmentVariable {
    SqlConnUrl,
    S3Endpoint,
    S3Region,
    S3Bucket,
    S3AccessKey,
    S3SecretKey,
}

impl std::fmt::Display for EnvironmentVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::SqlConnUrl => "SQL_CONN_URL",
            Self::S3Endpoint => "S3_ENDPOINT",
            Self::S3Region => "S3_REGION",
            Self::S3Bucket => "S3_BUCKET",
            Self::S3AccessKey => "S3_ACCESS_KEY",
            Self::S3SecretKey => "S3_SECRET_KEY",
        })
    }
}

pub fn env_var_required(var: EnvironmentVariable) -> String {
    std::env::var(var.to_string())
        .unwrap_or_else(|_| panic!("Environment variable '{var}' not defined!"))
}
