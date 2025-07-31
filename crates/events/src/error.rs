use thiserror::Error;

#[derive(Error, Debug)]
pub enum EventsError {
    #[error("Failed to serialize event message: {0}")]
    Serialization(String),
}