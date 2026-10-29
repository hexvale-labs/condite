pub enum EnvironmentVariable {
    SqlConnUrl,
    S3Endpoint,
    S3Region,
    S3Bucket,
    S3AccessKey,
    S3SecretKey
}

impl ToString for EnvironmentVariable {
    fn to_string(&self) -> String {
        match self {
            Self::SqlConnUrl => String::from("SQL_CONN_URL"),
            Self::S3Endpoint => String::from("S3_ENDPOINT"),
            Self::S3Region => String::from("S3_REGION"),
            Self::S3Bucket => String::from("S3_BUCKET"),
            Self::S3AccessKey => String::from("S3_ACCESS_KEY"),
            Self::S3SecretKey => String::from("S3_SECRET_KEY")
        }
    }
}

pub fn env_var_required(var: EnvironmentVariable) -> String {
    std::env::var(var.to_string()).expect(format!("Environment variable '{}' not defined!", var.to_string()).as_str())
}