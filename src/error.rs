use crate::{index_path::IndexPathState, Indexer, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("could not convert `{0:?}` to a map")]
    CouldNotConvertToMap(Value),

    #[error("could not append (`{0:?}`, `{1:?}`, `{2:?}`)")]
    CouldNotAppend(Value, Option<Indexer>, Value),

    #[error("parsing indexer ran into `{0:?}` in state `{1:?}` when parsing `{2:?}`")]
    CouldNotParseIndexer(Option<char>, IndexPathState, String),
}

pub type Result<T> = std::result::Result<T, Error>;
