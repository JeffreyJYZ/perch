//! What the board can fail at, and how a failed call reaches the model.

/// One board failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The registry answers in its own error, so a board failure is a tool failure with no new shape.
impl From<Error> for perch_tools::Error {
    fn from(error: Error) -> Self {
        Self::Message(error.to_string())
    }
}
