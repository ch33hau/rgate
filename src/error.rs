use thiserror::Error;
use warp::reject::Reject;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Failed to acquire mutex lock: {0}")]
    MutexPoisoned(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("HTTP request building error: {0}")]
    HttpRequestBuildError(#[from] warp::http::Error), // For errors from warp::http::Request::builder()
    #[error("Reqwest client error: {0}")]
    ReqwestClientError(#[from] reqwest::Error), // For errors from reqwest::Client
    #[error("Gzip decompression failed for URI: {0}")]
    GzipDecompressionError(String),
    #[error("URL parsing error: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Unknown error occurred")]
    Unknown,
}

#[derive(Debug)]
pub struct AppRejection(pub AppError);

impl Reject for AppRejection {}

impl From<AppError> for AppRejection {
    fn from(err: AppError) -> Self {
        AppRejection(err)
    }
}
