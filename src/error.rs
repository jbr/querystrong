use thiserror::Error as ThisError;

#[derive(Clone, ThisError, Debug)]
pub enum Error {
    #[error("{0}")]
    Stuff(String),
}
