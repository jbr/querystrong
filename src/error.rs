use std::{
    borrow::Cow,
    convert::Infallible,
    fmt::{self, Display, Formatter},
};

use crate::{Indexer, Value, index_path::IndexPathState};
use thiserror::Error;

/// An error produced during query-string parsing or value mutation.
///
/// Individual `Error` values are collected into [`ParseErrors`] when using the
/// permissive [`QueryStrong::parse`](crate::QueryStrong::parse).  They surface
/// directly through [`Value::append`](crate::Value::append) and
/// [`QueryStrong::parse_strict`](crate::QueryStrong::parse_strict).
#[derive(Debug, Error, Eq, PartialEq, Clone)]
pub enum Error<'a> {
    /// A list or sparse-list element could not be promoted to a map key.
    ///
    /// Occurs when a string-keyed bracket (e.g. `a[name]=v`) is applied to a
    /// list that contains non-string entries such as [`Value::Empty`](crate::Value::Empty).
    #[error("could not convert `{0:?}` to a map")]
    CouldNotConvertToMap(Value<'a>),

    /// An attempt to append a value was incompatible with the existing node.
    ///
    /// For example, appending `a[b]=2` when `a` is already the string `"1"`.
    /// The fields are: (existing value, indexer that was tried, new value).
    #[error("could not append (`{0:?}`, `{1:?}`, `{2:?}`)")]
    CouldNotAppend(Value<'a>, Option<Indexer<'a>>, Value<'a>),

    /// The index-path parser encountered an unexpected bracket character.
    ///
    /// The fields are: the unexpected character (if any), the parser state at
    /// the time, and the full key string being parsed.
    #[error("parsing indexer ran into `{0:?}` in state `{1:?}` when parsing {2:?}")]
    CouldNotParseIndexer(Option<char>, IndexPathState, Cow<'a, str>),
}

impl<'a> Error<'a> {
    pub fn into_owned(self) -> Error<'static> {
        match self {
            Error::CouldNotConvertToMap(value) => Error::CouldNotConvertToMap(value.into_owned()),
            Error::CouldNotAppend(value, indexer, value1) => Error::CouldNotAppend(
                value.into_owned(),
                indexer.map(Indexer::into_owned),
                value1.into_owned(),
            ),
            Error::CouldNotParseIndexer(a, b, c) => {
                Error::CouldNotParseIndexer(a, b, Cow::Owned(c.into_owned()))
            }
        }
    }
}

impl From<Infallible> for Error<'_> {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

pub type Result<'a, T> = std::result::Result<T, Error<'a>>;

pub type ParseResult<'a, T> = std::result::Result<T, ParseErrors<'a>>;

/// A collection of [`Error`]s accumulated while parsing a query string.
///
/// Produced by [`QueryStrong::parse`](crate::QueryStrong::parse) (via
/// [`QueryStrong::errors`](crate::QueryStrong::errors)) and returned directly
/// by [`QueryStrong::parse_strict`](crate::QueryStrong::parse_strict).
/// Retains the original input string so that error messages can include it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseErrors<'a> {
    input: Cow<'a, str>,
    errors: Vec<Error<'a>>,
}
impl std::error::Error for ParseErrors<'_> {}

impl Display for ParseErrors<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} {} parsing {:?}:",
            self.errors.len(),
            if self.errors.len() == 1 {
                "error"
            } else {
                "errors"
            },
            self.input
        )?;
        for error in &self.errors {
            writeln!(f, "  - {error}")?;
        }
        Ok(())
    }
}

impl<'a> ParseErrors<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            input: input.into(),
            errors: vec![],
        }
    }

    pub(crate) fn push(&mut self, e: Error<'a>) {
        self.errors.push(e)
    }

    pub fn into_owned(self) -> ParseErrors<'static> {
        ParseErrors {
            input: Cow::Owned(self.input.into_owned()),
            errors: self.errors.into_iter().map(Error::into_owned).collect(),
        }
    }

    /// The original query-string input that was parsed.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// The individual errors, in the order they were encountered during parsing.
    pub fn errors(&self) -> &[Error<'a>] {
        &self.errors
    }
}
