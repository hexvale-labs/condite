use std::fmt;

#[derive(Debug)]
pub enum Error {
    Input(String),
    Transport(reqwest::Error),
    Http {
        status: u16,
        body: String,
    },
    Decode {
        source: serde_json::Error,
    },
    Upload {
        request_id: String,
        stage: &'static str,
        source: Box<Error>,
    },
}
impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Input(_) => "input",
            Self::Transport(_) => "transport",
            Self::Http { .. } => "http",
            Self::Decode { .. } => "decode",
            Self::Upload { source, .. } => source.code(),
        }
    }
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } => Some(*status),
            Self::Upload { source, .. } => source.status(),
            _ => None,
        }
    }
    pub fn body(&self) -> Option<&str> {
        match self {
            Self::Http { body, .. } => Some(body),
            Self::Upload { source, .. } => source.body(),
            _ => None,
        }
    }
    pub fn stage(&self) -> Option<&'static str> {
        match self {
            Self::Upload { stage, .. } => Some(stage),
            _ => None,
        }
    }
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Upload { request_id, .. } => Some(request_id),
            _ => None,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(message) => write!(f, "invalid input: {message}"),
            Self::Transport(error) => write!(f, "request failed: {error}"),
            Self::Http { status, body } => write!(f, "HTTP {status}: {body}"),
            Self::Decode { source } => write!(f, "invalid API response: {source}"),
            Self::Upload {
                request_id,
                stage,
                source,
            } => write!(f, "upload {request_id} failed during {stage}: {source}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(e) => Some(e),
            Self::Decode { source } => Some(source),
            Self::Upload { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Transport(error)
    }
}
