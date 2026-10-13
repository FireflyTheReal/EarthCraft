//! Preserve typed causes until the application formats a user-facing status.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Xml(#[from] quick_xml::Error),
    #[error(transparent)]
    Task(#[from] tokio::task::JoinError),
    #[error("{0}")]
    Message(String),
}
impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}
impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Message(message.into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;
    #[test]
    fn io_errors_keep_their_source_and_display() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing tile");
        let error = Error::from(io);
        assert_eq!(error.to_string(), "missing tile");
        assert!(matches!(error, Error::Io(_)));
        // Transparent errors delegate their display to the original cause.
        assert!(error.source().is_none());
    }
}
