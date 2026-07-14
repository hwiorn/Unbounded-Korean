use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid resource: {0}")]
    InvalidResource(String),
    #[error("morphological analysis failed: {0}")]
    Morphology(String),
    #[error("invalid option: {0}")]
    InvalidOption(String),
}
