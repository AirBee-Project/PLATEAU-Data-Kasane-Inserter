use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("ZIP extraction error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("HTTP error status: {0}")]
    HttpStatus(reqwest::StatusCode),

    #[error("Other error: {0}")]
    Other(String),

    #[error("Nazori error{0}")]
    Nazori(#[from] nazori::Error),

    #[error("Kasane authentication failed")]
    Auth,

    #[error("Kasane API error: status={status}, body={body}")]
    KasaneApi {
        status: reqwest::StatusCode,
        body: String,
    },
}
